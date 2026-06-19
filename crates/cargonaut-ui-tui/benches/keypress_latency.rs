// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.22b — NFR-002: keymap dispatch latency. The full nfr-002 budget
// is "≤16 ms from keypress to first paint", which spans keymap +
// dispatch + render. This bench measures the keymap-lookup half only
// (the cheap one); the render half is benchmarked separately by
// large_dir_scroll.rs.
//
// Run with:  cargo bench -p cargonaut-ui-tui --bench keypress_latency

use cargonaut_ui_tui::{KeyChord, Keymap, Mode, SeqLookup};
use crossterm::event::{KeyCode, KeyModifiers};
use std::time::Instant;

const DEFAULT_KEYMAP: &str = include_str!("../../../design/contracts/keymap.toml");

fn main() {
    let km = Keymap::load(DEFAULT_KEYMAP).expect("default keymap must parse");

    // Seven chord patterns — includes ] and [ for tab cycling (Feature 053).
    let chords = [
        vec![KeyChord {
            code: KeyCode::Char('j'),
            modifiers: KeyModifiers::empty(),
        }],
        vec![KeyChord {
            code: KeyCode::F(5),
            modifiers: KeyModifiers::empty(),
        }],
        vec![KeyChord {
            code: KeyCode::Char('1'),
            modifiers: KeyModifiers::ALT,
        }],
        vec![
            KeyChord {
                code: KeyCode::Char('x'),
                modifiers: KeyModifiers::CONTROL,
            },
            KeyChord {
                code: KeyCode::Char('!'),
                modifiers: KeyModifiers::empty(),
            },
        ],
        vec![KeyChord {
            code: KeyCode::Char(']'),
            modifiers: KeyModifiers::empty(),
        }], // tab-next (Feature 053)
        vec![KeyChord {
            code: KeyCode::Char('['),
            modifiers: KeyModifiers::empty(),
        }], // tab-prev (Feature 053)
        vec![KeyChord {
            code: KeyCode::Char('Q'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT,
        }], // unbound
    ];

    const ITERS: u32 = 100_000;
    let start = Instant::now();
    let mut sink = 0u64;
    for i in 0..ITERS {
        let chord = &chords[i as usize % chords.len()];
        match km.lookup_sequence(Mode::Pane, chord) {
            SeqLookup::Command(_) => sink = sink.wrapping_add(1),
            SeqLookup::Pending => sink = sink.wrapping_add(2),
            SeqLookup::NoMatch => sink = sink.wrapping_add(3),
        }
    }
    let elapsed = start.elapsed();
    std::hint::black_box(sink);

    let avg_ns = elapsed.as_nanos() as f64 / ITERS as f64;
    println!(
        "keymap lookup: avg {avg_ns:.0} ns/lookup over {ITERS} iters ({} total)",
        humantime::format_duration(elapsed)
    );

    // Sanity gate: must be well under 1 ms per lookup. The 16 ms NFR-002
    // budget allocates the bulk of its time to render, not lookup.
    let gate_us = std::env::var("NFR002_LOOKUP_GATE_US")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(50.0);
    let avg_us = avg_ns / 1000.0;
    if avg_us > gate_us {
        eprintln!("NFR-002 FAIL (lookup component): {avg_us:.1} µs/lookup > {gate_us:.0} µs gate");
        std::process::exit(1);
    }
    println!("NFR-002 lookup OK ({avg_us:.1} µs ≤ {gate_us:.0} µs)");

    // --- Viewer handle_key latency (SC-002 component) ---
    // Open a small in-memory viewer and time handle_key(Down) in a tight loop.
    {
        use cargonaut_ui_tui::{FileViewerDialog, ViewMode};
        use crossterm::event::KeyCode;

        let lines: Vec<String> = (0..500).map(|i| format!("bench line {i:04}")).collect();
        let mut dlg = FileViewerDialog::new_text(
            std::path::PathBuf::from("/bench"),
            "bench.txt".into(),
            lines,
            false,
        );
        assert_eq!(dlg.mode, ViewMode::Text);

        const VIEWER_ITERS: u32 = 10_000;
        let start = Instant::now();
        let mut sink = 0u64;
        for _ in 0..VIEWER_ITERS {
            let action = dlg.handle_key(KeyCode::Down);
            // Prevent the compiler from optimizing away the call.
            sink = sink
                .wrapping_add(matches!(action, cargonaut_ui_tui::FileViewerAction::Swallow) as u64);
            if dlg.current_scroll_offset() >= 499 {
                // Reset so we keep scrolling.
                let _ = dlg.handle_key(KeyCode::Home);
            }
        }
        std::hint::black_box(sink);
        let elapsed = start.elapsed();

        let avg_ns = elapsed.as_nanos() as f64 / VIEWER_ITERS as f64;
        println!(
            "viewer handle_key: avg {avg_ns:.0} ns over {VIEWER_ITERS} iters ({})",
            humantime::format_duration(elapsed)
        );

        let viewer_gate_us = 500.0_f64; // generous: 500 µs well under the 16 ms SC-002 budget
        let avg_us = avg_ns / 1000.0;
        if avg_us > viewer_gate_us {
            eprintln!(
                "SC-002 FAIL (viewer handle_key): {avg_us:.1} µs > {viewer_gate_us:.0} µs gate"
            );
            std::process::exit(1);
        }
        println!("SC-002 viewer handle_key OK ({avg_us:.1} µs ≤ {viewer_gate_us:.0} µs)");
    }

    // --- 5-tab-per-side TabNext/TabPrev dispatch latency (Feature 053) ---
    // Verifies that cycling through 5 tabs stays well under the 16 ms NFR-002 budget.
    {
        use cargonaut_core::{App, Command};
        use tempfile::TempDir;

        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let rt = tokio::runtime::Runtime::new().unwrap();
        let mut app = rt.block_on(async {
            let mut a = App::new(
                cargonaut_config::Config::default(),
                td_l.path().to_str().unwrap(),
                td_r.path().to_str().unwrap(),
            )
            .await
            .unwrap();
            // Open 4 additional tabs on the left (5 total) and 4 on the right.
            for _ in 0..4 {
                a.dispatch(Command::TabNew).await.unwrap();
            }
            a.dispatch(Command::FocusRight).await.unwrap();
            for _ in 0..4 {
                a.dispatch(Command::TabNew).await.unwrap();
            }
            a.dispatch(Command::FocusLeft).await.unwrap();
            a
        });

        const TAB_ITERS: u32 = 10_000;
        let mut max_ns: u64 = 0;
        let overall_start = Instant::now();
        let mut sink = 0u64;
        for i in 0..TAB_ITERS {
            let cmd = if i % 2 == 0 {
                Command::TabNext
            } else {
                Command::TabPrev
            };
            let t0 = Instant::now();
            let events = rt.block_on(app.dispatch(cmd)).unwrap();
            let ns = t0.elapsed().as_nanos() as u64;
            max_ns = max_ns.max(ns);
            sink = sink.wrapping_add(events.len() as u64);
        }
        std::hint::black_box(sink);
        let total = overall_start.elapsed();
        let avg_ns = total.as_nanos() as f64 / TAB_ITERS as f64;
        let p99_estimate_ms = max_ns as f64 / 1_000_000.0;
        println!(
            "tab dispatch (5-tab): avg {avg_ns:.0} ns, observed max {p99_estimate_ms:.2} ms over {TAB_ITERS} iters"
        );

        let tab_gate_ms = 16.0_f64;
        if p99_estimate_ms > tab_gate_ms {
            eprintln!(
                "NFR-002 FAIL (tab dispatch): observed max {p99_estimate_ms:.2} ms > {tab_gate_ms} ms gate"
            );
            std::process::exit(1);
        }
        println!("NFR-002 tab dispatch OK (max {p99_estimate_ms:.2} ms ≤ {tab_gate_ms} ms)");
    }
}

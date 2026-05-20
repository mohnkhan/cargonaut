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

    // Five chord patterns we expect under steady-state use.
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
}

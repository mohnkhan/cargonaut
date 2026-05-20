// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.22c — NFR-002 (render half) + NFR-003 (10⁶-entry virtual scroll).
//
// Build a synthetic 1M-entry DirListing, wrap in a PaneView, scroll
// the cursor 10k times rendering each step into a TestBackend. Report
// per-frame ms + peak RSS delta (Linux only).
//
// Run with:  cargo bench -p cargonaut-ui-tui --bench large_dir_scroll

use cargonaut_ui_tui::PaneView;
use cargonaut_vfs::{DirEntry, DirListing, FileMode, Sort, VfsKind, VfsMetadata, VfsPath};
use ratatui::backend::TestBackend;
use ratatui::Terminal;
use smol_str::SmolStr;
use std::time::{Instant, SystemTime};

const N_ENTRIES: usize = 1_000_000;
const SCROLL_STEPS: u32 = 10_000;
const VIEWPORT_H: u16 = 40;
const VIEWPORT_W: u16 = 80;

fn synthetic_entry(i: usize) -> DirEntry {
    DirEntry {
        name: SmolStr::new(format!("file-{i:07}")),
        meta: VfsMetadata {
            size: i as u64,
            mtime: SystemTime::UNIX_EPOCH,
            mode: Some(FileMode {
                bits: 0o644,
                uid: None,
                gid: None,
            }),
            kind: VfsKind::File,
            is_hidden: false,
        },
    }
}

fn main() {
    eprintln!("large_dir_scroll: building {N_ENTRIES}-entry synthetic listing...");
    let entries: Vec<DirEntry> = (0..N_ENTRIES).map(synthetic_entry).collect();
    let listing = DirListing {
        entries,
        sort: Sort::NameAsc,
    };
    let cwd = VfsPath::parse("file:///tmp").unwrap();

    let baseline_rss = read_rss_kib();
    let mut view = PaneView::new(cwd, listing);
    let after_construct_rss = read_rss_kib();

    let backend = TestBackend::new(VIEWPORT_W, VIEWPORT_H);
    let mut term = Terminal::new(backend).unwrap();

    let mut total_render_ns: u64 = 0;
    for _ in 0..SCROLL_STEPS {
        view.cursor_down();
        let start = Instant::now();
        term.draw(|f| {
            view.render(f.size(), f.buffer_mut());
        })
        .unwrap();
        total_render_ns += start.elapsed().as_nanos() as u64;
    }

    let after_scroll_rss = read_rss_kib();
    let avg_us = total_render_ns as f64 / SCROLL_STEPS as f64 / 1000.0;

    println!("baseline RSS:        {baseline_rss} KiB");
    println!(
        "after PaneView ctor: {after_construct_rss} KiB ({} KiB)",
        after_construct_rss.saturating_sub(baseline_rss)
    );
    println!(
        "after 10k scrolls:   {after_scroll_rss} KiB ({} KiB)",
        after_scroll_rss.saturating_sub(baseline_rss)
    );
    println!("avg render: {avg_us:.1} µs/frame over {SCROLL_STEPS} steps");

    // NFR-002 budget: 16 ms / frame TOTAL. We're measuring just the
    // PaneView render component; gate at 8 ms (half budget) to leave
    // room for keymap lookup + other widgets.
    let gate_ms = std::env::var("NFR002_RENDER_GATE_MS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(8.0);
    let avg_ms = avg_us / 1000.0;
    if avg_ms > gate_ms {
        eprintln!("NFR-002 FAIL (render component): {avg_ms:.2} ms/frame > {gate_ms:.0} ms gate");
        std::process::exit(1);
    }
    println!("NFR-002 render OK ({avg_ms:.2} ms ≤ {gate_ms:.0} ms)");

    let cap_mib = std::env::var("NFR003_RSS_CAP_MIB")
        .ok()
        .and_then(|s| s.parse::<u64>().ok())
        .unwrap_or(64);
    let peak_mib = after_scroll_rss / 1024;
    if peak_mib > cap_mib {
        eprintln!("NFR-003 FAIL: peak {peak_mib} MiB > {cap_mib} MiB cap (1M-entry test)");
        std::process::exit(1);
    }
    println!("NFR-003 OK (peak {peak_mib} MiB ≤ {cap_mib} MiB for 1M entries)");
}

#[cfg(target_os = "linux")]
fn read_rss_kib() -> u64 {
    let s = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in s.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            return rest
                .split_whitespace()
                .next()
                .and_then(|n| n.parse().ok())
                .unwrap_or(0);
        }
    }
    0
}

#[cfg(not(target_os = "linux"))]
fn read_rss_kib() -> u64 {
    0
}

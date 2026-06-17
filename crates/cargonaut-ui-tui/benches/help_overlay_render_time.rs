// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Feature 047 — NFR-001: help overlay content-build latency.
// Iterates over HELP_SECTIONS and formats all rows to simulate the work
// done on the first keypress that opens the overlay.
//
// Run with:  cargo bench -p cargonaut-ui-tui --bench help_overlay_render_time

use cargonaut_ui_tui::dialog::HELP_SECTIONS;
use std::time::Instant;

fn main() {
    let iterations = 10_000u32;
    let start = Instant::now();
    for _ in 0..iterations {
        // Replicate the string-build that HelpOverlay::render does.
        let _text: String = HELP_SECTIONS
            .iter()
            .flat_map(|s| {
                std::iter::once(format!("=== {} ===\n", s.title)).chain(
                    s.rows
                        .iter()
                        .map(|r| format!("  {:20}  {}\n", r.key, r.desc)),
                )
            })
            .collect();
    }
    let elapsed = start.elapsed();
    let per_iter_us = elapsed.as_micros() / u64::from(iterations) as u128;
    println!("help-content build: {iterations} iters in {elapsed:?} ({per_iter_us} µs/iter)");
    // NFR-001: must complete in < 1 ms (1000 µs).
    assert!(
        per_iter_us < 1_000,
        "help content build too slow: {per_iter_us} µs/iter (limit 1000 µs)"
    );
}

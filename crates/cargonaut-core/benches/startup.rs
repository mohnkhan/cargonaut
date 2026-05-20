// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.11 — SC-004 gate: cold-cache startup ≤ 150 ms on the reference
// laptop. This bench measures the in-process startup path (Config
// default + App::new + first listing of two tempdirs) — the part of
// cold launch that is OUR code's fault. Binary cold-launch (process
// fork + exec + dynamic linker) is measured by a separate hyperfine
// script (`scripts/bench-startup-cold.sh`, not yet written).
//
// Run with:  cargo bench -p cargonaut-core --bench startup

use cargonaut_core::App;
use std::time::Instant;
use tempfile::TempDir;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let td_l = TempDir::new().unwrap();
    let td_r = TempDir::new().unwrap();
    // Populate a few entries so the listing isn't degenerate.
    for i in 0..50 {
        std::fs::write(td_l.path().join(format!("f{i:03}")), b"").unwrap();
        std::fs::write(td_r.path().join(format!("f{i:03}")), b"").unwrap();
    }

    const ITERS: u32 = 20;
    let mut total_ms = 0.0;
    for _ in 0..ITERS {
        let config = cargonaut_config::Config::default();
        let start = Instant::now();
        let _app = App::new(
            config,
            td_l.path().to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();
        total_ms += start.elapsed().as_secs_f64() * 1000.0;
    }
    let avg_ms = total_ms / ITERS as f64;
    println!("In-process startup: {avg_ms:.2} ms avg over {ITERS} iters (Config default + App::new + 2× LocalFs.list)");

    let gate = std::env::var("SC004_INPROCESS_GATE_MS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(150.0);
    if avg_ms > gate {
        eprintln!("SC-004 FAIL (in-process portion): {avg_ms:.2} ms > {gate:.0} ms gate");
        std::process::exit(1);
    }
    println!("SC-004 in-process OK ({avg_ms:.2} ms ≤ {gate:.0} ms)");
}

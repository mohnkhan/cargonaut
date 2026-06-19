// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.12 — SC-003 gate: RSS ≤ 64 MiB for the canonical session
// (per FR-009: 3 panes × 10k entries, no plugins, no concurrent
// transfers > 3).
//
// Phase 1 measurement: construct two PaneStates with 10k synthetic
// entries each + a third synthetic listing buffer, sample RSS via
// /proc/self/status on Linux. The bench is in-process so it
// undercounts vs the binary subprocess (which adds the ratatui +
// crossterm + tokio worker overhead). When the per-binary version
// lands (T1.23 release polish), the ceiling check moves there and
// this becomes a lower-bound guardrail.

use cargonaut_core::{App, Command};
use std::time::Duration;
use tempfile::TempDir;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    // Two tempdirs with 10k entries each (the third pane in the FR-009
    // canonical session is represented by holding a third listing in
    // memory below).
    let td_l = TempDir::new().unwrap();
    let td_r = TempDir::new().unwrap();
    eprintln!("rss_headroom: populating 2× 10k-entry tempdirs...");
    populate(&td_l, 10_000);
    populate(&td_r, 10_000);

    let baseline = read_rss_kib();

    let config = cargonaut_config::Config::default();
    let mut app = App::new(
        config,
        td_l.path().to_str().unwrap(),
        td_r.path().to_str().unwrap(),
    )
    .await
    .unwrap();

    // Feature 053: open 4 additional tabs per side (5 total each) to exercise
    // the SideState multi-tab memory layout under the SC-003 budget.
    for _ in 0..4 {
        app.dispatch(Command::TabNew).await.unwrap();
    }
    app.dispatch(Command::FocusRight).await.unwrap();
    for _ in 0..4 {
        app.dispatch(Command::TabNew).await.unwrap();
    }

    // Synthetic "third pane" listing held alongside the App.
    let _third = populate_in_memory(10_000);

    // Give the allocator a moment to settle.
    tokio::time::sleep(Duration::from_millis(100)).await;
    let peak = read_rss_kib();

    println!(
        "baseline RSS: {baseline} KiB  ;  with App (5-tab/side) + 3rd listing: {peak} KiB  ;  delta: {} KiB",
        peak.saturating_sub(baseline)
    );

    let cap_mib: u64 = std::env::var("SC003_RSS_CAP_MIB")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(64);
    let peak_mib = peak / 1024;
    if peak_mib > cap_mib {
        eprintln!("SC-003 FAIL: peak {peak_mib} MiB > {cap_mib} MiB cap (5-tab/side scenario)");
        std::process::exit(1);
    }
    println!("SC-003 OK (5-tab/side in-process peak {peak_mib} MiB ≤ {cap_mib} MiB)");
}

fn populate(td: &TempDir, n: usize) {
    for i in 0..n {
        std::fs::write(td.path().join(format!("f{i:05}")), b"").unwrap();
    }
}

fn populate_in_memory(n: usize) -> Vec<String> {
    (0..n).map(|i| format!("f{i:05}")).collect()
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
    // RSS introspection is OS-specific; non-Linux just returns 0 so
    // the bench prints zeros but doesn't crash.
    0
}

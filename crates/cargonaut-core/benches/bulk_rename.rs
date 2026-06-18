// T020 — SC-001/SC-004 gates: bulk rename + undo of 50 files ≤ 500 ms each.
//
// bench `bulk_rename_50`: tag 50 temp files, call apply_bulk_rename() 50-pair
//   rename, measure p95 over 100 iterations, assert ≤ 500 ms.
// bench `undo_rename_50`: undo the 50-file rename, same assertion.
//
// Run with: cargo bench -p cargonaut-core --bench bulk_rename

use cargonaut_core::{App, Command};
use std::time::Instant;
use tempfile::TempDir;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    bench_bulk_rename_50().await;
    bench_undo_rename_50().await;
}

async fn bench_bulk_rename_50() {
    const ITERS: u32 = 100;
    let mut times_ns: Vec<u128> = Vec::with_capacity(ITERS as usize);

    for _ in 0..ITERS {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for i in 0..50usize {
            std::fs::write(td_l.path().join(format!("file_{i:03}.txt")), b"x").unwrap();
        }
        let config = cargonaut_config::Config::default();
        let mut app = App::new(
            config,
            td_l.path().to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();

        let pairs: Vec<(String, String)> = (0..50usize)
            .map(|i| (format!("file_{i:03}.txt"), format!("renamed_{i:03}.txt")))
            .collect();

        let start = Instant::now();
        let _ = app.apply_bulk_rename(pairs).await.unwrap();
        times_ns.push(start.elapsed().as_nanos());
    }

    times_ns.sort_unstable();
    let p95_ns = times_ns[(ITERS as usize * 95 / 100).min(ITERS as usize - 1)];
    let avg_ns = times_ns.iter().sum::<u128>() / ITERS as u128;

    println!(
        "bulk_rename_50: avg={:.1} ms  p95={:.1} ms  ({ITERS} iters)",
        avg_ns as f64 / 1_000_000.0,
        p95_ns as f64 / 1_000_000.0,
    );

    assert!(
        p95_ns <= 500_000_000u128,
        "SC-001 breach: p95={p95_ns} ns > 500 ms gate for bulk rename of 50 files"
    );
    println!(
        "SC-001 OK: p95={:.1} ms ≤ 500 ms",
        p95_ns as f64 / 1_000_000.0
    );
}

async fn bench_undo_rename_50() {
    const ITERS: u32 = 100;
    let mut times_ns: Vec<u128> = Vec::with_capacity(ITERS as usize);

    for _ in 0..ITERS {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for i in 0..50usize {
            std::fs::write(td_l.path().join(format!("file_{i:03}.txt")), b"x").unwrap();
        }
        let config = cargonaut_config::Config::default();
        let mut app = App::new(
            config,
            td_l.path().to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();

        let pairs: Vec<(String, String)> = (0..50usize)
            .map(|i| (format!("file_{i:03}.txt"), format!("renamed_{i:03}.txt")))
            .collect();
        app.apply_bulk_rename(pairs).await.unwrap();

        let start = Instant::now();
        let _ = app.dispatch(Command::UndoLastOp).await.unwrap();
        times_ns.push(start.elapsed().as_nanos());
    }

    times_ns.sort_unstable();
    let p95_ns = times_ns[(ITERS as usize * 95 / 100).min(ITERS as usize - 1)];
    let avg_ns = times_ns.iter().sum::<u128>() / ITERS as u128;

    println!(
        "undo_rename_50: avg={:.1} ms  p95={:.1} ms  ({ITERS} iters)",
        avg_ns as f64 / 1_000_000.0,
        p95_ns as f64 / 1_000_000.0,
    );

    assert!(
        p95_ns <= 500_000_000u128,
        "SC-004 breach: p95={p95_ns} ns > 500 ms gate for undo of 50-file rename"
    );
    println!(
        "SC-004 OK: p95={:.1} ms ≤ 500 ms",
        p95_ns as f64 / 1_000_000.0
    );
}

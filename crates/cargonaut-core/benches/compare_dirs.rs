// T010 — SC-001 gate: compare_directories on 1,000-file panels ≤ 2,000 ms.
//
// Creates two tempdirs each with 1,000 files:
//   - 500 identical (same name + same content)
//   - 250 size-differing (same name, different byte counts)
//   - 250 content-differing (same name, same size, different bytes)
//
// Run with:  cargo bench -p cargonaut-core --bench compare_dirs

use cargonaut_core::{App, Command};
use std::time::Instant;
use tempfile::TempDir;

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let td_l = TempDir::new().unwrap();
    let td_r = TempDir::new().unwrap();

    // 500 identical files
    for i in 0..500usize {
        let name = format!("identical_{i:04}.txt");
        std::fs::write(td_l.path().join(&name), b"identical content").unwrap();
        std::fs::write(td_r.path().join(&name), b"identical content").unwrap();
    }
    // 250 size-differing files
    for i in 0..250usize {
        let name = format!("size_differ_{i:04}.txt");
        std::fs::write(td_l.path().join(&name), b"longer content here").unwrap();
        std::fs::write(td_r.path().join(&name), b"short").unwrap();
    }
    // 250 content-differing files (same size: 7 bytes)
    for i in 0..250usize {
        let name = format!("hash_differ_{i:04}.txt");
        std::fs::write(td_l.path().join(&name), format!("left{i:03}").as_bytes()).unwrap();
        std::fs::write(td_r.path().join(&name), format!("rgt{i:03}").as_bytes()).unwrap();
    }

    const ITERS: u32 = 5;
    let mut times_ms: Vec<f64> = Vec::with_capacity(ITERS as usize);

    for _ in 0..ITERS {
        let config = cargonaut_config::Config::default();
        let mut app = App::new(
            config,
            td_l.path().to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();

        let start = Instant::now();
        let events = app.dispatch(Command::CompareDirectories).await.unwrap();
        times_ms.push(start.elapsed().as_secs_f64() * 1000.0);

        let _ = events; // result used above for timing
    }

    times_ms.sort_by(|a, b| a.partial_cmp(b).unwrap());
    let p95_ms = times_ms[(ITERS as usize * 95 / 100).min(ITERS as usize - 1)];
    let avg_ms = times_ms.iter().sum::<f64>() / ITERS as f64;

    println!(
        "compare_dirs: avg={avg_ms:.1} ms  p95={p95_ms:.1} ms  (1,000 files per panel, {ITERS} iters)"
    );

    let gate_ms: f64 = std::env::var("SC001_GATE_MS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(2000.0);

    if p95_ms > gate_ms {
        eprintln!(
            "SC-001 FAIL: p95={p95_ms:.1} ms > {gate_ms:.0} ms gate"
        );
        std::process::exit(1);
    }
    println!("SC-001 OK: p95={p95_ms:.1} ms ≤ {gate_ms:.0} ms");
}

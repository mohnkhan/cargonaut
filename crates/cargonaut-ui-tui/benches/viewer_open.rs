// SC-001: ≤150 ms p50 for open_file_viewer on a 1 MiB text file.
//
// criterion does not support async natively; we wrap with a
// current-thread tokio runtime.  Run with:
//   cargo bench -p cargonaut-ui-tui --bench viewer_open

use cargonaut_ui_tui::open_file_viewer;
use std::io::Write;
use std::time::Instant;

fn main() {
    // Write a 1 MiB temp file of line-oriented text.
    let mut tmp = tempfile::NamedTempFile::new().expect("temp file");
    let line = "The quick brown fox jumps over the lazy dog.  Lorem ipsum dolor sit amet.\n";
    let target = 1024 * 1024; // 1 MiB
    let mut written = 0usize;
    while written < target {
        tmp.write_all(line.as_bytes()).unwrap();
        written += line.len();
    }
    tmp.flush().unwrap();
    let path = tmp.path().to_path_buf();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    // Warm-up (bring file into page cache).
    rt.block_on(open_file_viewer(path.clone(), "bench_file.txt".into()))
        .expect("warm-up open");

    const ITERS: u32 = 20;
    let start = Instant::now();
    for _ in 0..ITERS {
        rt.block_on(open_file_viewer(path.clone(), "bench_file.txt".into()))
            .expect("open_file_viewer");
    }
    let elapsed = start.elapsed();
    let p50_ms = elapsed.as_secs_f64() * 1000.0 / ITERS as f64;

    println!(
        "viewer_open: p50 ~ {p50_ms:.1} ms  ({ITERS} iters, total {})",
        humantime::format_duration(elapsed)
    );

    let gate_ms = std::env::var("SC001_GATE_MS")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(150.0);

    if p50_ms > gate_ms {
        eprintln!("SC-001 FAIL: p50 {p50_ms:.1} ms > {gate_ms:.0} ms gate");
        std::process::exit(1);
    }
    println!("SC-001 OK ({p50_ms:.1} ms ≤ {gate_ms:.0} ms)");
}

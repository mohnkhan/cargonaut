// SC-003: Opening a 1 GiB file in streaming mode keeps RSS ≤ 64 MiB.
//
// The file is a sparse file (no actual disk writes) so creation is instant.
// We open it, scroll through ≥ 5 page-downs, then measure RSS.
//
// Run with:  cargo bench -p cargonaut-ui-tui --bench rss_headroom

use cargonaut_ui_tui::open_file_viewer;
use crossterm::event::KeyCode;
use std::io::{Seek, SeekFrom, Write};

fn rss_bytes() -> u64 {
    // Read from /proc/self/status on Linux.
    let status = std::fs::read_to_string("/proc/self/status").unwrap_or_default();
    for line in status.lines() {
        if let Some(rest) = line.strip_prefix("VmRSS:") {
            let kb: u64 = rest
                .split_whitespace()
                .next()
                .unwrap_or("0")
                .parse()
                .unwrap_or(0);
            return kb * 1024;
        }
    }
    0
}

fn main() {
    // Create a 1 GiB sparse file (zero disk writes, instant creation).
    let tmp = tempfile::NamedTempFile::new().expect("temp file");
    {
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .open(tmp.path())
            .unwrap();
        f.seek(SeekFrom::Start(1_073_741_823)).unwrap(); // 1 GiB - 1
        f.write_all(&[0u8]).unwrap(); // one byte to materialise the final block
        f.flush().unwrap();
    }
    let path = tmp.path().to_path_buf();

    let rss_before = rss_bytes();

    let rt = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .expect("tokio runtime");

    let mut dlg = rt
        .block_on(open_file_viewer(path, "large_sparse.bin".into()))
        .expect("open viewer");

    // Scroll through ≥ 5 page-downs to exercise streaming.
    for _ in 0..5 {
        dlg.handle_key(KeyCode::PageDown);
    }

    let rss_after = rss_bytes();
    let delta_mib = (rss_after.saturating_sub(rss_before)) as f64 / (1024.0 * 1024.0);
    let total_mib = rss_after as f64 / (1024.0 * 1024.0);

    println!(
        "rss_headroom: RSS before={:.1} MiB  after={:.1} MiB  delta={delta_mib:.1} MiB",
        rss_before as f64 / (1024.0 * 1024.0),
        total_mib,
    );

    let gate_mib = std::env::var("SC003_RSS_GATE_MIB")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(64.0);

    if delta_mib > gate_mib {
        eprintln!("SC-003 FAIL: delta RSS {delta_mib:.1} MiB > {gate_mib:.0} MiB gate");
        std::process::exit(1);
    }
    println!("SC-003 OK (delta {delta_mib:.1} MiB ≤ {gate_mib:.0} MiB)");
}

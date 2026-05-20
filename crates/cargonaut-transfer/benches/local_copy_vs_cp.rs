// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.10 — SC-001 gate: local-to-local copy throughput vs `cp(1)`.
//
// Builds a 100 MiB random source in a TempDir, times cargonaut's
// `submit_transfer` end-to-end, then times `/usr/bin/cp` on the same
// payload, and prints the ratio. The SC-001 gate is ≥80% of cp(1).
//
// Run with:  cargo bench -p cargonaut-transfer --bench local_copy_vs_cp
//
// NOT a criterion bench — this is a `harness = false` binary so the
// dep tree stays small in Phase 1. Future polish should switch to
// criterion for warm/cold/percentile distributions; right now this
// emits one number and exits.

use cargonaut_transfer::{submit_transfer, TransferOptions, TransferState};
use cargonaut_vfs::{LocalFs, VfsBackend, VfsPath};
use std::fs;
use std::path::Path;
use std::process::Command;
use std::sync::Arc;
use std::time::Instant;
use tempfile::TempDir;

const PAYLOAD_BYTES: usize = 100 * 1024 * 1024;

fn vfs_path(p: &Path) -> VfsPath {
    VfsPath::parse(&format!("file://{}", p.to_str().unwrap())).unwrap()
}

#[tokio::main(flavor = "multi_thread")]
async fn main() {
    let td = TempDir::new().unwrap();
    let src = td.path().join("src.bin");
    let dst_cargo = td.path().join("dst_cargonaut.bin");
    let dst_cp = td.path().join("dst_cp.bin");

    eprintln!("local_copy_vs_cp: building {PAYLOAD_BYTES}-byte source...");
    let payload: Vec<u8> = (0..PAYLOAD_BYTES).map(|i| (i & 0xFF) as u8).collect();
    fs::write(&src, &payload).unwrap();

    // Cargonaut path.
    let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let opts = TransferOptions {
        verify_after_copy: false,
        buffer_size_bytes: 1024 * 1024,
        checkpoint_interval_bytes: 16 * 1024 * 1024,
        ..Default::default()
    };
    let start = Instant::now();
    let job = submit_transfer(
        Arc::clone(&lfs),
        vfs_path(&src),
        Arc::clone(&lfs),
        vfs_path(&dst_cargo),
        opts,
    )
    .await
    .unwrap();
    let mut rx = job.state.clone();
    loop {
        if matches!(*rx.borrow(), TransferState::Completed { .. }) {
            break;
        }
        rx.changed().await.unwrap();
    }
    let cargonaut_secs = start.elapsed().as_secs_f64();
    let cargonaut_mibps = (PAYLOAD_BYTES as f64) / cargonaut_secs / (1024.0 * 1024.0);

    // cp(1) path.
    let cp_path = which_cp().unwrap_or_else(|| "cp".into());
    let start = Instant::now();
    let status = Command::new(&cp_path)
        .arg(&src)
        .arg(&dst_cp)
        .status()
        .expect("spawn cp");
    assert!(status.success(), "cp failed");
    let cp_secs = start.elapsed().as_secs_f64();
    let cp_mibps = (PAYLOAD_BYTES as f64) / cp_secs / (1024.0 * 1024.0);

    let ratio = cargonaut_mibps / cp_mibps;
    println!("cargonaut: {cargonaut_secs:.3}s ({cargonaut_mibps:.1} MiB/s)");
    println!("cp:        {cp_secs:.3}s ({cp_mibps:.1} MiB/s)");
    println!("ratio:     {:.0}% of cp(1)", ratio * 100.0);

    let gate = std::env::var("SC001_RATIO_GATE")
        .ok()
        .and_then(|s| s.parse::<f64>().ok())
        .unwrap_or(0.80);
    if ratio < gate {
        eprintln!(
            "SC-001 FAIL: cargonaut at {:.0}% of cp(1), below {:.0}% gate",
            ratio * 100.0,
            gate * 100.0
        );
        std::process::exit(1);
    }
    println!("SC-001 OK ({:.0}% ≥ {:.0}%)", ratio * 100.0, gate * 100.0);
}

fn which_cp() -> Option<String> {
    for candidate in ["/usr/bin/cp", "/bin/cp"] {
        if std::path::Path::new(candidate).exists() {
            return Some(candidate.to_string());
        }
    }
    None
}

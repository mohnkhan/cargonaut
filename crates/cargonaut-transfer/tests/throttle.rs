// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Feature 037 (R-002): the transfer engine honors an opt-in throughput
// throttle via the `CARGONAUT_TRANSFER_THROTTLE_MIBPS` environment
// variable. This exists so the binary-level SC-002 SIGKILL-resume test
// can keep a copy in flight long enough to deterministically kill it
// mid-transfer (tmpfs copies otherwise finish in milliseconds).
//
// Isolated in its own integration-test binary so the process-global env
// var cannot leak into the parallel unit tests in `job.rs`.

use cargonaut_transfer::{submit_transfer, TransferOptions, TransferState};
use cargonaut_vfs::{VfsBackend, VfsPath};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::watch;

fn vfs_path_for(p: &std::path::Path) -> VfsPath {
    VfsPath::parse(&format!("file://{}", p.to_str().expect("UTF-8 path"))).expect("parses")
}

async fn wait_terminal(mut rx: watch::Receiver<TransferState>) -> TransferState {
    loop {
        {
            let s = rx.borrow();
            if matches!(
                *s,
                TransferState::Completed { .. }
                    | TransferState::Failed { .. }
                    | TransferState::Canceled
            ) {
                return s.clone();
            }
        }
        if rx.changed().await.is_err() {
            return TransferState::Failed {
                error: "watch sender dropped".into(),
                resumable: false,
            };
        }
    }
}

async fn run_copy_secs(payload: &[u8]) -> f64 {
    let td = tempfile::TempDir::new().unwrap();
    let src = td.path().join("src.bin");
    let dst = td.path().join("dst.bin");
    tokio::fs::write(&src, payload).await.unwrap();

    let lfs: Arc<dyn VfsBackend> = Arc::new(cargonaut_vfs::LocalFs::new());
    let opts = TransferOptions {
        buffer_size_bytes: 1024 * 1024,
        checkpoint_interval_bytes: 1024 * 1024,
        verify_after_copy: false,
        ..Default::default()
    };
    let started = Instant::now();
    let job = submit_transfer(
        Arc::clone(&lfs),
        vfs_path_for(&src),
        Arc::clone(&lfs),
        vfs_path_for(&dst),
        opts,
    )
    .await
    .unwrap();
    let final_state = tokio::time::timeout(Duration::from_secs(30), wait_terminal(job.state.clone()))
        .await
        .expect("transfer didn't terminate in 30s");
    assert!(matches!(final_state, TransferState::Completed { .. }));
    started.elapsed().as_secs_f64()
}

#[tokio::test]
async fn throttle_env_slows_transfer_measurably() {
    // 8 MiB payload. On tmpfs an unthrottled copy finishes in well under
    // 100 ms; throttled to 8 MiB/s it should take ~1 s.
    let payload = vec![0x5Au8; 8 * 1024 * 1024];

    // Unthrottled baseline (env unset).
    std::env::remove_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS");
    let unthrottled = run_copy_secs(&payload).await;

    // Throttled run.
    std::env::set_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS", "8");
    let throttled = run_copy_secs(&payload).await;
    std::env::remove_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS");

    assert!(
        throttled >= 0.5,
        "throttled 8 MiB @ 8 MiB/s should take ≥0.5s, took {throttled:.3}s"
    );
    assert!(
        throttled > unthrottled * 2.0,
        "throttled ({throttled:.3}s) should be markedly slower than unthrottled ({unthrottled:.3}s)"
    );
}

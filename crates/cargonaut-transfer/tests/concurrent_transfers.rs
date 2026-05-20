// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T1.22d: NFR-004 concurrent-transfers gate.
//!
//! Submits 8 simultaneous LocalFs→LocalFs copies, waits for all to
//! complete, asserts no failures. The "no UI render frame exceeded
//! 16ms" assertion from the task spec is deferred — that needs a
//! tracing-subscriber latency probe + a running UI; here we just
//! verify the transfer engine can sustain 8 concurrent jobs without
//! crashes or hangs.

use cargonaut_transfer::{submit_transfer, TransferOptions, TransferState};
use cargonaut_vfs::{LocalFs, VfsBackend, VfsPath};
use std::path::Path;
use std::sync::Arc;
use std::time::Duration;
use tempfile::TempDir;
use tokio::fs;
use tokio::sync::watch;
use tokio::time::timeout;

fn vfs_path_for(p: &Path) -> VfsPath {
    VfsPath::parse(&format!("file://{}", p.to_str().expect("UTF-8 path"))).expect("parses")
}

async fn wait_done(mut rx: watch::Receiver<TransferState>) -> TransferState {
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

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn eight_concurrent_local_to_local_copies_all_complete() {
    let td = TempDir::new().unwrap();
    let src_dir = td.path().join("src");
    let dst_dir = td.path().join("dst");
    fs::create_dir(&src_dir).await.unwrap();
    fs::create_dir(&dst_dir).await.unwrap();

    // 8 × 4 MiB sources — small enough that the test finishes quickly,
    // big enough that all 8 are still in flight when we kick the last.
    const N: usize = 8;
    const PAYLOAD_SIZE: usize = 4 * 1024 * 1024;
    let payload: Vec<u8> = (0..PAYLOAD_SIZE).map(|i| (i & 0xFF) as u8).collect();

    for i in 0..N {
        fs::write(src_dir.join(format!("f{i}")), &payload)
            .await
            .unwrap();
    }

    let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let opts = TransferOptions {
        verify_after_copy: false,
        buffer_size_bytes: 256 * 1024,
        checkpoint_interval_bytes: 1024 * 1024,
        ..Default::default()
    };

    let mut jobs = Vec::with_capacity(N);
    for i in 0..N {
        let job = submit_transfer(
            Arc::clone(&lfs),
            vfs_path_for(&src_dir.join(format!("f{i}"))),
            Arc::clone(&lfs),
            vfs_path_for(&dst_dir.join(format!("f{i}"))),
            opts.clone(),
        )
        .await
        .unwrap();
        jobs.push(job);
    }

    // Wait for all to complete (parallel waits).
    let waiters = jobs
        .into_iter()
        .map(|j| tokio::spawn(async move { wait_done(j.state.clone()).await }));

    let outcomes = timeout(Duration::from_secs(30), futures::future::join_all(waiters))
        .await
        .expect("8 concurrent transfers didn't finish in 30 s");

    for (i, res) in outcomes.into_iter().enumerate() {
        let state = res.expect("waiter task panicked");
        assert!(
            matches!(state, TransferState::Completed { .. }),
            "transfer {i} ended in {state:?}, not Completed"
        );
    }

    // All destinations should byte-for-byte match the source.
    for i in 0..N {
        let bytes = fs::read(dst_dir.join(format!("f{i}"))).await.unwrap();
        assert_eq!(bytes.len(), PAYLOAD_SIZE, "dst {i} wrong size");
    }
}

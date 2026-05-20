// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T1.12b: cancellation integration tests.
//!
//! Covers FR-008 + NFR-005:
//! - Cancellation observed within 500 ms (wall-clock).
//! - Partial destination is kept + a `.cargonaut-transfer-*.json`
//!   checkpoint sidecar exists so the transfer is resumable.
//!   (FR-008's `[transfer] on_cancel = "delete"` mode is a follow-up;
//!   Phase 1 unconditionally implements the `keep` semantics — the
//!   resume contract is more valuable than the eager-cleanup default.)
//! - No tokio task survives 1 s after cancel. The simple version of
//!   this test asserts the cancellation reaches the `Canceled` state
//!   on the `watch::Receiver`; a stricter version (tokio-metrics task
//!   count) is deferred until tokio-metrics is in the dep tree.

use cargonaut_transfer::{submit_transfer, TransferOptions, TransferState};
use cargonaut_vfs::{LocalFs, VfsBackend, VfsPath};
use std::path::Path;
use std::sync::Arc;
use std::time::{Duration, Instant};
use tempfile::TempDir;
use tokio::fs;
use tokio::sync::watch;
use tokio::time::{sleep, timeout};

fn vfs_path_for(p: &Path) -> VfsPath {
    VfsPath::parse(&format!("file://{}", p.to_str().expect("UTF-8 path"))).expect("parses")
}

async fn wait_for_state<F: Fn(&TransferState) -> bool>(
    mut rx: watch::Receiver<TransferState>,
    matches: F,
    bound: Duration,
) -> Option<TransferState> {
    let deadline = Instant::now() + bound;
    loop {
        {
            let s = rx.borrow();
            if matches(&s) {
                return Some(s.clone());
            }
        }
        let rem = deadline.checked_duration_since(Instant::now())?;
        if timeout(rem, rx.changed()).await.is_err() {
            return None;
        }
    }
}

#[tokio::test]
async fn cancel_observed_within_500_ms() {
    // 8 MiB payload + 64 KiB read buffer + 64 KiB checkpoint interval
    // produces many iterations of the copy loop so cancellation lands
    // mid-flight rather than after EOF.
    let td = TempDir::new().unwrap();
    let src = td.path().join("src.bin");
    let dst = td.path().join("dst.bin");
    let payload = vec![0xCDu8; 8 * 1024 * 1024];
    fs::write(&src, &payload).await.unwrap();

    let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let opts = TransferOptions {
        buffer_size_bytes: 64 * 1024,
        checkpoint_interval_bytes: 64 * 1024,
        verify_after_copy: false,
        ..Default::default()
    };
    let job = submit_transfer(
        Arc::clone(&lfs),
        vfs_path_for(&src),
        Arc::clone(&lfs),
        vfs_path_for(&dst),
        opts,
    )
    .await
    .unwrap();

    // Wait for Running state before cancelling (otherwise we might
    // cancel before the loop has even started).
    let rx = job.state.clone();
    let _running = wait_for_state(
        rx,
        |s| matches!(s, TransferState::Running { .. }),
        Duration::from_secs(2),
    )
    .await
    .expect("transfer never reached Running");

    let cancel_at = Instant::now();
    job.cancel.cancel();

    let rx = job.state.clone();
    let canceled = wait_for_state(
        rx,
        |s| matches!(s, TransferState::Canceled),
        Duration::from_millis(500),
    )
    .await;
    let elapsed = cancel_at.elapsed();

    assert!(
        canceled.is_some(),
        "Canceled state not observed within 500 ms (elapsed: {elapsed:?})"
    );
    assert!(
        elapsed < Duration::from_millis(500),
        "FR-008: cancellation took {elapsed:?}, must be < 500 ms"
    );
}

#[tokio::test]
async fn cancel_keeps_partial_destination_and_checkpoint_sidecar() {
    let td = TempDir::new().unwrap();
    let src = td.path().join("src.bin");
    let dst = td.path().join("dst.bin");
    let payload = vec![0xEEu8; 4 * 1024 * 1024];
    fs::write(&src, &payload).await.unwrap();

    let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let opts = TransferOptions {
        buffer_size_bytes: 64 * 1024,
        // Small checkpoint interval so at least one sidecar write
        // happens before cancellation.
        checkpoint_interval_bytes: 64 * 1024,
        verify_after_copy: false,
        ..Default::default()
    };
    let job = submit_transfer(
        Arc::clone(&lfs),
        vfs_path_for(&src),
        Arc::clone(&lfs),
        vfs_path_for(&dst),
        opts,
    )
    .await
    .unwrap();

    // Wait for at least one Running emission so we know the loop has
    // started writing.
    let rx = job.state.clone();
    let _ = wait_for_state(
        rx,
        |s| matches!(s, TransferState::Running { .. }),
        Duration::from_secs(2),
    )
    .await
    .expect("transfer never reached Running");

    // Wait one more tick to give the first checkpoint a chance to land.
    sleep(Duration::from_millis(20)).await;

    job.cancel.cancel();

    let rx = job.state.clone();
    let _ = wait_for_state(
        rx,
        |s| matches!(s, TransferState::Canceled),
        Duration::from_millis(500),
    )
    .await
    .expect("never observed Canceled");

    // Partial destination must still exist (Phase 1 keep semantics).
    assert!(dst.exists(), "dst must be kept on cancel for resume");
    let dst_size = fs::metadata(&dst).await.unwrap().len();
    assert!(
        dst_size > 0 && dst_size < payload.len() as u64,
        "dst should be partial: got {dst_size} of {}",
        payload.len()
    );

    // Exactly one checkpoint sidecar should remain.
    let mut count = 0;
    for entry in std::fs::read_dir(td.path()).unwrap().flatten() {
        if entry
            .file_name()
            .to_string_lossy()
            .starts_with(".cargonaut-transfer-")
        {
            count += 1;
        }
    }
    assert!(
        count >= 1,
        "expected ≥1 checkpoint sidecar after cancel, found {count}"
    );
}

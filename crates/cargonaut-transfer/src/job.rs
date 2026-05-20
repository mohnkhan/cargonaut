// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `TransferJob` submission + the resumable copy loop.

use super::checkpoint::ResumableTransfer;
use cargonaut_vfs::{VfsBackend, VfsPath};
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::watch;
use tokio_util::sync::CancellationToken;
use uuid::Uuid;

/// Opaque job identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TransferId(pub Uuid);

/// Copy or move?
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TransferMode {
    /// Copy (source kept).
    Copy,
    /// Move (source removed after successful copy).
    Move,
}

/// Per-transfer options.
#[derive(Debug, Clone)]
pub struct TransferOptions {
    /// Mode.
    pub mode: TransferMode,
    /// Re-read destination after copy and verify SHA-256 matches source.
    pub verify_after_copy: bool,
    /// Bytes between fsync'd checkpoints.
    pub checkpoint_interval_bytes: u64,
    /// Read/write buffer size.
    pub buffer_size_bytes: usize,
}

impl Default for TransferOptions {
    fn default() -> Self {
        Self {
            mode: TransferMode::Copy,
            verify_after_copy: true,
            checkpoint_interval_bytes: 8 * 1024 * 1024,
            buffer_size_bytes: 16 * 1024 * 1024,
        }
    }
}

/// Snapshot of transfer progress, observed via `watch::Receiver`.
#[derive(Debug, Clone)]
pub enum TransferState {
    /// Queued, not yet started.
    Queued,
    /// Running.
    Running {
        /// Bytes written so far.
        bytes_done: u64,
        /// Source size.
        bytes_total: u64,
        /// Estimated seconds remaining.
        eta_secs: u32,
        /// Current throughput in MiB/s.
        throughput_mibs: f32,
    },
    /// Paused (user action).
    Paused,
    /// Successfully completed.
    Completed {
        /// True if destination SHA-256 matches source SHA-256.
        sha256_match: bool,
    },
    /// Failed; `resumable` = true if a checkpoint exists for resume.
    Failed {
        /// Failure reason.
        error: String,
        /// Can the user retry from the checkpoint?
        resumable: bool,
    },
    /// Canceled by the user.
    Canceled,
}

/// Re-exported alias for the progress watcher.
pub type Progress = TransferState;

/// A handle to a submitted transfer.
pub struct TransferJob {
    /// Stable identifier.
    pub id: TransferId,
    /// Source VFS + path.
    pub src: (Arc<dyn VfsBackend>, VfsPath),
    /// Destination VFS + path.
    pub dst: (Arc<dyn VfsBackend>, VfsPath),
    /// Mode (copy or move).
    pub mode: TransferMode,
    /// Subscribe to state updates.
    pub state: watch::Receiver<TransferState>,
    /// Cancel this transfer.
    pub cancel: CancellationToken,
}

/// Errors from submitting / running a transfer.
#[derive(Debug, Error)]
pub enum TransferError {
    /// Underlying VFS error.
    #[error("vfs error: {0}")]
    Vfs(#[from] cargonaut_vfs::VfsError),

    /// Checkpoint file parse / write error.
    #[error("checkpoint error: {0}")]
    Checkpoint(String),
}

/// Submit a new transfer. Spawns a tokio task; returns the handle.
///
/// T1.13 implements the actual copy loop.
pub async fn submit_transfer(
    _src_backend: Arc<dyn VfsBackend>,
    _src_path: VfsPath,
    _dst_backend: Arc<dyn VfsBackend>,
    _dst_path: VfsPath,
    _opts: TransferOptions,
) -> Result<TransferJob, TransferError> {
    unimplemented!("T1.13 — see design/tasks.md")
}

/// Scan a destination directory for orphan checkpoint files and validate
/// each one (source SHA-256 prefix match + destination CRC chain match).
///
/// T1.14 implements.
pub async fn scan_resumable(
    _dst_backend: Arc<dyn VfsBackend>,
    _dst_dir: VfsPath,
) -> Result<Vec<ResumableTransfer>, TransferError> {
    unimplemented!("T1.14")
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargonaut_vfs::LocalFs;
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::fs;
    use tokio::time::{timeout, Duration};

    fn vfs_path_for(p: &Path) -> VfsPath {
        VfsPath::parse(&format!("file://{}", p.to_str().expect("UTF-8 path"))).expect("parses")
    }

    /// Drive the `watch::Receiver` until a terminal state, returning the final state.
    async fn wait_terminal(mut rx: watch::Receiver<TransferState>) -> TransferState {
        loop {
            // Check current state without awaiting (might already be terminal).
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
            // Otherwise wait for next change.
            if rx.changed().await.is_err() {
                // Sender dropped without terminal; treat as failure.
                return TransferState::Failed {
                    error: "watch sender dropped".into(),
                    resumable: false,
                };
            }
        }
    }

    /// Like `wait_terminal` but also collects every distinct state seen.
    async fn collect_states(mut rx: watch::Receiver<TransferState>) -> Vec<TransferState> {
        let mut seen = vec![rx.borrow().clone()];
        loop {
            let is_terminal = matches!(
                *rx.borrow(),
                TransferState::Completed { .. }
                    | TransferState::Failed { .. }
                    | TransferState::Canceled
            );
            if is_terminal {
                return seen;
            }
            if rx.changed().await.is_err() {
                return seen;
            }
            seen.push(rx.borrow().clone());
        }
    }

    #[tokio::test]
    async fn copies_small_file_byte_for_byte() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        let payload: Vec<u8> = (0..=255u8).cycle().take(1024).collect();
        fs::write(&src, &payload).await.unwrap();

        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let job = submit_transfer(
            Arc::clone(&lfs),
            vfs_path_for(&src),
            Arc::clone(&lfs),
            vfs_path_for(&dst),
            TransferOptions::default(),
        )
        .await
        .unwrap();

        let final_state = timeout(Duration::from_secs(5), wait_terminal(job.state.clone()))
            .await
            .expect("transfer didn't terminate in 5s");
        assert!(matches!(final_state, TransferState::Completed { .. }));
        assert_eq!(fs::read(&dst).await.unwrap(), payload);
    }

    #[tokio::test]
    async fn emits_multiple_running_updates_for_multi_chunk_transfer() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        // 4 MiB payload + 1 MiB read buffer + 1 MiB checkpoint interval -> 4 chunks.
        let payload = vec![0x55u8; 4 * 1024 * 1024];
        fs::write(&src, &payload).await.unwrap();

        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            buffer_size_bytes: 1024 * 1024,
            checkpoint_interval_bytes: 1024 * 1024,
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

        let states = timeout(Duration::from_secs(5), collect_states(job.state.clone()))
            .await
            .expect("transfer didn't terminate in 5s");
        let running_count = states
            .iter()
            .filter(|s| matches!(s, TransferState::Running { .. }))
            .count();
        assert!(
            running_count >= 2,
            "expected ≥2 Running states for multi-chunk transfer, got {running_count}; states = {states:?}"
        );
    }

    #[tokio::test]
    async fn checkpoint_sidecar_absent_after_successful_completion() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        // Use a small checkpoint interval so checkpoint code path actually runs.
        let payload = vec![0xAAu8; 256];
        fs::write(&src, &payload).await.unwrap();

        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            buffer_size_bytes: 64,
            checkpoint_interval_bytes: 64,
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

        let final_state = timeout(Duration::from_secs(5), wait_terminal(job.state.clone()))
            .await
            .expect("transfer didn't terminate in 5s");
        assert!(matches!(final_state, TransferState::Completed { .. }));

        // No `.cargonaut-transfer-*.json` should remain.
        let mut rd = std::fs::read_dir(td.path()).unwrap();
        let leftovers: Vec<_> = rd
            .by_ref()
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.file_name()
                    .to_string_lossy()
                    .starts_with(".cargonaut-transfer-")
            })
            .map(|e| e.file_name())
            .collect();
        assert!(
            leftovers.is_empty(),
            "checkpoint sidecar(s) leaked: {leftovers:?}"
        );
    }

    #[tokio::test]
    async fn missing_source_returns_error() {
        let td = TempDir::new().unwrap();
        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let res = submit_transfer(
            Arc::clone(&lfs),
            vfs_path_for(&td.path().join("nope.bin")),
            Arc::clone(&lfs),
            vfs_path_for(&td.path().join("dst.bin")),
            TransferOptions::default(),
        )
        .await;
        assert!(
            res.is_err(),
            "missing source must error before spawning task"
        );
    }

    #[tokio::test]
    async fn verify_after_copy_sets_sha256_match_true_for_identical_data() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        let payload: Vec<u8> = (0..2048u32)
            .flat_map(|n| (n as u32).to_le_bytes())
            .collect();
        fs::write(&src, &payload).await.unwrap();

        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            verify_after_copy: true,
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

        let final_state = timeout(Duration::from_secs(5), wait_terminal(job.state.clone()))
            .await
            .expect("transfer didn't terminate in 5s");
        match final_state {
            TransferState::Completed { sha256_match } => {
                assert!(sha256_match, "verify_after_copy should report match");
            }
            other => panic!("expected Completed, got {other:?}"),
        }
    }
}

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

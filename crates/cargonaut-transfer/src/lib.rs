// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut resumable copy/move engine.
//!
//! Spawns one tokio task per transfer; writes a `TransferCheckpoint`
//! after every N MiB fsync'd (configurable, default 8 MiB). On relaunch,
//! [`scan_resumable`] walks a destination directory for orphan checkpoint
//! files and returns offers to resume.
//!
//! Phase 1 scope: copy + move within `LocalFs`. Phase 2+ extends to
//! cross-VFS transfers (e.g. SFTP → S3) — the same checkpoint format
//! works because it only references VFS URIs + byte offsets.

#![warn(missing_docs)]

pub mod checkpoint;
pub mod job;

pub use checkpoint::{scan_resumable, ResumableTransfer, TransferCheckpoint};
pub use job::{
    submit_transfer, Progress, TransferError, TransferId, TransferJob, TransferMode,
    TransferOptions, TransferState,
};

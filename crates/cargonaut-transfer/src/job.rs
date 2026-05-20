// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `TransferJob` submission + the resumable copy loop.

use super::checkpoint::TransferCheckpoint;
use cargonaut_vfs::{ByteRange, VfsBackend, VfsPath, WriteMode};
use futures::{AsyncReadExt, AsyncWriteExt};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use std::time::{Instant, SystemTime, UNIX_EPOCH};
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

/// Submit a new transfer. Stats the source synchronously (so the caller
/// learns about NotFound / permission errors immediately rather than via
/// a `Failed` state), then spawns a tokio task that runs the resumable
/// copy loop. The returned [`TransferJob`] hands back a
/// [`watch::Receiver`] for progress + a [`CancellationToken`] to abort.
///
/// The copy loop:
/// - Computes a SHA-256 prefix of the first 1 MiB of the source (used by
///   resume to detect "source changed since checkpoint").
/// - Opens `src.read_stream(FULL)` and `dst.write_stream(0, Truncate)`.
/// - Reads `opts.buffer_size_bytes` chunks; writes each to the destination.
/// - Every `opts.checkpoint_interval_bytes` of written data, drains the
///   accumulated bytes into a chunk, CRC32s it, appends to the
///   [`TransferCheckpoint::chunk_crcs`] chain, flushes the writer, and
///   writes/overwrites the checkpoint sidecar at
///   `<dst-parent>/.cargonaut-transfer-<job-id>.json`.
/// - Emits a `Running` state on every chunk read (bytes_done, throughput,
///   ETA estimate from the wall-clock).
/// - Checks the cancellation token at the top of every iteration; on
///   cancel, sets `Canceled` and leaves the checkpoint in place so the
///   transfer is resumable.
/// - On EOF + final flush: optionally re-reads source and destination to
///   verify SHA-256 (if `opts.verify_after_copy`); unlinks the checkpoint
///   sidecar (best-effort); emits `Completed { sha256_match }`.
pub async fn submit_transfer(
    src_backend: Arc<dyn VfsBackend>,
    src_path: VfsPath,
    dst_backend: Arc<dyn VfsBackend>,
    dst_path: VfsPath,
    opts: TransferOptions,
) -> Result<TransferJob, TransferError> {
    let id = TransferId(Uuid::new_v4());
    let cancel = CancellationToken::new();
    let (state_tx, state_rx) = watch::channel(TransferState::Queued);

    // Stat source synchronously — caller wants immediate error feedback.
    let src_meta = src_backend.stat(&src_path).await?;
    let src_size = src_meta.size;

    // SHA-256 of the first 1 MiB of source — used by resume to detect a
    // swapped/modified source.
    let src_sha256_prefix = compute_src_prefix(&*src_backend, &src_path).await?;

    // Checkpoint sidecar lives at <dst-parent>/.cargonaut-transfer-<id>.json
    // (per spec §14 clarification: beside the destination, hidden filename).
    let dst_parent = dst_path
        .parent()
        .ok_or_else(|| TransferError::Checkpoint("dst path has no parent".into()))?;
    let checkpoint_path = dst_parent.join(&format!(".cargonaut-transfer-{}.json", id.0));

    let mode = opts.mode;
    let job = TransferJob {
        id,
        src: (Arc::clone(&src_backend), src_path.clone()),
        dst: (Arc::clone(&dst_backend), dst_path.clone()),
        mode,
        state: state_rx,
        cancel: cancel.clone(),
    };

    tokio::spawn(run_transfer(
        id,
        src_backend,
        src_path,
        dst_backend,
        dst_path,
        opts,
        src_size,
        src_sha256_prefix,
        checkpoint_path,
        state_tx,
        cancel,
    ));

    Ok(job)
}

#[allow(clippy::too_many_arguments)] // private helper; arguments are a bag of pre-resolved inputs.
async fn run_transfer(
    id: TransferId,
    src_backend: Arc<dyn VfsBackend>,
    src_path: VfsPath,
    dst_backend: Arc<dyn VfsBackend>,
    dst_path: VfsPath,
    opts: TransferOptions,
    src_size: u64,
    src_sha256_prefix: [u8; 32],
    checkpoint_path: VfsPath,
    state_tx: watch::Sender<TransferState>,
    cancel: CancellationToken,
) {
    let start = Instant::now();
    let created_at = now_secs();

    // Open source reader.
    let mut reader = match src_backend.read_stream(&src_path, ByteRange::FULL).await {
        Ok(r) => r,
        Err(e) => {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("open src: {e}"),
                resumable: false,
            });
            return;
        }
    };

    // Open destination writer (truncating).
    let mut writer = match dst_backend
        .write_stream(&dst_path, 0, WriteMode::Truncate)
        .await
    {
        Ok(w) => w,
        Err(e) => {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("open dst: {e}"),
                resumable: false,
            });
            return;
        }
    };

    let mut bytes_written: u64 = 0;
    let mut chunk_crcs: Vec<u32> = Vec::new();
    let mut pending_chunk: Vec<u8> = Vec::with_capacity(opts.checkpoint_interval_bytes as usize);
    let mut read_buf = vec![0u8; opts.buffer_size_bytes];

    loop {
        // Cancellation check — fast path between chunks; keeps response
        // time bounded by chunk-read latency (≤ buffer_size_bytes /
        // throughput).
        if cancel.is_cancelled() {
            // Leave the checkpoint sidecar in place — the partial dst is
            // resumable.
            let _ = state_tx.send(TransferState::Canceled);
            return;
        }

        // Read next chunk.
        let n = match reader.read(&mut read_buf).await {
            Ok(0) => break, // EOF
            Ok(n) => n,
            Err(e) => {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("read src: {e}"),
                    resumable: true,
                });
                return;
            }
        };

        // Write the chunk.
        if let Err(e) = writer.write_all(&read_buf[..n]).await {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("write dst: {e}"),
                resumable: true,
            });
            return;
        }
        bytes_written += n as u64;
        pending_chunk.extend_from_slice(&read_buf[..n]);

        // Emit a Running update on every chunk read.
        let elapsed = start.elapsed().as_secs_f64();
        let throughput_mibs = if elapsed > 0.0 {
            (bytes_written as f64 / elapsed / (1024.0 * 1024.0)) as f32
        } else {
            0.0
        };
        let eta_secs = if throughput_mibs > 0.0 && bytes_written < src_size {
            let remaining_mib = (src_size - bytes_written) as f64 / (1024.0 * 1024.0);
            (remaining_mib / throughput_mibs as f64) as u32
        } else {
            0
        };
        let _ = state_tx.send(TransferState::Running {
            bytes_done: bytes_written,
            bytes_total: src_size,
            eta_secs,
            throughput_mibs,
        });

        // Drain full checkpoint-interval chunks; write/refresh sidecar
        // after each.
        let interval = opts.checkpoint_interval_bytes as usize;
        while pending_chunk.len() >= interval {
            let chunk: Vec<u8> = pending_chunk.drain(..interval).collect();
            chunk_crcs.push(crc32fast::hash(&chunk));

            if let Err(e) = writer.flush().await {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("flush dst: {e}"),
                    resumable: true,
                });
                return;
            }

            let cp = TransferCheckpoint {
                version: TransferCheckpoint::VERSION,
                job_id: id.0.to_string(),
                src_uri: src_path.display(),
                src_size,
                src_sha256_prefix,
                dst_uri: dst_path.display(),
                bytes_written,
                chunk_crcs: chunk_crcs.clone(),
                chunk_size_bytes: opts.checkpoint_interval_bytes,
                created_at,
                last_update_at: now_secs(),
            };
            if let Err(e) = write_checkpoint(&*dst_backend, &checkpoint_path, &cp).await {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("checkpoint write: {e}"),
                    resumable: true,
                });
                return;
            }
        }
    }

    // Final flush + close.
    if let Err(e) = writer.flush().await {
        let _ = state_tx.send(TransferState::Failed {
            error: format!("final flush: {e}"),
            resumable: true,
        });
        return;
    }
    if let Err(e) = writer.close().await {
        let _ = state_tx.send(TransferState::Failed {
            error: format!("close dst: {e}"),
            resumable: true,
        });
        return;
    }

    // Optional SHA-256 verify (re-reads both sides — expensive; opt-in
    // via opts.verify_after_copy, default ON because correctness > speed).
    let sha256_match = if opts.verify_after_copy {
        verify_full_sha256(&*src_backend, &src_path, &*dst_backend, &dst_path)
            .await
            .unwrap_or_default()
    } else {
        // Without verification we can't claim a match; report false to
        // keep the contract honest.
        false
    };

    // Best-effort cleanup of the checkpoint sidecar. NotFound is OK
    // (small transfers never wrote one).
    let _ = dst_backend.unlink(&checkpoint_path).await;

    let _ = state_tx.send(TransferState::Completed { sha256_match });
}

async fn write_checkpoint(
    backend: &dyn VfsBackend,
    path: &VfsPath,
    cp: &TransferCheckpoint,
) -> Result<(), TransferError> {
    let json =
        serde_json::to_string_pretty(cp).map_err(|e| TransferError::Checkpoint(e.to_string()))?;
    let mut w = backend
        .write_stream(path, 0, WriteMode::Truncate)
        .await
        .map_err(|e| TransferError::Checkpoint(e.to_string()))?;
    w.write_all(json.as_bytes())
        .await
        .map_err(|e| TransferError::Checkpoint(e.to_string()))?;
    w.flush()
        .await
        .map_err(|e| TransferError::Checkpoint(e.to_string()))?;
    w.close()
        .await
        .map_err(|e| TransferError::Checkpoint(e.to_string()))?;
    Ok(())
}

async fn compute_src_prefix(
    backend: &dyn VfsBackend,
    path: &VfsPath,
) -> Result<[u8; 32], TransferError> {
    let mut reader = backend
        .read_stream(
            path,
            ByteRange {
                start: 0,
                end: Some(1024 * 1024),
            },
        )
        .await?;
    let mut buf = Vec::with_capacity(1024 * 1024);
    reader
        .read_to_end(&mut buf)
        .await
        .map_err(|e| TransferError::Checkpoint(format!("read src prefix: {e}")))?;
    let mut h = Sha256::new();
    h.update(&buf);
    Ok(h.finalize().into())
}

async fn verify_full_sha256(
    src_backend: &dyn VfsBackend,
    src: &VfsPath,
    dst_backend: &dyn VfsBackend,
    dst: &VfsPath,
) -> Result<bool, TransferError> {
    let s = full_sha256(src_backend, src).await?;
    let d = full_sha256(dst_backend, dst).await?;
    Ok(s == d)
}

async fn full_sha256(backend: &dyn VfsBackend, path: &VfsPath) -> Result<[u8; 32], TransferError> {
    let mut reader = backend.read_stream(path, ByteRange::FULL).await?;
    let mut h = Sha256::new();
    let mut buf = vec![0u8; 64 * 1024];
    loop {
        let n = reader
            .read(&mut buf)
            .await
            .map_err(|e| TransferError::Checkpoint(format!("read for verify: {e}")))?;
        if n == 0 {
            break;
        }
        h.update(&buf[..n]);
    }
    Ok(h.finalize().into())
}

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
}

/// Resume a previously-checkpointed transfer. Re-validates the destination
/// CRC chain (defensive — state may have changed between `scan_resumable`
/// and now), opens both streams at `checkpoint.bytes_written`, and spawns
/// the same copy loop as [`submit_transfer`] using the existing CRC chain
/// as the starting state.
///
/// Emits the same [`TransferState`] shape as `submit_transfer`. The
/// returned [`TransferJob::id`] is **the same** as the original transfer
/// (taken from `checkpoint.job_id`) so audit logs can be correlated.
///
/// Fails fast (returns `Err` without spawning) if:
/// - The destination CRC chain doesn't match `checkpoint.chunk_crcs` —
///   resuming would silently corrupt the file.
/// - Either URI in the checkpoint can't be parsed.
/// - `bytes_written > src_size` (shouldn't happen for a well-formed
///   checkpoint, but defended).
pub async fn resume_transfer(
    src_backend: Arc<dyn VfsBackend>,
    dst_backend: Arc<dyn VfsBackend>,
    checkpoint: TransferCheckpoint,
    opts: TransferOptions,
) -> Result<TransferJob, TransferError> {
    let src_path = VfsPath::parse(&checkpoint.src_uri)
        .map_err(|e| TransferError::Checkpoint(format!("parse src_uri: {e}")))?;
    let dst_path = VfsPath::parse(&checkpoint.dst_uri)
        .map_err(|e| TransferError::Checkpoint(format!("parse dst_uri: {e}")))?;

    if checkpoint.bytes_written > checkpoint.src_size {
        return Err(TransferError::Checkpoint(format!(
            "checkpoint bytes_written {} > src_size {}",
            checkpoint.bytes_written, checkpoint.src_size
        )));
    }

    // Defensive re-verification (scan_resumable may have run minutes ago).
    let dest_intact =
        super::checkpoint::verify_dst_crc_chain(&*dst_backend, &dst_path, &checkpoint)
            .await
            .unwrap_or(false);
    if !dest_intact {
        return Err(TransferError::Checkpoint(
            "destination CRC chain mismatch — refusing to resume".into(),
        ));
    }

    let id = TransferId(Uuid::parse_str(&checkpoint.job_id).unwrap_or_else(|_| Uuid::new_v4()));
    let cancel = CancellationToken::new();
    let (state_tx, state_rx) = watch::channel(TransferState::Queued);

    let dst_parent = dst_path
        .parent()
        .ok_or_else(|| TransferError::Checkpoint("dst path has no parent".into()))?;
    let checkpoint_path = dst_parent.join(&format!(".cargonaut-transfer-{}.json", id.0));

    let mode = opts.mode;
    let job = TransferJob {
        id,
        src: (Arc::clone(&src_backend), src_path.clone()),
        dst: (Arc::clone(&dst_backend), dst_path.clone()),
        mode,
        state: state_rx,
        cancel: cancel.clone(),
    };

    let resumed_state = ResumedState {
        bytes_already_written: checkpoint.bytes_written,
        existing_chunk_crcs: checkpoint.chunk_crcs.clone(),
        src_sha256_prefix: checkpoint.src_sha256_prefix,
        created_at: checkpoint.created_at,
    };

    tokio::spawn(run_transfer_with_state(
        id,
        src_backend,
        src_path,
        dst_backend,
        dst_path,
        opts,
        checkpoint.src_size,
        checkpoint_path,
        state_tx,
        cancel,
        resumed_state,
    ));

    Ok(job)
}

#[derive(Debug, Clone)]
struct ResumedState {
    bytes_already_written: u64,
    existing_chunk_crcs: Vec<u32>,
    src_sha256_prefix: [u8; 32],
    created_at: u64,
}

/// Resume-aware copy loop. Same shape as `run_transfer` but starts at
/// `resumed.bytes_already_written` with the existing CRC chain pre-loaded.
/// Used only by [`resume_transfer`]; `submit_transfer` keeps its own
/// `run_transfer` for the from-scratch case (less branching there).
#[allow(clippy::too_many_arguments)]
async fn run_transfer_with_state(
    id: TransferId,
    src_backend: Arc<dyn VfsBackend>,
    src_path: VfsPath,
    dst_backend: Arc<dyn VfsBackend>,
    dst_path: VfsPath,
    opts: TransferOptions,
    src_size: u64,
    checkpoint_path: VfsPath,
    state_tx: watch::Sender<TransferState>,
    cancel: CancellationToken,
    resumed: ResumedState,
) {
    let start = Instant::now();
    let created_at = resumed.created_at;
    let mut bytes_written = resumed.bytes_already_written;
    let mut chunk_crcs = resumed.existing_chunk_crcs;
    let src_sha256_prefix = resumed.src_sha256_prefix;

    // Open source at the resume offset.
    let mut reader = match src_backend
        .read_stream(
            &src_path,
            ByteRange {
                start: bytes_written,
                end: None,
            },
        )
        .await
    {
        Ok(r) => r,
        Err(e) => {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("open src for resume: {e}"),
                resumable: false,
            });
            return;
        }
    };

    // Open destination for append-at-offset (file must exist).
    let mut writer = match dst_backend
        .write_stream(&dst_path, bytes_written, WriteMode::AppendAtOffset)
        .await
    {
        Ok(w) => w,
        Err(e) => {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("open dst for resume: {e}"),
                resumable: false,
            });
            return;
        }
    };

    let mut pending_chunk: Vec<u8> = Vec::with_capacity(opts.checkpoint_interval_bytes as usize);
    let mut read_buf = vec![0u8; opts.buffer_size_bytes];

    loop {
        if cancel.is_cancelled() {
            let _ = state_tx.send(TransferState::Canceled);
            return;
        }

        let n = match reader.read(&mut read_buf).await {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) => {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("read src: {e}"),
                    resumable: true,
                });
                return;
            }
        };

        if let Err(e) = writer.write_all(&read_buf[..n]).await {
            let _ = state_tx.send(TransferState::Failed {
                error: format!("write dst: {e}"),
                resumable: true,
            });
            return;
        }
        bytes_written += n as u64;
        pending_chunk.extend_from_slice(&read_buf[..n]);

        let elapsed = start.elapsed().as_secs_f64();
        let throughput_mibs = if elapsed > 0.0 {
            (bytes_written as f64 / elapsed / (1024.0 * 1024.0)) as f32
        } else {
            0.0
        };
        let eta_secs = if throughput_mibs > 0.0 && bytes_written < src_size {
            let remaining_mib = (src_size - bytes_written) as f64 / (1024.0 * 1024.0);
            (remaining_mib / throughput_mibs as f64) as u32
        } else {
            0
        };
        let _ = state_tx.send(TransferState::Running {
            bytes_done: bytes_written,
            bytes_total: src_size,
            eta_secs,
            throughput_mibs,
        });

        let interval = opts.checkpoint_interval_bytes as usize;
        while pending_chunk.len() >= interval {
            let chunk: Vec<u8> = pending_chunk.drain(..interval).collect();
            chunk_crcs.push(crc32fast::hash(&chunk));

            if let Err(e) = writer.flush().await {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("flush dst: {e}"),
                    resumable: true,
                });
                return;
            }

            let cp = TransferCheckpoint {
                version: TransferCheckpoint::VERSION,
                job_id: id.0.to_string(),
                src_uri: src_path.display(),
                src_size,
                src_sha256_prefix,
                dst_uri: dst_path.display(),
                bytes_written,
                chunk_crcs: chunk_crcs.clone(),
                chunk_size_bytes: opts.checkpoint_interval_bytes,
                created_at,
                last_update_at: now_secs(),
            };
            if let Err(e) = write_checkpoint(&*dst_backend, &checkpoint_path, &cp).await {
                let _ = state_tx.send(TransferState::Failed {
                    error: format!("checkpoint write: {e}"),
                    resumable: true,
                });
                return;
            }
        }
    }

    if let Err(e) = writer.flush().await {
        let _ = state_tx.send(TransferState::Failed {
            error: format!("final flush: {e}"),
            resumable: true,
        });
        return;
    }
    if let Err(e) = writer.close().await {
        let _ = state_tx.send(TransferState::Failed {
            error: format!("close dst: {e}"),
            resumable: true,
        });
        return;
    }

    let sha256_match = if opts.verify_after_copy {
        verify_full_sha256(&*src_backend, &src_path, &*dst_backend, &dst_path)
            .await
            .unwrap_or_default()
    } else {
        false
    };

    let _ = dst_backend.unlink(&checkpoint_path).await;
    let _ = state_tx.send(TransferState::Completed { sha256_match });
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

    // ============================================================
    // resume_transfer tests
    // ============================================================

    use sha2::{Digest, Sha256};

    /// Stage a partially-copied transfer + matching checkpoint, then
    /// hand back (src_path, dst_path, checkpoint) for resume.
    async fn stage_partial_transfer(
        td: &TempDir,
        payload_size: usize,
        bytes_already_done: usize,
        chunk_size: usize,
    ) -> (std::path::PathBuf, std::path::PathBuf, TransferCheckpoint) {
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        let payload: Vec<u8> = (0..payload_size).map(|i| (i & 0xFF) as u8).collect();
        fs::write(&src, &payload).await.unwrap();
        fs::write(&dst, &payload[..bytes_already_done])
            .await
            .unwrap();

        let mut h = Sha256::new();
        h.update(&payload[..payload.len().min(1024 * 1024)]);
        let src_sha256_prefix: [u8; 32] = h.finalize().into();

        // Build the chunk_crcs chain for the bytes already on disk.
        let mut chunk_crcs = Vec::new();
        let mut off = 0;
        while off + chunk_size <= bytes_already_done {
            chunk_crcs.push(crc32fast::hash(&payload[off..off + chunk_size]));
            off += chunk_size;
        }
        // bytes_already_done must be a multiple of chunk_size for the
        // CRC chain to validate; pin that as a test pre-condition.
        assert_eq!(
            off, bytes_already_done,
            "test setup: bytes_already_done must be a multiple of chunk_size"
        );

        let cp = TransferCheckpoint {
            version: TransferCheckpoint::VERSION,
            job_id: Uuid::new_v4().to_string(),
            src_uri: format!("file://{}", src.to_str().unwrap()),
            src_size: payload.len() as u64,
            src_sha256_prefix,
            dst_uri: format!("file://{}", dst.to_str().unwrap()),
            bytes_written: bytes_already_done as u64,
            chunk_crcs,
            chunk_size_bytes: chunk_size as u64,
            created_at: 0,
            last_update_at: 0,
        };
        (src, dst, cp)
    }

    #[tokio::test]
    async fn resume_completes_partial_transfer() {
        let td = TempDir::new().unwrap();
        let (src, dst, cp) = stage_partial_transfer(&td, 4096, 1024, 512).await;
        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            buffer_size_bytes: 256,
            checkpoint_interval_bytes: 512,
            verify_after_copy: false,
            ..Default::default()
        };
        let job = resume_transfer(Arc::clone(&lfs), Arc::clone(&lfs), cp, opts)
            .await
            .unwrap();
        let final_state = timeout(Duration::from_secs(5), wait_terminal(job.state.clone()))
            .await
            .expect("resume didn't terminate in 5s");
        assert!(matches!(final_state, TransferState::Completed { .. }));
        // Destination now has the full source.
        let src_bytes = fs::read(&src).await.unwrap();
        let dst_bytes = fs::read(&dst).await.unwrap();
        assert_eq!(src_bytes, dst_bytes);
    }

    #[tokio::test]
    async fn resume_rejects_corrupted_dst() {
        let td = TempDir::new().unwrap();
        let (_src, dst, cp) = stage_partial_transfer(&td, 4096, 1024, 512).await;
        // Corrupt the destination after staging — CRC chain no longer matches.
        let mut bytes = fs::read(&dst).await.unwrap();
        bytes[0] ^= 0xFF;
        fs::write(&dst, &bytes).await.unwrap();

        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let res = resume_transfer(
            Arc::clone(&lfs),
            Arc::clone(&lfs),
            cp,
            TransferOptions::default(),
        )
        .await;
        assert!(
            res.is_err(),
            "corrupted dst must fail-fast at resume_transfer"
        );
    }

    #[tokio::test]
    async fn resume_bytes_written_too_large_rejected() {
        let td = TempDir::new().unwrap();
        let (_src, _dst, mut cp) = stage_partial_transfer(&td, 4096, 1024, 512).await;
        cp.bytes_written = cp.src_size + 1; // impossible
        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let res = resume_transfer(
            Arc::clone(&lfs),
            Arc::clone(&lfs),
            cp,
            TransferOptions::default(),
        )
        .await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn resume_preserves_job_id_from_checkpoint() {
        let td = TempDir::new().unwrap();
        let (_src, _dst, cp) = stage_partial_transfer(&td, 2048, 512, 256).await;
        let original_id = cp.job_id.clone();
        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            buffer_size_bytes: 128,
            checkpoint_interval_bytes: 256,
            verify_after_copy: false,
            ..Default::default()
        };
        let job = resume_transfer(Arc::clone(&lfs), Arc::clone(&lfs), cp, opts)
            .await
            .unwrap();
        assert_eq!(job.id.0.to_string(), original_id);
    }

    #[tokio::test]
    async fn resume_first_running_state_starts_at_bytes_written() {
        let td = TempDir::new().unwrap();
        let (_src, _dst, cp) = stage_partial_transfer(&td, 4096, 1024, 512).await;
        let cp_bytes_written = cp.bytes_written;
        let lfs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let opts = TransferOptions {
            buffer_size_bytes: 128,
            checkpoint_interval_bytes: 512,
            verify_after_copy: false,
            ..Default::default()
        };
        let job = resume_transfer(Arc::clone(&lfs), Arc::clone(&lfs), cp, opts)
            .await
            .unwrap();
        let states = timeout(Duration::from_secs(5), collect_states(job.state.clone()))
            .await
            .unwrap();
        // First Running must show bytes_done > cp_bytes_written (we picked up where the checkpoint left off).
        let first_running = states.iter().find_map(|s| match s {
            TransferState::Running { bytes_done, .. } => Some(*bytes_done),
            _ => None,
        });
        let bytes_done = first_running.expect("expected at least one Running state");
        assert!(
            bytes_done > cp_bytes_written,
            "first Running.bytes_done={bytes_done} should exceed checkpoint bytes_written={cp_bytes_written}"
        );
    }

    #[tokio::test]
    async fn verify_after_copy_sets_sha256_match_true_for_identical_data() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        let payload: Vec<u8> = (0..2048u32).flat_map(u32::to_le_bytes).collect();
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

//! Transfer-checkpoint serialization + resume-offer discovery.

use serde::{Deserialize, Serialize};

/// Persisted checkpoint sidecar written next to the destination file.
/// Filename: `.cargonaut-transfer-<job-id>.json`.
///
/// CRC chain lets a resume validate that the existing destination bytes
/// match what THIS transfer wrote — defending against same-name file
/// from an unrelated source.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct TransferCheckpoint {
    /// Schema version; bump on incompatible change.
    pub version: u32,
    /// Job UUID (matches the in-memory TransferJob.id).
    pub job_id: String,
    /// Source URI as parsed.
    pub src_uri: String,
    /// Source size at the time the transfer started.
    pub src_size: u64,
    /// SHA-256 of the first 1 MiB of the source — used to detect "the
    /// source has changed since we started" on resume.
    pub src_sha256_prefix: [u8; 32],
    /// Destination URI as parsed.
    pub dst_uri: String,
    /// Bytes already written to destination AND fsync'd.
    pub bytes_written: u64,
    /// CRC32 per checkpoint interval; chain lets resume detect a corrupted
    /// destination prefix without re-reading from source.
    pub chunk_crcs: Vec<u32>,
    /// Checkpoint interval in bytes (constant per transfer).
    pub chunk_size_bytes: u64,
    /// Epoch seconds when the transfer was first submitted.
    pub created_at: u64,
    /// Epoch seconds of the last checkpoint write.
    pub last_update_at: u64,
}

impl TransferCheckpoint {
    /// Current on-disk schema version.
    pub const VERSION: u32 = 1;
}

/// A resumable transfer found by [`super::job::scan_resumable`].
#[derive(Debug, Clone)]
pub struct ResumableTransfer {
    /// The checkpoint as read from disk.
    pub checkpoint: TransferCheckpoint,
    /// On-disk path to the checkpoint file (so we can delete it after resume completes).
    pub checkpoint_path: std::path::PathBuf,
    /// Pre-validated: source still has the same SHA-256 prefix.
    pub source_unchanged: bool,
    /// Pre-validated: destination bytes match the CRC chain up to `bytes_written`.
    pub dest_intact: bool,
}

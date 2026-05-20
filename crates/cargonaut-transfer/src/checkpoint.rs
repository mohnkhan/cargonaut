// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn checkpoint_strategy() -> impl Strategy<Value = TransferCheckpoint> {
        (
            any::<u32>(),                                                   // version
            "[a-f0-9]{8}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{4}-[a-f0-9]{12}", // job_id (uuid-shaped)
            "(file|sftp|s3)://[a-zA-Z0-9._/-]{1,32}",                       // src_uri
            any::<u64>(),                                                   // src_size
            any::<[u8; 32]>(),                                              // src_sha256_prefix
            "(file|sftp|s3)://[a-zA-Z0-9._/-]{1,32}",                       // dst_uri
            any::<u64>(),                                                   // bytes_written
            proptest::collection::vec(any::<u32>(), 0..32),                 // chunk_crcs
            1u64..=(1024 * 1024 * 1024),                                    // chunk_size_bytes
            any::<u64>(),                                                   // created_at
            any::<u64>(),                                                   // last_update_at
        )
            .prop_map(
                |(
                    version,
                    job_id,
                    src_uri,
                    src_size,
                    src_sha256_prefix,
                    dst_uri,
                    bytes_written,
                    chunk_crcs,
                    chunk_size_bytes,
                    created_at,
                    last_update_at,
                )| TransferCheckpoint {
                    version,
                    job_id,
                    src_uri,
                    src_size,
                    src_sha256_prefix,
                    dst_uri,
                    bytes_written,
                    chunk_crcs,
                    chunk_size_bytes,
                    created_at,
                    last_update_at,
                },
            )
    }

    proptest! {
        #[test]
        fn roundtrip(cp in checkpoint_strategy()) {
            let json = serde_json::to_string(&cp).expect("serialize");
            let back: TransferCheckpoint = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(back, cp);
        }

        #[test]
        fn roundtrip_pretty(cp in checkpoint_strategy()) {
            // Pretty-printed form must also round-trip (used by `cargonaut audit`
            // dumps and any human-edited checkpoint file).
            let json = serde_json::to_string_pretty(&cp).expect("serialize");
            let back: TransferCheckpoint = serde_json::from_str(&json).expect("deserialize");
            prop_assert_eq!(back, cp);
        }
    }

    #[test]
    fn default_version_is_one() {
        // Constant — guards against silent schema-version bumps without
        // a migration path being added at the call site.
        assert_eq!(TransferCheckpoint::VERSION, 1);
    }

    #[test]
    fn empty_chunk_crcs_serializes() {
        // Newly-submitted job before the first checkpoint interval has
        // an empty CRC vec; must still round-trip.
        let cp = TransferCheckpoint {
            version: TransferCheckpoint::VERSION,
            job_id: "00000000-0000-0000-0000-000000000000".into(),
            src_uri: "file:///src".into(),
            src_size: 0,
            src_sha256_prefix: [0u8; 32],
            dst_uri: "file:///dst".into(),
            bytes_written: 0,
            chunk_crcs: vec![],
            chunk_size_bytes: 8 * 1024 * 1024,
            created_at: 0,
            last_update_at: 0,
        };
        let json = serde_json::to_string(&cp).unwrap();
        let back: TransferCheckpoint = serde_json::from_str(&json).unwrap();
        assert_eq!(cp, back);
    }
}

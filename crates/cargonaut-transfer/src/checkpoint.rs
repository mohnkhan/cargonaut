// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Transfer-checkpoint serialization + resume-offer discovery.

use super::job::TransferError;
use cargonaut_vfs::{ByteRange, LocalFs, Sort, VfsBackend, VfsPath};
use futures::AsyncReadExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::sync::Arc;

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

/// A resumable transfer found by [`scan_resumable`].
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

/// Scan a destination directory for orphan checkpoint sidecars
/// (`.cargonaut-transfer-*.json`), parse each, and pre-validate both
/// halves of the resume contract:
///
/// - **`source_unchanged`**: re-computes the SHA-256 of the first 1 MiB
///   of the recorded source and compares to the stored
///   [`TransferCheckpoint::src_sha256_prefix`]. Phase 1 only resolves
///   `file://` source URIs; other schemes mark the flag `false` until
///   the backend registry lands.
///
/// - **`dest_intact`**: re-reads the destination file from offset 0,
///   chunks it per [`TransferCheckpoint::chunk_size_bytes`], CRC32s
///   each chunk, and compares to the stored
///   [`TransferCheckpoint::chunk_crcs`] chain.
///
/// Sidecars whose schema version doesn't match
/// [`TransferCheckpoint::VERSION`], or which fail to parse, are silently
/// skipped — they predate this version of the engine and the user
/// should delete them by hand.
///
/// Listing failures bubble up as [`TransferError::Vfs`]; per-sidecar
/// read failures are absorbed (the offer just doesn't appear in the
/// returned vec).
pub async fn scan_resumable(
    dst_backend: Arc<dyn VfsBackend>,
    dst_dir: VfsPath,
) -> Result<Vec<ResumableTransfer>, TransferError> {
    let listing = dst_backend.list(&dst_dir, Sort::NameAsc).await?;
    let mut out = Vec::new();

    for entry in listing.entries {
        let name = entry.name.as_str();
        if !is_sidecar_name(name) {
            continue;
        }
        let cp_path = dst_dir.join(name);

        // Read the sidecar JSON.
        let bytes = match read_full(&*dst_backend, &cp_path).await {
            Ok(b) => b,
            Err(_) => continue,
        };
        let cp: TransferCheckpoint = match serde_json::from_slice(&bytes) {
            Ok(cp) => cp,
            Err(_) => continue,
        };
        if cp.version != TransferCheckpoint::VERSION {
            continue;
        }

        // Resolve source backend (Phase 1: file:// → LocalFs; others unverifiable).
        let src_uri = match VfsPath::parse(&cp.src_uri) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let source_unchanged = if src_uri.scheme.as_str() == "file" {
            let src_backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
            match compute_src_prefix_for_resume(&*src_backend, &src_uri).await {
                Ok(prefix) => prefix == cp.src_sha256_prefix,
                Err(_) => false,
            }
        } else {
            false
        };

        // Validate destination CRC chain.
        let dst_uri = match VfsPath::parse(&cp.dst_uri) {
            Ok(p) => p,
            Err(_) => continue,
        };
        let dest_intact = verify_dst_crc_chain(&*dst_backend, &dst_uri, &cp)
            .await
            .unwrap_or(false);

        out.push(ResumableTransfer {
            checkpoint: cp,
            checkpoint_path: vfs_path_to_local_pathbuf(&cp_path),
            source_unchanged,
            dest_intact,
        });
    }

    Ok(out)
}

fn is_sidecar_name(name: &str) -> bool {
    name.starts_with(".cargonaut-transfer-") && name.ends_with(".json")
}

/// Phase 1 helper: convert a `file:///abs/path` [`VfsPath`] to a
/// `PathBuf`. Bypasses backend dispatch — only sound for `file://`.
fn vfs_path_to_local_pathbuf(p: &VfsPath) -> std::path::PathBuf {
    let mut buf = std::path::PathBuf::from("/");
    for seg in &p.segments {
        buf.push(seg.as_str());
    }
    buf
}

async fn read_full(backend: &dyn VfsBackend, path: &VfsPath) -> Result<Vec<u8>, TransferError> {
    let mut reader = backend.read_stream(path, ByteRange::FULL).await?;
    let mut buf = Vec::new();
    reader
        .read_to_end(&mut buf)
        .await
        .map_err(|e| TransferError::Checkpoint(format!("read sidecar: {e}")))?;
    Ok(buf)
}

async fn compute_src_prefix_for_resume(
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

async fn verify_dst_crc_chain(
    backend: &dyn VfsBackend,
    dst: &VfsPath,
    cp: &TransferCheckpoint,
) -> Result<bool, TransferError> {
    if cp.chunk_size_bytes == 0 {
        return Ok(false);
    }
    if cp.chunk_crcs.is_empty() {
        return Ok(true);
    }
    let mut reader = backend.read_stream(dst, ByteRange::FULL).await?;
    let chunk_size = cp.chunk_size_bytes as usize;
    let mut buf = vec![0u8; chunk_size];
    for (i, expected) in cp.chunk_crcs.iter().enumerate() {
        let mut filled = 0;
        while filled < chunk_size {
            let n = reader
                .read(&mut buf[filled..])
                .await
                .map_err(|e| TransferError::Checkpoint(format!("read dst chunk {i}: {e}")))?;
            if n == 0 {
                break;
            }
            filled += n;
        }
        if filled < chunk_size {
            return Ok(false);
        }
        if crc32fast::hash(&buf[..filled]) != *expected {
            return Ok(false);
        }
    }
    Ok(true)
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

    // ============================================================
    // scan_resumable tests
    // ============================================================

    use std::path::Path;
    use tempfile::TempDir;
    use tokio::fs;
    use uuid::Uuid;

    fn vfs_path_for(p: &Path) -> VfsPath {
        VfsPath::parse(&format!("file://{}", p.to_str().expect("UTF-8 path"))).expect("parses")
    }

    /// Write a sidecar at the given path with the given checkpoint body.
    async fn write_sidecar_for(td: &TempDir, cp: &TransferCheckpoint) -> std::path::PathBuf {
        let path = td
            .path()
            .join(format!(".cargonaut-transfer-{}.json", cp.job_id));
        let json = serde_json::to_string_pretty(cp).unwrap();
        fs::write(&path, json).await.unwrap();
        path
    }

    /// Compute SHA-256 of the first up-to-1MiB of a file (matches the
    /// `submit_transfer` convention).
    async fn compute_prefix_of_file(p: &Path) -> [u8; 32] {
        let bytes = fs::read(p).await.unwrap();
        let prefix = &bytes[..bytes.len().min(1024 * 1024)];
        let mut h = Sha256::new();
        h.update(prefix);
        h.finalize().into()
    }

    /// Stage a happy-path resume scenario: src exists, dst has the first
    /// `bytes_written` bytes, sidecar matches.
    async fn stage_happy_resume(td: &TempDir) -> (std::path::PathBuf, TransferCheckpoint) {
        let src = td.path().join("src.bin");
        let dst = td.path().join("dst.bin");
        let payload: Vec<u8> = (0..16384u32).flat_map(u32::to_le_bytes).collect();
        fs::write(&src, &payload).await.unwrap();
        let written = 8192;
        fs::write(&dst, &payload[..written]).await.unwrap();
        let src_sha256_prefix = compute_prefix_of_file(&src).await;
        let cp = TransferCheckpoint {
            version: TransferCheckpoint::VERSION,
            job_id: Uuid::new_v4().to_string(),
            src_uri: format!("file://{}", src.to_str().unwrap()),
            src_size: payload.len() as u64,
            src_sha256_prefix,
            dst_uri: format!("file://{}", dst.to_str().unwrap()),
            bytes_written: written as u64,
            chunk_crcs: vec![crc32fast::hash(&payload[..written])],
            chunk_size_bytes: written as u64,
            created_at: 0,
            last_update_at: 0,
        };
        let sidecar = write_sidecar_for(td, &cp).await;
        (sidecar, cp)
    }

    #[tokio::test]
    async fn scan_empty_dir_returns_empty() {
        let td = TempDir::new().unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn scan_ignores_unrelated_files() {
        let td = TempDir::new().unwrap();
        fs::write(td.path().join("readme.txt"), b"x").await.unwrap();
        fs::write(td.path().join(".cargonaut-transfer-notjson"), b"x")
            .await
            .unwrap();
        fs::write(td.path().join("cargonaut-transfer-abc.json"), b"x")
            .await
            .unwrap(); // missing leading dot
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert!(v.is_empty(), "got: {v:?}");
    }

    #[tokio::test]
    async fn scan_finds_one_checkpoint_with_both_validations_passing() {
        let td = TempDir::new().unwrap();
        let (_sidecar, cp) = stage_happy_resume(&td).await;
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        let rt = &v[0];
        assert!(rt.source_unchanged);
        assert!(rt.dest_intact);
        assert_eq!(rt.checkpoint.job_id, cp.job_id);
    }

    #[tokio::test]
    async fn scan_marks_source_changed_after_src_modification() {
        let td = TempDir::new().unwrap();
        let (_sidecar, cp) = stage_happy_resume(&td).await;
        // Modify src — its SHA-256 prefix no longer matches the checkpoint.
        let src_path = std::path::PathBuf::from(cp.src_uri.strip_prefix("file://").unwrap());
        fs::write(&src_path, b"different content entirely")
            .await
            .unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        assert!(!v[0].source_unchanged);
    }

    #[tokio::test]
    async fn scan_marks_dest_corrupt_when_crc_mismatches() {
        let td = TempDir::new().unwrap();
        let (_sidecar, cp) = stage_happy_resume(&td).await;
        // Flip one byte of dst.
        let dst_path = std::path::PathBuf::from(cp.dst_uri.strip_prefix("file://").unwrap());
        let mut bytes = fs::read(&dst_path).await.unwrap();
        bytes[0] ^= 0xFF;
        fs::write(&dst_path, &bytes).await.unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        assert!(!v[0].dest_intact);
        assert!(v[0].source_unchanged); // src wasn't touched
    }

    #[tokio::test]
    async fn scan_marks_dest_corrupt_when_dst_truncated() {
        let td = TempDir::new().unwrap();
        let (_sidecar, cp) = stage_happy_resume(&td).await;
        // Truncate dst to fewer bytes than the checkpoint expects.
        let dst_path = std::path::PathBuf::from(cp.dst_uri.strip_prefix("file://").unwrap());
        fs::write(&dst_path, b"too short").await.unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert_eq!(v.len(), 1);
        assert!(!v[0].dest_intact);
    }

    #[tokio::test]
    async fn scan_skips_sidecars_with_wrong_schema_version() {
        let td = TempDir::new().unwrap();
        let (_sidecar, mut cp) = stage_happy_resume(&td).await;
        // Bump version + rewrite sidecar.
        cp.version = TransferCheckpoint::VERSION + 1;
        let new_path = td
            .path()
            .join(format!(".cargonaut-transfer-{}.json", cp.job_id));
        fs::write(&new_path, serde_json::to_string_pretty(&cp).unwrap())
            .await
            .unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert!(
            v.is_empty(),
            "future-version sidecar must be skipped, got: {v:?}"
        );
    }

    #[tokio::test]
    async fn scan_skips_malformed_json() {
        let td = TempDir::new().unwrap();
        let bogus = td.path().join(".cargonaut-transfer-bogus.json");
        fs::write(&bogus, b"{this is not json").await.unwrap();
        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert!(v.is_empty());
    }

    #[tokio::test]
    async fn scan_finds_multiple_checkpoints() {
        let td = TempDir::new().unwrap();
        let (_a, _) = stage_happy_resume(&td).await;
        // Stage a second concurrent transfer (different dst file).
        let src2 = td.path().join("src2.bin");
        let dst2 = td.path().join("dst2.bin");
        let payload: Vec<u8> = vec![0xAA; 1024];
        fs::write(&src2, &payload).await.unwrap();
        let written = 512;
        fs::write(&dst2, &payload[..written]).await.unwrap();
        let mut h = Sha256::new();
        h.update(&payload);
        let prefix: [u8; 32] = h.finalize().into();
        let cp2 = TransferCheckpoint {
            version: TransferCheckpoint::VERSION,
            job_id: Uuid::new_v4().to_string(),
            src_uri: format!("file://{}", src2.to_str().unwrap()),
            src_size: payload.len() as u64,
            src_sha256_prefix: prefix,
            dst_uri: format!("file://{}", dst2.to_str().unwrap()),
            bytes_written: written as u64,
            chunk_crcs: vec![crc32fast::hash(&payload[..written])],
            chunk_size_bytes: written as u64,
            created_at: 0,
            last_update_at: 0,
        };
        write_sidecar_for(&td, &cp2).await;

        let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let v = scan_resumable(backend, vfs_path_for(td.path()))
            .await
            .unwrap();
        assert_eq!(v.len(), 2);
        for rt in &v {
            assert!(rt.source_unchanged);
            assert!(rt.dest_intact);
        }
    }
}

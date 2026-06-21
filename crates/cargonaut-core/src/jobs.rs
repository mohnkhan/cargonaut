// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `jobs` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// FR-026 — user-facing snapshot of an in-flight transfer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressView {
    /// Bytes copied so far.
    pub bytes_done: u64,
    /// Total bytes to copy.
    pub bytes_total: u64,
    /// Estimated seconds remaining.
    pub eta_secs: u32,
    /// Current throughput in MiB/s.
    pub throughput_mibs: f32,
}

/// Feature 039 — UI-agnostic status of one transfer as rendered by the
/// tasks/jobs panel. Projects the transfer engine's `TransferState` (plus
/// the App's user-paused marker) into the states the panel distinguishes,
/// keeping `cargonaut-transfer`'s types out of the UI.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    /// Submitted, not yet progressing.
    Queued,
    /// In flight, with progress.
    Running {
        /// Bytes copied so far.
        bytes_done: u64,
        /// Total bytes to copy.
        bytes_total: u64,
        /// Estimated seconds remaining.
        eta_secs: u32,
        /// Current throughput in MiB/s.
        throughput_mibs: f32,
    },
    /// Paused by the user (resumable).
    Paused,
    /// Finished; `verified` mirrors the post-copy SHA-256 check.
    Completed {
        /// True if the destination SHA-256 matched the source.
        verified: bool,
    },
    /// Errored; `resumable` is true if a checkpoint exists for retry.
    Failed {
        /// Can the transfer be resumed from its checkpoint?
        resumable: bool,
    },
    /// Cancelled by the user (distinct from [`JobStatus::Paused`]).
    Cancelled,
}

/// Feature 039 — UI-facing projection of one registry transfer (one row of
/// the tasks/jobs panel). Built by [`App::job_views`]; mirrors the role of
/// [`ProgressView`] / [`ResumeOfferView`] so the UI never touches the
/// transfer crate's types.
#[derive(Debug, Clone, PartialEq)]
pub struct JobView {
    /// Stable identity; the target of per-row actions.
    pub id: TransferId,
    /// Source path/URI for display (caller may shorten).
    pub src: String,
    /// Destination path/URI for display.
    pub dst: String,
    /// Copy or move.
    pub mode: TransferMode,
    /// Classified status, including progress.
    pub status: JobStatus,
}

/// Feature 037 — UI-agnostic projection of one resumable transfer found
/// on launch. Lets the UI build its resume prompt without depending on
/// the transfer crate's types (mirrors [`ProgressView`]). The UI maps
/// this onto its own per-row summary widget.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumeOfferView {
    /// Source URI/path, as recorded in the checkpoint.
    pub src: String,
    /// Destination URI/path, as recorded in the checkpoint.
    pub dst: String,
    /// Bytes already written, expressed in MiB.
    pub bytes_written_mib: f32,
    /// Source size, expressed in MiB.
    pub src_size_mib: f32,
    /// True if the source's content fingerprint is unchanged since the
    /// checkpoint was written.
    pub source_unchanged: bool,
    /// True if the partial destination still matches the checkpoint's
    /// integrity chain.
    pub dest_intact: bool,
}

/// Project a [`ResumableTransfer`] into the UI-facing [`ResumeOfferView`].
pub(crate) fn resume_offer_view(rt: &ResumableTransfer) -> ResumeOfferView {
    const MIB: f32 = 1024.0 * 1024.0;
    ResumeOfferView {
        src: rt.checkpoint.src_uri.clone(),
        dst: rt.checkpoint.dst_uri.clone(),
        bytes_written_mib: rt.checkpoint.bytes_written as f32 / MIB,
        src_size_mib: rt.checkpoint.src_size as f32 / MIB,
        source_unchanged: rt.source_unchanged,
        dest_intact: rt.dest_intact,
    }
}

/// Snapshot helper: poll a `TransferJob::state` once. Mostly for the
/// UI to render the current state without owning a `watch::Receiver`
/// borrow across awaits.
pub fn transfer_state_snapshot(job: &TransferJob) -> TransferState {
    job.state.borrow().clone()
}

/// Feature 039 — classify a raw `TransferState` into the panel's
/// [`JobStatus`], honoring the user-paused marker. A paused, still-active
/// transfer renders as `Paused`; one that already reached a terminal state
/// keeps that terminal status (defensive: a finished job is never shown as
/// paused).
pub(crate) fn job_status_from(raw: TransferState, paused: bool) -> JobStatus {
    match raw {
        TransferState::Completed { sha256_match } => JobStatus::Completed {
            verified: sha256_match,
        },
        TransferState::Failed { resumable, .. } => JobStatus::Failed { resumable },
        TransferState::Canceled if paused => JobStatus::Paused,
        TransferState::Canceled => JobStatus::Cancelled,
        _ if paused => JobStatus::Paused,
        TransferState::Queued => JobStatus::Queued,
        TransferState::Paused => JobStatus::Paused,
        TransferState::Running {
            bytes_done,
            bytes_total,
            eta_secs,
            throughput_mibs,
        } => JobStatus::Running {
            bytes_done,
            bytes_total,
            eta_secs,
            throughput_mibs,
        },
    }
}

/// CRC32 hash of a file's content for directory comparison (FR-002 / R-001 / R-002).
///
/// Strategy: for files ≤ 4 MiB, hashes the full content. For larger files,
/// hashes only the first 512 KiB (head-only), trading accuracy for speed.
/// Returns `None` on any I/O error so the caller can classify as "unreadable".
pub(crate) fn crc32_partial(path: &std::path::Path, size: u64) -> Option<u32> {
    const FULL_THRESHOLD: u64 = 4 * 1024 * 1024;
    const HEAD_BYTES: usize = 512 * 1024;

    if size <= FULL_THRESHOLD {
        let data = std::fs::read(path).ok()?;
        Some(crc32fast::hash(&data))
    } else {
        use std::io::Read;
        let mut f = std::fs::File::open(path).ok()?;
        let mut buf = vec![0u8; HEAD_BYTES];
        let n = f.read(&mut buf).ok()?;
        Some(crc32fast::hash(&buf[..n]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[test]
    fn crc32_partial_same_content_same_hash() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"hello world").unwrap();
        std::fs::write(&b, b"hello world").unwrap();
        let ha = crc32_partial(&a, 11);
        let hb = crc32_partial(&b, 11);
        assert!(ha.is_some());
        assert_eq!(ha, hb, "identical content must produce identical hash");
    }

    #[test]
    fn crc32_partial_different_content_different_hash() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a");
        let b = dir.path().join("b");
        std::fs::write(&a, b"aaaaaaa").unwrap();
        std::fs::write(&b, b"bbbbbbb").unwrap();
        let ha = crc32_partial(&a, 7);
        let hb = crc32_partial(&b, 7);
        assert!(ha.is_some());
        assert!(hb.is_some());
        assert_ne!(ha, hb, "different content must produce different hash");
    }

    #[test]
    fn crc32_partial_large_file_uses_head_only() {
        // File is 5 MiB (> 4 MiB threshold) — we only read the first 512 KiB.
        // Two 5 MiB files with identical first 512 KiB but different tails
        // must produce the same hash.
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("a_large");
        let b = dir.path().join("b_large");
        let five_mib = 5 * 1024 * 1024;
        let head = vec![0xABu8; 512 * 1024];
        let tail_a = vec![0x11u8; five_mib - 512 * 1024];
        let tail_b = vec![0x22u8; five_mib - 512 * 1024];
        let mut content_a = head.clone();
        content_a.extend_from_slice(&tail_a);
        let mut content_b = head.clone();
        content_b.extend_from_slice(&tail_b);
        std::fs::write(&a, &content_a).unwrap();
        std::fs::write(&b, &content_b).unwrap();
        let ha = crc32_partial(&a, five_mib as u64);
        let hb = crc32_partial(&b, five_mib as u64);
        assert!(ha.is_some());
        assert_eq!(
            ha, hb,
            "large files: same head 512 KiB => same hash (head-only strategy)"
        );
    }

    #[test]
    fn crc32_partial_unreadable_path_returns_none() {
        let nonexistent = std::path::Path::new("/tmp/cargonaut_test_nonexistent_xyz_f049");
        assert_eq!(crc32_partial(nonexistent, 0), None);
    }

    #[test]
    fn crc32_partial_empty_file_consistent_hash() {
        let dir = tempfile::tempdir().unwrap();
        let a = dir.path().join("empty_a");
        let b = dir.path().join("empty_b");
        std::fs::write(&a, b"").unwrap();
        std::fs::write(&b, b"").unwrap();
        let ha = crc32_partial(&a, 0);
        let hb = crc32_partial(&b, 0);
        assert_eq!(ha, hb, "empty files must produce same hash");
        assert!(ha.is_some(), "empty file should not return None");
    }
}

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! The [`VfsBackend`] trait — the contract every adapter (local, sftp, s3, …)
//! implements, plus the small helper types (`VfsCaps`, `WriteMode`,
//! `ByteRange`) that travel with it.
//!
//! ## Why this trait exists
//!
//! Cargonaut's higher layers (the transfer engine, the UI, the audit log)
//! never talk to a concrete filesystem. They hold an
//! `Arc<dyn VfsBackend>` and a [`VfsPath`] (the
//! [`VfsRef`](../../design/data-model.md) pair) and dispatch through this
//! trait. Adding a new backend is therefore a matter of implementing
//! `VfsBackend` and registering the scheme — no other crate has to change.
//!
//! ## What backends must guarantee
//!
//! Beyond per-method semantics documented below, two cross-cutting rules apply:
//!
//! 1. **Object-safety.** The trait is used through `Arc<dyn VfsBackend>`
//!    (pinned by `tests/dyn_dispatch.rs`). Implementations therefore cannot
//!    add generic methods or `Self`-returning methods.
//! 2. **Capability honesty.** [`VfsBackend::caps`] is consulted by the transfer
//!    engine to pick a strategy. Backends MUST NOT silently emulate a
//!    capability they don't advertise (e.g. faking `ATOMIC_RENAME` via
//!    copy+delete). Emulation is a higher-layer concern; the trait surfaces
//!    raw capability so callers can make the trade-off.

use super::error::VfsError;
use super::types::{DirListing, Sort, VfsMetadata, VfsPath};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::pin::Pin;

bitflags::bitflags! {
    /// Capability flags a backend declares it supports.
    ///
    /// Used by the transfer engine to pick the most efficient transfer
    /// strategy and by the UI to enable/disable features (e.g. hide the
    /// "resume" affordance when neither end is `RANDOM_WRITE`).
    ///
    /// **Stability**: [`VfsBackend::caps`] MUST return the same value for
    /// every call on the same instance; callers cache it.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VfsCaps: u32 {
        /// Backend supports partial-range reads (HTTP-style `Range` requests).
        /// Required for [`VfsBackend::read_stream`] with a non-default
        /// [`ByteRange`].
        const SEEKABLE = 0b0000_0001;
        /// Backend supports write-at-offset (resumable uploads).
        /// Required for [`WriteMode::AppendAtOffset`].
        const RANDOM_WRITE = 0b0000_0010;
        /// Backend reports Unix-style permissions and ownership in
        /// [`VfsMetadata::mode`]. Backends without this cap leave `mode = None`.
        const METADATA_RICH = 0b0000_0100;
        /// Backend supports `rename` atomically within the same authority
        /// (POSIX `rename(2)` semantics).
        const ATOMIC_RENAME = 0b0000_1000;
        /// Backend reports symlinks as [`crate::types::VfsKind::Symlink`]
        /// (without following them).
        const SYMLINKS = 0b0001_0000;
    }
}

/// What to do if the destination file already exists when opening for write.
///
/// Defines the precondition checks [`VfsBackend::write_stream`] applies before
/// returning a writer; see that method's docs for the full semantics matrix.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Fail if the file exists. `offset` MUST be `0`.
    CreateNew,
    /// Truncate to zero length first (creating if missing). `offset` MUST be `0`.
    Truncate,
    /// Open at the given offset; assume the prefix is valid (resume case).
    /// Requires [`VfsCaps::RANDOM_WRITE`].
    AppendAtOffset,
}

/// Half-open byte range for partial reads.
///
/// - `start` is inclusive.
/// - `end` is exclusive. `None` means "to EOF".
/// - `end == Some(start)` is an empty range; the reader yields EOF immediately.
/// - `start > file_size` yields an at-EOF reader (not an error).
/// - `end > file_size` is clamped to EOF.
///
/// `ByteRange { start: 0, end: None }` is the canonical "whole file" request
/// and every backend MUST honor it regardless of [`VfsCaps::SEEKABLE`].
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    /// First byte offset (inclusive).
    pub start: u64,
    /// Last byte offset (exclusive). `None` means EOF.
    pub end: Option<u64>,
}

impl ByteRange {
    /// `ByteRange { start: 0, end: None }` — the whole-file request.
    pub const FULL: ByteRange = ByteRange {
        start: 0,
        end: None,
    };
}

/// The contract every VFS backend implements.
///
/// All methods are async. Implementations must be `Send + Sync + 'static` so
/// they can travel inside `Arc<dyn VfsBackend>`.
///
/// ## Error mapping
///
/// Every method returns [`VfsError`]. The canonical mapping is:
///
/// | Condition                                                     | Variant                       |
/// |---------------------------------------------------------------|-------------------------------|
/// | Path does not exist                                           | [`VfsError::NotFound`]        |
/// | OS / remote denied the operation                              | [`VfsError::PermissionDenied`]|
/// | Caller asked for a capability the backend doesn't advertise   | [`VfsError::Unsupported`]     |
/// | Transport / syscall failure with an underlying `io::Error`    | [`VfsError::Io`]              |
/// | Credential failure (remote backends)                          | [`VfsError::AuthFailed`]      |
/// | Anything else                                                 | [`VfsError::Other`]           |
///
/// Backends SHOULD prefer the most specific variant. In particular,
/// classifying "the path you gave me is the wrong kind of object" (e.g.
/// passing a directory to [`Self::unlink`]) as `Unsupported` lets the UI
/// surface a useful message without inspecting the inner `io::Error`.
#[async_trait]
pub trait VfsBackend: Send + Sync + 'static {
    /// The URI scheme this backend handles, e.g. `"file"` for LocalFs,
    /// `"sftp"` for the SFTP adapter.
    ///
    /// **Invariants:**
    /// - The returned `&'static str` MUST be the same value for every call
    ///   on the same instance (the VFS registry keys on it).
    /// - It MUST match the `scheme` component of every [`VfsPath`] the
    ///   backend accepts; calls with a mismatched scheme MUST return
    ///   [`VfsError::Unsupported`].
    fn scheme(&self) -> &'static str;

    /// The capability bitset this backend supports.
    ///
    /// **Invariants:**
    /// - Stable per instance (callers cache).
    /// - Self-consistent: every cap claimed here MUST work in the
    ///   corresponding method. E.g. claiming [`VfsCaps::RANDOM_WRITE`]
    ///   means `write_stream(_, _, AppendAtOffset)` will not return
    ///   [`VfsError::Unsupported`] for that reason.
    fn caps(&self) -> VfsCaps;

    /// List the directory at `path`, returning a pre-sorted snapshot.
    ///
    /// **Semantics:**
    /// - The returned [`DirListing::entries`] is already sorted per `sort`;
    ///   the UI does not re-sort.
    /// - Hidden entries (Unix dotfiles) are included; UI-level masking is
    ///   the job of higher layers (`FR-015`).
    /// - The listing is a one-shot snapshot. Backends MUST NOT hold a
    ///   directory handle open after return or stream further entries
    ///   into the returned vector.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `path` does not exist.
    /// - [`VfsError::Unsupported`] if `path` exists but is not a directory.
    /// - [`VfsError::PermissionDenied`] on OS-level reject.
    /// - [`VfsError::Io`] / [`VfsError::Other`] on transport faults.
    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError>;

    /// Return metadata for the entry at `path`.
    ///
    /// **Semantics:**
    /// - Symlinks MUST be reported as
    ///   [`crate::types::VfsKind::Symlink { target }`](crate::types::VfsKind::Symlink)
    ///   without being followed. Callers that want the resolved target
    ///   call `stat` again on the target.
    /// - A symlink to a missing target is still reported as `Symlink`,
    ///   not `NotFound`.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `path` does not exist.
    /// - [`VfsError::PermissionDenied`] on OS-level reject.
    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError>;

    /// Open a streaming reader for `path` over the byte range `range`.
    ///
    /// **Semantics:**
    /// - [`ByteRange::FULL`] (whole-file) MUST succeed for every backend
    ///   regardless of [`VfsCaps::SEEKABLE`].
    /// - Any other range requires [`VfsCaps::SEEKABLE`]; backends without
    ///   that cap MUST return [`VfsError::Unsupported`].
    /// - `range.start > file_size` yields a reader at EOF (not an error),
    ///   which simplifies resumable transfers when the source has shrunk.
    /// - `range.end > file_size` is clamped to EOF.
    /// - The returned reader has no implicit timeout; cancellation is the
    ///   caller's responsibility (drop the future or its task).
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `path` does not exist.
    /// - [`VfsError::Unsupported`] if `path` is a directory, or if `range`
    ///   is non-trivial but the backend lacks `SEEKABLE`.
    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError>;

    /// Open a streaming writer for `path` with the given offset + mode.
    ///
    /// **Semantics by [`WriteMode`]:**
    /// - [`WriteMode::CreateNew`][]: fails (typically `Io` mapping to `EEXIST`)
    ///   if `path` already exists. `offset` MUST be `0`.
    /// - [`WriteMode::Truncate`][]: creates `path` if missing; if it exists,
    ///   truncates to zero before writing. `offset` MUST be `0`.
    /// - [`WriteMode::AppendAtOffset`][]: requires [`VfsCaps::RANDOM_WRITE`];
    ///   opens for write at `offset`, leaving the prefix in place. Used by
    ///   the transfer engine for resumable copies.
    ///
    /// **Preconditions:**
    /// - The parent directory MUST already exist; backends MUST NOT
    ///   autocreate it. Callers needing `mkdir -p` semantics call
    ///   [`Self::mkdir`] first.
    ///
    /// **Lifecycle:**
    /// - Dropping the writer flushes. Callers SHOULD explicitly
    ///   `flush().await` for backends where drop-time errors are silenced
    ///   (most remote backends).
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if the parent directory is missing.
    /// - [`VfsError::PermissionDenied`] on OS-level reject.
    /// - [`VfsError::Unsupported`] if `mode == AppendAtOffset` without
    ///   `RANDOM_WRITE`, or `offset != 0` for `CreateNew` / `Truncate`.
    async fn write_stream(
        &self,
        path: &VfsPath,
        offset: u64,
        mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError>;

    /// Remove a regular file or symlink at `path`.
    ///
    /// **Semantics:**
    /// - Symlinks are removed without following — the link itself goes
    ///   away, the target is untouched.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `path` does not exist.
    /// - [`VfsError::Unsupported`] if `path` is a directory (callers must
    ///   use [`Self::rmdir`]; recursive removal is built on `list` +
    ///   `unlink`/`rmdir` at a higher layer).
    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Remove the empty directory at `path`.
    ///
    /// **Semantics:**
    /// - The directory MUST be empty. Non-empty calls MUST fail (typically
    ///   `Io` mapping to `ENOTEMPTY`); recursive removal is a higher-level
    ///   operation built on `list` + `unlink`/`rmdir`.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `path` does not exist.
    /// - [`VfsError::Unsupported`] if `path` is not a directory.
    async fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Move / rename `src` to `dest`, both within the same backend authority.
    ///
    /// **Semantics:**
    /// - `src` and `dest` MUST share the same scheme **and** authority.
    ///   Cross-authority moves (`file://` → `sftp://`, two different SFTP
    ///   hosts, etc.) MUST return [`VfsError::Unsupported`] — the transfer
    ///   engine (`cargonaut-transfer`) handles those by copy + delete.
    /// - Backends advertising [`VfsCaps::ATOMIC_RENAME`] MUST perform the
    ///   rename atomically (POSIX `rename(2)` semantics): no observable
    ///   state where both names exist or neither exists.
    /// - Overwrite policy follows POSIX on `LocalFs` (silently overwrites
    ///   a regular file at `dest`, fails if `dest` is a non-empty
    ///   directory). Remote backends may differ; document per adapter.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] if `src` does not exist.
    /// - [`VfsError::Unsupported`] for cross-authority moves.
    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError>;

    /// Create a directory at `path`.
    ///
    /// **Semantics:**
    /// - `recursive = false`: MUST fail if any parent is missing, OR if
    ///   `path` already exists.
    /// - `recursive = true`: equivalent to `mkdir -p`. MUST succeed if
    ///   `path` already exists as a directory; MUST fail if `path` exists
    ///   as a non-directory.
    ///
    /// **Errors:**
    /// - [`VfsError::NotFound`] (non-recursive) if any parent is missing.
    /// - [`VfsError::PermissionDenied`] on OS-level reject.
    /// - [`VfsError::Io`] if `path` exists but isn't a directory (recursive).
    async fn mkdir(&self, path: &VfsPath, recursive: bool) -> Result<(), VfsError>;
}

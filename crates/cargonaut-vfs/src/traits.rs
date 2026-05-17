//! The `VfsBackend` trait that every adapter (local, sftp, s3, ...) implements.

use super::types::{DirListing, Sort, VfsMetadata, VfsPath};
use super::error::VfsError;
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::pin::Pin;

bitflags::bitflags! {
    /// Capability flags a backend declares it supports.
    /// Used by the transfer engine to pick the most efficient transfer
    /// strategy + by the UI to enable/disable features.
    #[derive(Debug, Clone, Copy, PartialEq, Eq)]
    pub struct VfsCaps: u32 {
        /// Backend supports partial-range reads (`Range` requests).
        const SEEKABLE = 0b0000_0001;
        /// Backend supports write-at-offset (resumable uploads).
        const RANDOM_WRITE = 0b0000_0010;
        /// Backend reports Unix-style permissions.
        const METADATA_RICH = 0b0000_0100;
        /// Backend supports `rename` atomically within the same authority.
        const ATOMIC_RENAME = 0b0000_1000;
        /// Backend supports symlinks.
        const SYMLINKS = 0b0001_0000;
    }
}

/// What to do if the destination file already exists when opening for write.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteMode {
    /// Fail if the file exists.
    CreateNew,
    /// Truncate to zero length first.
    Truncate,
    /// Open at the given offset; assume the prefix is valid (resume).
    AppendAtOffset,
}

/// Byte range for partial reads.
#[derive(Debug, Clone, Copy)]
pub struct ByteRange {
    /// First byte offset (inclusive).
    pub start: u64,
    /// Last byte offset (exclusive). `None` means EOF.
    pub end: Option<u64>,
}

/// The contract every VFS backend implements.
///
/// All methods are async. Implementations must be `Send + Sync + 'static`.
/// Bounds-checks + capability rejections produce [`VfsError`]; in particular,
/// a backend that lacks [`VfsCaps::RANDOM_WRITE`] must return
/// [`VfsError::Unsupported`] from `write_stream` when called with
/// [`WriteMode::AppendAtOffset`].
#[async_trait]
pub trait VfsBackend: Send + Sync + 'static {
    /// The URI scheme this backend handles, e.g. `"file"`.
    fn scheme(&self) -> &'static str;

    /// Capabilities this backend supports.
    fn caps(&self) -> VfsCaps;

    /// List a directory.
    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError>;

    /// Stat a single entry.
    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError>;

    /// Open a read stream for `path` over `range`.
    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError>;

    /// Open a write stream for `path` at `offset` with `mode` semantics.
    async fn write_stream(
        &self,
        path: &VfsPath,
        offset: u64,
        mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError>;

    /// Remove a regular file. Fails for directories — caller must use [`Self::rmdir`].
    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Remove an empty directory.
    async fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError>;

    /// Atomically (per `ATOMIC_RENAME` cap) move/rename within the backend.
    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError>;

    /// Create a directory; `recursive` = `mkdir -p`.
    async fn mkdir(&self, path: &VfsPath, recursive: bool) -> Result<(), VfsError>;
}

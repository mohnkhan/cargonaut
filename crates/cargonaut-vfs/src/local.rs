//! `LocalFs` — the file-system backend.
//!
//! Phase 1 implementation skeleton. Full body in T1.06.

use super::error::VfsError;
use super::traits::{ByteRange, VfsBackend, VfsCaps, WriteMode};
use super::types::{DirListing, Sort, VfsMetadata, VfsPath};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::pin::Pin;

/// File-system backend over `tokio::fs`.
pub struct LocalFs;

impl LocalFs {
    /// Create a new instance. No state — instances are cheap.
    pub fn new() -> Self {
        Self
    }
}

impl Default for LocalFs {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl VfsBackend for LocalFs {
    fn scheme(&self) -> &'static str {
        "file"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::SEEKABLE
            | VfsCaps::RANDOM_WRITE
            | VfsCaps::METADATA_RICH
            | VfsCaps::ATOMIC_RENAME
            | VfsCaps::SYMLINKS
    }

    async fn list(&self, _path: &VfsPath, _sort: Sort) -> Result<DirListing, VfsError> {
        unimplemented!("T1.06 — see design/tasks.md")
    }

    async fn stat(&self, _path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        unimplemented!("T1.06")
    }

    async fn read_stream(
        &self,
        _path: &VfsPath,
        _range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        unimplemented!("T1.06")
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        unimplemented!("T1.06")
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        unimplemented!("T1.06")
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        unimplemented!("T1.06")
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        unimplemented!("T1.06")
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        unimplemented!("T1.06")
    }
}

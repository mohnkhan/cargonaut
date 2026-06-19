// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`ZipFs`] — read-only [`VfsBackend`] for `zip://` paths.
//!
//! Stub implementation — replaced in Phase 3 (Task T013).

use crate::{ByteRange, DirListing, Sort, VfsBackend, VfsCaps, VfsError, VfsMetadata, VfsPath, WriteMode};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::path::PathBuf;
use std::pin::Pin;

/// Read-only VFS backend that exposes a ZIP archive as a directory tree.
///
/// Full implementation: Task T013 (Phase 3).
#[allow(dead_code)]
#[derive(Debug)]
pub struct ZipFs {
    /// Absolute path to the archive file on the host filesystem.
    pub(crate) archive_path: PathBuf,
}

impl ZipFs {
    /// Open an archive at the given host filesystem path.
    ///
    /// Full implementation: Task T013.
    pub fn open(_archive_path: PathBuf) -> Result<Self, VfsError> {
        Err(VfsError::Other("ZipFs not yet implemented (stub — Task T013)".to_string()))
    }
}

#[async_trait]
impl VfsBackend for ZipFs {
    fn scheme(&self) -> &'static str {
        "zip"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::empty()
    }

    async fn list(&self, _path: &VfsPath, _sort: Sort) -> Result<DirListing, VfsError> {
        Err(VfsError::Other("ZipFs not yet implemented".to_string()))
    }

    async fn stat(&self, _path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        Err(VfsError::Other("ZipFs not yet implemented".to_string()))
    }

    async fn read_stream(
        &self,
        _path: &VfsPath,
        _range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        Err(VfsError::Other("ZipFs not yet implemented".to_string()))
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }
}

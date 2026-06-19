// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`SftpFs`] — read/write [`VfsBackend`] for `sftp://` paths.
//!
//! Stub implementation — replaced in Phase 5 (Tasks T024-T025).

use crate::{ByteRange, DirListing, Sort, VfsBackend, VfsCaps, VfsError, VfsMetadata, VfsPath, WriteMode};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::pin::Pin;

/// Read/write VFS backend for SFTP servers (sftp:// scheme).
///
/// Full implementation: Tasks T024-T025 (Phase 5).
#[allow(dead_code)]
#[derive(Debug)]
pub struct SftpFs {
    /// `"user@host:port"` authority string.
    pub(crate) authority: smol_str::SmolStr,
}

#[async_trait]
impl VfsBackend for SftpFs {
    fn scheme(&self) -> &'static str {
        "sftp"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::SEEKABLE
            | VfsCaps::RANDOM_WRITE
            | VfsCaps::METADATA_RICH
            | VfsCaps::ATOMIC_RENAME
            | VfsCaps::SYMLINKS
    }

    async fn list(&self, _path: &VfsPath, _sort: Sort) -> Result<DirListing, VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn stat(&self, _path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn read_stream(
        &self,
        _path: &VfsPath,
        _range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        Err(VfsError::Other("SftpFs not yet implemented".to_string()))
    }
}

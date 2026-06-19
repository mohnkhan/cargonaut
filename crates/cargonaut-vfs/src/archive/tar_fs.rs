// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`TarFs`] — read-only [`VfsBackend`] for `tar://` paths.
//!
//! Stub implementation — replaced in Phase 4 (Task T020).

use crate::{ByteRange, DirListing, Sort, VfsBackend, VfsCaps, VfsError, VfsMetadata, VfsPath, WriteMode};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use std::path::PathBuf;
use std::pin::Pin;

/// Compression codec detected from the archive file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarCompression {
    /// Uncompressed `.tar`.
    None,
    /// Gzip-compressed `.tar.gz` / `.tgz`.
    Gz,
    /// Bzip2-compressed `.tar.bz2` / `.tbz2`.
    Bz2,
    /// XZ-compressed `.tar.xz` / `.txz`.
    Xz,
}

impl TarCompression {
    /// Detect compression from a file extension (case-insensitive).
    pub fn from_extension(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar") {
            Some(Self::None)
        } else if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::Gz)
        } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
            Some(Self::Bz2)
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            Some(Self::Xz)
        } else {
            None
        }
    }
}

/// Read-only VFS backend that exposes a TAR archive as a directory tree.
///
/// Full implementation: Task T020 (Phase 4).
#[allow(dead_code)]
#[derive(Debug)]
pub struct TarFs {
    /// Absolute path to the archive file on the host filesystem.
    pub(crate) archive_path: PathBuf,
    /// Detected compression codec.
    pub(crate) compression: TarCompression,
}

impl TarFs {
    /// Open an archive at the given host filesystem path with the given compression.
    ///
    /// Full implementation: Task T020.
    pub fn open(_archive_path: PathBuf, _compression: TarCompression) -> Result<Self, VfsError> {
        Err(VfsError::Other("TarFs not yet implemented (stub — Task T020)".to_string()))
    }
}

#[async_trait]
impl VfsBackend for TarFs {
    fn scheme(&self) -> &'static str {
        "tar"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::empty()
    }

    async fn list(&self, _path: &VfsPath, _sort: Sort) -> Result<DirListing, VfsError> {
        Err(VfsError::Other("TarFs not yet implemented".to_string()))
    }

    async fn stat(&self, _path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        Err(VfsError::Other("TarFs not yet implemented".to_string()))
    }

    async fn read_stream(
        &self,
        _path: &VfsPath,
        _range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        Err(VfsError::Other("TarFs not yet implemented".to_string()))
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }
}

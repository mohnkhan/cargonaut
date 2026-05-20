// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::VfsError;
    use crate::types::{Sort, VfsKind};
    use crate::{ByteRange, VfsCaps, WriteMode};
    use futures::{AsyncReadExt, AsyncWriteExt};
    use std::path::Path;
    use tempfile::TempDir;
    use tokio::fs;

    fn vfs_path_for(p: &Path) -> VfsPath {
        let s = p.to_str().expect("test path is UTF-8");
        VfsPath::parse(&format!("file://{}", s)).expect("test path parses")
    }

    async fn read_all(mut r: std::pin::Pin<Box<dyn futures::AsyncRead + Send>>) -> Vec<u8> {
        let mut buf = Vec::new();
        r.read_to_end(&mut buf).await.expect("read_to_end");
        buf
    }

    // ---------- caps + scheme (cheap regression) ----------

    #[tokio::test]
    async fn caps_includes_seekable_random_write_atomic_rename() {
        let backend = LocalFs::new();
        let caps = backend.caps();
        assert!(caps.contains(VfsCaps::SEEKABLE));
        assert!(caps.contains(VfsCaps::RANDOM_WRITE));
        assert!(caps.contains(VfsCaps::ATOMIC_RENAME));
    }

    // ---------- list ----------

    #[tokio::test]
    async fn list_empty_dir() {
        let td = TempDir::new().unwrap();
        let listing = LocalFs::new()
            .list(&vfs_path_for(td.path()), Sort::NameAsc)
            .await
            .unwrap();
        assert_eq!(listing.entries.len(), 0);
        assert_eq!(listing.sort, Sort::NameAsc);
    }

    #[tokio::test]
    async fn list_populated_dir_sorted_name_asc() {
        let td = TempDir::new().unwrap();
        for n in ["banana", "apple", "cherry"] {
            fs::write(td.path().join(n), b"x").await.unwrap();
        }
        fs::create_dir(td.path().join("subdir")).await.unwrap();
        let listing = LocalFs::new()
            .list(&vfs_path_for(td.path()), Sort::NameAsc)
            .await
            .unwrap();
        let names: Vec<_> = listing.entries.iter().map(|e| e.name.as_str()).collect();
        assert_eq!(names, vec!["apple", "banana", "cherry", "subdir"]);
    }

    #[tokio::test]
    async fn list_large_dir_500_entries() {
        let td = TempDir::new().unwrap();
        for i in 0..500u32 {
            fs::write(td.path().join(format!("f{i:04}")), b"")
                .await
                .unwrap();
        }
        let listing = LocalFs::new()
            .list(&vfs_path_for(td.path()), Sort::NameAsc)
            .await
            .unwrap();
        assert_eq!(listing.entries.len(), 500);
        assert_eq!(listing.entries[0].name.as_str(), "f0000");
        assert_eq!(listing.entries[499].name.as_str(), "f0499");
    }

    #[tokio::test]
    async fn list_missing_path_returns_not_found() {
        let td = TempDir::new().unwrap();
        let err = LocalFs::new()
            .list(&vfs_path_for(&td.path().join("nope")), Sort::NameAsc)
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn list_on_regular_file_is_unsupported() {
        let td = TempDir::new().unwrap();
        let file = td.path().join("f.txt");
        fs::write(&file, b"x").await.unwrap();
        let err = LocalFs::new()
            .list(&vfs_path_for(&file), Sort::NameAsc)
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::Unsupported(_)), "got: {err:?}");
    }

    // ---------- stat ----------

    #[tokio::test]
    async fn stat_regular_file_reports_size() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("hello.txt");
        fs::write(&p, b"hello").await.unwrap();
        let meta = LocalFs::new().stat(&vfs_path_for(&p)).await.unwrap();
        assert_eq!(meta.size, 5);
        assert_eq!(meta.kind, VfsKind::File);
    }

    #[tokio::test]
    async fn stat_directory_reports_dir_kind() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("d");
        fs::create_dir(&p).await.unwrap();
        let meta = LocalFs::new().stat(&vfs_path_for(&p)).await.unwrap();
        assert_eq!(meta.kind, VfsKind::Dir);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn stat_symlink_reports_symlink_kind_without_following() {
        use std::os::unix::fs::symlink;
        let td = TempDir::new().unwrap();
        let target = td.path().join("target.txt");
        fs::write(&target, b"x").await.unwrap();
        let link = td.path().join("link");
        symlink(&target, &link).unwrap();
        let meta = LocalFs::new().stat(&vfs_path_for(&link)).await.unwrap();
        match &meta.kind {
            VfsKind::Symlink { target: t } => {
                assert!(
                    t.display().ends_with("target.txt"),
                    "target: {}",
                    t.display()
                );
            }
            other => panic!("expected Symlink, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn stat_missing_returns_not_found() {
        let td = TempDir::new().unwrap();
        let err = LocalFs::new()
            .stat(&vfs_path_for(&td.path().join("nope")))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)), "got: {err:?}");
    }

    // ---------- read_stream ----------

    #[tokio::test]
    async fn read_stream_full_file() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("data.bin");
        let payload: Vec<u8> = (0..=255u8).collect();
        fs::write(&p, &payload).await.unwrap();
        let r = LocalFs::new()
            .read_stream(&vfs_path_for(&p), ByteRange::FULL)
            .await
            .unwrap();
        assert_eq!(read_all(r).await, payload);
    }

    #[tokio::test]
    async fn read_stream_range() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("data.bin");
        let payload: Vec<u8> = (0..200u8).collect();
        fs::write(&p, &payload).await.unwrap();
        let r = LocalFs::new()
            .read_stream(
                &vfs_path_for(&p),
                ByteRange {
                    start: 50,
                    end: Some(150),
                },
            )
            .await
            .unwrap();
        assert_eq!(read_all(r).await, payload[50..150]);
    }

    #[tokio::test]
    async fn read_stream_past_eof_yields_empty() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("x");
        fs::write(&p, b"abc").await.unwrap();
        let r = LocalFs::new()
            .read_stream(
                &vfs_path_for(&p),
                ByteRange {
                    start: 1000,
                    end: None,
                },
            )
            .await
            .unwrap();
        assert!(read_all(r).await.is_empty());
    }

    #[tokio::test]
    async fn read_stream_on_directory_is_unsupported() {
        let td = TempDir::new().unwrap();
        let err = LocalFs::new()
            .read_stream(&vfs_path_for(td.path()), ByteRange::FULL)
            .await
            .map(|_| ())
            .unwrap_err();
        assert!(matches!(err, VfsError::Unsupported(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn read_stream_missing_returns_not_found() {
        let td = TempDir::new().unwrap();
        let err = LocalFs::new()
            .read_stream(&vfs_path_for(&td.path().join("nope")), ByteRange::FULL)
            .await
            .map(|_| ())
            .unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)), "got: {err:?}");
    }

    // ---------- write_stream ----------

    #[tokio::test]
    async fn write_stream_create_new_creates_file() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("new.txt");
        let mut w = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 0, WriteMode::CreateNew)
            .await
            .unwrap();
        w.write_all(b"hello").await.unwrap();
        w.close().await.unwrap();
        assert_eq!(fs::read(&p).await.unwrap(), b"hello");
    }

    #[tokio::test]
    async fn write_stream_create_new_errors_if_exists() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("exists.txt");
        fs::write(&p, b"x").await.unwrap();
        let res = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 0, WriteMode::CreateNew)
            .await;
        assert!(res.is_err(), "create_new on existing path must error");
    }

    #[tokio::test]
    async fn write_stream_truncate_creates_when_missing() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("trunc.txt");
        let mut w = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 0, WriteMode::Truncate)
            .await
            .unwrap();
        w.write_all(b"abc").await.unwrap();
        w.close().await.unwrap();
        assert_eq!(fs::read(&p).await.unwrap(), b"abc");
    }

    #[tokio::test]
    async fn write_stream_truncate_overwrites_existing() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("trunc.txt");
        fs::write(&p, b"old-content-long").await.unwrap();
        let mut w = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 0, WriteMode::Truncate)
            .await
            .unwrap();
        w.write_all(b"new").await.unwrap();
        w.close().await.unwrap();
        assert_eq!(fs::read(&p).await.unwrap(), b"new");
    }

    #[tokio::test]
    async fn write_stream_append_at_offset_preserves_prefix() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("resume.bin");
        fs::write(&p, b"AAAAAAAA").await.unwrap();
        let mut w = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 8, WriteMode::AppendAtOffset)
            .await
            .unwrap();
        w.write_all(b"BBBB").await.unwrap();
        w.close().await.unwrap();
        assert_eq!(fs::read(&p).await.unwrap(), b"AAAAAAAABBBB");
    }

    #[tokio::test]
    async fn write_stream_create_new_with_nonzero_offset_is_unsupported() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("new.txt");
        let err = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 5, WriteMode::CreateNew)
            .await
            .map(|_| ())
            .unwrap_err();
        assert!(matches!(err, VfsError::Unsupported(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn write_stream_parent_missing_returns_not_found() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("noparent/file.txt");
        let err = LocalFs::new()
            .write_stream(&vfs_path_for(&p), 0, WriteMode::Truncate)
            .await
            .map(|_| ())
            .unwrap_err();
        assert!(
            matches!(err, VfsError::NotFound(_) | VfsError::Io(_)),
            "got: {err:?}"
        );
    }

    // ---------- unlink ----------

    #[tokio::test]
    async fn unlink_removes_file() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("doomed.txt");
        fs::write(&p, b"x").await.unwrap();
        LocalFs::new().unlink(&vfs_path_for(&p)).await.unwrap();
        assert!(!p.exists());
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn unlink_removes_symlink_not_target() {
        use std::os::unix::fs::symlink;
        let td = TempDir::new().unwrap();
        let target = td.path().join("target.txt");
        fs::write(&target, b"keepme").await.unwrap();
        let link = td.path().join("link");
        symlink(&target, &link).unwrap();
        LocalFs::new().unlink(&vfs_path_for(&link)).await.unwrap();
        assert!(!link.exists());
        assert!(target.exists(), "unlink must not follow symlinks");
    }

    #[tokio::test]
    async fn unlink_on_directory_is_unsupported() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("d");
        fs::create_dir(&p).await.unwrap();
        let err = LocalFs::new().unlink(&vfs_path_for(&p)).await.unwrap_err();
        assert!(matches!(err, VfsError::Unsupported(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn unlink_missing_returns_not_found() {
        let td = TempDir::new().unwrap();
        let err = LocalFs::new()
            .unlink(&vfs_path_for(&td.path().join("nope")))
            .await
            .unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)), "got: {err:?}");
    }

    // ---------- rename ----------

    #[tokio::test]
    async fn rename_within_authority_succeeds() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.txt");
        let dst = td.path().join("dst.txt");
        fs::write(&src, b"data").await.unwrap();
        LocalFs::new()
            .rename(&vfs_path_for(&src), &vfs_path_for(&dst))
            .await
            .unwrap();
        assert!(!src.exists());
        assert_eq!(fs::read(&dst).await.unwrap(), b"data");
    }

    #[tokio::test]
    async fn rename_overwrites_existing_file_per_posix() {
        let td = TempDir::new().unwrap();
        let src = td.path().join("src.txt");
        let dst = td.path().join("dst.txt");
        fs::write(&src, b"new").await.unwrap();
        fs::write(&dst, b"old").await.unwrap();
        LocalFs::new()
            .rename(&vfs_path_for(&src), &vfs_path_for(&dst))
            .await
            .unwrap();
        assert_eq!(fs::read(&dst).await.unwrap(), b"new");
    }

    #[tokio::test]
    async fn rename_cross_scheme_is_unsupported() {
        let td = TempDir::new().unwrap();
        let src = vfs_path_for(&td.path().join("a"));
        let dst = VfsPath::parse("sftp://host/b").unwrap();
        let err = LocalFs::new().rename(&src, &dst).await.unwrap_err();
        assert!(matches!(err, VfsError::Unsupported(_)), "got: {err:?}");
    }

    #[tokio::test]
    async fn rename_missing_src_returns_not_found() {
        let td = TempDir::new().unwrap();
        let src = vfs_path_for(&td.path().join("nope"));
        let dst = vfs_path_for(&td.path().join("dst"));
        let err = LocalFs::new().rename(&src, &dst).await.unwrap_err();
        assert!(matches!(err, VfsError::NotFound(_)), "got: {err:?}");
    }

    // ---------- mkdir + rmdir (the other trait methods) ----------

    #[tokio::test]
    async fn mkdir_creates_new_directory() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("new-dir");
        LocalFs::new()
            .mkdir(&vfs_path_for(&p), false)
            .await
            .unwrap();
        assert!(p.is_dir());
    }

    #[tokio::test]
    async fn mkdir_non_recursive_fails_if_parent_missing() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("missing/child");
        let res = LocalFs::new().mkdir(&vfs_path_for(&p), false).await;
        assert!(res.is_err());
    }

    #[tokio::test]
    async fn mkdir_recursive_succeeds_when_dir_already_exists() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("d");
        fs::create_dir(&p).await.unwrap();
        LocalFs::new().mkdir(&vfs_path_for(&p), true).await.unwrap();
        assert!(p.is_dir());
    }

    #[tokio::test]
    async fn rmdir_removes_empty_directory() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("e");
        fs::create_dir(&p).await.unwrap();
        LocalFs::new().rmdir(&vfs_path_for(&p)).await.unwrap();
        assert!(!p.exists());
    }

    #[tokio::test]
    async fn rmdir_non_empty_returns_io_error() {
        let td = TempDir::new().unwrap();
        let p = td.path().join("ne");
        fs::create_dir(&p).await.unwrap();
        fs::write(p.join("blocker"), b"x").await.unwrap();
        let res = LocalFs::new().rmdir(&vfs_path_for(&p)).await;
        assert!(res.is_err());
    }
}

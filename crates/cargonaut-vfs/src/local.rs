// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! `LocalFs` — the file-system backend (`file://` scheme) over `tokio::fs`.

use super::error::VfsError;
use super::traits::{ByteRange, VfsBackend, VfsCaps, WriteMode};
use super::types::{DirEntry, DirListing, FileMode, Sort, VfsKind, VfsMetadata, VfsPath};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::path::{Path, PathBuf};
use std::pin::Pin;
use std::time::SystemTime;
use tokio::io::{AsyncReadExt, AsyncSeekExt};
use tokio_util::compat::{TokioAsyncReadCompatExt, TokioAsyncWriteCompatExt};

/// File-system backend over `tokio::fs`.
pub struct LocalFs;

impl LocalFs {
    /// Create a new instance. No state — instances are cheap.
    pub fn new() -> Self {
        Self
    }

    /// Validate scheme + authority and convert a [`VfsPath`] into an absolute
    /// `PathBuf` rooted at `/`. Rejects non-`file://` paths and any non-empty
    /// authority with [`VfsError::Unsupported`].
    fn to_std_path(p: &VfsPath) -> Result<PathBuf, VfsError> {
        if p.scheme.as_str() != "file" {
            return Err(VfsError::Unsupported("LocalFs only accepts file:// paths"));
        }
        if p.authority.is_some() {
            return Err(VfsError::Unsupported(
                "LocalFs requires an empty file:// authority",
            ));
        }
        let mut buf = PathBuf::from("/");
        for seg in &p.segments {
            buf.push(seg.as_str());
        }
        Ok(buf)
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

    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError> {
        let p = Self::to_std_path(path)?;
        let meta = tokio::fs::metadata(&p).await.map_err(|e| map_io(e, &p))?;
        if !meta.is_dir() {
            return Err(VfsError::Unsupported("list called on non-directory"));
        }

        let mut rd = tokio::fs::read_dir(&p).await.map_err(|e| map_io(e, &p))?;
        let mut entries: Vec<DirEntry> = Vec::new();
        while let Some(ent) = rd.next_entry().await.map_err(|e| map_io(e, &p))? {
            let entry_path = ent.path();
            let entry_meta = tokio::fs::symlink_metadata(&entry_path)
                .await
                .map_err(|e| map_io(e, &entry_path))?;
            let meta = build_vfs_metadata(&entry_meta, &entry_path).await?;
            let name_os = ent.file_name();
            let name = name_os.to_string_lossy();
            entries.push(DirEntry {
                name: SmolStr::new(name.as_ref()),
                meta,
            });
        }
        sort_entries(&mut entries, sort);
        Ok(DirListing { entries, sort })
    }

    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        let p = Self::to_std_path(path)?;
        let meta = tokio::fs::symlink_metadata(&p)
            .await
            .map_err(|e| map_io(e, &p))?;
        build_vfs_metadata(&meta, &p).await
    }

    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        let p = Self::to_std_path(path)?;
        let meta = tokio::fs::metadata(&p).await.map_err(|e| map_io(e, &p))?;
        if meta.is_dir() {
            return Err(VfsError::Unsupported("read_stream called on directory"));
        }
        let mut file = tokio::fs::File::open(&p).await.map_err(|e| map_io(e, &p))?;
        if range.start > 0 {
            // Clamp past-EOF starts to the file size; the spec says the caller
            // gets an at-EOF reader, not an error.
            let target = std::cmp::min(range.start, meta.len());
            file.seek(std::io::SeekFrom::Start(target))
                .await
                .map_err(|e| map_io(e, &p))?;
        }
        let reader: Pin<Box<dyn AsyncRead + Send>> = match range.end {
            Some(end) => {
                let bytes_to_read = end.saturating_sub(range.start);
                Box::pin(file.take(bytes_to_read).compat())
            }
            None => Box::pin(file.compat()),
        };
        Ok(reader)
    }

    async fn write_stream(
        &self,
        path: &VfsPath,
        offset: u64,
        mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        let p = Self::to_std_path(path)?;
        let mut opts = tokio::fs::OpenOptions::new();
        opts.write(true);
        match mode {
            WriteMode::CreateNew => {
                if offset != 0 {
                    return Err(VfsError::Unsupported("CreateNew requires offset == 0"));
                }
                opts.create_new(true);
            }
            WriteMode::Truncate => {
                if offset != 0 {
                    return Err(VfsError::Unsupported("Truncate requires offset == 0"));
                }
                opts.create(true).truncate(true);
            }
            WriteMode::AppendAtOffset => {
                // The file must already exist — that's the resume contract.
                // No create / truncate; we'll seek to `offset` below.
            }
        }
        let mut file = opts.open(&p).await.map_err(|e| map_io(e, &p))?;
        if matches!(mode, WriteMode::AppendAtOffset) && offset > 0 {
            file.seek(std::io::SeekFrom::Start(offset))
                .await
                .map_err(|e| map_io(e, &p))?;
        }
        Ok(Box::pin(file.compat_write()))
    }

    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError> {
        let p = Self::to_std_path(path)?;
        let meta = tokio::fs::symlink_metadata(&p)
            .await
            .map_err(|e| map_io(e, &p))?;
        if meta.file_type().is_dir() {
            return Err(VfsError::Unsupported(
                "unlink called on directory; use rmdir",
            ));
        }
        tokio::fs::remove_file(&p).await.map_err(|e| map_io(e, &p))
    }

    async fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let p = Self::to_std_path(path)?;
        let meta = tokio::fs::metadata(&p).await.map_err(|e| map_io(e, &p))?;
        if !meta.is_dir() {
            return Err(VfsError::Unsupported("rmdir called on non-directory"));
        }
        tokio::fs::remove_dir(&p).await.map_err(|e| map_io(e, &p))
    }

    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError> {
        if src.scheme != dest.scheme || src.authority != dest.authority {
            return Err(VfsError::Unsupported(
                "cross-authority rename; use the transfer engine",
            ));
        }
        let s = Self::to_std_path(src)?;
        let d = Self::to_std_path(dest)?;
        tokio::fs::rename(&s, &d).await.map_err(|e| map_io(e, &s))
    }

    async fn mkdir(&self, path: &VfsPath, recursive: bool) -> Result<(), VfsError> {
        let p = Self::to_std_path(path)?;
        let res = if recursive {
            tokio::fs::create_dir_all(&p).await
        } else {
            tokio::fs::create_dir(&p).await
        };
        res.map_err(|e| map_io(e, &p))
    }
}

// -------- helpers --------

/// Map a `std::io::Error` to the most specific [`VfsError`] variant per the
/// canonical mapping in the [`VfsBackend`] trait docs.
fn map_io(e: std::io::Error, path: &Path) -> VfsError {
    use std::io::ErrorKind;
    match e.kind() {
        ErrorKind::NotFound => VfsError::NotFound(path.display().to_string()),
        ErrorKind::PermissionDenied => VfsError::PermissionDenied(path.display().to_string()),
        _ => VfsError::Io(e),
    }
}

/// Convert an arbitrary `&Path` (absolute or relative) into a `file://` [`VfsPath`].
/// Used for reporting symlink targets — bypasses [`VfsPath::parse`] because the
/// segments are already-tokenized and may legitimately contain non-URL-safe
/// characters that the parser would reject.
fn path_to_vfs_path(p: &Path) -> VfsPath {
    let segments: SmallVec<[SmolStr; 8]> = p
        .components()
        .filter_map(|c| {
            // Skip the root prefix (`/`) and any odd components; we only keep Normal parts.
            // `..` and `.` would be rejected by `VfsPath::parse` and have no canonical
            // representation here, so we drop them. Symlink targets containing them are
            // a Phase-2+ concern (resolve relative to the link's parent).
            match c {
                std::path::Component::Normal(s) => s.to_str().map(SmolStr::new),
                _ => None,
            }
        })
        .collect();
    VfsPath {
        scheme: SmolStr::new("file"),
        authority: None,
        segments,
    }
}

/// Build a [`VfsMetadata`] from a `std::fs::Metadata` + the entry's own path
/// (the path is needed for symlink target resolution and the hidden-name check).
async fn build_vfs_metadata(
    meta: &std::fs::Metadata,
    path: &Path,
) -> Result<VfsMetadata, VfsError> {
    let ft = meta.file_type();
    let kind = if ft.is_symlink() {
        let target = tokio::fs::read_link(path)
            .await
            .map_err(|e| map_io(e, path))?;
        VfsKind::Symlink {
            target: Box::new(path_to_vfs_path(&target)),
        }
    } else if ft.is_dir() {
        VfsKind::Dir
    } else if ft.is_file() {
        VfsKind::File
    } else {
        VfsKind::Other
    };

    let mtime = meta.modified().unwrap_or(SystemTime::UNIX_EPOCH);

    #[cfg(unix)]
    let mode = {
        use std::os::unix::fs::MetadataExt;
        Some(FileMode {
            bits: meta.mode() & 0o777,
            uid: Some(meta.uid()),
            gid: Some(meta.gid()),
        })
    };
    #[cfg(not(unix))]
    let mode: Option<FileMode> = None;

    let is_hidden = path
        .file_name()
        .and_then(|s| s.to_str())
        .map(|s| s.starts_with('.'))
        .unwrap_or(false);

    Ok(VfsMetadata {
        size: meta.len(),
        mtime,
        mode,
        kind,
        is_hidden,
    })
}

/// Sort `entries` in place per [`Sort`]. UI does not re-sort.
fn sort_entries(entries: &mut [DirEntry], sort: Sort) {
    match sort {
        Sort::NameAsc => entries.sort_by(|a, b| a.name.cmp(&b.name)),
        Sort::NameDesc => entries.sort_by(|a, b| b.name.cmp(&a.name)),
        Sort::SizeDesc => entries.sort_by_key(|e| std::cmp::Reverse(e.meta.size)),
        Sort::MtimeDesc => entries.sort_by_key(|e| std::cmp::Reverse(e.meta.mtime)),
        Sort::ExtAsc => entries.sort_by(|a, b| {
            let ea = a.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            let eb = b.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
            ea.cmp(eb).then_with(|| a.name.cmp(&b.name))
        }),
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

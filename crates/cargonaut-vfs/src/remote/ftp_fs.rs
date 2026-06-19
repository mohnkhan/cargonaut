// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`FtpFs`] — read/write [`VfsBackend`] for `ftp://` paths.
//!
//! Full implementation — Task T031 (Phase 6).
//!
//! ## Design
//!
//! FTP has no native byte-range read or write-at-offset; every transfer
//! downloads or uploads a whole file. As a result:
//!
//! - `read_stream` only accepts [`ByteRange::FULL`] (`start == 0`, `end == None`).
//!   Any other range returns [`VfsError::Unsupported`].
//! - `write_stream` only accepts [`WriteMode::Truncate`] at offset 0.
//!   [`WriteMode::AppendAtOffset`] returns [`VfsError::Unsupported`].
//! - `caps()` advertises only `ATOMIC_RENAME`; `SEEKABLE`, `RANDOM_WRITE`,
//!   `METADATA_RICH`, and `SYMLINKS` are NOT claimed.
//!
//! ### Pluggable ops
//!
//! Production usage routes through [`RealFtpOps`] which wraps a
//! `suppaftp::tokio::AsyncFtpStream`. Tests inject a `MockFtpOps` via
//! [`FtpFs::with_ops`] without needing a live server.

#![cfg(feature = "remote")]

use crate::{
    ByteRange, DirEntry, DirListing, Sort, VfsBackend, VfsCaps, VfsError, VfsKind, VfsMetadata,
    VfsPath, WriteMode,
};
use async_trait::async_trait;
use futures::{AsyncRead, AsyncWrite};
use smol_str::SmolStr;
use std::{
    cmp::Reverse,
    future::Future,
    io,
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::SystemTime,
};

// ---------------------------------------------------------------------------
// FtpOps — session abstraction (enables mock injection in tests)
// ---------------------------------------------------------------------------

/// Thin async abstraction over an active FTP session.
///
/// [`RealFtpOps`] is the production implementation backed by `suppaftp`.
/// Tests inject a mock via [`FtpFs::with_ops`].
#[async_trait]
pub trait FtpOps: Send + Sync + 'static {
    /// List a directory; returns `(name, metadata)` pairs.
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError>;

    /// Stat a single path.
    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError>;

    /// Download the entire file at `path`.
    ///
    /// FTP has no native byte-range read, so only whole-file transfers
    /// are supported at this layer.
    async fn read_file(&self, path: &str) -> Result<Vec<u8>, VfsError>;

    /// Upload `data` as the entire content of `path` (truncate semantics).
    ///
    /// FTP has no native seek/append, so only truncate-mode uploads are
    /// supported at this layer.
    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), VfsError>;

    /// Remove a file.
    async fn unlink(&self, path: &str) -> Result<(), VfsError>;

    /// Remove an empty directory.
    async fn rmdir(&self, path: &str) -> Result<(), VfsError>;

    /// Rename / move within the same server.
    async fn rename(&self, src: &str, dest: &str) -> Result<(), VfsError>;

    /// Create a directory.
    async fn mkdir(&self, path: &str) -> Result<(), VfsError>;
}

// ---------------------------------------------------------------------------
// RealFtpOps — production suppaftp wrapper
// ---------------------------------------------------------------------------

/// Parse a single MLSD line into `(name, VfsMetadata)`.
///
/// MLSD lines look like: `Type=file;Size=1234;Modify=20240101120000; filename.txt`
fn parse_mlsd_line(line: &str) -> Option<(String, VfsMetadata)> {
    // Split at the first space: facts are before, name is after.
    let space = line.find(' ')?;
    let facts_str = &line[..space];
    let name = line[space + 1..].trim().to_string();
    if name.is_empty() || name == "." || name == ".." {
        return None;
    }

    let mut is_dir = false;
    let mut size: u64 = 0;

    for fact in facts_str.split(';') {
        if let Some((key, val)) = fact.split_once('=') {
            match key.to_ascii_lowercase().as_str() {
                "type" => {
                    is_dir = val.eq_ignore_ascii_case("dir")
                        || val.eq_ignore_ascii_case("cdir")
                        || val.eq_ignore_ascii_case("pdir");
                }
                "size" => {
                    size = val.parse().unwrap_or(0);
                }
                _ => {}
            }
        }
    }

    let kind = if is_dir { VfsKind::Dir } else { VfsKind::File };
    let meta = VfsMetadata {
        size,
        mtime: SystemTime::UNIX_EPOCH,
        mode: None,
        kind,
        is_hidden: name.starts_with('.'),
    };
    Some((name, meta))
}

/// Parse a single Unix-format LIST line into `(name, VfsMetadata)`.
///
/// Canonical Unix format: `drwxr-xr-x 2 user group 4096 Jan  1 12:00 dirname`
fn parse_list_line(line: &str) -> Option<(String, VfsMetadata)> {
    let parts: Vec<&str> = line.split_whitespace().collect();
    // Standard Unix listing has at least 9 fields; the name is the last one.
    if parts.len() < 9 {
        return None;
    }
    let name = parts[8..].join(" ");
    if name == "." || name == ".." {
        return None;
    }
    let is_dir = line.starts_with('d');
    let size: u64 = parts[4].parse().unwrap_or(0);
    let kind = if is_dir { VfsKind::Dir } else { VfsKind::File };
    let meta = VfsMetadata {
        size,
        mtime: SystemTime::UNIX_EPOCH,
        mode: None,
        kind,
        is_hidden: name.starts_with('.'),
    };
    Some((name, meta))
}

struct RealFtpOps {
    stream: Arc<tokio::sync::Mutex<suppaftp::tokio::AsyncFtpStream>>,
}

#[async_trait]
impl FtpOps for RealFtpOps {
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError> {
        let mut ftp = self.stream.lock().await;
        let path_opt: Option<&str> = if path == "/" { None } else { Some(path) };

        // Try MLSD first (machine-readable); fall back to LIST.
        if let Ok(lines) = ftp.mlsd(path_opt).await {
            let result = lines.iter().filter_map(|l| parse_mlsd_line(l)).collect();
            return Ok(result);
        }

        // Fallback: LIST (Unix-style)
        let raw = ftp
            .list(path_opt)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        let result = raw.iter().filter_map(|l| parse_list_line(l)).collect();
        Ok(result)
    }

    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError> {
        let mut ftp = self.stream.lock().await;
        let path_opt: Option<&str> = if path == "/" { None } else { Some(path) };

        if let Ok(line) = ftp.mlst(path_opt).await {
            if let Some((_, meta)) = parse_mlsd_line(&line) {
                return Ok(meta);
            }
        }

        // Fallback: list the parent directory and locate the entry.
        drop(ftp);
        let parent = path.rfind('/').map(|i| &path[..i]).unwrap_or("/");
        let parent = if parent.is_empty() { "/" } else { parent };
        let entries = self.list_dir(parent).await?;
        let name = path.rsplit('/').next().unwrap_or(path);
        entries
            .into_iter()
            .find(|(n, _)| n == name)
            .map(|(_, m)| m)
            .ok_or_else(|| VfsError::NotFound(path.to_string()))
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        use tokio::io::AsyncReadExt;
        let mut ftp = self.stream.lock().await;
        let mut reader = ftp
            .retr_as_stream(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut buf = Vec::new();
        reader
            .read_to_end(&mut buf)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        // suppaftp requires finalizing the stream after retrieval
        ftp.finalize_retr_stream(reader)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(buf)
    }

    async fn write_file(&self, path: &str, data: &[u8]) -> Result<(), VfsError> {
        let mut ftp = self.stream.lock().await;
        // put_file takes &mut R where R: AsyncRead (tokio).
        let mut cursor = std::io::Cursor::new(data);
        ftp.put_file(path, &mut cursor)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn unlink(&self, path: &str) -> Result<(), VfsError> {
        let mut ftp = self.stream.lock().await;
        ftp.rm(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn rmdir(&self, path: &str) -> Result<(), VfsError> {
        let mut ftp = self.stream.lock().await;
        ftp.rmdir(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn rename(&self, src: &str, dest: &str) -> Result<(), VfsError> {
        let mut ftp = self.stream.lock().await;
        ftp.rename(src, dest)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), VfsError> {
        let mut ftp = self.stream.lock().await;
        ftp.mkdir(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// FtpBufferWriter — buffers writes in memory, uploads on close
// ---------------------------------------------------------------------------

enum FtpWriteState {
    Buffering {
        buf: Vec<u8>,
        path: String,
        ops: Arc<dyn FtpOps>,
    },
    Closing(Pin<Box<dyn Future<Output = Result<(), VfsError>> + Send>>),
    Closed,
}

struct FtpBufferWriter {
    state: FtpWriteState,
}

// SAFETY: FtpWriteState::Buffering holds Arc<dyn FtpOps: Send+Sync> and Vec<u8>
// — all Send. FtpWriteState::Closing holds a Send Future.
unsafe impl Send for FtpBufferWriter {}

impl futures::io::AsyncWrite for FtpBufferWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.state {
            FtpWriteState::Buffering { buf: inner, .. } => {
                inner.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            }
            FtpWriteState::Closing(_) | FtpWriteState::Closed => {
                Poll::Ready(Err(io::Error::other("write after close")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.state {
                FtpWriteState::Buffering { .. } => {
                    let (buf, path, ops) =
                        match std::mem::replace(&mut self.state, FtpWriteState::Closed) {
                            FtpWriteState::Buffering { buf, path, ops } => (buf, path, ops),
                            _ => unreachable!(),
                        };
                    let fut: Pin<Box<dyn Future<Output = Result<(), VfsError>> + Send>> =
                        Box::pin(async move { ops.write_file(&path, &buf).await });
                    self.state = FtpWriteState::Closing(fut);
                }
                FtpWriteState::Closing(fut) => match fut.as_mut().poll(cx) {
                    Poll::Pending => return Poll::Pending,
                    Poll::Ready(Ok(())) => {
                        self.state = FtpWriteState::Closed;
                        return Poll::Ready(Ok(()));
                    }
                    Poll::Ready(Err(VfsError::Io(e))) => {
                        self.state = FtpWriteState::Closed;
                        return Poll::Ready(Err(e));
                    }
                    Poll::Ready(Err(other)) => {
                        self.state = FtpWriteState::Closed;
                        return Poll::Ready(Err(io::Error::other(other.to_string())));
                    }
                },
                FtpWriteState::Closed => return Poll::Ready(Ok(())),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn vfs_path_to_ftp_path(path: &VfsPath) -> String {
    if path.segments.is_empty() {
        "/".into()
    } else {
        format!(
            "/{}",
            path.segments
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join("/")
        )
    }
}

// ---------------------------------------------------------------------------
// FtpFs — the public VFS backend
// ---------------------------------------------------------------------------

/// Read/write VFS backend for FTP servers (`ftp://` scheme).
///
/// ## Capability constraints
///
/// FTP has no byte-range reads or write-at-offset; both are missing from the
/// protocol. As a result `caps()` advertises only `ATOMIC_RENAME`:
///
/// | Cap            | Supported |
/// |----------------|-----------|
/// | SEEKABLE       | No        |
/// | RANDOM_WRITE   | No        |
/// | METADATA_RICH  | No        |
/// | ATOMIC_RENAME  | Yes       |
/// | SYMLINKS       | No        |
pub struct FtpFs {
    ops: Arc<dyn FtpOps>,
}

impl FtpFs {
    /// Construct from an existing [`FtpOps`] implementation (e.g. a mock).
    pub fn with_ops(ops: Arc<dyn FtpOps>) -> Self {
        Self { ops }
    }

    /// Open a real FTP connection to `authority` (`"host:port"`),
    /// authenticate with `user` and `pass`, and return a ready [`FtpFs`].
    pub async fn connect(authority: &str, user: &str, pass: &str) -> Result<Self, VfsError> {
        let mut stream = suppaftp::tokio::AsyncFtpStream::connect(authority)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        stream
            .login(user, pass)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(Self {
            ops: Arc::new(RealFtpOps {
                stream: Arc::new(tokio::sync::Mutex::new(stream)),
            }),
        })
    }
}

// ---------------------------------------------------------------------------
// VfsBackend impl
// ---------------------------------------------------------------------------

#[async_trait]
impl VfsBackend for FtpFs {
    fn scheme(&self) -> &'static str {
        "ftp"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::ATOMIC_RENAME
    }

    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError> {
        let ftp_path = vfs_path_to_ftp_path(path);
        let raw = self.ops.list_dir(&ftp_path).await?;

        let mut entries: Vec<DirEntry> = raw
            .into_iter()
            .map(|(name, meta)| DirEntry {
                name: SmolStr::new(&name),
                meta,
            })
            .collect();

        match sort {
            Sort::NameAsc => entries.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::NameDesc => entries.sort_by(|a, b| b.name.cmp(&a.name)),
            Sort::SizeDesc => entries.sort_by_key(|e| Reverse(e.meta.size)),
            Sort::MtimeDesc => entries.sort_by_key(|e| Reverse(e.meta.mtime)),
            Sort::ExtAsc => entries.sort_by(|a, b| {
                let ext_a = a.name.rfind('.').map(|i| &a.name[i..]).unwrap_or("");
                let ext_b = b.name.rfind('.').map(|i| &b.name[i..]).unwrap_or("");
                ext_a.cmp(ext_b).then(a.name.cmp(&b.name))
            }),
        }

        Ok(DirListing { entries, sort })
    }

    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        let ftp_path = vfs_path_to_ftp_path(path);
        self.ops.stat(&ftp_path).await
    }

    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        // FTP has no byte-range reads; only full-file downloads are supported.
        if range.start != 0 || range.end.is_some() {
            return Err(VfsError::Unsupported("FTP range reads not supported"));
        }
        let ftp_path = vfs_path_to_ftp_path(path);
        let bytes = self.ops.read_file(&ftp_path).await?;
        Ok(Box::pin(futures::io::Cursor::new(bytes)))
    }

    async fn write_stream(
        &self,
        path: &VfsPath,
        offset: u64,
        mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        // FTP has no write-at-offset; only truncate-mode uploads at offset 0.
        if mode != WriteMode::Truncate || offset != 0 {
            return Err(VfsError::Unsupported("FTP append writes not supported"));
        }
        let ftp_path = vfs_path_to_ftp_path(path);
        let writer = FtpBufferWriter {
            state: FtpWriteState::Buffering {
                buf: vec![],
                path: ftp_path,
                ops: self.ops.clone(),
            },
        };
        Ok(Box::pin(writer))
    }

    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError> {
        let ftp_path = vfs_path_to_ftp_path(path);
        self.ops.unlink(&ftp_path).await
    }

    async fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let ftp_path = vfs_path_to_ftp_path(path);
        self.ops.rmdir(&ftp_path).await
    }

    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError> {
        let src_path = vfs_path_to_ftp_path(src);
        let dest_path = vfs_path_to_ftp_path(dest);
        self.ops.rename(&src_path, &dest_path).await
    }

    async fn mkdir(&self, path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        let ftp_path = vfs_path_to_ftp_path(path);
        self.ops.mkdir(&ftp_path).await
    }
}

// ---------------------------------------------------------------------------
// Unit tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vfs_path_to_ftp_root() {
        let p = VfsPath::parse("ftp://host/").unwrap();
        assert_eq!(vfs_path_to_ftp_path(&p), "/");
    }

    #[test]
    fn vfs_path_to_ftp_nested() {
        let p = VfsPath::parse("ftp://host/pub/data").unwrap();
        assert_eq!(vfs_path_to_ftp_path(&p), "/pub/data");
    }

    #[test]
    fn parse_mlsd_line_file() {
        let line = "Type=file;Size=1234;Modify=20240101120000; foo.txt";
        let (name, meta) = parse_mlsd_line(line).unwrap();
        assert_eq!(name, "foo.txt");
        assert_eq!(meta.size, 1234);
        assert!(matches!(meta.kind, VfsKind::File));
    }

    #[test]
    fn parse_mlsd_line_dir() {
        let line = "Type=dir;Size=0; subdir";
        let (name, meta) = parse_mlsd_line(line).unwrap();
        assert_eq!(name, "subdir");
        assert!(matches!(meta.kind, VfsKind::Dir));
    }

    #[test]
    fn parse_mlsd_line_dot_skipped() {
        assert!(parse_mlsd_line("Type=dir;Size=0; .").is_none());
        assert!(parse_mlsd_line("Type=dir;Size=0; ..").is_none());
    }

    #[test]
    fn parse_list_line_file() {
        let line = "-rw-r--r-- 1 user group 512 Jan  1 12:00 readme.txt";
        let (name, meta) = parse_list_line(line).unwrap();
        assert_eq!(name, "readme.txt");
        assert_eq!(meta.size, 512);
        assert!(matches!(meta.kind, VfsKind::File));
    }

    #[test]
    fn parse_list_line_dir() {
        let line = "drwxr-xr-x 2 user group 4096 Jan  1 12:00 mydir";
        let (name, meta) = parse_list_line(line).unwrap();
        assert_eq!(name, "mydir");
        assert!(matches!(meta.kind, VfsKind::Dir));
    }
}

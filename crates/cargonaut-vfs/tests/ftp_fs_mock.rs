// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T030 (red): Mock-backed unit tests for FtpFs.
//!
//! These tests drive T031 (green): full FtpFs implementation.
//! They compile-fail until `FtpOps` and `FtpFs::with_ops` are added
//! to the public API.

#![cfg(feature = "remote")]

use async_trait::async_trait;
use cargonaut_vfs::FtpFs; // T030 red: doesn't exist yet → compile error
use cargonaut_vfs::{
    ByteRange,
    FtpOps, // T030 red: doesn't exist yet → compile error
    Sort,
    VfsBackend,
    VfsCaps,
    VfsError,
    VfsKind,
    VfsMetadata,
    VfsPath,
    WriteMode,
};
use futures::AsyncWriteExt;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::SystemTime;

// ─── Mock FtpOps implementation ──────────────────────────────────────────────

#[derive(Debug, Clone)]
struct MockFtpOps {
    list_responses: HashMap<String, Result<Vec<(String, VfsMetadata)>, String>>,
    stat_responses: HashMap<String, Result<VfsMetadata, String>>,
    read_responses: HashMap<String, Result<Vec<u8>, String>>,
    write_ok: bool,
    mutate_ok: bool,
    connect_fail: bool,
}

impl MockFtpOps {
    fn new() -> Self {
        Self {
            list_responses: HashMap::new(),
            stat_responses: HashMap::new(),
            read_responses: HashMap::new(),
            write_ok: true,
            mutate_ok: true,
            connect_fail: false,
        }
    }

    fn with_dir(mut self, path: &str, entries: Vec<(String, VfsMetadata)>) -> Self {
        self.list_responses.insert(path.to_string(), Ok(entries));
        self
    }

    fn with_stat(mut self, path: &str, meta: VfsMetadata) -> Self {
        self.stat_responses.insert(path.to_string(), Ok(meta));
        self
    }

    fn with_read(mut self, path: &str, bytes: Vec<u8>) -> Self {
        self.read_responses.insert(path.to_string(), Ok(bytes));
        self
    }

    fn with_connect_fail(mut self) -> Self {
        self.connect_fail = true;
        self
    }
}

#[async_trait]
impl FtpOps for MockFtpOps {
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError> {
        match self.list_responses.get(path) {
            Some(Ok(entries)) => Ok(entries.clone()),
            Some(Err(msg)) => Err(VfsError::Other(msg.clone())),
            None => Err(VfsError::NotFound(format!("no mock for list_dir({path})"))),
        }
    }

    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError> {
        if self.connect_fail {
            return Err(VfsError::Io(std::io::Error::new(
                std::io::ErrorKind::ConnectionRefused,
                "mock connect failure",
            )));
        }
        match self.stat_responses.get(path) {
            Some(Ok(meta)) => Ok(meta.clone()),
            Some(Err(msg)) => Err(VfsError::Other(msg.clone())),
            None => Err(VfsError::NotFound(format!("no mock for stat({path})"))),
        }
    }

    async fn read_file(&self, path: &str) -> Result<Vec<u8>, VfsError> {
        match self.read_responses.get(path) {
            Some(Ok(bytes)) => Ok(bytes.clone()),
            Some(Err(msg)) => Err(VfsError::Other(msg.clone())),
            None => Err(VfsError::NotFound(format!("no mock for read_file({path})"))),
        }
    }

    async fn write_file(&self, _path: &str, _data: &[u8]) -> Result<(), VfsError> {
        if self.write_ok {
            Ok(())
        } else {
            Err(VfsError::Other("mock write failure".to_string()))
        }
    }

    async fn unlink(&self, _path: &str) -> Result<(), VfsError> {
        if self.mutate_ok {
            Ok(())
        } else {
            Err(VfsError::Other("mock unlink failure".to_string()))
        }
    }

    async fn rmdir(&self, _path: &str) -> Result<(), VfsError> {
        if self.mutate_ok {
            Ok(())
        } else {
            Err(VfsError::Other("mock rmdir failure".to_string()))
        }
    }

    async fn rename(&self, _src: &str, _dest: &str) -> Result<(), VfsError> {
        if self.mutate_ok {
            Ok(())
        } else {
            Err(VfsError::Other("mock rename failure".to_string()))
        }
    }

    async fn mkdir(&self, _path: &str) -> Result<(), VfsError> {
        if self.mutate_ok {
            Ok(())
        } else {
            Err(VfsError::Other("mock mkdir failure".to_string()))
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn file_meta(size: u64) -> VfsMetadata {
    VfsMetadata {
        size,
        mtime: SystemTime::UNIX_EPOCH,
        mode: None,
        kind: VfsKind::File,
        is_hidden: false,
    }
}

fn dir_meta() -> VfsMetadata {
    VfsMetadata {
        size: 0,
        mtime: SystemTime::UNIX_EPOCH,
        mode: None,
        kind: VfsKind::Dir,
        is_hidden: false,
    }
}

fn make_ftp_fs(ops: MockFtpOps) -> FtpFs {
    FtpFs::with_ops(Arc::new(ops))
}

// ─── scheme / caps ───────────────────────────────────────────────────────────

#[test]
fn ftp_fs_scheme_is_ftp() {
    let fs = make_ftp_fs(MockFtpOps::new());
    assert_eq!(fs.scheme(), "ftp");
}

#[test]
fn ftp_fs_caps_are_atomic_rename_only() {
    let fs = make_ftp_fs(MockFtpOps::new());
    assert_eq!(fs.caps(), VfsCaps::ATOMIC_RENAME);
    assert!(!fs.caps().contains(VfsCaps::SEEKABLE));
    assert!(!fs.caps().contains(VfsCaps::RANDOM_WRITE));
    assert!(!fs.caps().contains(VfsCaps::METADATA_RICH));
    assert!(!fs.caps().contains(VfsCaps::SYMLINKS));
}

// ─── list ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_list_returns_entries() {
    let ops = MockFtpOps::new().with_dir(
        "/",
        vec![
            ("foo.txt".to_string(), file_meta(42)),
            ("bar.txt".to_string(), file_meta(100)),
        ],
    );
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/").unwrap();
    let listing = fs.list(&path, Sort::NameAsc).await.expect("list root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"foo.txt"), "missing foo.txt: {names:?}");
    assert!(names.contains(&"bar.txt"), "missing bar.txt: {names:?}");
}

// ─── stat ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_stat_file_returns_metadata() {
    let ops = MockFtpOps::new().with_stat("/foo.txt", file_meta(512));
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/foo.txt").unwrap();
    let meta = fs.stat(&path).await.expect("stat file");
    assert!(matches!(meta.kind, VfsKind::File));
    assert_eq!(meta.size, 512);
}

#[tokio::test]
async fn ftp_fs_stat_dir_returns_dir_kind() {
    let ops = MockFtpOps::new().with_stat("/subdir", dir_meta());
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/subdir").unwrap();
    let meta = fs.stat(&path).await.expect("stat dir");
    assert!(matches!(meta.kind, VfsKind::Dir));
}

// ─── read_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_read_stream_full_returns_bytes() {
    use futures::AsyncReadExt;
    let ops = MockFtpOps::new().with_read("/readme.txt", b"hello ftp\n".to_vec());
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/readme.txt").unwrap();
    let mut stream = fs
        .read_stream(&path, ByteRange::FULL)
        .await
        .expect("read_stream full");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read all");
    assert_eq!(buf, b"hello ftp\n");
}

#[tokio::test]
async fn ftp_fs_read_stream_range_returns_unsupported() {
    let ops = MockFtpOps::new().with_read("/data.bin", b"0123456789".to_vec());
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/data.bin").unwrap();
    let range = ByteRange {
        start: 10,
        end: None,
    };
    let result = fs.read_stream(&path, range).await;
    let err = result.err().expect("range read must fail");
    assert!(
        matches!(err, VfsError::Unsupported(_)),
        "expected Unsupported, got {err:?}"
    );
}

// ─── write_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_write_stream_truncate_succeeds() {
    let ops = MockFtpOps::new();
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/newfile.txt").unwrap();
    let mut writer = fs
        .write_stream(&path, 0, WriteMode::Truncate)
        .await
        .expect("write_stream truncate");
    writer.write_all(b"data").await.expect("write bytes");
    writer.close().await.expect("close writer");
}

#[tokio::test]
async fn ftp_fs_write_stream_append_returns_unsupported() {
    let ops = MockFtpOps::new();
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/append.txt").unwrap();
    let result = fs.write_stream(&path, 100, WriteMode::AppendAtOffset).await;
    let err = result.err().expect("append must fail");
    assert!(
        matches!(err, VfsError::Unsupported(_)),
        "expected Unsupported, got {err:?}"
    );
}

// ─── mutating ops ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_unlink_succeeds() {
    let fs = make_ftp_fs(MockFtpOps::new());
    let path = VfsPath::parse("ftp://user@host/old.txt").unwrap();
    fs.unlink(&path).await.expect("unlink");
}

#[tokio::test]
async fn ftp_fs_rmdir_succeeds() {
    let fs = make_ftp_fs(MockFtpOps::new());
    let path = VfsPath::parse("ftp://user@host/old_dir").unwrap();
    fs.rmdir(&path).await.expect("rmdir");
}

#[tokio::test]
async fn ftp_fs_rename_succeeds() {
    let fs = make_ftp_fs(MockFtpOps::new());
    let src = VfsPath::parse("ftp://user@host/a.txt").unwrap();
    let dst = VfsPath::parse("ftp://user@host/b.txt").unwrap();
    fs.rename(&src, &dst).await.expect("rename");
}

#[tokio::test]
async fn ftp_fs_mkdir_succeeds() {
    let fs = make_ftp_fs(MockFtpOps::new());
    let path = VfsPath::parse("ftp://user@host/newdir").unwrap();
    fs.mkdir(&path, false).await.expect("mkdir");
}

// ─── error mapping ────────────────────────────────────────────────────────────

#[tokio::test]
async fn ftp_fs_connect_error_returns_io() {
    let ops = MockFtpOps::new().with_connect_fail();
    let fs = make_ftp_fs(ops);
    let path = VfsPath::parse("ftp://user@host/any.txt").unwrap();
    let err = fs
        .stat(&path)
        .await
        .expect_err("must fail on connect error");
    assert!(matches!(err, VfsError::Io(_)), "expected Io, got {err:?}");
}

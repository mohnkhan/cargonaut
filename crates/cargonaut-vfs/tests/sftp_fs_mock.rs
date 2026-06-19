// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T023 (red): Mock-backed unit tests for SftpFs.
//!
//! These tests drive T024 (green): full SftpFs implementation.
//! They compile-fail until `SftpCredentials`, `SftpOps`, and
//! `SftpFs::with_ops` are added to the public API.

#![cfg(feature = "remote")]

use async_trait::async_trait;
use cargonaut_vfs::{
    ByteRange, Sort, VfsBackend, VfsCaps, VfsError, VfsKind, VfsMetadata, VfsPath, WriteMode,
    SftpCredentials, SftpOps,
};
use futures::AsyncWriteExt;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::SystemTime;
use tracing_subscriber::prelude::*;

// ─── Mock SftpOps implementation ─────────────────────────────────────────────

#[derive(Debug, Clone)]
#[allow(dead_code)]
struct MockEntry {
    name: String,
    size: u64,
    is_dir: bool,
    mtime: SystemTime,
    is_symlink: bool,
    symlink_target: Option<String>,
    uid: Option<u32>,
    gid: Option<u32>,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum MockAction {
    /// Return the DirListing for the path.
    Listing(Vec<MockEntry>),
    /// Return the stat for the path.
    Stat(MockEntry),
    /// Return file bytes on read.
    ReadBytes(Vec<u8>),
    /// Succeeds silently.
    Ok,
    /// Return a specific error.
    Fail(String),
    /// Auth failure.
    AuthFail,
    /// Transport failure (for retry tests).
    TransportFail,
}

/// Minimal SFTP operations abstraction, used by SftpFs.
///
/// In production this wraps a real russh-sftp session;
/// in tests a MockSftpOps injects pre-canned responses.
struct MockSftpOps {
    /// path → MockAction for list()
    list_responses: HashMap<String, MockAction>,
    /// path → MockAction for stat()
    stat_responses: HashMap<String, MockAction>,
    /// path → bytes for read_bytes()
    read_responses: HashMap<String, Vec<u8>>,
    /// default action for write/unlink/rmdir/rename/mkdir
    default_mutate: MockAction,
    /// consecutive transport failures to simulate before success
    transport_fail_count: Mutex<usize>,
}

impl MockSftpOps {
    fn new() -> Self {
        Self {
            list_responses: HashMap::new(),
            stat_responses: HashMap::new(),
            read_responses: HashMap::new(),
            default_mutate: MockAction::Ok,
            transport_fail_count: Mutex::new(0),
        }
    }

    fn with_dir(mut self, path: &str, entries: Vec<MockEntry>) -> Self {
        self.list_responses
            .insert(path.to_string(), MockAction::Listing(entries));
        self
    }

    fn with_stat(mut self, path: &str, entry: MockEntry) -> Self {
        self.stat_responses
            .insert(path.to_string(), MockAction::Stat(entry));
        self
    }

    fn with_read(mut self, path: &str, bytes: Vec<u8>) -> Self {
        self.read_responses.insert(path.to_string(), bytes);
        self
    }

    fn with_transport_failures(self, count: usize) -> Self {
        *self.transport_fail_count.lock().unwrap() = count;
        self
    }
}

#[async_trait]
impl SftpOps for MockSftpOps {
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError> {
        match self.list_responses.get(path) {
            Some(MockAction::Listing(entries)) => {
                let result = entries
                    .iter()
                    .map(|e| {
                        let meta = VfsMetadata {
                            size: e.size,
                            mtime: e.mtime,
                            mode: None,
                            kind: if e.is_dir {
                                VfsKind::Dir
                            } else {
                                VfsKind::File
                            },
                            is_hidden: e.name.starts_with('.'),
                        };
                        (e.name.clone(), meta)
                    })
                    .collect();
                Ok(result)
            }
            Some(MockAction::Fail(msg)) => Err(VfsError::Other(msg.clone())),
            Some(MockAction::AuthFail) => Err(VfsError::AuthFailed("mock auth failure".into())),
            _ => Err(VfsError::NotFound(format!("no mock for list_dir({path})"))),
        }
    }

    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError> {
        // Check transport fail counter
        {
            let mut count = self.transport_fail_count.lock().unwrap();
            if *count > 0 {
                *count -= 1;
                return Err(VfsError::Io(std::io::Error::new(
                    std::io::ErrorKind::BrokenPipe,
                    "mock transport failure",
                )));
            }
        }
        match self.stat_responses.get(path) {
            Some(MockAction::Stat(entry)) => Ok(VfsMetadata {
                size: entry.size,
                mtime: entry.mtime,
                mode: None,
                kind: if entry.is_dir {
                    VfsKind::Dir
                } else {
                    VfsKind::File
                },
                is_hidden: entry.name.starts_with('.'),
            }),
            Some(MockAction::Fail(msg)) => Err(VfsError::Other(msg.clone())),
            Some(MockAction::AuthFail) => Err(VfsError::AuthFailed("mock auth failure".into())),
            _ => Err(VfsError::NotFound(format!("no mock for stat({path})"))),
        }
    }

    async fn read_bytes(&self, path: &str, offset: u64, len: Option<u64>) -> Result<Vec<u8>, VfsError> {
        match self.read_responses.get(path) {
            Some(bytes) => {
                let start = offset as usize;
                let end = len.map(|l| (start + l as usize).min(bytes.len())).unwrap_or(bytes.len());
                Ok(bytes[start..end].to_vec())
            }
            None => Err(VfsError::NotFound(format!("no mock for read({path})"))),
        }
    }

    async fn write_all(&self, _path: &str, _data: &[u8], _offset: u64, _truncate: bool) -> Result<(), VfsError> {
        match &self.default_mutate {
            MockAction::Ok => Ok(()),
            MockAction::Fail(msg) => Err(VfsError::Other(msg.clone())),
            _ => Ok(()),
        }
    }

    async fn unlink(&self, _path: &str) -> Result<(), VfsError> {
        match &self.default_mutate {
            MockAction::Ok => Ok(()),
            MockAction::Fail(msg) => Err(VfsError::Other(msg.clone())),
            _ => Ok(()),
        }
    }

    async fn rmdir(&self, _path: &str) -> Result<(), VfsError> {
        match &self.default_mutate {
            MockAction::Ok => Ok(()),
            MockAction::Fail(msg) => Err(VfsError::Other(msg.clone())),
            _ => Ok(()),
        }
    }

    async fn rename(&self, _src: &str, _dest: &str) -> Result<(), VfsError> {
        match &self.default_mutate {
            MockAction::Ok => Ok(()),
            MockAction::Fail(msg) => Err(VfsError::Other(msg.clone())),
            _ => Ok(()),
        }
    }

    async fn mkdir(&self, _path: &str) -> Result<(), VfsError> {
        match &self.default_mutate {
            MockAction::Ok => Ok(()),
            MockAction::Fail(msg) => Err(VfsError::Other(msg.clone())),
            _ => Ok(()),
        }
    }
}

// ─── Helpers ─────────────────────────────────────────────────────────────────

fn file_entry(name: &str, size: u64) -> MockEntry {
    MockEntry {
        name: name.to_string(),
        size,
        is_dir: false,
        mtime: SystemTime::UNIX_EPOCH,
        is_symlink: false,
        symlink_target: None,
        uid: Some(1000),
        gid: Some(1000),
    }
}

fn dir_entry(name: &str) -> MockEntry {
    MockEntry {
        name: name.to_string(),
        size: 0,
        is_dir: true,
        mtime: SystemTime::UNIX_EPOCH,
        is_symlink: false,
        symlink_target: None,
        uid: Some(1000),
        gid: Some(1000),
    }
}

fn make_sftp_fs(ops: MockSftpOps) -> cargonaut_vfs::SftpFs {
    cargonaut_vfs::SftpFs::with_ops(Arc::new(ops))
}

// ─── scheme / caps ───────────────────────────────────────────────────────────

#[test]
fn sftp_fs_scheme_is_sftp() {
    let fs = make_sftp_fs(MockSftpOps::new());
    assert_eq!(fs.scheme(), "sftp");
}

#[test]
fn sftp_fs_caps_are_full_remote_set() {
    let fs = make_sftp_fs(MockSftpOps::new());
    let expected = VfsCaps::SEEKABLE
        | VfsCaps::RANDOM_WRITE
        | VfsCaps::METADATA_RICH
        | VfsCaps::ATOMIC_RENAME
        | VfsCaps::SYMLINKS;
    assert_eq!(fs.caps(), expected);
}

// ─── list ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_list_root_returns_entries() {
    let ops = MockSftpOps::new()
        .with_dir("/", vec![dir_entry("home"), file_entry("etc_hosts", 1024)]);
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/").unwrap();
    let listing = fs.list(&path, Sort::NameAsc).await.expect("list root");
    let names: Vec<&str> = listing.entries.iter().map(|e| e.name.as_str()).collect();
    assert!(names.contains(&"home"), "missing home: {names:?}");
    assert!(names.contains(&"etc_hosts"), "missing etc_hosts: {names:?}");
}

#[tokio::test]
async fn sftp_fs_list_not_found_returns_not_found() {
    let ops = MockSftpOps::new();
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/no/such/dir").unwrap();
    let err = fs.list(&path, Sort::NameAsc).await.expect_err("must fail");
    assert!(matches!(err, VfsError::NotFound(_)));
}

// ─── stat ─────────────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_stat_file_returns_metadata_rich_fields() {
    let entry = MockEntry {
        name: "data.bin".to_string(),
        size: 4096,
        is_dir: false,
        mtime: SystemTime::UNIX_EPOCH + std::time::Duration::from_secs(1_700_000_000),
        is_symlink: false,
        symlink_target: None,
        uid: Some(1000),
        gid: Some(1000),
    };
    let ops = MockSftpOps::new().with_stat("/data.bin", entry);
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/data.bin").unwrap();
    let meta = fs.stat(&path).await.expect("stat");
    assert_eq!(meta.size, 4096);
    assert!(matches!(meta.kind, VfsKind::File));
    // METADATA_RICH: mtime must be non-epoch when backend provides it
    assert_ne!(meta.mtime, SystemTime::UNIX_EPOCH);
}

#[tokio::test]
async fn sftp_fs_stat_directory_returns_dir_kind() {
    let ops = MockSftpOps::new().with_stat("/home", dir_entry("home"));
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/home").unwrap();
    let meta = fs.stat(&path).await.expect("stat dir");
    assert!(matches!(meta.kind, VfsKind::Dir));
}

// ─── read_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_read_stream_full_returns_bytes() {
    use futures::AsyncReadExt;
    let ops = MockSftpOps::new().with_read("/readme.txt", b"hello sftp\n".to_vec());
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/readme.txt").unwrap();
    let mut stream = fs.read_stream(&path, ByteRange::FULL).await.expect("read_stream");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read all");
    assert_eq!(buf, b"hello sftp\n");
}

#[tokio::test]
async fn sftp_fs_read_stream_byte_range_returns_slice() {
    use futures::AsyncReadExt;
    let content = b"0123456789abcdef".to_vec();
    let ops = MockSftpOps::new().with_read("/data.bin", content);
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/data.bin").unwrap();
    let range = ByteRange { start: 4, end: Some(8) };
    let mut stream = fs.read_stream(&path, range).await.expect("ranged read");
    let mut buf = Vec::new();
    stream.read_to_end(&mut buf).await.expect("read ranged");
    assert_eq!(buf, b"4567");
}

// ─── write_stream ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_write_stream_truncate_succeeds() {
    let ops = MockSftpOps::new();
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/newfile.txt").unwrap();
    let mut writer = fs.write_stream(&path, 0, WriteMode::Truncate).await.expect("write_stream");
    writer.write_all(b"data").await.expect("write");
    writer.close().await.expect("close");
}

#[tokio::test]
async fn sftp_fs_write_stream_append_at_offset_succeeds() {
    let ops = MockSftpOps::new();
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/append.txt").unwrap();
    let mut writer = fs.write_stream(&path, 100, WriteMode::AppendAtOffset).await.expect("append_at_offset");
    writer.write_all(b"more").await.expect("write");
    writer.close().await.expect("close");
}

// ─── mutating ops ─────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_unlink_succeeds() {
    let fs = make_sftp_fs(MockSftpOps::new());
    let path = VfsPath::parse("sftp://user@host/old.txt").unwrap();
    fs.unlink(&path).await.expect("unlink");
}

#[tokio::test]
async fn sftp_fs_rmdir_succeeds() {
    let fs = make_sftp_fs(MockSftpOps::new());
    let path = VfsPath::parse("sftp://user@host/old_dir").unwrap();
    fs.rmdir(&path).await.expect("rmdir");
}

#[tokio::test]
async fn sftp_fs_rename_succeeds() {
    let fs = make_sftp_fs(MockSftpOps::new());
    let src = VfsPath::parse("sftp://user@host/a.txt").unwrap();
    let dst = VfsPath::parse("sftp://user@host/b.txt").unwrap();
    fs.rename(&src, &dst).await.expect("rename");
}

#[tokio::test]
async fn sftp_fs_mkdir_succeeds() {
    let fs = make_sftp_fs(MockSftpOps::new());
    let path = VfsPath::parse("sftp://user@host/newdir").unwrap();
    fs.mkdir(&path, false).await.expect("mkdir");
}

// ─── error mapping ────────────────────────────────────────────────────────────

#[tokio::test]
async fn sftp_fs_auth_failure_returns_auth_failed() {
    let mut ops = MockSftpOps::new();
    ops.list_responses.insert("/".to_string(), MockAction::AuthFail);
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/").unwrap();
    let err = fs.list(&path, Sort::NameAsc).await.expect_err("must fail");
    assert!(matches!(err, VfsError::AuthFailed(_)), "expected AuthFailed, got {err:?}");
}

#[tokio::test]
async fn sftp_fs_transport_failure_retries_up_to_3_times_then_io() {
    // 3 consecutive transport failures → after 3 retries → VfsError::Io
    let ops = MockSftpOps::new().with_transport_failures(4); // more than retry budget
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/file.txt").unwrap();
    let err = fs.stat(&path).await.expect_err("must fail after retries");
    assert!(
        matches!(err, VfsError::Io(_)),
        "expected Io after retries exhausted, got {err:?}"
    );
}

#[tokio::test]
async fn sftp_fs_transport_failure_succeeds_after_retry() {
    // 2 transport failures then success — within the 3-retry budget
    let ops = MockSftpOps::new()
        .with_transport_failures(2)
        .with_stat("/ok.txt", file_entry("ok.txt", 42));
    let fs = make_sftp_fs(ops);
    let path = VfsPath::parse("sftp://user@host/ok.txt").unwrap();
    let meta = fs.stat(&path).await.expect("should succeed after 2 retries");
    assert_eq!(meta.size, 42);
}

// ─── SECURITY GATE: credential redaction ─────────────────────────────────────

/// Verifies that authentication failure tracing events do NOT contain the
/// raw password from `SftpCredentials::Password`. This is a mandatory security
/// gate per Constitution §Dev Workflow.
#[tokio::test]
async fn sftp_auth_failure_log_does_not_leak_password() {
    use std::sync::{Arc, Mutex};

    // Capture tracing events into a Vec<String>
    let captured: Arc<Mutex<Vec<String>>> = Arc::new(Mutex::new(Vec::new()));
    let captured_clone = captured.clone();

    let make_writer = move || {
        let cap = captured_clone.clone();
        cargonaut_vfs::sftp::testing::CaptureWriter::new(cap)
    };

    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .with_writer(make_writer)
            .with_ansi(false),
    );

    let _guard = tracing::subscriber::set_default(subscriber);

    const SECRET: &str = "super_secret_password_do_not_log";
    let creds = SftpCredentials::Password(SECRET.to_string());

    let mut ops = MockSftpOps::new();
    ops.list_responses.insert("/".to_string(), MockAction::AuthFail);
    let fs = cargonaut_vfs::SftpFs::with_ops_and_creds(Arc::new(ops), creds);

    let path = VfsPath::parse("sftp://user@host/").unwrap();
    let _ = fs.list(&path, Sort::NameAsc).await; // expect AuthFailed

    let logs = captured.lock().unwrap();
    for line in logs.iter() {
        assert!(
            !line.contains(SECRET),
            "SECURITY GATE FAILED: password leaked in log line:\n  {line}"
        );
    }
}

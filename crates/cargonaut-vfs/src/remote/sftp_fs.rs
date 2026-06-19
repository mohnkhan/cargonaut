// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`SftpFs`] — read/write [`VfsBackend`] for `sftp://` paths.

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
    time::{Duration, SystemTime},
};
use tracing::{info, warn};

// ---------------------------------------------------------------------------
// Public types
// ---------------------------------------------------------------------------

/// Credential strategy for [`SftpFs::connect`].
pub enum SftpCredentials {
    /// Try `~/.ssh/id_ed25519` then `~/.ssh/id_rsa` (no passphrase).
    Agent,
    /// Explicit private-key file path (no passphrase).
    KeyFile(std::path::PathBuf),
    /// Plaintext password. NEVER logged.
    Password(String),
}

/// Event emitted when the server's host key is not in `known_hosts`.
///
/// The handler must send `true` (accept) or `false` (reject) through
/// `accept_tx`.  Sending `true` also writes the key to `known_hosts` so
/// subsequent connections are silent.
pub struct HostKeyEvent {
    /// SHA-256 fingerprint suitable for display, e.g. `"SHA256:..."`.
    pub fingerprint: String,
    /// `"host:port"` string.
    pub host: String,
    /// Channel back to the connection handler.  Drop without sending to reject.
    pub accept_tx: tokio::sync::oneshot::Sender<bool>,
}

// ---------------------------------------------------------------------------
// SftpOps — session abstraction (enables mock injection in tests)
// ---------------------------------------------------------------------------

/// Thin async abstraction over an active SFTP session.
///
/// `RealSftpOps` is the production implementation; tests inject a mock.
#[async_trait]
pub trait SftpOps: Send + Sync + 'static {
    /// List a directory; returns `(name, metadata)` pairs.
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError>;

    /// Stat a single path.
    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError>;

    /// Read bytes from `path`.  `offset` and optional `len` select a range.
    async fn read_bytes(
        &self,
        path: &str,
        offset: u64,
        len: Option<u64>,
    ) -> Result<Vec<u8>, VfsError>;

    /// Write `data` to `path`.
    ///
    /// - `truncate = true` creates/truncates the file, then writes.
    /// - `truncate = false` opens with APPEND and writes at `offset`.
    async fn write_all(
        &self,
        path: &str,
        data: &[u8],
        offset: u64,
        truncate: bool,
    ) -> Result<(), VfsError>;

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
// Helper: convert FileAttributes → VfsMetadata
// ---------------------------------------------------------------------------

fn attrs_to_metadata(meta: &russh_sftp::protocol::FileAttributes, name: &str) -> VfsMetadata {
    let size = meta.size.unwrap_or(0);
    let mtime = meta
        .mtime
        .map(|t| SystemTime::UNIX_EPOCH + Duration::from_secs(t as u64))
        .unwrap_or(SystemTime::UNIX_EPOCH);
    let file_mode = meta.permissions.map(|bits| crate::types::FileMode {
        bits: bits & 0o777,
        uid: meta.uid,
        gid: meta.gid,
    });
    let kind = if meta.is_dir() {
        VfsKind::Dir
    } else if meta.is_symlink() {
        // SFTP metadata does not carry the link target inline; a separate
        // readlink RPC would be needed.  We surface a zero-segment target so
        // callers can detect the symlink type and issue their own readlink.
        VfsKind::Symlink {
            target: Box::new(VfsPath {
                scheme: SmolStr::new("sftp"),
                authority: None,
                segments: smallvec::smallvec![],
            }),
        }
    } else {
        VfsKind::File
    };
    VfsMetadata {
        size,
        mtime,
        mode: file_mode,
        kind,
        is_hidden: name.starts_with('.'),
    }
}

// ---------------------------------------------------------------------------
// RealSftpOps
// ---------------------------------------------------------------------------

struct RealSftpOps {
    sftp: Arc<russh_sftp::client::SftpSession>,
}

#[async_trait]
impl SftpOps for RealSftpOps {
    async fn list_dir(&self, path: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError> {
        let dir = self
            .sftp
            .read_dir(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        let mut entries = Vec::new();
        for entry in dir {
            let name = entry.file_name();
            let raw_meta = entry.metadata();
            let meta = attrs_to_metadata(&raw_meta, &name);
            entries.push((name, meta));
        }
        Ok(entries)
    }

    async fn stat(&self, path: &str) -> Result<VfsMetadata, VfsError> {
        let attrs = self
            .sftp
            .metadata(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        // Use the last segment of the path as the name for is_hidden detection.
        let name = path.rsplit('/').next().unwrap_or(path);
        Ok(attrs_to_metadata(&attrs, name))
    }

    async fn read_bytes(
        &self,
        path: &str,
        offset: u64,
        len: Option<u64>,
    ) -> Result<Vec<u8>, VfsError> {
        let full = self
            .sftp
            .read(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        let start = (offset as usize).min(full.len());
        let slice = &full[start..];
        let slice = match len {
            Some(l) => &slice[..(l as usize).min(slice.len())],
            None => slice,
        };
        Ok(slice.to_vec())
    }

    async fn write_all(
        &self,
        path: &str,
        data: &[u8],
        offset: u64,
        truncate: bool,
    ) -> Result<(), VfsError> {
        use russh_sftp::protocol::OpenFlags;
        use tokio::io::{AsyncSeekExt, AsyncWriteExt};

        if truncate {
            // CREATE | TRUNCATE | WRITE — creates or overwrites.
            let mut file = self
                .sftp
                .open_with_flags(path, OpenFlags::WRITE | OpenFlags::CREATE | OpenFlags::TRUNCATE)
                .await
                .map_err(|e| io::Error::other(e.to_string()))?;
            file.write_all(data)
                .await
                .map_err(|e| io::Error::other(e.to_string()))?;
        } else {
            let mut file = self
                .sftp
                .open_with_flags(path, OpenFlags::WRITE | OpenFlags::APPEND)
                .await
                .map_err(|e| io::Error::other(e.to_string()))?;
            if offset > 0 {
                file.seek(std::io::SeekFrom::Start(offset))
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?;
            }
            file.write_all(data)
                .await
                .map_err(|e| io::Error::other(e.to_string()))?;
        }
        Ok(())
    }

    async fn unlink(&self, path: &str) -> Result<(), VfsError> {
        self.sftp
            .remove_file(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn rmdir(&self, path: &str) -> Result<(), VfsError> {
        self.sftp
            .remove_dir(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn rename(&self, src: &str, dest: &str) -> Result<(), VfsError> {
        self.sftp
            .rename(src, dest)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }

    async fn mkdir(&self, path: &str) -> Result<(), VfsError> {
        self.sftp
            .create_dir(path)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// ClientHandler — russh client handler
// ---------------------------------------------------------------------------

struct ClientHandler {
    host: String,
    port: u16,
    host_key_tx: tokio::sync::mpsc::UnboundedSender<HostKeyEvent>,
}

impl russh::client::Handler for ClientHandler {
    type Error = russh::Error;

    async fn check_server_key(
        &mut self,
        server_public_key: &russh::keys::ssh_key::PublicKey,
    ) -> Result<bool, Self::Error> {
        use russh::keys::known_hosts;
        use russh::keys::ssh_key::HashAlg;

        match known_hosts::check_known_hosts(&self.host, self.port, server_public_key) {
            Ok(true) => {
                // Known and fingerprint matches.
                return Ok(true);
            }
            Ok(false) => {
                // Not in known_hosts yet — ask the user.
            }
            Err(_) => {
                // Key mismatch or file-read error — treat conservatively: ask the user.
            }
        }

        let fingerprint = server_public_key.fingerprint(HashAlg::Sha256).to_string();
        let host_str = format!("{}:{}", self.host, self.port);

        let (accept_tx, accept_rx) = tokio::sync::oneshot::channel();
        let event = HostKeyEvent {
            fingerprint,
            host: host_str.clone(),
            accept_tx,
        };

        // Fire-and-forget: if the UI has shut down the channel, treat as reject.
        if self.host_key_tx.send(event).is_err() {
            return Ok(false);
        }

        let accepted = accept_rx.await.unwrap_or(false);
        if accepted {
            if let Err(e) =
                known_hosts::learn_known_hosts(&self.host, self.port, server_public_key)
            {
                warn!(host = %host_str, error = %e, "failed to persist known_hosts entry");
            }
        }
        Ok(accepted)
    }
}

// ---------------------------------------------------------------------------
// SftpBufferWriter — buffers writes in memory, flushes on close
// ---------------------------------------------------------------------------

enum WriteState {
    Buffering {
        buf: Vec<u8>,
        path: String,
        offset: u64,
        truncate: bool,
        ops: Arc<dyn SftpOps>,
    },
    Closing(Pin<Box<dyn Future<Output = Result<(), VfsError>> + Send>>),
    Closed,
}

struct SftpBufferWriter {
    state: WriteState,
}

// SAFETY: WriteState::Buffering holds Arc<dyn SftpOps: Send+Sync>, Vec<u8>,
// String, u64 — all Send.  WriteState::Closing holds a Send Future.
unsafe impl Send for SftpBufferWriter {}

impl futures::io::AsyncWrite for SftpBufferWriter {
    fn poll_write(
        mut self: Pin<&mut Self>,
        _cx: &mut Context<'_>,
        buf: &[u8],
    ) -> Poll<io::Result<usize>> {
        match &mut self.state {
            WriteState::Buffering { buf: inner, .. } => {
                inner.extend_from_slice(buf);
                Poll::Ready(Ok(buf.len()))
            }
            WriteState::Closing(_) | WriteState::Closed => {
                Poll::Ready(Err(io::Error::other("write after close")))
            }
        }
    }

    fn poll_flush(self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        // All bytes are held in memory; no partial flush to wire.
        Poll::Ready(Ok(()))
    }

    fn poll_close(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<io::Result<()>> {
        loop {
            match &mut self.state {
                WriteState::Buffering { .. } => {
                    // Pull data out of Buffering, build the write Future, advance to Closing.
                    let (buf, path, offset, truncate, ops) =
                        match std::mem::replace(&mut self.state, WriteState::Closed) {
                            WriteState::Buffering {
                                buf,
                                path,
                                offset,
                                truncate,
                                ops,
                            } => (buf, path, offset, truncate, ops),
                            _ => unreachable!(),
                        };
                    let fut: Pin<Box<dyn Future<Output = Result<(), VfsError>> + Send>> =
                        Box::pin(async move { ops.write_all(&path, &buf, offset, truncate).await });
                    self.state = WriteState::Closing(fut);
                    // Continue in loop to poll the future.
                }
                WriteState::Closing(fut) => {
                    match fut.as_mut().poll(cx) {
                        Poll::Pending => return Poll::Pending,
                        Poll::Ready(Ok(())) => {
                            self.state = WriteState::Closed;
                            return Poll::Ready(Ok(()));
                        }
                        Poll::Ready(Err(VfsError::Io(e))) => {
                            self.state = WriteState::Closed;
                            return Poll::Ready(Err(e));
                        }
                        Poll::Ready(Err(other)) => {
                            self.state = WriteState::Closed;
                            return Poll::Ready(Err(io::Error::other(other.to_string())));
                        }
                    }
                }
                WriteState::Closed => return Poll::Ready(Ok(())),
            }
        }
    }
}

// ---------------------------------------------------------------------------
// SftpFs — the public VFS backend
// ---------------------------------------------------------------------------

/// Read/write VFS backend for SFTP servers (`sftp://` scheme).
pub struct SftpFs {
    ops: Arc<dyn SftpOps>,
    /// Stored for potential reconnect logic; currently unused after connect.
    #[allow(dead_code)]
    creds: Option<SftpCredentials>,
    /// Delays (ms) between retries: `[attempt-1, attempt-2, attempt-3]`.
    retry_delays_ms: [u64; 3],
}

impl SftpFs {
    /// Construct from an existing `SftpOps` implementation (e.g. a mock).
    /// No retries — all retry delays are 0.
    pub fn with_ops(ops: Arc<dyn SftpOps>) -> Self {
        Self {
            ops,
            creds: None,
            retry_delays_ms: [0, 0, 0],
        }
    }

    /// Construct from an existing `SftpOps` and a credential strategy.
    /// No retries — all retry delays are 0.
    pub fn with_ops_and_creds(ops: Arc<dyn SftpOps>, creds: SftpCredentials) -> Self {
        Self {
            ops,
            creds: Some(creds),
            retry_delays_ms: [0, 0, 0],
        }
    }

    /// Open a real SFTP connection to `authority` (`"user@host:port"`).
    ///
    /// The `host_key_tx` channel receives a [`HostKeyEvent`] for every unknown
    /// or mismatched host key — the caller (typically the UI) must respond via
    /// `accept_tx` before the connection continues.
    pub async fn connect(
        authority: &str,
        credentials: SftpCredentials,
        config: Arc<russh::client::Config>,
        host_key_tx: tokio::sync::mpsc::UnboundedSender<HostKeyEvent>,
    ) -> Result<Self, VfsError> {
        let (user, host, port) = parse_authority_parts(authority);

        let handler = ClientHandler {
            host: host.clone(),
            port,
            host_key_tx,
        };

        let mut session = russh::client::connect(config, (host.as_str(), port), handler)
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        let authed = match &credentials {
            SftpCredentials::Password(pwd) => {
                let result = session
                    .authenticate_password(user.as_str(), pwd.as_str())
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?;
                result.success()
            }
            SftpCredentials::KeyFile(path) => {
                let key_pair = russh::keys::load_secret_key(path, None)
                    .map_err(|e| io::Error::other(e.to_string()))?;
                let key = russh::keys::PrivateKeyWithHashAlg::new(Arc::new(key_pair), None);
                let result = session
                    .authenticate_publickey(user.as_str(), key)
                    .await
                    .map_err(|e| io::Error::other(e.to_string()))?;
                result.success()
            }
            SftpCredentials::Agent => {
                // Try ~/.ssh/id_ed25519 then ~/.ssh/id_rsa; no passphrase.
                let home = std::env::var("HOME").unwrap_or_else(|_| "/root".into());
                let candidates = [
                    std::path::PathBuf::from(format!("{home}/.ssh/id_ed25519")),
                    std::path::PathBuf::from(format!("{home}/.ssh/id_rsa")),
                ];
                let mut success = false;
                for key_path in &candidates {
                    if !key_path.exists() {
                        continue;
                    }
                    match russh::keys::load_secret_key(key_path, None) {
                        Ok(key_pair) => {
                            let key = russh::keys::PrivateKeyWithHashAlg::new(
                                Arc::new(key_pair),
                                None,
                            );
                            match session.authenticate_publickey(user.as_str(), key).await {
                                Ok(result) if result.success() => {
                                    success = true;
                                    break;
                                }
                                _ => continue,
                            }
                        }
                        Err(_) => continue,
                    }
                }
                success
            }
        };

        if !authed {
            warn!(user = %user, host = %host, "sftp auth failed");
            return Err(VfsError::AuthFailed(format!(
                "authentication failed for {user}@{host}"
            )));
        }

        info!(user = %user, host = %host, port, "sftp authenticated");

        let channel = session
            .channel_open_session()
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        channel
            .request_subsystem(true, "sftp")
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;
        let sftp_session = russh_sftp::client::SftpSession::new(channel.into_stream())
            .await
            .map_err(|e| io::Error::other(e.to_string()))?;

        Ok(Self {
            ops: Arc::new(RealSftpOps {
                sftp: Arc::new(sftp_session),
            }),
            creds: Some(credentials),
            retry_delays_ms: [200, 400, 800],
        })
    }

    /// Retry wrapper: 4 total attempts (initial + 3 retries).
    ///
    /// - `VfsError::Io` → warn and retry after `retry_delays_ms[attempt]`.
    /// - `VfsError::AuthFailed` → warn and return immediately (no retry).
    /// - Other errors → return immediately.
    async fn with_retry<F, Fut, T>(&self, user: &str, host: &str, f: F) -> Result<T, VfsError>
    where
        F: Fn() -> Fut,
        Fut: Future<Output = Result<T, VfsError>>,
    {
        let mut last_err = None;
        for attempt in 0u32..4 {
            if attempt > 0 {
                let delay_ms = self.retry_delays_ms[(attempt - 1) as usize];
                if delay_ms > 0 {
                    tokio::time::sleep(Duration::from_millis(delay_ms)).await;
                }
            }
            match f().await {
                Ok(val) => return Ok(val),
                Err(VfsError::Io(e)) => {
                    warn!(
                        attempt = attempt + 1,
                        user = %user,
                        host = %host,
                        error = %e,
                        "sftp transport error"
                    );
                    last_err = Some(VfsError::Io(e));
                    // retry
                }
                Err(VfsError::AuthFailed(msg)) => {
                    warn!(user = %user, host = %host, "sftp auth failed");
                    return Err(VfsError::AuthFailed(msg));
                }
                Err(other) => return Err(other),
            }
        }
        Err(last_err.unwrap())
    }
}

// ---------------------------------------------------------------------------
// Path helpers
// ---------------------------------------------------------------------------

fn vfs_path_to_sftp_path(path: &VfsPath) -> String {
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

/// Parse `"user@host:port"` into `(user, host, port)`.
///
/// Defaults: user = `"anonymous"`, port = `22`.
fn parse_authority_parts(authority: &str) -> (String, String, u16) {
    // Determine the user@host portion and whether there is a port suffix.
    let (user_host, port_str) = match authority.rfind(':') {
        Some(colon_idx) => {
            let possible_port = &authority[colon_idx + 1..];
            if !possible_port.is_empty() && possible_port.chars().all(|c| c.is_ascii_digit()) {
                (&authority[..colon_idx], possible_port)
            } else {
                (authority, "22")
            }
        }
        None => (authority, "22"),
    };
    let port: u16 = port_str.parse().unwrap_or(22);

    let (user, host) = match user_host.find('@') {
        Some(at_idx) => (&user_host[..at_idx], &user_host[at_idx + 1..]),
        None => ("anonymous", user_host),
    };

    (user.to_string(), host.to_string(), port)
}

// ---------------------------------------------------------------------------
// VfsBackend impl
// ---------------------------------------------------------------------------

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

    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        let raw = self
            .with_retry(&user, &host, || {
                let sftp_path = sftp_path.clone();
                let ops = ops.clone();
                async move { ops.list_dir(&sftp_path).await }
            })
            .await?;

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
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        self.with_retry(&user, &host, || {
            let sftp_path = sftp_path.clone();
            let ops = ops.clone();
            async move { ops.stat(&sftp_path).await }
        })
        .await
    }

    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        let len = range.end.map(|end| end.saturating_sub(range.start));
        let bytes = self
            .with_retry(&user, &host, || {
                let sftp_path = sftp_path.clone();
                let ops = ops.clone();
                async move { ops.read_bytes(&sftp_path, range.start, len).await }
            })
            .await?;
        Ok(Box::pin(futures::io::Cursor::new(bytes)))
    }

    async fn write_stream(
        &self,
        path: &VfsPath,
        offset: u64,
        mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let truncate = mode == WriteMode::Truncate;
        let writer = SftpBufferWriter {
            state: WriteState::Buffering {
                buf: vec![],
                path: sftp_path,
                offset,
                truncate,
                ops: self.ops.clone(),
            },
        };
        Ok(Box::pin(writer))
    }

    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        self.with_retry(&user, &host, || {
            let sftp_path = sftp_path.clone();
            let ops = ops.clone();
            async move { ops.unlink(&sftp_path).await }
        })
        .await
    }

    async fn rmdir(&self, path: &VfsPath) -> Result<(), VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        self.with_retry(&user, &host, || {
            let sftp_path = sftp_path.clone();
            let ops = ops.clone();
            async move { ops.rmdir(&sftp_path).await }
        })
        .await
    }

    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError> {
        let src_path = vfs_path_to_sftp_path(src);
        let dest_path = vfs_path_to_sftp_path(dest);
        let authority = src.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        self.with_retry(&user, &host, || {
            let src_path = src_path.clone();
            let dest_path = dest_path.clone();
            let ops = ops.clone();
            async move { ops.rename(&src_path, &dest_path).await }
        })
        .await
    }

    async fn mkdir(&self, path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        let sftp_path = vfs_path_to_sftp_path(path);
        let authority = path.authority.as_deref().unwrap_or("");
        let (user, host, _port) = parse_authority_parts(authority);
        let ops = self.ops.clone();
        self.with_retry(&user, &host, || {
            let sftp_path = sftp_path.clone();
            let ops = ops.clone();
            async move { ops.mkdir(&sftp_path).await }
        })
        .await
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_authority_full() {
        let (user, host, port) = parse_authority_parts("alice@example.com:2222");
        assert_eq!(user, "alice");
        assert_eq!(host, "example.com");
        assert_eq!(port, 2222);
    }

    #[test]
    fn parse_authority_no_port() {
        let (user, host, port) = parse_authority_parts("bob@myserver");
        assert_eq!(user, "bob");
        assert_eq!(host, "myserver");
        assert_eq!(port, 22);
    }

    #[test]
    fn parse_authority_no_user() {
        let (user, host, port) = parse_authority_parts("somehost:22");
        assert_eq!(user, "anonymous");
        assert_eq!(host, "somehost");
        assert_eq!(port, 22);
    }

    #[test]
    fn parse_authority_no_user_no_port() {
        let (user, host, port) = parse_authority_parts("barehost");
        assert_eq!(user, "anonymous");
        assert_eq!(host, "barehost");
        assert_eq!(port, 22);
    }

    #[test]
    fn vfs_path_to_sftp_root() {
        let p = VfsPath::parse("sftp://host/").unwrap();
        assert_eq!(vfs_path_to_sftp_path(&p), "/");
    }

    #[test]
    fn vfs_path_to_sftp_nested() {
        let p = VfsPath::parse("sftp://host/var/log/app").unwrap();
        assert_eq!(vfs_path_to_sftp_path(&p), "/var/log/app");
    }

    struct NullOps;

    #[async_trait]
    impl SftpOps for NullOps {
        async fn list_dir(&self, _: &str) -> Result<Vec<(String, VfsMetadata)>, VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn stat(&self, _: &str) -> Result<VfsMetadata, VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn read_bytes(
            &self,
            _: &str,
            _: u64,
            _: Option<u64>,
        ) -> Result<Vec<u8>, VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn write_all(
            &self,
            _: &str,
            _: &[u8],
            _: u64,
            _: bool,
        ) -> Result<(), VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn unlink(&self, _: &str) -> Result<(), VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn rmdir(&self, _: &str) -> Result<(), VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn rename(&self, _: &str, _: &str) -> Result<(), VfsError> {
            Err(VfsError::Other("null".into()))
        }
        async fn mkdir(&self, _: &str) -> Result<(), VfsError> {
            Err(VfsError::Other("null".into()))
        }
    }

    #[test]
    fn with_ops_scheme_and_caps() {
        let fs = SftpFs::with_ops(Arc::new(NullOps));
        assert_eq!(fs.scheme(), "sftp");
        assert!(fs.caps().contains(VfsCaps::SEEKABLE));
        assert!(fs.caps().contains(VfsCaps::RANDOM_WRITE));
        assert!(fs.caps().contains(VfsCaps::METADATA_RICH));
        assert!(fs.caps().contains(VfsCaps::ATOMIC_RENAME));
        assert!(fs.caps().contains(VfsCaps::SYMLINKS));
    }

    #[test]
    fn with_ops_and_creds_stores_creds() {
        let fs = SftpFs::with_ops_and_creds(
            Arc::new(NullOps),
            SftpCredentials::Password("secret".into()),
        );
        assert!(fs.creds.is_some());
        assert_eq!(fs.retry_delays_ms, [0, 0, 0]);
    }
}

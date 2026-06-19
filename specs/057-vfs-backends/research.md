# Research: Feature 057 — VFS Backends (Archives + Remote)

## R-001 — ZIP crate API

**Decision**: Use `zip = "2.6"` (zip 2.6.0) with `spawn_blocking` for all I/O.

**Rationale**: The `zip` crate is the de-facto Rust ZIP library; v2 adds support for ZIP64 and cleans up the API. Sync I/O wrapped in `tokio::task::spawn_blocking` is idiomatic for blocking archive reading inside an async runtime — avoids starving the executor.

**Key API surface**:
- `ZipArchive::new(reader: R) -> Result<ZipArchive<R>, ZipError>` — opens the central directory.
- `archive.len()` — number of entries.
- `archive.by_index(i)` → `ZipFile` — open entry by index; provides `.name()`, `.size()`, `.compressed_size()`, `.last_modified()`, `.compression()`, `.is_dir()`.
- `archive.by_name(name)` → `ZipFile` — open by name (requires scanning, use cached index).
- Entry names are arbitrary strings; may contain `/` separating path components; entries ending in `/` are directories.
- Encryption: `ZipFile::encrypted()` → `bool`. Opening an encrypted entry without a password yields `ZipError::UnsupportedArchive("Password required")`.
- ZIP64 is transparent; large archives (>4 GiB, >65535 entries) work without caller changes.
- `ZipArchive` is conditionally `Send+Sync` but `by_index`/`by_name` take `&mut self` — only one entry at a time per instance. We use `Arc<Mutex<ZipArchive<File>>>` for the cached reader.
- **Path traversal safety**: Use `entry.enclosed_name()` (not `.name()`) — `enclosed_name()` returns `None` for any entry whose path contains `..` or an absolute root component. This is the correct API; using `.name()` silently exposes traversal paths.
- Build a `HashMap<String, usize>` name→index cache on open (since `by_name()` is O(n) linear scan).

**SEEKABLE decision**: `ZipFs` does NOT declare `VfsCaps::SEEKABLE`. Whole-file reads only (clarified in spec). The backend decompresses each entry from its compressed start on every read — acceptable for the read-via-copy-out use case.

**Alternatives considered**: `rc-zip` (streaming parser, more complex API). Rejected — zip crate has better ecosystem support and simpler sync API.

---

## R-002 — TAR + compression crates

**Decision**: `tar = "0.4.46"` + `flate2 = "1.1"` + `bzip2 = "0.6"` + `xz2 = "0.1"`, all wrapped in `spawn_blocking`.

**⚠️ C dependency**: `xz2` wraps `liblzma` (a C library). This means the `archives` feature is NOT fully pure-Rust for `.xz` support. Two options: (A) accept the C dep and require `liblzma-dev` in CI; (B) gate `.xz` support behind a separate `archives-xz` feature. **Choice: option A** — liblzma is universally available on Linux; CI already runs Linux.

**Rationale**: These are the canonical Rust crates for their respective formats. All are synchronous and well-maintained.

**Key API surface**:
- `Archive::new(reader)` → archive over any `Read`.
- For `.tar.gz`: `Archive::new(GzDecoder::new(file))`; for `.tar.bz2`: `BzDecoder`; for `.tar.xz`: `XzDecoder`.
- `archive.entries()` → iterator of `Entry`. Each provides: `.path()` (yields `Cow<Path>`), `.header().size()`, `.header().mtime()`, `.header().entry_type()` (`Regular`, `Directory`, `Symlink`, etc.), `.header().link_name()` for symlinks.
- TAR is sequential — to read entry content after building the index, we must re-open the archive file and scan to the recorded byte offset.

**Byte-offset index approach**: On first `list()`, scan all entries recording `(entry_path, byte_offset, size, metadata)`. On `read_stream(path)`, re-open the file, seek to `byte_offset` using `File::seek`, then wrap in the appropriate decompressor and read `size` bytes.
- Works for uncompressed TAR (file seek is O(1)).
- For compressed TAR: `raw_file_position()` records offsets in the *compressed* byte stream — seeking into a streaming decoder is not feasible. On each `read_stream` call, re-open and re-scan the archive from the beginning, counting entries until reaching the target (identified by name). For read-then-copy use cases this is acceptable; performance degrades for very large compressed archives but is bounded by archive size.
- **Mandatory drain**: Every `Entry` must be fully drained via `io::copy(&mut entry, &mut io::sink())` before the iterator can advance; omitting this silently corrupts subsequent entries.

**Path traversal safety**: The `tar` crate does NOT automatically reject `../` entries. We MUST call `entry.path()?.components()` and skip any entry whose components include `Component::ParentDir` or `Component::RootDir`. This check runs during the initial scan; unsafe entries are logged at `WARN` and omitted from the index.

**`TarFs` SEEKABLE**: Not declared. Byte-range reads return `VfsError::Unsupported` (aligned with `ZipFs` policy from spec clarification).

**Alternatives considered**: `async-tar` (unstable, not maintained). Rejected.

---

## R-003 — SFTP: russh + russh-sftp

**Decision**: `russh = "0.61"` (v0.61.2) + `russh-sftp = "2.1"` (v2.1.2, patches CVE-2026-46673 OOM). Crypto backend: `features = ["aws-lc-rs"]` (required; must choose one of `aws-lc-rs` or `ring`).

**Rationale**: Only pure-Rust async SSH library with active maintenance. Avoids the `libssh2` C dependency that would complicate cross-compilation. `russh-sftp` is the official SFTP subsystem companion.

**Key API surface**:

*Connection establishment*:
```rust
// 1. TCP connect
let stream = TcpStream::connect(addr).await?;
// 2. SSH handshake (key verification via custom Handler)
let handler = HostKeyHandler { known_hosts_path, pending_accept_tx };
let (client, _) = russh::client::connect(Arc::new(config), addr, handler).await?;
// 3. Authenticate
client.authenticate_publickey("user", key_pair).await?;
// 4. Open SFTP channel
let channel = client.channel_open_session().await?;
channel.request_subsystem(true, "sftp").await?;
let sftp = SftpSession::new(channel.into_stream()).await?;
```

*SFTP operations*:
- `sftp.read_dir(path)` → `Vec<(String, FileAttributes)>` — list directory.
- `sftp.metadata(path)` → `FileAttributes` — stat (follows symlinks).
- `sftp.symlink_metadata(path)` → `FileAttributes` — lstat (does not follow).
- `sftp.open(path)` → `File` — opens for reading; `file.read_at(offset, buf)` for range reads.
- `sftp.create(path)` → `File` — truncates and opens for writing.
- `sftp.remove_file(path)` — unlink.
- `sftp.remove_dir(path)` — rmdir.
- `sftp.rename(from, to)` — atomic rename.
- `sftp.create_dir(path)` — mkdir (non-recursive only; we layer recursive on top).
- `sftp.symlink(target, link)` — symlink.

*Host-key verification*: Implement `russh::client::Handler` trait; override `check_server_key(key: &ssh_key::PublicKey) -> Result<bool>`. Built-in helpers: `russh::keys::check_known_hosts(host, port, key)` (checks `~/.ssh/known_hosts`) and `russh::keys::learn_known_hosts_path(host, port, key, path)` (appends accepted key, creates file if absent, handles HMAC-hashed entries and bracket notation for non-22 ports). No separate `ssh-key` crate needed — import types via `russh::keys::ssh_key::PublicKey`.

On an unknown key: the handler sends the fingerprint through a `oneshot` channel to the UI layer (blocking modal) and awaits the bool response before returning.

**Breaking API note (since 0.50)**: `authenticate_publickey` now takes `PrivateKeyWithHashAlg::new(Arc::new(key), best_supported_rsa_hash)`. Non-RSA keys pass `None` for hash algorithm.

*Reconnection*: On `Io` or `ConnectionClosed` errors, retry the full connect sequence up to 3 times with exponential backoff (200ms × 2^attempt, capped at 5s). Wrap in a helper `async fn with_reconnect<F, R>(...)`.

*Pipelining*: `SftpFs` can issue up to `config.sftp.pipelined_reads` concurrent `file.read_at(...)` calls over the same SFTP session using `tokio::join!` / `FuturesUnordered`. The session multiplexes over one TCP connection internally.

*Connection sharing*: `SftpSession` is `Clone` (it's an `Arc` internally) — multiple `SftpFs` clones can share one session. The `App` creates one `SftpFs` per connected server and stores it in `VfsRegistry`; panes receive `Arc<dyn VfsBackend>`.

**Alternatives considered**: `openssh` (spawns real `ssh` binary — not pure-Rust). `async-ssh2-lite` (wraps libssh2). Both rejected for C dependency / process-spawn approach.

---

## R-004 — FTP: suppaftp

**Decision**: `suppaftp = "8"` (v8.0.4), `features = ["tokio"]`, with TLS features disabled (plain FTP only; FTPS is out of scope).

**Rationale**: `suppaftp` is the most actively maintained async FTP client for Rust, with MLSD/MLST support. Its `AsyncFtpStream` is built on `tokio`.

**Key API surface**:
- `AsyncFtpStream::connect(addr)` → stream; `.login(user, pass).await?`.
- `stream.nlst(path)` → `Vec<String>` (just names); `stream.list(path)` → raw LIST output.
- `stream.mlsd(path)` → `Vec<Entry>` — structured MLSD (Modern LIST); preferred when server supports it. Each entry has `name`, `size`, `modify` (timestamp), `type` (file/dir/..link..).
- `stream.size(path)` → `u64` — file size (SIZE command).
- `stream.retr_as_stream(path)` → `DataStream` — download.
- `stream.put_file(path, reader)` — upload.
- `stream.rm(path)` — delete file.
- `stream.rmdir(path)` — remove directory.
- `stream.rename(from, to)` — rename (RNFR/RNTO).
- `stream.mkdir(path)` — create directory.

**Limitations**:
- `AsyncFtpStream` is NOT `Send + Sync` after `connect`; must be wrapped in `Arc<Mutex<AsyncFtpStream>>`. All operations serialize over the single control connection.
- FTP has no byte-range read (REST + RETR is unreliable on passive-mode servers); `FtpFs` does not declare `SEEKABLE` or `RANDOM_WRITE`.
- MLSD detection: `suppaftp` negotiates MLSD automatically via the `FEAT` command; falls back to `LIST` parsing if unsupported.
- Symlinks: FTP has no standardised symlink representation; `FtpFs` does not declare `SYMLINKS`.

**Thread-safety**: `AsyncFtpStream` is `Send + Sync` (explicitly guaranteed since v6.0.3). However it takes `&mut self` on all operations — internal concurrency is blocked by `data_connection_open` flag; a second data transfer on the same connection returns `DataConnectionAlreadyOpen`.

**Connection model**: One `Arc<Mutex<AsyncFtpStream>>` per `FtpFs` instance. Operations acquire the mutex, perform the FTP command, and release. Since FTP data transfers hold the mutex for their duration, concurrent transfers over the same FTP connection are serialised (FTP's architectural limitation). A connection pool is out of scope for this feature.

**MLSD**: `mlsd()` is a first-class method (v5.4.0+); NOT auto-negotiated. `FtpFs::list()` tries `mlsd()` first; falls back to `list()` + `ListParser::parse_posix()` on failure. The `mlst()` method gives structured single-entry metadata, used for `stat()`.

**Alternatives considered**: `async-ftp` (unmaintained). `ftp-async` (less complete). Both rejected.

---

## R-005 — VfsRegistry design

**Decision**: `VfsRegistry` holds two lookup tables:
1. `scheme_map: HashMap<SmolStr, Arc<dyn VfsBackend>>` — for singleton backends indexed by scheme only (LocalFs = `"file"`; future S3 etc.).
2. `remote_map: HashMap<SmolStr, Arc<dyn VfsBackend>>` — for connection-scoped backends indexed by `"{scheme}://{authority}"` (e.g. `"sftp://user@host:22"`).

Archive backends (`ZipFs`, `TarFs`) are NOT stored in the registry — they are ephemeral, per-pane, created on Descend-into-archive and dropped when the pane leaves the archive.

**Lookup**: `VfsRegistry::resolve(path: &VfsPath) -> Option<Arc<dyn VfsBackend>>`:
1. If `path.authority.is_none()`: look up `scheme_map[path.scheme]`.
2. Else: look up `remote_map["{scheme}://{authority}"]`; fall back to `scheme_map[path.scheme]`.

**`PaneState` change**: Add `backend: Arc<dyn VfsBackend>`. Populated by `App::navigate_to` from either the registry (for file/sftp/ftp) or a newly-constructed archive backend (for zip/tar).

**`App` change**: Replace `local_fs: Arc<dyn VfsBackend>` with `registry: Arc<VfsRegistry>`. All `self.local_fs` call sites become `self.registry.local()` (a convenience accessor returning the `file` backend).

---

## R-006 — VfsPath percent-encoding for archive authority

**Decision**: Archive backends decode `%2F` → `/` in the authority field when constructing the host filesystem path. Encoding happens at the call site (the UI / `navigate_to` logic) when building the `VfsPath` for a new archive mount.

**Implementation note**: The existing `VfsPath::parse()` accepts any non-empty authority string. The `display()` method emits it verbatim (no additional encoding). We add a helper `VfsPath::decode_authority() -> Option<String>` that applies URL percent-decoding to `self.authority`. Archive backends use this instead of calling `.authority.as_deref()` directly.

---

## R-007 — SSH known_hosts and host-key UI

**Decision**: Use `ssh-key = "0.6"` (the same crate `russh` already pulls in) to parse `~/.ssh/known_hosts`. On an unknown key:
1. `SftpFs::connect` suspends the connection via a `tokio::sync::oneshot::channel`.
2. The sender end is passed up to the `App` as an `AppEvent::HostKeyVerification { fingerprint, sender }`.
3. The UI layer presents `ActiveDialog::HostKeyVerify { widget, sender }`.
4. On "Accept": the key is appended to `~/.ssh/known_hosts`; `sender.send(true)`.
5. On "Reject": `sender.send(false)` → `SftpFs::connect` returns `VfsError::AuthFailed`.

This keeps the blocking nature of the modal without adding async complexity to the `russh::client::Handler` implementation (which runs on the SSH library's internal task, not the UI task).

---

## R-008 — Tracing / debug log infrastructure

**Decision**: The existing `tracing-subscriber` setup in the binary already writes to a log file (or stderr). We confirm that `~/.local/share/cargonaut/debug.log` is the configured sink. Connection events are emitted with `tracing::info!` / `tracing::warn!` inside `SftpFs` and `FtpFs` method bodies.

If the log file sink isn't already wired, we add it in `cargonaut-bin/src/main.rs` behind a `--log-file` flag (or always active). This is a small `tracing_subscriber::fmt::layer().with_writer(log_file)` addition.

---

## R-009 — Cargo feature structure

**Decision**:

```toml
# cargonaut-vfs/Cargo.toml
[features]
default = ["archives", "remote"]
archives = ["dep:zip", "dep:tar", "dep:flate2", "dep:bzip2", "dep:xz2"]
remote   = ["dep:russh", "dep:russh-sftp", "dep:ssh-key", "dep:suppaftp"]
```

All archive and remote code is in `cargonaut-vfs` behind `#[cfg(feature = "archives")]` / `#[cfg(feature = "remote")]` gates. The `cargonaut-bin` workspace member inherits both features by default.

Binary size constraint (SC-008): With both features enabled, total binary size must not exceed baseline + 1.5 MiB. `suppaftp` (~200 KiB), `russh` + `russh-sftp` (~600 KiB), `zip` (~150 KiB), `tar`+compression (~200 KiB) ≈ ~1.15 MiB of code; within budget. The `check-binary-size.sh` script is updated with a new threshold.

---

## R-010 — Existing `SftpConfig` / `RemoteConfig` in cargonaut-config

**Finding**: `cargonaut-config` already defines `RemoteConfig { sftp: SftpConfig, s3: S3Config }` and `SftpConfig { connect_timeout_secs, keepalive_secs, pipelined_reads }`. These fields align exactly with our implementation needs. No new config types are required. We add `FtpConfig` to `RemoteConfig` as a parallel peer to `SftpConfig`.

**FtpConfig fields**: `connect_timeout_secs: u32` (default 30), `passive_mode: bool` (default true).

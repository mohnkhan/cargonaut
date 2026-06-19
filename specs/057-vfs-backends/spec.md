# Feature Specification: VFS Backends — Archives + Remote (SFTP/FTP)

**Feature Branch**: `057-vfs-backends`

**Created**: 2026-06-19

**Status**: Draft

**Input**: User description: "Feature 057 — VFS backends: archives-as-directories + remote (SFTP/FTP/fish). Closes issue #48."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Browse a ZIP archive as a directory (Priority: P1)

A user has a `.zip` file in their active pane. They press Enter on it and the pane navigates into the archive, showing its entries as if they were regular files and directories. They can descend into sub-directories inside the archive, view file sizes and timestamps, and copy individual files out to the other pane using F5. Pressing Backspace or navigating to `..` exits the archive and returns to the local filesystem.

**Why this priority**: This is the most commonly encountered archive format and is the foundational "browse archive as directory" use case. Delivering it alone gives users meaningful immediate value (read + copy-out), and the design it establishes (archive path encoding in VfsPath, entry caching, UI Descend hook) is reused by all other backends.

**Independent Test**: Open any `.zip` file, navigate into it, verify entries appear with correct names/sizes, copy one file out to a local directory — without any SFTP or FTP connectivity required.

**Acceptance Scenarios**:

1. **Given** a pane showing a local directory containing `archive.zip`, **When** the user presses Enter on `archive.zip`, **Then** the pane header updates to `zip:///…/archive.zip/` and the entries of the archive are listed sorted by name.
2. **Given** the pane is inside a ZIP archive at a sub-folder, **When** the user navigates to `..`, **Then** the pane ascends one level within the archive (or back to the local filesystem if at the archive root).
3. **Given** the pane is inside a ZIP archive, **When** the user presses F5 to copy a file to the opposite pane (local `file://`), **Then** the file is transferred correctly with matching content.
4. **Given** a corrupt or password-protected ZIP, **When** Enter is pressed on it, **Then** a dismissible error banner is shown in the pane; no crash or empty panic occurs.

---

### User Story 2 — Browse a TAR archive (uncompressed, .gz, .bz2, .xz) (Priority: P2)

A user navigates into a `.tar.gz` file and sees its entries. They can copy files out. Because TAR is sequential, range reads decompress from the start; the user is unaware of this constraint at the UI level.

**Why this priority**: TAR/GZ is the dominant format for source tarballs and Linux packages. Functionally identical to the ZIP story from the user's perspective; the complexity difference is internal (sequential vs. random access).

**Independent Test**: Open a `.tar.gz`, list entries, copy one file out — independently of ZIP, SFTP, or FTP.

**Acceptance Scenarios**:

1. **Given** a pane showing `source.tar.gz`, **When** Enter is pressed, **Then** pane header shows `tar:///…/source.tar.gz/` and entries are listed.
2. **Given** an entry inside a `.tar.bz2` archive, **When** copied to a local destination, **Then** the file content matches the uncompressed entry.
3. **Given** a `.xz`-compressed archive, **When** browsed, **Then** entries list correctly.
4. **Given** a truncated or corrupt archive, **When** Enter is pressed, **Then** an error banner appears; no panic.

---

### User Story 3 — Connect to an SFTP server and browse/transfer files (Priority: P3)

A user opens the User Menu (F2), selects "Connect → SFTP", and is presented with a URL input pre-filled with `sftp://user@host/`. They fill in their server details, confirm, and the active pane navigates to the SFTP root. They can browse directories, copy files to/from the local pane, delete files, rename, and create directories — the same operations available on local `file://` panes.

**Why this priority**: SFTP is the primary real-world use case for remote file management. The SFTP backend is the most feature-complete of the remote backends (full read-write) and serves as the reference implementation for the registry / connection lifecycle patterns used by FTP.

**Independent Test**: With a real or test-double SFTP server, connect, list the root, copy one file to local — independently of archive backends or FTP.

**Acceptance Scenarios**:

1. **Given** the user selects "Connect → SFTP" and enters `sftp://testuser@localhost/tmp`, **When** connection succeeds, **Then** the pane shows the `/tmp` directory on the remote server.
2. **Given** the user is browsing an SFTP pane, **When** they copy a file to the opposite local pane, **Then** the file arrives with matching content.
3. **Given** the connection drops mid-operation, **When** the backend reconnects (up to 3 retries), **Then** the operation either completes or surfaces a dismissible error banner; no hang.
4. **Given** SSH key authentication fails and no password is supplied, **Then** `VfsError::AuthFailed` is surfaced as a banner; no crash.
5. **Given** the pane is on an SFTP backend, **When** the user creates a directory, renames a file, or deletes a file, **Then** the remote filesystem reflects the change on next listing.

---

### User Story 4 — Connect to an FTP server and browse/transfer files (Priority: P4)

A user connects to an FTP server via F2 → "Connect → FTP", browses its directory tree, and copies files between the FTP pane and a local pane. Write operations (upload, delete, mkdir, rename) are supported; byte-range resume is not.

**Why this priority**: FTP remains prevalent for legacy systems, vendor portals, and managed-hosting providers. Lower priority than SFTP because its write semantics are narrower (no resume, no symlinks) and its usage is declining.

**Independent Test**: With a test FTP server, connect, list root, copy one file to local — independently of SFTP and archive backends.

**Acceptance Scenarios**:

1. **Given** the user enters `ftp://anon@ftp.example.com/pub`, **When** connection succeeds, **Then** the pane lists the `/pub` directory.
2. **Given** an FTP pane and a local pane, **When** the user copies a file from FTP to local, **Then** the file content is identical.
3. **Given** an upload to FTP fails mid-stream, **When** the banner is dismissed, **Then** the app remains stable; no partially-written file is silently ignored (the transfer engine marks it failed).

---

### User Story 5 — Backend registry: pane carries its own VfsPath + backend (Priority: P1, cross-cutting)

Internally, every pane knows which backend it is on. Transfers between any two panes (local↔local, local↔archive, local↔SFTP, SFTP↔SFTP, etc.) route through the same transfer engine — `read_stream` from source + `write_stream` to dest — without any special-casing in the UI layer.

**Why this priority**: This is the architectural prerequisite for all other stories. Without the registry and the `(VfsPath, Arc<dyn VfsBackend>)` pane model, the archive and remote backends cannot be wired to the UI.

**Independent Test**: Add `VfsRegistry`, populate it with `LocalFs`, verify the existing local-only tests still pass — before any archive or remote code is written.

**Acceptance Scenarios**:

1. **Given** the app starts, **Then** a `VfsRegistry` is initialised with at least `LocalFs` registered under `"file"`.
2. **Given** a pane navigates into a ZIP archive, **Then** its internal backend reference changes to the `ZipFs` instance; the other pane's backend is unaffected.
3. **Given** two panes on different backends, **When** F5 copy is initiated, **Then** the transfer engine receives `(src_path, src_backend, dst_path, dst_backend)` and completes correctly.

---

### Edge Cases

- What happens when an archive entry has a path with `..` traversal components? → Entry is silently skipped (logged at debug); no directory traversal outside the archive.
- What happens when an archive exceeds available RAM for the in-memory listing cache? → The listing is built lazily in chunks up to a configurable cap (default 50 000 entries); excess entries are omitted and a warning banner is shown.
- What happens when an SFTP server sends an unknown file type (device node, FIFO)? → Reported as `VfsKind::Other`; copy/delete operations on it return `VfsError::Unsupported`.
- What happens when the user presses Backspace at the root of an archive? → The pane returns to the parent local directory that contained the archive file.
- What happens when both panes are on the same SFTP host? → The transfer uses `rename` (atomic, within same authority) if source and destination are on the same path prefix; otherwise falls back to read+write.
- What happens when the app is closed while an SFTP connection is active? → The connection is dropped; no teardown dialog is shown (sessions are stateless from the user's perspective).

## Requirements *(mandatory)*

### Functional Requirements

**Registry**

- **FR-001**: The system MUST provide a `VfsRegistry` type that maps URI scheme strings to backend instances and resolves the correct backend for any `VfsPath`.
- **FR-002**: Every `PaneState` MUST carry an explicit `(VfsPath, Arc<dyn VfsBackend>)` pair; no implicit "local only" assumption may remain in the pane model.
- **FR-003**: The app MUST initialise the registry at startup with `LocalFs` registered under `"file"`, preserving all existing local-filesystem behaviour without regression.

**ZIP backend**

- **FR-004**: The system MUST implement a `ZipFs` backend (`zip://` scheme) that supports `list`, `stat`, and `read_stream` (full file only and byte-range).
- **FR-005**: `ZipFs` MUST cache the archive's entry index in memory on first `list` call; subsequent `list` calls on the same instance MUST NOT re-scan the archive file.
- **FR-006**: `ZipFs` MUST return `VfsError::PermissionDenied` for encrypted entries and `VfsError::Io` for corrupt archive data.
- **FR-007**: `ZipFs` write operations (`write_stream`, `mkdir`, `unlink`, `rmdir`, `rename`) MUST return `VfsError::Unsupported`.

**TAR backend**

- **FR-008**: The system MUST implement a `TarFs` backend (`tar://` scheme) supporting uncompressed `.tar`, `.tar.gz`, `.tar.bz2`, and `.tar.xz` archives (compression auto-detected by file extension).
- **FR-009**: `TarFs` MUST support `list` and `stat`; `read_stream` MUST support whole-file reads. Byte-range reads MUST return `VfsError::Unsupported` (TAR is sequential).
- **FR-010**: `TarFs` entry index MUST be cached in memory on first `list` call.
- **FR-011**: TAR entries with path components that escape the archive root (e.g. `../etc/passwd`) MUST be silently skipped.
- **FR-012**: `TarFs` write operations MUST return `VfsError::Unsupported`.

**SFTP backend**

- **FR-013**: The system MUST implement an `SftpFs` backend (`sftp://` scheme) with a pure-async SSH implementation (no `libssh2` dependency) supporting `list`, `stat`, `read_stream` (with byte-range), `write_stream` (Truncate and AppendAtOffset), `unlink`, `rmdir`, `rename`, `mkdir`, `symlink`.
- **FR-014**: `SftpFs` MUST attempt SSH public-key authentication first (ssh-agent socket, then `~/.ssh/id_ed25519`, then `~/.ssh/id_rsa`); on failure it MUST accept a `SftpCredentials::Password` value supplied at construction time.
- **FR-015**: On transport failure, `SftpFs` MUST reconnect automatically up to 3 times with exponential backoff before returning `VfsError::Io`.
- **FR-016**: `SftpFs` MUST declare capabilities: `SEEKABLE | RANDOM_WRITE | METADATA_RICH | ATOMIC_RENAME | SYMLINKS`.
- **FR-017**: `SftpFs` authority format is `user@host` or `user@host:port`; the default port is 22.

**FTP backend**

- **FR-018**: The system MUST implement an `FtpFs` backend (`ftp://` scheme) supporting `list`, `stat`, `read_stream` (full file only), `write_stream` (Truncate only), `unlink`, `rmdir`, `rename`, `mkdir`.
- **FR-019**: `FtpFs` MUST declare capabilities: `ATOMIC_RENAME` only. `SEEKABLE`, `RANDOM_WRITE`, `SYMLINKS`, and `METADATA_RICH` MUST NOT be declared.
- **FR-020**: `FtpFs` `stat` is best-effort, derived from `LIST` output parsing; missing fields (owner, group, symlink target) return `None`.

**UI — archive navigation**

- **FR-021**: When the cursor is on a file with extension `.zip`, `.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`, `.txz`, pressing Enter MUST mount the appropriate archive backend and navigate the pane into the archive root.
- **FR-022**: The pane header MUST display the full `VfsPath.display()` string whenever the active backend is not `file://`.
- **FR-023**: Navigating to `..` at the archive root MUST return the pane to the parent local directory and re-activate `LocalFs` for that pane.

**UI — remote connection**

- **FR-024**: The User Menu (F2) MUST include "Connect → SFTP" and "Connect → FTP" menu items.
- **FR-025**: Selecting a connect item MUST open a `PathInputDialog` pre-filled with `sftp://user@host/` or `ftp://user@host/` respectively, allowing the user to edit the URL before confirming.
- **FR-026**: After the user confirms the URL, the app MUST display a "Connecting…" indicator in the pane while the connection is established, then navigate the pane to the remote root on success.
- **FR-027**: On connection failure, the app MUST show a dismissible error banner in the pane; the pane MUST remain on its prior backend (no blank/broken state).

**Error handling**

- **FR-028**: All backend errors (corrupt archive, auth failure, connection drop, permission denied) MUST be surfaced as dismissible error banners in the affected pane; no operation may result in an application panic or a silent empty listing for an error condition.

**Transfer engine integration**

- **FR-029**: The transfer engine MUST accept `(src: VfsPath, src_backend: Arc<dyn VfsBackend>, dst: VfsPath, dst_backend: Arc<dyn VfsBackend>)` and execute the copy via `read_stream` + `write_stream` regardless of whether the backends are the same type or different.

### Key Entities

- **VfsRegistry**: Maps URI scheme strings to `Arc<dyn VfsBackend>` instances; resolved by the app layer to dispatch pane operations.
- **ZipFs**: Read-only `VfsBackend` for the `zip://` scheme; wraps an in-memory entry index over a `zip` crate archive.
- **TarFs**: Read-only `VfsBackend` for the `tar://` scheme; supports uncompressed, gzip, bzip2, and xz-compressed tarballs; in-memory entry index.
- **SftpFs**: Read-write `VfsBackend` for the `sftp://` scheme; holds a persistent multiplexed SSH connection; reconnects on transport failure.
- **SftpCredentials**: Enum of `PublicKey { agent: bool, key_path: Option<PathBuf> }` and `Password(String)` — passed to `SftpFs::connect`.
- **FtpFs**: Read-write `VfsBackend` for the `ftp://` scheme; wraps an async FTP client connection.
- **PaneState**: Updated to carry `(VfsPath, Arc<dyn VfsBackend>)` instead of a bare `PathBuf`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can open a ZIP or TAR archive (any supported compression) and list its entries within 500 ms for archives up to 10 000 entries.
- **SC-002**: Copying a file out of a ZIP or TAR archive to a local destination completes with byte-for-byte fidelity, verified by checksum comparison.
- **SC-003**: An SFTP connection to a server on the local network is established and the root directory is listed within 3 seconds of confirming the URL.
- **SC-004**: File transfer throughput between an SFTP pane and a local pane reaches ≥70% of the theoretical network bandwidth (measured over a loopback or LAN connection, files ≥10 MiB).
- **SC-005**: All existing local-filesystem tests pass without modification after the registry refactor (zero regression in the `file://` path).
- **SC-006**: A corrupt or encrypted archive surfaces an error banner within 1 second of pressing Enter; the app remains interactive.
- **SC-007**: An SFTP connection failure after 3 retries surfaces an error banner; the app remains stable (no panic, no hung task).
- **SC-008**: The release binary size does not increase by more than 1.5 MiB compared to the pre-feature baseline (managed via cargo features to keep SFTP/FTP behind optional flags).

## Assumptions

- Archive files are accessible on the local filesystem at the time of mounting; network-mounted archives are not a supported scenario for this feature.
- SFTP servers support SFTPv3 or later; servers advertising only SFTPv1/v2 may have degraded capability (no symlink support).
- FTP servers are reachable on a non-TLS plain connection; FTPS (FTP-over-TLS) is out of scope and deferred.
- SSH host-key verification uses the system `~/.ssh/known_hosts`; unknown host keys prompt the user with an accept/reject dialog (a UI-layer concern; the backend returns `VfsError::AuthFailed` if the key is rejected).
- The `zip`, `tar`, `flate2`, `bzip2`, and `xz2` crates are added as dependencies; `russh`/`russh-sftp` for SFTP; a suitable async FTP crate (`suppaftp`) for FTP. All are pure-Rust or link only stable system libraries.
- The SFTP and FTP backends are gated behind Cargo features (`sftp`, `ftp`) so users who do not need remote access can build a smaller binary.
- fish/TRAMP-style `sh://` backend is out of scope for this feature.
- S3/GCS/Azure object-storage backends are out of scope for this feature.
- SFTP key management UI (key generation wizard) is out of scope; credential input is limited to URL entry + the existing `PathInputDialog`.
- The app's transfer engine already supports streaming between arbitrary `VfsBackend` instances; no transfer-crate API changes are anticipated beyond passing the backend references through.

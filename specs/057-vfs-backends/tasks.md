# Tasks: VFS Backends — Archives + Remote (SFTP/FTP)

**Feature**: 057-vfs-backends | **Branch**: `057-vfs-backends`

**Input**: Design documents from `specs/057-vfs-backends/`

**TDD Convention** (Constitution §II): Every functional task pair follows `(red)` → `(green)`. The red commit adds failing tests; the green commit adds the implementation that makes them pass. Commit messages MUST include `(red)` / `(green)` markers as appropriate.

**Format**: `[ID] [P?] [Story?] Description — file path`

- **[P]**: Parallelizable (different files, no deps on incomplete tasks in the same phase)
- **[USN]**: User story label (US5=architecture, US1=ZIP, US2=TAR, US3=SFTP, US4=FTP)
- **Tests are mandatory** (Constitution §II TDD)

---

## Phase 1: Setup — Cargo Feature Gates & Dependencies

**Purpose**: Wire new Cargo features and dependency declarations. No code yet — just build configuration.

- [X] T001 Add `archives` and `remote` Cargo features + optional deps (`zip`, `tar`, `flate2`, `bzip2`, `xz2`, `russh`, `russh-sftp`, `suppaftp`) to `crates/cargonaut-vfs/Cargo.toml`
- [X] T002 [P] Add `FtpConfig { connect_timeout_secs: u32, passive_mode: bool }` field to `RemoteConfig` in `crates/cargonaut-config/src/lib.rs`; update serde defaults and tests
- [X] T003 [P] Create stub modules `crates/cargonaut-vfs/src/registry.rs`, `src/archive/mod.rs`, `src/archive/zip_fs.rs`, `src/archive/tar_fs.rs`, `src/remote/mod.rs`, `src/remote/sftp_fs.rs`, `src/remote/ftp_fs.rs` (empty `pub` declarations behind `#[cfg(feature)]` guards) so `cargo build` compiles clean

**Checkpoint**: `cargo build --workspace` succeeds; `cargo clippy --workspace --all-targets -- -D warnings` clean.

---

## Phase 2: Foundational — VfsRegistry + PaneState Refactor (US5)

**Purpose**: The `VfsRegistry` + `PaneState.backend` architectural change is a prerequisite for every user story. All existing tests must pass throughout. Implements FR-001..FR-003.

**⚠️ CRITICAL**: No archive or remote backend work can begin until T010 (PaneState.backend + App.registry) is merged and all existing tests pass.

### T004-T005: VfsPath::decode_authority (FR-001 helper)

- [X] T004 [US5] (red) Write failing unit tests for `VfsPath::decode_authority()` covering: percent-encoded slash roundtrip, multiple segments, empty authority → None, unencoded authority passthrough — in `crates/cargonaut-vfs/src/types.rs` (test module) and extend proptest in same file
- [X] T005 [US5] (green) Implement `pub fn decode_authority(&self) -> Option<String>` on `VfsPath` in `crates/cargonaut-vfs/src/types.rs`; apply full percent-decoding (`%2F`→`/`, `%XX` generally)

### T006-T007: VfsRegistry (FR-001)

- [X] T006 [US5] (red) Write failing unit tests for `VfsRegistry`: resolve `file://` → local backend; resolve unknown scheme → None; resolve registered `sftp://user@host:22/` → registered backend; overwrite on re-register — in `crates/cargonaut-vfs/tests/registry.rs`
- [X] T007 [US5] (green) Implement `VfsRegistry { local, remote_map }` with `new`, `local()`, `register_remote`, `resolve` in `crates/cargonaut-vfs/src/registry.rs`; re-export from `crates/cargonaut-vfs/src/lib.rs`

### T008-T010: PaneState.backend + App.registry (FR-002, FR-003)

- [X] T008 [US5] (red) Write failing tests asserting: `PaneState` has a `backend: Arc<dyn VfsBackend>` field; `App::new` populates both panes' backends with the local backend; `App` exposes `registry()` accessor returning `Arc<VfsRegistry>` — in `crates/cargonaut-core/src/lib.rs` (inline test module)
- [X] T009 [US5] (green) Add `pub backend: Arc<dyn VfsBackend>` to `PaneState` in `crates/cargonaut-core/src/lib.rs`; replace `App.local_fs` with `App.registry: Arc<VfsRegistry>`; populate both panes' backends from `registry.local()` in `App::new`
- [X] T010 [US5] (green) **Depends on T009** (PaneState.backend must exist first). Migrate ALL `self.local_fs.XXX()` call sites in `crates/cargonaut-core/src/lib.rs` to `self.pane(id).backend.XXX()` or `self.registry.local().XXX()`; update `navigate_to` signature to `navigate_to(id, new_cwd: VfsPath, backend: Arc<dyn VfsBackend>)`; run all existing tests (605+) to verify zero regression

### T011: Object-safety gate

- [X] T011 [US5] [P] Extend `crates/cargonaut-vfs/tests/dyn_dispatch.rs` to assert `Arc<dyn VfsBackend>` is constructable from each new backend type (`ZipFs`, `TarFs`, `SftpFs`, `FtpFs`) behind respective feature flags; these tests will fail until Phase 3+ implementations land — commit as (red) stubs now

### T040: FR-029 transfer engine API pre-verification (FR-029)

- [X] T040 [US5] (red + green) Verify `cargonaut-transfer` `submit_transfer` (or equivalent) already accepts `Arc<dyn VfsBackend>` for both source and destination: write a compile-only test in `crates/cargonaut-transfer/tests/api_shape.rs` calling `submit_transfer(src_path, arc_local.clone(), dst_path, arc_local.clone())` — if the signature is already correct the test is green on first commit; if not, update the transfer crate signature to accept `Arc<dyn VfsBackend>` pairs before any backend implementations depend on it. This verifies assumption "transfer engine already supports `Arc<dyn VfsBackend>`" before Phase 3 begins. (**Addresses analysis finding M6**)

**Checkpoint**: `cargo test --workspace` shows 0 failures on existing tests. `PaneState.backend` is live. `App.registry` is live. FR-029 assumption verified.

---

## Phase 3: User Story 1 — ZIP Archive Browsing (Priority: P1) 🎯 MVP

**Goal**: User presses Enter on a `.zip` file; pane navigates into the archive; user can browse and copy files out.

**Independent Test**: Open any `.zip` file via the binary, list entries, copy one file to a local pane — all without any SFTP/FTP connectivity.

### T012-T013: ZipFs core (FR-004..FR-007)

- [X] T012 [US1] (red) Write failing unit tests for `ZipFs`: list archive root → correct entry names/sizes; list subdirectory → filtered entries; stat file → correct VfsMetadata; read_stream FULL → bytes match fixture; read_stream range → Unsupported; encrypted entry → PermissionDenied; corrupt zip → Io; write ops → Unsupported; `caps()` == empty; `scheme()` == "zip" — in `crates/cargonaut-vfs/tests/zip_fs.rs` (use `tempfile` + embed test zip bytes as `include_bytes!`)
- [X] T013 [US1] (green) Implement `ZipFs` in `crates/cargonaut-vfs/src/archive/zip_fs.rs`: `ZipFs::open(archive_host_path: PathBuf) -> Result<Self, VfsError>`; `ZipIndex` (Vec<ZipEntryMeta> + name→idx HashMap, built via `enclosed_name()` scan, drops unsafe paths silently); `impl VfsBackend`: list, stat, read_stream (FULL only via `spawn_blocking`), all write ops → Unsupported; T011 dyn_dispatch test now turns green

### T014-T015: Archive VfsPath encoding + navigate_to wiring (FR-021, FR-023)

- [X] T014 [US1] (red) Write two failing unit tests in `crates/cargonaut-ui-tui/src/lib.rs` (inline test module) or `crates/cargonaut-core/src/lib.rs`: (1) **happy path** — given a local pane with cursor on a `.zip` file, dispatching `Command::DescendOrOpen` produces an `Event` showing the pane navigated to a `zip://` VfsPath with correct authority (percent-encoded archive path) and empty segments; (2) **error path** — given a local pane with cursor on a corrupt `.zip` file (ZipFs::open returns Err), dispatching `Command::DescendOrOpen` produces a pane-level error banner event and does NOT crash or navigate away (**addresses FR-028 archive open failure path; analysis finding M3**)
- [X] T015 [US1] (green) In `crates/cargonaut-ui-tui/src/lib.rs` `DescendOrOpen` handler: detect `.zip` extension on focused regular-file entry; build `zip://` `VfsPath` (authority = percent-encode(host_path), segments=[]); `ZipFs::open(host_path)`; call `app.navigate_to(id, zip_vfs_path, Arc::new(zip_fs))`

### T016a-T016b: Pane header — non-local path display (FR-022)

- [X] T016a [US1] (red) Write failing inline test in `crates/cargonaut-ui-tui/src/chrome.rs` (or test module): construct a `PaneState` with `backend.scheme() == "zip"` and a `zip://` cwd; assert the pane header render output contains the full `pane.cwd.display()` string rather than a basename — test fails since the condition doesn't exist yet (**split from combined red+green per Constitution §II; analysis finding M4**)
- [X] T016b [US1] (green) In `crates/cargonaut-ui-tui/src/chrome.rs`: add condition `if pane.backend.scheme() != "file"` before rendering the header path; render `pane.cwd.display()` instead of the basename; T016a test now passes

### T017-T018: Backspace at archive root → local parent (FR-023)

- [X] T017 [US1] (red) Write failing test: navigating `..` from a `zip://` pane with empty segments navigates pane to the local parent directory of the archive file (file:// backend restored) — in `crates/cargonaut-core/src/lib.rs` or TUI test
- [X] T018 [US1] (green) In `navigate_up` (or `App::dispatch` on `Command::Ascend`): if `pane.cwd.segments.is_empty()` and `pane.backend.scheme() != "file"`, decode authority → host path → local parent VfsPath; call `navigate_to(id, local_parent, registry.local())`

**Checkpoint**: Binary navigates into `.zip` files, displays entries, allows F5 copy-out to local pane, backspaces back to local filesystem. `cargo test --workspace` clean.

---

## Phase 4: User Story 2 — TAR Archive Browsing (Priority: P2)

**Goal**: User presses Enter on `.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`, `.txz` files and browses entries; copy-out works.

**Independent Test**: Open a `.tar.gz`, list entries, copy one file to local — without ZIP, SFTP, or FTP.

### T019-T020: TarFs core (FR-008..FR-012)

- [X] T019 [US2] (red) Write failing unit tests for `TarFs`: list/stat for uncompressed `.tar`; list/read for `.tar.gz`; list/read for `.tar.bz2`; list/read for `.tar.xz`; path-traversal entry silently skipped (use a fixture tar with `../etc/evil`); corrupt archive → Io; range read → Unsupported; write ops → Unsupported; `caps()` == empty; `scheme()` == "tar" — in `crates/cargonaut-vfs/tests/tar_fs.rs`
- [X] T020 [US2] (green) Implement `TarFs` in `crates/cargonaut-vfs/src/archive/tar_fs.rs`: `TarCompression` enum; `TarFs::open(path, compression)`; entry index built in `spawn_blocking` scanning all entries (drain each via `io::copy(&mut e, &mut sink())`), skipping `../` paths with `warn!`; `read_stream(FULL)` re-opens + re-scans to entry by seq_index; all write ops → Unsupported; T011 `TarFs` dyn_dispatch stub turns green

### T021a-T021b: UI Enter → TAR open (FR-021)

- [X] T021a [US2] (red) Write failing unit tests in `crates/cargonaut-ui-tui/src/lib.rs` test module asserting `Command::DescendOrOpen` on a `.tar.gz` file produces a pane navigation event with a `tar://` VfsPath; include separate test stubs for `.tar`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`, `.txz` — all fail since handler doesn't yet recognise TAR extensions (**split per Constitution §II; analysis finding M4**)
- [X] T021b [US2] (green) Extend `DescendOrOpen` handler in `crates/cargonaut-ui-tui/src/lib.rs` to recognise all TAR extensions (`.tar`, `.tar.gz`, `.tgz`, `.tar.bz2`, `.tbz2`, `.tar.xz`, `.txz`); detect compression from extension; `TarFs::open(path, compression)`; `navigate_to` with `tar://` VfsPath; T021a tests now pass

**Checkpoint**: Binary navigates into all TAR variants; entries listed; copy-out works; path traversal safety verified. `cargo test --workspace` clean.

---

## Phase 5: User Story 3 — SFTP Connect + Browse + Transfer (Priority: P3)

**Goal**: User connects to an SFTP server via F2 menu; browses remote filesystem; copies files between remote and local panes.

**Independent Test**: With a test SFTP server (or mock), connect, list root, copy one file to local — independently of ZIP/TAR/FTP.

### T022: SftpConfig default values test (FR-017)

- [ ] T022 [US3] (red + green) Write a unit test in `crates/cargonaut-config/src/lib.rs` asserting `SftpConfig::default()` has `connect_timeout_secs = 30`, `keepalive_secs = 60`, `pipelined_reads = 4`; if fields are missing, add them with correct defaults to `SftpConfig` — red commit if any field/default is wrong, green commit once all defaults match plan requirements (**transformed from passive verification step to proper TDD task; analysis finding M5**)

### T023-T025: SftpFs core (FR-013..FR-016, FR-030)

- [X] T023 [US3] (red) Write failing unit tests for `SftpFs` using a mock SFTP session trait injected at construction: list → DirListing; stat → VfsMetadata with METADATA_RICH fields; read_stream with ByteRange → bytes; write_stream Truncate → bytes written; write_stream AppendAtOffset → bytes at offset; unlink → ok; rmdir → ok; rename → ok; mkdir → ok; symlink → ok; auth failure → VfsError::AuthFailed; transport error → retry up to 3 times then Io; `caps()` == SEEKABLE|RANDOM_WRITE|METADATA_RICH|ATOMIC_RENAME|SYMLINKS; `scheme()` == "sftp"; tracing events emitted on connect and error; **SECURITY GATE: add a test asserting that when auth failure is logged at WARN level, the captured tracing output does NOT contain the raw password value from `SftpCredentials::Password`** (**analysis finding H2: credential redaction required per Constitution §Dev Workflow**) — in `crates/cargonaut-vfs/tests/sftp_fs_mock.rs`
- [X] T024 [US3] (green) Implement `SftpCredentials` enum and `SftpFs` in `crates/cargonaut-vfs/src/remote/sftp_fs.rs`: `SftpFs::connect(authority, credentials, config, host_key_tx)` — TCP connect, russh handshake with `HostKeyHandler`, authenticate (agent → key file → password), open SFTP channel, `SftpSession::new`; implement all `VfsBackend` methods via `SftpSession` async API; `with_retry` helper (3 attempts, 200/400/800ms backoff); **tracing format MUST use `tracing::warn!("sftp auth failed user={user} host={host}")` — NEVER include the credentials value or password in log format strings** (Constitution §Dev Workflow: "secrets MUST be redacted"); `tracing::info!`/`tracing::warn!` on connect/error; T011 `SftpFs` dyn_dispatch stub turns green
- [X] T025 [US3] (green) Implement `HostKeyHandler` in `crates/cargonaut-vfs/src/remote/sftp_fs.rs`: `check_server_key` checks `~/.ssh/known_hosts` via `russh::keys::check_known_hosts`; on unknown key, sends `HostKeyEvent { fingerprint, accept_tx }` through channel, awaits bool; on Accept calls `russh::keys::learn_known_hosts_path`; on Reject returns Err → `VfsError::AuthFailed`

### T026-T027: HostKeyVerify dialog (FR-014a)

- [ ] T026 [US3] (red) Write failing test: `ActiveDialog::HostKeyVerify` renders a dialog showing the fingerprint string and two buttons Accept/Reject; pressing Enter on Accept sends `true` on the oneshot; pressing Esc or Reject sends `false` — in `crates/cargonaut-ui-tui/src/lib.rs` test module
- [ ] T027 [US3] (green) Add `AppEvent::HostKeyVerification { fingerprint: String, accept_tx: oneshot::Sender<bool> }` to `cargonaut-core`; add `ActiveDialog::HostKeyVerify { fingerprint: String, accept_tx: ... }` variant to enum in `crates/cargonaut-ui-tui/src/lib.rs`; implement `HostKeyVerifyDialog` widget in `crates/cargonaut-ui-tui/src/dialog.rs` (two-button Accept/Reject layout); wire event dispatch in `handle_dialog_key`

### T028-T029: F2 "Connect SFTP…" + RemoteConnect dialog (FR-024..FR-027)

- [ ] T028 [US3] (red) Write failing test: dispatching `Command::ShowUserMenu` and selecting the "Connect SFTP…" built-in item opens `ActiveDialog::RemoteConnect { kind: Sftp, widget }` pre-filled with `sftp://user@host/`; submitting a valid URL initiates connection and navigates pane — in `crates/cargonaut-ui-tui/src/lib.rs` test module
- [ ] T029 [US3] (green) Add `RemoteKind { Sftp, Ftp }` enum and `ActiveDialog::RemoteConnect { kind: RemoteKind, widget: PathInputDialog }` in `crates/cargonaut-ui-tui/src/lib.rs`; implement `RemoteConnectDialog` widget in `crates/cargonaut-ui-tui/src/dialog.rs`; add "Connect SFTP…" as a built-in item in the `ShowUserMenu` handler; on submit: parse URL, show "Connecting…" in pane header, call `SftpFs::connect`, `registry.register_remote`, `navigate_to`; on error: dismiss dialog, show error banner (FR-028)

**Checkpoint**: Binary connects to SFTP server, lists remote directory, copies files bidirectionally, shows host-key dialog on first connect. `cargo test --workspace` clean.

---

## Phase 6: User Story 4 — FTP Connect + Browse + Transfer (Priority: P4)

**Goal**: User connects to an FTP server via F2 menu; browses and transfers files.

**Independent Test**: With a test FTP server (or mock), connect, list root, copy one file to local — independently of ZIP/TAR/SFTP.

### T030-T031: FtpFs core (FR-018..FR-020)

- [ ] T030 [US4] (red) Write failing unit tests for `FtpFs` using a mock FTP connection: list with MLSD → DirListing; list fallback to LIST → DirListing; stat via MLST → VfsMetadata; read_stream FULL → bytes; read_stream range → Unsupported; write_stream Truncate → bytes; write_stream AppendAtOffset → Unsupported; unlink → ok; rmdir → ok; rename → ok; mkdir → ok; `caps()` == ATOMIC_RENAME; `scheme()` == "ftp"; connect timeout → Io — in `crates/cargonaut-vfs/tests/ftp_fs_mock.rs`
- [ ] T031 [US4] (green) Implement `FtpFs` in `crates/cargonaut-vfs/src/remote/ftp_fs.rs`: `FtpFs::connect(authority, user, pass, config)` → `AsyncFtpStream::connect` + `login`; `Arc<Mutex<AsyncFtpStream>>` internal; list tries `mlsd()` first, falls back to `list()` + parse; stat via `mlst()`; read via `retr_as_stream` (FULL only); write via `put_with_stream` (Truncate only); `tracing::info!`/`tracing::warn!`; T011 `FtpFs` dyn_dispatch stub turns green

### T032-T033: F2 "Connect FTP…" (FR-024..FR-027)

- [ ] T032 [US4] (red) Write failing test: "Connect FTP…" built-in item opens `ActiveDialog::RemoteConnect { kind: Ftp }` pre-filled with `ftp://user@host/`; submitting valid URL connects and navigates pane — in `crates/cargonaut-ui-tui/src/lib.rs` test module
- [ ] T033 [US4] (green) Add "Connect FTP…" built-in item in `ShowUserMenu` handler in `crates/cargonaut-ui-tui/src/lib.rs`; wire `RemoteKind::Ftp` → `FtpFs::connect`; on success `registry.register_remote` + `navigate_to`; on error: error banner (reuses T029 RemoteConnect infrastructure)

**Checkpoint**: Binary connects to FTP server, lists remote directory, copies files to local pane. `cargo test --workspace` clean.

---

## Phase 7: Polish & Cross-Cutting Concerns

**Purpose**: CI gates, observability wiring, documentation, binary size enforcement.

### T034: Archive listing benchmark — hard CI gate (SC-001)

- [ ] T034 [P] (red + green) Add criterion bench `crates/cargonaut-vfs/benches/archive_listing.rs` that generates a test ZIP with 10 000 entries and measures `ZipFs::list(root)` time; **the bench MUST use `criterion::Criterion::sample_size(10).measurement_time(Duration::from_secs(10))` and save a baseline on first run; add `scripts/bench-check.sh` (or extend `Makefile` `bench:` target) that runs `cargo bench --bench archive_listing -- --load-baseline main --baseline feature057` and fails with non-zero exit code if the mean is >500 ms**; wire this script into `.github/workflows/ci.yml` so it runs on every PR — a soft "reviewed report" is NOT sufficient per Constitution §II which requires a CI gate that **fails the build** on regression (**analysis finding H1**)

### T034b: SC-006 corrupt archive timing gate (SC-006)

- [ ] T034b [P] Add a sub-bench or a `#[test]` in `crates/cargonaut-vfs/tests/zip_fs.rs` asserting that `ZipFs::open(corrupt_bytes)` returns `Err(VfsError::Io)` within 1 second (measure with `std::time::Instant`; assert `elapsed < Duration::from_secs(1)`); this fulfils SC-006's "within 1 second" constraint as a CI gate (**analysis finding L2; Constitution §II requires CI gate for every SC**)

### T035: Debug log sink (FR-030)

- [ ] T035 Add `tracing_subscriber::fmt::layer().with_writer(log_file)` to subscriber init in `crates/cargonaut-bin/src/main.rs`; log file path = `~/.local/share/cargonaut/debug.log`; create parent dirs if absent; add integration-level smoke test asserting a connection `WARN` event appears in the log file

### T036: Binary size CI gate update (SC-008)

- [ ] T036 Update `scripts/check-binary-size.sh` to measure baseline (with `--no-default-features`) and full build (with both features); assert delta ≤ 1 500 000 bytes; wire into CI pipeline (verify `.github/workflows/ci.yml` references the script or runs `make ci-local`)

### T037: keymap.toml documentation (Constitution §III)

- [ ] T037 [P] Add `enter = "descend-or-open-archive"` (or equivalent) to `[pane]` section of `design/contracts/keymap.toml` with a comment explaining archive auto-detect; verify `help_covers_all_keymap_bindings` test still passes

### T038-T039: Required documentation (CLAUDE.md mandatory)

- [ ] T038 [P] Update `README.md`: "At a Glance" metrics table (test count, feature count, binary size); add Feature 057 entry to "Feature History" section
- [ ] T039 [P] Append `Learnings.md` §057 with ≥3 bullet points covering: what was hard (e.g., russh API changes, VfsPath authority encoding, TAR sequential read model), root causes, non-obvious decisions (e.g., xz2 C dep accepted, FtpFs serialised connection, ZipFs not SEEKABLE)

### T041: SC-003/SC-004 Docker SFTP integration test (SC-003, SC-004)

- [ ] T041 [P] (red + green) Add a Docker-based SFTP integration test to satisfy Constitution §II CI gate for SC-003 and SC-004: (a) add a `docker-compose.ci.yml` (or `Makefile` `ci-sftp-up:` / `ci-sftp-down:` targets) launching `atmoz/sftp testuser:testpass:1001` bound to `localhost:2222`; (b) add a `#[tokio::test] #[cfg(feature = "ci-integration")]` test in `crates/cargonaut-vfs/tests/sftp_integration.rs` that connects `SftpFs::connect("testuser@localhost:2222", ...)`, lists the root, and asserts completion within 5 seconds (latency gate for SC-003); (c) transfer a 10 MiB file from SFTP to a temp local file and assert transfer time implies ≥70% of 1 Gbps loopback (throughput gate for SC-004); (d) wire `cargo test --features ci-integration` step into `.github/workflows/ci.yml` after the Docker service is up; this is the ONLY way to satisfy Constitution §II for SC-003/SC-004 without a live server (**analysis finding M1**) (**Note**: this task may be deferred to a follow-up issue if Docker integration adds >30 min to CI; in that case open a GitHub issue + ROADMAP row per CLAUDE.md §Deferrals)

---

## Dependencies & Execution Order

### Phase Dependencies

```
Phase 1 (Setup)       — no deps; start immediately
Phase 2 (Foundation)  — depends on Phase 1; BLOCKS all user story phases
Phase 3 (US1 ZIP)     — depends on Phase 2
Phase 4 (US2 TAR)     — depends on Phase 2; can run in parallel with Phase 3
Phase 5 (US3 SFTP)    — depends on Phase 2; can run after Phase 3 (pane header reused)
Phase 6 (US4 FTP)     — depends on Phase 2 + T029 (RemoteConnect dialog)
Phase 7 (Polish)      — depends on all user story phases
```

### User Story Dependencies

- **US5 (Foundation)**: Must complete first — adds `PaneState.backend`, `App.registry`, `navigate_to` signature
- **US1 (ZIP)**: Depends on US5. Fully independent of US2/US3/US4
- **US2 (TAR)**: Depends on US5. Fully independent of US1/US3/US4; shares `navigate_up` logic from US1
- **US3 (SFTP)**: Depends on US5 + pane header rendering from US1 (T016). Fully independent of US2/US4
- **US4 (FTP)**: Depends on US5 + RemoteConnect dialog from US3 (T029)

### Within-Story Task Order

```
Each story: red tests → green implementation → integration wiring → test passes
```

---

## Parallel Opportunities

### Phase 2 parallelism
```
T004-T005 (VfsPath helper)   ─┐
T006-T007 (VfsRegistry)       ├─ can start simultaneously (different files)
T008-T010 (App refactor)      ─┘ depends on T006-T007 registry type
T011 (dyn_dispatch stubs)     ─── can run with T004-T005
```

### Phase 3 parallelism
```
T012-T013 (ZipFs impl)       ─┐
T014-T015 (UI Enter→zip)      ├─ T014-T015 depend on T012-T013 type; otherwise parallel file sets
T016 (chrome.rs header)       ─┘ fully parallel with T012-T013
T017-T018 (backspace)         ─── depends on T015 (navigate_to wiring)
```

### Phase 5 parallelism
```
T023-T025 (SftpFs core)       ─── fully parallel with T026-T027 (dialog, different files)
T028-T029 (F2 + connect)      ─── depends on T023-T025 and T026-T027 both complete
```

### Phase 7 parallelism
```
T034 [P], T035, T036, T037 [P], T038 [P], T039 [P] — all parallelizable (different files)
```

---

## Parallel Execution Examples

### Phase 3 (US1 — ZIP) parallel launch
```
Agent A: T012 (red) + T013 (green) — cargonaut-vfs/src/archive/zip_fs.rs + tests/zip_fs.rs
Agent B: T016 (red+green) — cargonaut-ui-tui/src/chrome.rs
# Then sequentially:
T014 (red) → T015 (green) → T017 (red) → T018 (green)
```

### Phase 5 (US3 — SFTP) parallel launch
```
Agent A: T023 (red) + T024-T025 (green) — sftp_fs.rs + tests/sftp_fs_mock.rs
Agent B: T026 (red) + T027 (green) — dialog.rs + AppEvent
# Then:
T028 (red) + T029 (green) — lib.rs ShowUserMenu wiring
```

---

## Implementation Strategy

### MVP First (US5 + US1 Only)

1. Complete Phase 1: Setup (T001-T003)
2. Complete Phase 2: Foundation — US5 (T004-T011)
3. Complete Phase 3: US1 ZIP browsing (T012-T018)
4. **STOP and VALIDATE**: Run quickstart.md Scenarios 1-4 manually; confirm copy-out works
5. Tag as MVP checkpoint

### Incremental Delivery

1. Foundation (US5) → ZIP browsing (US1) → **MVP demo**
2. Add TAR browsing (US2) → **all archive formats**
3. Add SFTP (US3) → **remote file access**
4. Add FTP (US4) → **legacy remote support**
5. Polish + CI gates → **ship**

---

## Task Count Summary

| Phase | Tasks | User Story |
|---|---|---|
| Phase 1 — Setup | T001-T003 (3 tasks) | — |
| Phase 2 — Foundation | T004-T011 + T040 (9 tasks) | US5 |
| Phase 3 — ZIP | T012-T018 + T016a/T016b split (8 tasks) | US1 |
| Phase 4 — TAR | T019-T021 → T021a/T021b split (4 tasks) | US2 |
| Phase 5 — SFTP | T022-T029 (8 tasks) | US3 |
| Phase 6 — FTP | T030-T033 (4 tasks) | US4 |
| Phase 7 — Polish | T034+T034b+T035-T039+T041 (9 tasks) | — |
| **Total** | **45 tasks** | |

**Changes from initial 39:** +T040 (FR-029 verification), +T016b (T016 red/green split), +T021b (T021 red/green split), +T034b (SC-006 timing gate), +T041 (SC-003/SC-004 Docker integration) = +6 tasks; T022 transformed (not deleted).

---

## Notes

- `[P]` tasks touch different files and have no cross-task dependencies within the same phase
- TDD red→green commit pairs MUST appear in git history (Constitution §II)
- Every `VfsBackend` impl must pass the `dyn_dispatch.rs` object-safety test (T011 stubs → turn green as each backend is implemented)
- `cargo test --workspace` must be green at every checkpoint before advancing to the next phase
- Binary size (T036) must be checked before marking Phase 7 complete
- README + Learnings (T038-T039) are mandatory per CLAUDE.md — the PR will be rejected without them

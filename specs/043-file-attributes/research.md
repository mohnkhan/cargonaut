# Research: File Attribute Operations

**Feature**: 043-file-attributes | **Date**: 2026-06-17

All Technical Context items resolved (no NEEDS CLARIFICATION after
`/speckit-clarify`). Decisions + existing-code findings.

## R-001: New VFS ops on the trait, default `Unsupported`, implemented in LocalFs

- **Decision**: Add to `VfsBackend` (`crates/cargonaut-vfs/src/traits.rs`):
  `chmod(path, mode: u32)`, `chown(path, uid: Option<u32>, gid: Option<u32>)`,
  `symlink(target: &str, link: &VfsPath)`, `hard_link(src: &VfsPath, link: &VfsPath)`
  — each `async fn … -> Result<(), VfsError>` with a **default body returning
  `VfsError::Unsupported(...)`**, implemented concretely in `LocalFs`.
- **Rationale**: Default-`Unsupported` means future backends (SFTP/S3/archive,
  #48) compile without these and report cleanly (FR-006), while `LocalFs` opts
  in. `LocalFs` already uses `std::os::unix::fs` (it has `symlink` at
  `local.rs:439`) and a `map_io(e, path)` error mapper (`local.rs:215`) — reuse
  both. Implementations: `fs::set_permissions(p, Permissions::from_mode(bits))`,
  `std::os::unix::fs::chown(p, uid, gid)`, `std::os::unix::fs::symlink(target, link)`,
  `std::fs::hard_link(src, link)`.
- **Alternatives considered**: a separate `AttrBackend` trait — rejected; the
  ops belong with the other filesystem mutations on `VfsBackend`. Adding a
  `VfsCaps::FILE_ATTRS` bit — skipped; default-`Unsupported` already satisfies
  FR-006 and keeps scope tight.

## R-002: Mode parsing is a pure `ModeSpec` in cargonaut-vfs (octal + symbolic)

- **Decision**: New `crates/cargonaut-vfs/src/mode.rs` with
  `ModeSpec::parse(&str) -> Result<ModeSpec, ModeError>` and
  `ModeSpec::apply(current_bits: u32) -> u32`. Octal (`644`, `0755`, 3–4 digits,
  digits `0-7`) parses to an absolute value; symbolic (`[ugoa]*[+-=][rwx]*`,
  comma-separated) parses to a list of clauses applied relative to the file's
  current bits.
- **Rationale**: Parsing is pure Unix-mode logic that belongs with `FileMode` in
  the vfs crate, and a pure function is the clean SC-004 gate (invalid input
  rejected before any filesystem touch). Splitting **parse** (once, validates)
  from **apply** (per file, against that file's current bits) is what makes a
  symbolic change correct across a multi-file selection where files start from
  different modes.
- **Alternatives considered**: parsing in core — rejected (couples
  orchestration to mode grammar); a third-party chmod-string crate — rejected
  (trivial to implement, avoids a dep).

## R-003: chown name resolution via `nix` (already in the tree), no `unsafe`

- **Decision**: chown accepts `user`, `:group`, or `user:group`, where each part
  is a name **or** a numeric id. Names resolve to ids via
  `nix::unistd::User::from_name` / `Group::from_name`. `nix` is promoted from a
  transitive dependency (already in `Cargo.lock`) to a direct dependency of
  `cargonaut-vfs`. The actual ownership change uses `std::os::unix::fs::chown`.
- **Rationale**: Reference OFMs accept names; `nix` gives **safe** name lookup
  (no `getpwnam` FFI / `unsafe`, satisfying Constitution I) and is already
  compiled into the binary, so NFR-001 is unaffected. An unknown name →
  `AppError::BadAttr` with no change (FR-009).
- **Alternatives considered**: numeric-only (rejected — user-hostile, and the
  safe path is cheap); raw `libc::getpwnam_r` (rejected — adds documented
  `unsafe` for no benefit over `nix`).

## R-004: Operations reuse the existing selection + refresh + dialog seam

- **Decision**: Core methods take the target list from the existing
  `selection_or_focused(active)` (`core/lib.rs:1246`) — tagged files else focused
  entry, already excluding the `..` row via `focused_entry_index()`. Each method
  mirrors `App::mkdir` (`core/lib.rs:975`): build `cwd.join(name)`, call the VFS
  op per target, collect per-file failures, `refresh_active_pane()`, return a
  status `Event`. The UI opens a `TextInputDialog` (prefilled) and on submit
  calls the App method — the Feature 038/042 `InputKind` pattern.
- **Rationale**: Zero new selection/refresh machinery; `..`-exclusion and
  multi-file iteration come for free. Partial failures are reported by
  accumulating per-file errors into the status (FR-010) without rollback.
- **Alternatives considered**: a generic "attribute command" enum carrying a
  payload — more indirection than the four explicit methods need.

## R-005: chown is the one op that chains a confirmation (FR-007)

- **Decision**: chmod, symlink, hard_link apply directly on dialog submit (the
  dedicated dialog's Set/Cancel is the deliberate action; link creation is
  non-destructive and refused if the name exists). **chown** additionally chains
  a `ConfirmDialog` ("Change owner of N item(s) to `<owner>`?") before applying.
- **Rationale**: FR-007 requires explicit confirmation for ownership changes;
  reusing `ConfirmDialog` after the owner-input submit is the faithful, widget-
  reusing way. chmod of a single file needs no second confirm (spec/Assumptions).
- **Alternatives considered**: confirm on every op (too noisy); no confirm on
  chown (violates FR-007).

## R-006: Ownership has no visible listing column — verified by re-stat + status

- **Decision**: The listing shows the permission column (`chrome::perms_string`
  over `FileMode.bits`) but **not** an owner column. chmod changes are visible in
  the refreshed perms column (SC-001); chown success is confirmed by a status
  message and by the refreshed metadata (`FileMode.uid/gid` re-stat), which is
  how SC-006 is tested. A dedicated owner column is **out of scope**.
- **Rationale**: Adding an owner column is a listing-layout change beyond this
  feature's intent; `FileMode` already carries `uid/gid` so the change is real
  and test-verifiable without UI surface.
- **Alternatives considered**: add an owner column — deferred (listing-layout
  work, separate concern).

## R-007: Recursion deferred — needs a tracked follow-up before merge

- **Decision**: No recursive subtree apply (clarified). A selected directory has
  only its own mode/owner changed.
- **Rationale**: Keeps effort at M and the VFS ops single-path. Per the project's
  deferral discipline (CLAUDE.md), a GitHub issue + ROADMAP row must exist before
  the PR merges — handled in the Polish phase.
- **Alternatives considered**: ship recursion now — pushes to L; out of the
  agreed scope.

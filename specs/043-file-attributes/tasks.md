# Tasks: File Attribute Operations (chmod / chown / links)

**Input**: Design documents from `specs/043-file-attributes/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/attr-ops-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR/SC
gets a red→green pair; git history MUST show `(red)` before `(green)`. The pure
`ModeSpec` truth table is the gating SC-004 test.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files / disjoint regions)
- **[Story]**: US1 / US2 / US3, or SETUP/FOUND/POLISH
- File paths are exact.

## Conventions

- VFS ops + mode parsing: `crates/cargonaut-vfs/src/{traits.rs,local.rs,mode.rs,lib.rs}`
- App orchestration: `crates/cargonaut-core/src/lib.rs`
- Dialogs / dispatch / menu / keymap / help: `crates/cargonaut-ui-tui/src/{lib.rs,keymap.rs}`
- Keymap contract: `design/contracts/keymap.toml`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands
  the implementation.

---

## Phase 1: Setup

- [ ] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). Add `nix` (already in
  `Cargo.lock` transitively) as a direct dependency of `crates/cargonaut-vfs/Cargo.toml`
  with the `user` feature (for `User::from_name`/`Group::from_name`); confirm the
  workspace still builds.

---

## Phase 2: Foundational (Blocking — mode parsing + VFS ops)

**Purpose**: The pure `ModeSpec` parser and the four `VfsBackend` operations are
the substrate every user story builds on. This phase also carries the SC-004
gate.

**⚠️ No user-story phase can start until this is complete.**

- [ ] T002 [P] [FOUND] (red) In `crates/cargonaut-vfs/src/mode.rs` add failing
  tests for `ModeSpec::parse`/`apply` covering the contract §1 truth table (octal
  absolute, symbolic relative incl. `u+x`/`go-w`/`a=r`/`u+x,g+x`, and the
  `Empty`/`BadOctal`/`BadSymbolic` error cases).
- [ ] T003 [FOUND] (green) Implement `ModeSpec` (`Octal`/`Symbolic`), `SymClause`,
  `ModeError`, `parse`, and `apply` in `crates/cargonaut-vfs/src/mode.rs`; add
  `pub mod mode;` + re-export in `lib.rs`. Make T002 pass. `#![warn(missing_docs)]`
  clean.
- [ ] T004 [P] [FOUND] (red) In `crates/cargonaut-vfs/src/local.rs` add failing
  `#[tokio::test]`s for `LocalFs`: `chmod` sets bits (re-stat); `symlink` creates
  a (possibly dangling) link resolving to target; `hard_link` shares content and
  errors on a directory; `chown` no-op to current uid/gid returns `Ok` and a
  re-stat shows the ids (runnable unprivileged) (contract §2).
- [ ] T005 [FOUND] (green) Add `chmod`/`chown`/`symlink`/`hard_link` to the
  `VfsBackend` trait in `crates/cargonaut-vfs/src/traits.rs` with default bodies
  returning `VfsError::Unsupported(...)`; implement them in `LocalFs`
  (`fs::set_permissions`+`PermissionsExt`, `std::os::unix::fs::chown`,
  `std::os::unix::fs::symlink`, `std::fs::hard_link`) mapping errors via the
  existing `map_io`. Make T004 pass.
- [ ] T006 [FOUND] (red) In `crates/cargonaut-vfs/src/local.rs` add a failing
  test that the default trait impls return `Unsupported` (use a tiny stub backend
  or assert the doc-contract) — FR-006.
- [ ] T007 [FOUND] (green) Verify/adjust the default trait bodies so T006 passes.

**Checkpoint**: mode parsing + filesystem ops are solid and gated, independent of UI.

---

## Phase 3: User Story 1 — Change permissions (chmod) (Priority: P1) 🎯 MVP

**Goal**: chmod the selection via octal or symbolic input; the perms column
updates.

**Independent Test**: highlight `rw-r--r--`, set `755` (or `u+x`), see `rwxr-xr-x`.

- [ ] T008 [P] [US1] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s (tempdirs): `chmod_selection("755")` sets `0o755` on the
  focused file (re-stat); a multi-file tagged selection all change in one call
  (SC-003); `chmod_selection("xyz")` ⇒ `Err(AppError::BadAttr)` with no change
  (SC-004); a batch with one unwritable target reports the failure and still
  changes the others (SC-005/FR-010); with the cursor on the synthetic `..` row
  and nothing tagged, `chmod_selection` targets nothing and makes no change
  (FR-005).
- [ ] T009 [US1] (green) Add `AppError::BadAttr(String)` and
  `App::chmod_selection(spec)` in `crates/cargonaut-core/src/lib.rs`: parse via
  `ModeSpec` (BadAttr on error), iterate `selection_or_focused(active)`, apply to
  each file's current bits, `local_fs.chmod`, collect per-file failures,
  `refresh_active_pane`, status (mirrors `App::mkdir`). Make T008 pass.
- [ ] T010 [P] [US1] (red) In `crates/cargonaut-ui-tui/src/keymap.rs` add a
  failing test that `C-x c` resolves to `Command::Chmod` (pane mode), no
  collision.
- [ ] T011 [US1] (green) Add `Command::Chmod` to `keymap.rs`; add the binding to
  `design/contracts/keymap.toml` (`pane`, `C-x c`, `chmod`). Make T010 pass.
- [ ] T012 [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs` add a failing test
  that `dispatch_ui_command(Command::Chmod, …)` opens a `TextInputDialog`
  (`InputKind::Chmod`) prefilled with the focused entry's current octal mode and
  sets `Mode::Dialog`.
- [ ] T013 [US1] (green) Wire `Command::Chmod` in `dispatch_ui_command` to open
  the prefilled input dialog; add `InputKind::Chmod`; on submit call
  `app.chmod_selection(text)` and apply events (invalid ⇒ inline error/status,
  Esc ⇒ close). Add a File-menu "Chmod" entry. Make T012 pass. Include an
  explicit test that Esc on the dialog closes it with panes + files unchanged
  (FR-012).

**Checkpoint**: US1 works end-to-end — chmod via key/menu, octal + symbolic. **MVP.**

---

## Phase 4: User Story 2 — Create symbolic and hard links (Priority: P2)

**Goal**: create a symlink / hardlink to the focused entry, named by the user.

**Independent Test**: highlight `file.txt`, create a symlink, see it resolve.

- [ ] T014 [P] [US2] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s: `create_symlink("ln")` makes `ln` in the active cwd pointing
  at the focused entry (appears on refresh); existing name ⇒ `Err`, nothing
  overwritten; `create_hard_link("h")` shares content; hard-linking a directory
  ⇒ reported error, no crash (SC-002).
- [ ] T015 [US2] (green) Implement `App::create_symlink(name)` and
  `App::create_hard_link(name)` (focused entry as source; `cwd.join(name)` as the
  new link; blank/duplicate ⇒ `BadAttr`; refresh; status). Make T014 pass.
- [ ] T016 [P] [US2] (red) In `crates/cargonaut-ui-tui/src/keymap.rs` add failing
  tests that `C-x s` ⇒ `Command::CreateSymlink` and `C-x l` ⇒
  `Command::CreateHardLink` (pane mode).
- [ ] T017 [US2] (green) Add the two `Command` variants + bindings in
  `keymap.toml` (`C-x s` symlink, `C-x l` hardlink). Make T016 pass.
- [ ] T018 [US2] (green) Wire both commands in `dispatch_ui_command` to open a
  `TextInputDialog` (`InputKind::Symlink`/`HardLink`) prefilled with the target's
  name; on submit call the matching App method. Add File-menu "Symlink" +
  "Hardlink" entries. (Covered by a dispatch test added here, red-then-green
  within this task.)

**Checkpoint**: links creatable via key/menu.

---

## Phase 5: User Story 3 — Change ownership (chown) (Priority: P3)

**Goal**: change user/group of the selection (with confirmation), names or ids.

**Independent Test**: chgrp to an owned group succeeds; unknown name errors;
unprivileged foreign chown reports permission denied.

- [ ] T019 [P] [US3] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s: `chown_selection("<self-user>:<self-group>")` (or numeric
  current ids) ⇒ `Ok`, re-stat shows ids (SC-006); `chown_selection("baduser")`
  ⇒ `Err(BadAttr)` no change (FR-009); owner-string parsing (`user`, `:group`,
  `user:group`, numeric).
- [ ] T020 [US3] (green) Implement `App::chown_selection(owner)`: parse
  `user[:group]` resolving names via `nix::unistd::User::from_name`/`Group::from_name`
  (or numeric), BadAttr on unknown; `local_fs.chown` each target; collect
  failures; refresh; status. Make T019 pass.
- [ ] T021 [US3] (red) In `crates/cargonaut-ui-tui/src/keymap.rs` add a failing
  test that `C-x o` ⇒ `Command::Chown`.
- [ ] T022 [US3] (green) Add `Command::Chown` + `keymap.toml` binding (`C-x o`).
  Make T021 pass.
- [ ] T023 [US3] (green) Wire `Command::Chown` in `dispatch_ui_command`: open a
  prefilled owner `TextInputDialog` (`InputKind::Chown`); on submit chain a
  `ConfirmDialog` ("Change owner of N item(s) to `<owner>`?"); on confirm call
  `app.chown_selection` (FR-007). Add a File-menu "Chown" entry. (Dispatch +
  confirm-chain test added here, red-then-green within this task.)

**Checkpoint**: all three attribute operations work via key + menu.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T024 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs` add a
  failing test that the F1 help text contains `C-x c` and "chmod".
- [ ] T025 [POLISH] (green) Update the help overlay (`HELP_BODY`) to document the
  attribute keys (`C-x c` chmod, `C-x o` chown, `C-x s` symlink, `C-x l`
  hardlink). Make T024 pass.
- [ ] T026 [POLISH] Run `make ci-local` (fmt, clippy `-D warnings`, test, release
  build, docs-gate); then `cargo run -p cargonaut-bin -- ~ /tmp` and walk
  quickstart.md steps 1–9. Fix any clippy/fmt issues.
- [ ] T027 [P] [POLISH] **Deferral paper trail (CLAUDE.md MANDATORY)**: open a
  GitHub issue for **recursive chmod/chown** (problem, why deferred, suggested
  approach: a recurse flag + tree walk + per-entry error reporting, effort,
  Tier + `follow-up` label) and add a `ROADMAP.md` row referencing it (Feature
  043 §clarify deferral).
- [ ] T028 [P] [POLISH] Docs (Constitution / CLAUDE.md MANDATORY): update
  `README.md` ("At a Glance" test count + binary size; Feature History one-liner
  for Feature 043) and append a Feature 043 section to `Learnings.md` (≥3 bullets:
  default-Unsupported trait methods, pure ModeSpec testable seam, nix-already-in-tree
  for safe name resolution). Update `CHANGELOG.md`.
- [ ] T029 [POLISH] Close issue #46: confirm chmod + links + chown delivered;
  reference the merged PR; remove the #46 row from `ROADMAP.md` (resolved).

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T007)** → user stories.
- **US1 (T008–T013)**: depends on Foundational. **MVP.**
- **US2 (T014–T018)**: depends on Foundational (VFS symlink/hard_link); independent of US1.
- **US3 (T019–T023)**: depends on Foundational (chown) + the `AppError::BadAttr` added in US1 (T009).
- **Polish (T024–T029)**: after the stories. T027 (recursion deferral) and T028 (docs) gate the PR.

## Parallel Opportunities

- T002 (mode tests) ∥ T004 (VFS op tests) — different regions of vfs.
- T008 (core chmod test) ∥ T010 (keymap test) — different crates.
- T024 (help) ∥ T027 (deferral) ∥ T028 (docs) in Polish.

## Independent Test Criteria

- **US1**: chmod a file (octal + symbolic) via key/menu; perms column updates.
- **US2**: create a symlink and a hardlink to a file; both appear.
- **US3**: chown to an owned group succeeds (confirmed); unknown name errors;
  unprivileged foreign chown reports without crashing.

## Suggested MVP Scope

**Phase 1 + 2 + 3 (US1)** — chmod via `C-x c`/menu with octal + symbolic,
backed by the VFS ops + pure `ModeSpec`. US2 (links) and US3 (chown) are
incremental.

## Format Validation

All tasks use `- [ ] TNNN [P?] [Story] description + exact path`. Setup/
Foundational/Polish carry SETUP/FOUND/POLISH; story tasks carry US1–US3.

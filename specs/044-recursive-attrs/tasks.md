# Tasks: Recursive chmod / chown into Subtrees

**Input**: Design documents from `specs/044-recursive-attrs/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/recursive-attrs-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR/SC
gets a red→green pair; git history MUST show `(red)` before `(green)`.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files / disjoint regions)
- **[Story]**: US1 / US2, or SETUP/FOUND/POLISH
- File paths are exact.

## Conventions

- Walk + recursive App methods + core commands: `crates/cargonaut-core/src/lib.rs`
- Keymap: `crates/cargonaut-ui-tui/src/keymap.rs` + `design/contracts/keymap.toml`
- Dispatch / dialogs / menu / help: `crates/cargonaut-ui-tui/src/lib.rs`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; `(green)` lands the implementation.
- **No new crates; no `cargonaut-vfs` changes** — reuse Feature 043's per-path
  `chmod`/`chown`, `ModeSpec`, `parse_owner`, `attr_status`.

---

## Phase 1: Setup

- [ ] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). Confirm no `Cargo.toml`
  changes are needed (no new deps).

---

## Phase 2: Foundational (Blocking — the bounded subtree collector)

**Purpose**: The `collect_subtree` walk is the substrate both user stories build
on (enumerate the tree, no-follow symlinks, bounded, ordered).

**⚠️ No user-story phase can start until this is complete.**

- [ ] T002 [FOUND] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s for `collect_subtree`: a nested tree (`a/b/c/deep`) yields all
  entry paths incl. the deep one in shallow→deep order; a `VfsKind::Symlink`
  directory is **not** descended (its target's entries absent); a file root
  contributes only itself (FR-009); a lowered cap (test seam) sets the
  `truncated` flag (SC-004).
- [ ] T003 [FOUND] (green) Implement `async fn collect_subtree(&self, roots:
  &[VfsPath]) -> (Vec<VfsPath>, bool)` in `crates/cargonaut-core/src/lib.rs`:
  BFS via `local_fs.list`, push only `VfsKind::Dir` children (never `Symlink`),
  cap at `NODE_CAP` (reuse `recursive_dir_size`'s value; make the cap overridable
  for the truncation test, e.g. a const + a `#[cfg(test)]` hook or an internal
  parameter). Make T002 pass.

**Checkpoint**: the subtree enumeration is correct, bounded, and symlink-safe.

---

## Phase 3: User Story 1 — Recursive chmod (Priority: P1) 🎯 MVP

**Goal**: `C-x C` applies a permission change to a directory's whole subtree,
after confirmation.

**Independent Test**: recursive chmod a tree; a file levels deep has the new mode.

- [ ] T004 [P] [US1] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s for `App::chmod_recursive`: applies at depth (SC-001);
  symbolic applied per entry relative to its mode (FR-003); deepest-first so
  `chmod_recursive("0")` still changes the deepest entry (no lock-out, FR-011);
  a symlinked dir's target is unchanged (SC-005); one unwritable deep entry ⇒
  reported, others changed (SC-006); invalid mode ⇒ `Err(BadAttr)`, no walk;
  file-only selection ⇒ shallow (FR-009).
- [ ] T005 [US1] (green) Implement `App::chmod_recursive(spec)` in
  `crates/cargonaut-core/src/lib.rs`: parse `ModeSpec` (BadAttr on fail); roots =
  `selection_or_focused`; `collect_subtree`; apply chmod **deepest-first**
  (reverse), `apply` per entry's current bits; aggregate via `attr_status("chmod
  -R", …)` + truncation note; refresh. Add `Command::ChmodRecursive(String)` to
  the core `Command` enum + dispatch arm. Make T004 pass.
- [ ] T006 [P] [US1] (red) In `crates/cargonaut-ui-tui/src/keymap.rs` add a
  failing test that `C-x C` resolves to `Command::ChmodRecursive` (pane), no
  collision with `C-x c`.
- [ ] T007 [US1] (green) Add `Command::ChmodRecursive` to `keymap.rs` + the
  binding to `design/contracts/keymap.toml` (`pane`, `C-x C`, `chmod-recursive`).
  Make T006 pass.
- [ ] T008 [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs` add a failing test
  that `dispatch_ui_command(Command::ChmodRecursive, …)` opens a prefilled
  `TextInputDialog` (`InputKind::ChmodRecursive`, current octal), `Mode::Dialog`;
  and that submitting it chains a `ConfirmDialog` (FR-002).
- [ ] T009 [US1] (green) Add `InputKind::ChmodRecursive`; wire the `C-x C`
  dispatch arm (open prefilled input) and the Input-submit arm (open
  `ConfirmDialog` with `on_confirm = AppCommand::ChmodRecursive(text)`); add a
  File-menu "Chmod -R" entry. Make T008 pass.

**Checkpoint**: recursive chmod works end-to-end via key + menu + confirm. **MVP.**

---

## Phase 4: User Story 2 — Recursive chown (Priority: P2)

**Goal**: `C-x O` applies an ownership change to a directory's whole subtree,
after confirmation.

**Independent Test**: recursive chown a tree; a nested entry reflects the owner.

- [ ] T010 [P] [US2] (red) In `crates/cargonaut-core/src/lib.rs` add failing
  `#[tokio::test]`s for `App::chown_recursive`: no-op chown to current `uid:gid`
  applied at depth (SC-002); deepest-first; unknown owner ⇒ `Err(BadAttr)` no
  walk; partial failure aggregated (SC-006).
- [ ] T011 [US2] (green) Implement `App::chown_recursive(owner)` in
  `crates/cargonaut-core/src/lib.rs` (mirror `chmod_recursive`, using
  `parse_owner` + `local_fs.chown`, `attr_status("chown -R", …)`); add
  `Command::ChownRecursive(String)` + dispatch arm. Make T010 pass.
- [ ] T012 [P] [US2] (red) In `crates/cargonaut-ui-tui/src/keymap.rs` add a
  failing test that `C-x O` resolves to `Command::ChownRecursive`.
- [ ] T013 [US2] (green) Add `Command::ChownRecursive` + binding (`C-x O`,
  `chown-recursive`). Make T012 pass.
- [ ] T014 [US2] (green) Add `InputKind::ChownRecursive`; wire `C-x O` dispatch
  (prefilled owner input) + Input-submit → `ConfirmDialog` with `on_confirm =
  AppCommand::ChownRecursive(text)`; add File-menu "Chown -R". (Dispatch +
  confirm-chain test added here, red-then-green within this task.)

**Checkpoint**: both recursive operations work via key + menu + confirm.

---

## Phase 5: Polish & Cross-Cutting Concerns

- [ ] T015 [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs` add a failing
  test that the F1 help text contains `C-x C` and "recursive" (or "-R").
- [ ] T016 [POLISH] (green) Update the help overlay (`HELP_BODY`) to document the
  recursive keys (`C-x C` recursive chmod, `C-x O` recursive chown). Make T015 pass.
- [ ] T017 [POLISH] Run `make ci-local` (fmt, clippy `-D warnings`, test, release
  build, docs-gate); then `cargo run -p cargonaut-bin -- /tmp/rtest /tmp` and walk
  quickstart.md steps 1–9. Fix any clippy/fmt issues.
- [ ] T018 [P] [POLISH] Docs (Constitution / CLAUDE.md MANDATORY): update
  `README.md` ("At a Glance" test count + binary size; Feature History one-liner
  for Feature 044) and append a Feature 044 section to `Learnings.md` (≥3 bullets:
  no-vfs-change orchestration, deepest-first apply avoids lock-out,
  symlink-excluded-for-free via `VfsKind`). Update `CHANGELOG.md`.
- [ ] T019 [POLISH] Close issue #65: confirm recursive chmod/chown delivered;
  reference the merged PR; remove the #65 row from `ROADMAP.md` (resolved).

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T003)** → user stories.
- **US1 (T004–T009)**: depends on `collect_subtree`. **MVP.**
- **US2 (T010–T014)**: depends on `collect_subtree`; independent of US1 (parallel
  after Foundational), though both add core commands + dispatch arms in the same
  files (sequence those edits).
- **Polish (T015–T019)**: after the stories. T018 (docs) gates the PR.

## Parallel Opportunities

- T004 (core chmod test) ∥ T006 (keymap test) — different crates.
- T010 (US2 core test) can be authored in parallel with US1 once Foundational lands.

## Independent Test Criteria

- **US1**: recursive chmod changes a file several levels deep; confirm required.
- **US2**: recursive chown changes a nested entry; confirm required.

## Suggested MVP Scope

**Phase 1 + 2 + 3 (US1)** — recursive chmod via `C-x C` + menu, bounded walk,
deepest-first, symlink-safe, confirmed. US2 (chown) is the increment.

## Format Validation

All tasks use `- [ ] TNNN [P?] [Story] description + exact path`. Setup/
Foundational/Polish carry SETUP/FOUND/POLISH; story tasks carry US1/US2.

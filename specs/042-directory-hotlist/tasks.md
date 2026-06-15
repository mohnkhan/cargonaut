# Tasks: Directory Hotlist / Bookmarks

**Input**: Design documents from `specs/042-directory-hotlist/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/hotlist-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR/SC
gets a red→green pair; git history MUST show `(red)` before `(green)`. The
config-crate persistence round-trip is the gating SC-002/SC-005 test.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files / disjoint regions)
- **[Story]**: US1 / US2 / US3 / US4, or SETUP/FOUND/POLISH
- File paths are exact.

## Conventions

- Persistence + data types: `crates/cargonaut-config/src/lib.rs`
- App state + ops: `crates/cargonaut-core/src/lib.rs`
- Popup widget: `crates/cargonaut-ui-tui/src/dialog.rs`
- Dispatch / key wiring / help: `crates/cargonaut-ui-tui/src/lib.rs`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands
  the implementation. `[P]` tests within a phase may be authored together.
- **No new crates** — `serde`/`toml` already in `cargonaut-config`. Keymap binding
  (`C-b` → `bookmarks-menu`) already present — no `keymap.toml` change.

---

## Phase 1: Setup

- [X] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). Verify no `Cargo.toml`
  changes are needed (no new dependencies).

---

## Phase 2: Foundational (Blocking — data types + persistence)

**Purpose**: The `Bookmark`/`Hotlist` types and their load/save are the substrate
every user story builds on. This is also the SC-002/SC-005 persistence gate.

**⚠️ No user-story phase can start until this is complete.**

- [X] T002 [P] [FOUND] (red) In `crates/cargonaut-config/src/lib.rs`, add failing
  tests for `Bookmark`/`Hotlist`: TOML round-trip (`save`→`load` equal, incl.
  `group`), absent-file `load` ⇒ empty, malformed-file `load` ⇒ empty (no
  panic), `save` creates missing parent dirs, and `default_hotlist_path()`
  honors `$XDG_STATE_HOME` else `~/.local/state/cargonaut/hotlist.toml`
  (contract §1). Use `tempfile` + scoped env for the path test.
- [X] T003 [FOUND] (green) Implement `Bookmark { name, path, group: Option<String> }`,
  `Hotlist { bookmarks: Vec<Bookmark> }` (serde, `#[serde(rename = "bookmark")]`
  array-of-tables), `default_hotlist_path()` (mirror `default_config_path()`),
  and `Hotlist::load`(never-errors)/`save`/`add`/`remove`. Make T002 pass.
  `#![warn(missing_docs)]` clean.
- [X] T004 [P] [FOUND] (red) In `crates/cargonaut-config/src/lib.rs`, add a
  failing test for `Hotlist::grouped()` — buckets by group, ungrouped in a
  default section, original indices preserved (contract §1 / SC-007).
- [X] T005 [FOUND] (green) Implement `Hotlist::grouped()`. Make T004 pass.

**Checkpoint**: hotlist data + persistence are solid and gated, independent of UI.

---

## Phase 3: User Story 1 — Bookmark current dir and jump back (Priority: P1) 🎯 MVP

**Goal**: From the popup, add the active pane's cwd as a named bookmark and later
select it to navigate the active pane there.

**Independent Test**: Add a bookmark, move the pane, open Ctrl-b, select it →
pane is at the bookmarked dir.

- [ ] T006 [P] [US1] (red) In `crates/cargonaut-core/src/lib.rs`, add failing
  `#[tokio::test]`s (tempdir + injected `app.hotlist_path`): `add_bookmark("x",
  Some("g"))` uses the active pane cwd as `path`, appears in `bookmarks()`, and
  the on-disk file contains it; `add_bookmark("", _)` ⇒ `AppError`, unchanged
  (FR-011); adding two bookmarks with the **same name** ⇒ both coexist (FR-011
  duplicate-name default); `jump_to_bookmark(i)` to a valid dir navigates the
  active pane (contract §2).
- [X] T007 [US1] (green) Add `hotlist: Hotlist` + `hotlist_path: PathBuf` fields
  to `App`; load in `App::new` via `default_hotlist_path()` (best-effort).
  Implement `bookmarks()`, `add_bookmark(name, group)` (build from active pane
  cwd, push, `save`, status), and `jump_to_bookmark(index)` (reuse
  `quick_cd(path)`). Make T006 pass.
- [ ] T008 [P] [US1] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a
  failing test for `HotlistDialog`: `new(rows)` renders entries (`TestBackend`);
  nav keys move selection; the select key ⇒ `HotlistAction::Select(i)`; Esc ⇒
  `Close` (contract §3). Add `HotlistAction`.
- [X] T009 [US1] (green) Implement `HotlistDialog` + `HotlistAction`
  (`Select|Add|Remove|Close`) modeled on `TasksPanelDialog` (modal list, `Clear`
  first, `theme.dialog_style()`). Make T008 pass.
- [ ] T010 [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a failing
  test that `dispatch_ui_command(Command::BookmarksMenu, …)` opens
  `ActiveDialog::Hotlist` from `app.bookmarks()` and sets `Mode::Dialog`
  (replacing the "not yet available" path).
- [ ] T011 [US1] (green) Add `ActiveDialog::Hotlist { widget: HotlistDialog }`;
  wire `Command::BookmarksMenu` in `dispatch_ui_command` to open it; add the
  render arm; handle the dialog keys — `Select(i)` → `app.jump_to_bookmark(i)`
  then close; `Add` → open a `TextInputDialog` (group/name) then
  `app.add_bookmark` and reopen; Esc/`Close` → close. Map index → bookmark via a
  fresh `app.bookmarks()` snapshot. Make T010 pass.

**Checkpoint**: US1 works end-to-end — add (with name prompt) + jump. **MVP done.**

---

## Phase 4: User Story 2 — Bookmarks persist across sessions (Priority: P2)

**Goal**: Bookmarks added in one session are present after relaunch.

**Independent Test**: Add a bookmark, drop the `App`, reconstruct from the same
`hotlist_path`, confirm it's loaded.

- [ ] T012 [US2] (red) In `crates/cargonaut-core/src/lib.rs`, add a failing
  `#[tokio::test]`: add a bookmark with `app.hotlist_path` = tempfile, drop and
  rebuild an `App` pointed at the same file (or reload), and assert the bookmark
  is present with name/group/path intact (SC-002). Also assert a malformed file
  yields an empty hotlist without panic at construction (FR-013).
- [ ] T013 [US2] (green) Ensure `App::new` loads from `hotlist_path` and
  `add_bookmark`/`remove_bookmark` persist via `Hotlist::save`. Make T012 pass.
  (Mostly verifies T007 wiring + adds the malformed-at-construction guard/log.)

**Checkpoint**: persistence proven at the App level on top of the config gate.

---

## Phase 5: User Story 3 — Remove a bookmark (Priority: P2)

**Goal**: Delete a bookmark from the popup; it stays gone (and after relaunch).

**Independent Test**: With a bookmark present, remove it; absent on reopen + reload.

- [ ] T014 [US3] (red) In `crates/cargonaut-core/src/lib.rs`, add a failing
  `#[tokio::test]`: `remove_bookmark(i)` drops entry `i`, persists, and a reload
  confirms it's gone (SC-005); out-of-range ⇒ error/no-op, no panic.
- [ ] T015 [US3] (green) Implement `App::remove_bookmark(index)` (remove, `save`,
  status). Make T014 pass.
- [ ] T016 [US3] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a failing
  test that the remove key ⇒ `HotlistAction::Remove(i)`.
- [ ] T017 [US3] (green) Handle the remove key in `HotlistDialog` and wire
  `Remove(i)` → `app.remove_bookmark(i)` + refresh the popup in
  `crates/cargonaut-ui-tui/src/lib.rs`. Make T016 pass.

**Checkpoint**: full add / jump / remove / persist loop usable.

---

## Phase 6: User Story 4 — Organize bookmarks into groups (Priority: P3)

**Goal**: The popup presents bookmarks organized by group; ungrouped under a
default section.

**Independent Test**: Add `work/a` and `b` (no group); the popup shows `a` under
"work" and `b` under the default section.

- [ ] T018 [US4] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a failing
  test that the add flow parses a `group/name` `TextInputDialog` submission into
  `app.add_bookmark(name, Some(group))` (and a bare name → `None`).
- [ ] T019 [US4] (green) Implement the `group/name` split at the add-submit site
  (split on first `/`; no `/` ⇒ whole text is the name, group `None`). Make T018
  pass.
- [ ] T020 [US4] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a failing
  `TestBackend` test: rows built from `Hotlist::grouped()` render a group header
  per group plus its bookmarks, with ungrouped under a default section (SC-007).
- [ ] T021 [US4] (green) Build the popup rows from `grouped()` (group headers +
  entries) so display is organized by group. Make T020 pass.

**Checkpoint**: all four user stories independently testable and green.

---

## Phase 7: Polish & Cross-Cutting Concerns

- [ ] T022 [US1] (red) In `crates/cargonaut-core/src/lib.rs`, add a failing
  `#[tokio::test]` for FR-008/SC-004: `jump_to_bookmark` to a removed/missing
  directory returns an error/status, leaves both panes unchanged, and keeps the
  bookmark in `bookmarks()`.
- [ ] T023 [US1] (green) Confirm `jump_to_bookmark` surfaces the `quick_cd`
  resolution error without mutating panes or the hotlist; make T022 pass (likely
  no new code beyond T007 — lock it with the test).
- [ ] T024 [POLISH] (red) In `crates/cargonaut-ui-tui/src/dialog.rs` (or `lib.rs`),
  add a failing test that opening the hotlist with **zero** bookmarks renders a
  clear empty-state row (FR-010/SC-006).
- [ ] T025 [POLISH] (green) Render an empty-state row when there are no
  bookmarks. Make T024 pass.
- [ ] T026 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  failing test that the F1 help text contains "Ctrl-b" and "bookmark".
- [ ] T027 [POLISH] (green) Update the help overlay (`HELP_BODY`) to document
  `Ctrl-b` (open hotlist) + the in-popup add/remove keys. Make T026 pass.
- [ ] T028 [POLISH] Run `make ci-local` (fmt, clippy `-D warnings`, test, release
  build, docs-gate). Then `XDG_STATE_HOME=$(mktemp -d) cargo run -p cargonaut-bin`
  and walk quickstart.md steps 1–8. Fix any clippy/fmt issues.
- [ ] T029 [P] [POLISH] Docs (Constitution / CLAUDE.md MANDATORY): update
  `README.md` ("At a Glance" metrics — test count, binary size; Feature History
  one-liner for Feature 042) and append a Feature 042 section to `Learnings.md`
  (≥3 bullets: config-crate persistence seam for race-free gates, jump-reuses-
  quick_cd, in-popup actions + group/name convention). Update `CHANGELOG.md`.
- [ ] T030 [POLISH] Close issue #42: confirm the hotlist delivered; reference the
  merged PR. (Resolves a deferral — no new ROADMAP row needed; remove the #42 row
  from `ROADMAP.md` per its "when an issue is closed, delete its row" rule.)

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T005)** → user stories.
- **US1 (T006–T011)**: depends on Foundational. **MVP.**
- **US2 (T012–T013)**: depends on US1 (persistence wiring lives in add/remove).
- **US3 (T014–T017)**: depends on US1 (popup + App ops).
- **US4 (T018–T021)**: depends on US1 (add flow + popup) and `grouped()` (T005).
- **Polish (T022–T030)**: after the stories. T029 (docs) gates the PR.

## Parallel Opportunities

- T002 ∥ T004 (config tests, disjoint).
- T006 (core test) ∥ T008 (dialog test) — different crates.
- T026 (help test) ∥ T029 (docs) in Polish.

## Independent Test Criteria

- **US1**: add via popup (name prompt) + select to jump navigates the active pane.
- **US2**: a bookmark survives an App rebuild from the same `hotlist_path`.
- **US3**: removed bookmark is gone on reopen and after reload.
- **US4**: popup groups entries; ungrouped under a default section.

## Suggested MVP Scope

**Phase 1 + 2 + 3 (US1)** — a working Ctrl-b hotlist with add (name prompt) and
jump, persisted via the config-crate substrate. US2/US3/US4 are incremental.

## Format Validation

All tasks use `- [ ] TNNN [P?] [Story] description + exact path`. Setup/
Foundational/Polish carry SETUP/FOUND/POLISH; story tasks carry US1–US4.

# Tasks: Find-File and Panelize

**Input**: Design documents from `specs/052-find-file-panelize/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/find-file-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR gets
a red→green pair; git history MUST show `(red)` before `(green)`. The pure
decision functions (`plan_content_available`, phase-transition truth tables
from contracts §3) are the gating correctness tests.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files / disjoint regions, no ordering dependency)
- **[Story]**: US1 / US2 / US3, or SETUP/FOUND/POLISH
- File paths are exact.

## Conventions

- Keymap contract: `design/contracts/keymap.toml`
- Command enum + keymap tests: `crates/cargonaut-ui-tui/src/keymap.rs`
- Dialog struct + enums + tests: `crates/cargonaut-ui-tui/src/dialog.rs`
- Event loop / dispatch / panelize / status / help: `crates/cargonaut-ui-tui/src/lib.rs`
- UI-TUI crate manifest: `crates/cargonaut-ui-tui/Cargo.toml`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands
  the implementation. `[P]` tests within a phase may be authored together.
- **No new external crates** — `globset = { workspace = true }` is the only Cargo.toml change.

---

## Phase 1: Setup

- [X] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). Verify the workspace
  `globset` dep (line ~50 of `Cargo.toml`) — add `globset = { workspace = true }`
  to `crates/cargonaut-ui-tui/Cargo.toml` dependencies, then `make build` to
  confirm it resolves. No other `Cargo.toml` changes needed.

---

## Phase 2: Foundational (Blocking — Command variant + keymap binding)

**Purpose**: Register the new action so every user story can dispatch it. Until
the `Command::FindFilePopup` variant and `M-?` binding exist, no story can be
dispatched or tested end-to-end.

**⚠️ No user-story phase can start until this is complete.**

- [X] T002 [FOUND] (red) In `crates/cargonaut-ui-tui/src/keymap.rs`, add a
  failing test asserting `Keymap::load(DEFAULT_KEYMAP_TOML)` succeeds and
  `lookup(Mode::Pane, M-?)` resolves to `Command::FindFilePopup` (contract §1/§2).
  Add a non-collision assertion: no other binding resolves `M-?`.
  (Will not compile until the variant exists — that is the red state.)

- [X] T003 [FOUND] (green) Add the `FindFilePopup` variant to the `Command`
  enum in `crates/cargonaut-ui-tui/src/keymap.rs` with a doc comment
  (`/// Open find-file overlay (Alt-?) — FR-001 (issue #41).`). Add the binding
  block to `design/contracts/keymap.toml` (`mode = "pane"`, `key = "M-?"`,
  `action = "find-file-popup"  # FR-001 (issue #41)`). Make T002 pass.

**Checkpoint**: `M-?` parses and resolves to `FindFilePopup`; nothing handles it yet.

---

## Phase 3: User Story 1 — Search by filename glob and panelize (Priority: P1) 🎯 MVP

**Goal**: `Alt-?` opens the find-file dialog; user types a glob pattern and presses
`Enter`; results populate incrementally; a second `Enter` panelizes results into
the active panel as a flat synthetic listing; all existing panel operations work
on the panelized files.

**Independent Test**: Open dialog, enter `*.toml`, `Enter` → results appear;
`Enter` on result list → panel lists exactly the `.toml` files; `Space` tags one;
`F5` opens copy dialog for that file.

- [X] T004 [P] [US1] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add failing
  unit tests for `plan_content_available(rg_path)` covering the truth table
  (contract §3a): valid path → `true`; non-existent path → `false`. Add failing
  unit tests for `SearchMode` Tab-toggle: `Name → Content` when `content_available=true`;
  no-op + notice when `content_available=false`. Add failing phase-transition unit
  tests covering the `Enter`-key truth table (contract §3b rows `InputFocused`
  and `ResultsFocused`).

- [X] T005 [US1] (green) In `crates/cargonaut-ui-tui/src/dialog.rs`, implement
  `SearchMode` enum, `DialogPhase` enum, `FindEvent` enum, `FindOutcome` enum,
  and `FindFileDialog` struct with all fields from data-model.md (including the `cursor: usize` highlighted-result field added by H2 remediation). Implement
  `plan_content_available(rg_path: &str) -> bool` (pure: checks `Command::new(rg_path).arg("--version").status().is_ok()`).
  Implement `FindFileDialog::new(content_available: bool) -> Self` and
  `FindFileDialog::handle_key` for the phase transitions tested in T004.
  All new public items carry doc comments. Make T004 pass.

- [X] T006 [US1] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add three failing
  tests for `FindFileDialog::start_walk` (name mode): (a) Happy path: create a
  `tempfile` dir with 3 known files, call `start_walk` with a glob matching 2 of them,
  poll `poll_results()` in a loop, assert `results.len() == 2` and `phase ==
  ResultsFocused`. (Use `tokio::runtime::Runtime::block_on` for the test runtime.)
  (b) FR-018 unreadable root: create a `tempfile` dir, use `std::fs::set_permissions`
  to set mode 0o000 (no read), call `start_walk` with root = that dir, poll results;
  assert `results.len() == 0`, `phase == NoResults`, and `notice` contains "Cannot read
  directory". (Skip on platforms where permission removal is ineffective, e.g. root user.)
  (c) **SC-001 timing gate** (CI gate for Constitution §II): create a `tempfile` dir
  with 200 files (all named `file_NNN.tmp`), call `start_walk` with pattern `*.tmp`
  (matches all 200), record `std::time::Instant::now()` before the call, poll
  `poll_results()` in a loop until `phase != Walking`, assert elapsed < 5 s and
  `results.len() == 200`. This integration test is the CI gate for SC-001 (≤5 s name
  search) per Constitution §II — run unconditionally (no `#[ignore]`).

- [X] T007 [US1] (green) Implement `FindFileDialog::start_walk` for Name mode
  in `crates/cargonaut-ui-tui/src/dialog.rs`: first check `std::fs::read_dir(&root)`
  is readable — if not, set `phase = NoResults` and `notice = Some(format!("Cannot
  read directory: {}", root.display()))` and return without spawning (FR-018 root
  guard). Otherwise spawn `tokio::task::spawn_blocking` running a BFS
  (`std::collections::VecDeque<PathBuf>`) over `std::fs::read_dir`; match each
  filename with `globset::GlobBuilder::new(pattern)?.build()?.compile_matcher()`;
  check `abort_flag.load(Ordering::Relaxed)` at each iteration (Relaxed ordering is correct for a best-effort cancellation flag — no synchronization point is needed); silently skip unreadable subdirs
  (FR-018 subdir guard); send `FindEvent::Found(path)` for matches,
  `FindEvent::Done { truncated }` when walk ends or `max_results` reached.
  Implement `FindFileDialog::poll_results()` to drain `walk_rx` via `try_recv()`
  loop, update `results`, handle `Done`. Implement `FindFileDialog::cancel()`.
  Make T006 pass.

- [X] T008 [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add failing
  integration tests for the panelize action (FR-009, SC-004 — all panel ops):
  Given a `tempfile` dir with known files, simulate `dispatch_ui_command(Command::FindFilePopup, …)`
  opening the dialog, simulate walk completion, simulate `Enter` on `ResultsFocused`
  producing `FindOutcome::Panelize { paths, pattern }`, then call the panelize
  helper and assert:
  - `listing.entries.len()` equals the expected file count and `ui.find_label == Some(pattern)` (SC-004 entry count).
  - Pressing `Space` on entry 0 tags it (tag op).
  - `dispatch_ui_command(Command::Copy, …)` is dispatched without panic (copy / F5 op).
  - `dispatch_ui_command(Command::Move, …)` is dispatched without panic (move / F6 op — FR-009/E1).
  - `dispatch_ui_command(Command::Delete, …)` is dispatched without panic (delete / F8 op — FR-009/E2).
  - `dispatch_ui_command(Command::ViewFile, …)` is dispatched without panic (view / F3 op — FR-009/E3).
  - `dispatch_ui_command(Command::Edit, …)` is dispatched without panic (edit / F4 op — FR-009/C1).
  Each assertion uses `assert!(result.is_ok())` or equivalent — verifying dispatch reaches the correct op, not full execution. (Full copy/move/delete/view/edit execution is covered by existing test suites for those commands.)

- [X] T009 [US1] (green) In `crates/cargonaut-ui-tui/src/lib.rs`:
  - Add `find_label: Option<String>` to `UiState`.
  - Add `ActiveDialog::FindFile { widget: dialog::FindFileDialog, root: PathBuf }`
    variant to the `ActiveDialog` enum.
  - Add the `Command::FindFilePopup` arm to `dispatch_ui_command`: check rg
    availability via `plan_content_available`, construct `FindFileDialog::new(...)`,
    set `active_dialog = Some(ActiveDialog::FindFile { widget, root: active_pane_cwd })`.
  - Add the `ActiveDialog::FindFile { widget, root }` arm to the event-loop
    `handle_key` dispatch: route keys to `widget.handle_key(key, config)`;
    on `FindOutcome::Panelize { paths, pattern }` call the panelize helper; on
    `FindOutcome::Cancelled` clear `active_dialog`.
  - Implement the panelize helper: for each `PathBuf` in `paths` call
    `std::fs::metadata` → build `DirEntry`; construct `DirListing { entries, sort: Sort::None }`;
    call `active_pane.set_listing(listing)`; set `ui.find_label = Some(pattern)`.
  - In the 100ms tick handler, call `widget.poll_results()` when `active_dialog`
    is `ActiveDialog::FindFile`.
  - Clear `find_label` in `navigate_to` when a real directory is loaded.
  - **FR-010 status bar render (M1 — explicit step)**: In the active pane's status-bar
    render path in `lib.rs`, read `ui.find_label`: when `Some(s)`, render `[Find: s]`
    in place of the current directory path string; when `None`, render the directory path
    as before. The passive pane's status bar is unaffected.
  Make T008 pass.

- [X] T010 [US1] (red) In `crates/cargonaut-ui-tui/src/dialog.rs` (or `lib.rs`
  render tests), add failing `TestBackend` render tests: (a) `FindFileDialog` in
  `InputFocused` phase renders a bordered overlay with the title "Find File"; (b)
  in `ResultsFocused` with 2 results, renders those paths in the list and the header
  `2 matches`; (c) in `Walking` renders a progress indicator; (d) long-path
  truncation: inject a result with a 300-char absolute path into a 40-col-wide dialog
  area and assert the rendered row contains `…` and ends with the filename (left-truncated,
  spec edge case). (E6)

- [X] T011 [US1] (green) Implement `FindFileDialog::render(f, area, theme)` in
  `crates/cargonaut-ui-tui/src/dialog.rs`: draw a centered overlay block using
  `ratatui::widgets::Block::default().title("Find File").borders(Borders::ALL)`;
  draw mode indicator (`[Name]`/`[Content]`); draw the input field; draw the
  result list (scrollable, cursor-highlighted); draw the match count header
  (`N matches` or `N matches (truncated)` or `0 matches`); draw `notice` text
  when `Some`. Use typed theme colors (no hardcoded ANSI). Make T010 pass.

**Checkpoint**: US1 fully functional — name-glob search, panelize, bulk ops. **MVP done.**

---

## Phase 4: User Story 2 — Content search via ripgrep (Priority: P2)

**Goal**: `Tab` in the find-file dialog switches to Content mode; ripgrep is
invoked with `--files-with-matches`; results are file-level paths only;
panelize works identically to US1.

**Independent Test**: With `rg` on PATH, dialog in Content mode, type a pattern
known to match files; results equal `rg <pattern> --files-with-matches <root>` output; panelize works.

- [X] T012 [P] [US2] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add two
  failing tests for `FindFileDialog::start_walk` in Content mode (SC-003):
  (a) Basic: create a `tempfile` dir with 2 text files (one containing "needle",
  one not); call `start_walk` with Content mode pattern `needle`; poll results;
  assert `results.len() == 1` and the result path matches the file containing "needle".
  Skip with `return` (early runtime skip, not `#[ignore]`) if
  `std::process::Command::new("rg").arg("--version").status().is_ok()` is false —
  **do NOT use `#[cfg_attr(not(rg_available), ignore)]`** as `rg_available` is
  not a valid cfg predicate.
  (b) Differential: create a 5-file tempdir; collect results from `start_walk`
  Content mode; collect results from `rg needle --files-with-matches <root>`
  via `std::process::Command`; sort both; assert they are equal (SC-003 full-set
  comparison, not just count).

- [X] T013 [US2] (green) Extend `FindFileDialog::start_walk` to handle
  `SearchMode::Content` in `crates/cargonaut-ui-tui/src/dialog.rs`: use
  **`tokio::process::Command`** (not `std::process::Command`) — `tokio::process::Command`
  supports `kill_on_drop` for async-native cancellation. Spawn
  `tokio::process::Command::new(rg_path).args([pattern, "--files-with-matches", "--no-messages", root_str]).stdout(Stdio::piped()).spawn()`;
  read stdout line-by-line via `tokio::io::AsyncBufReadExt`; send `FindEvent::Found(path)`
  per line, then `FindEvent::Done { truncated: count >= max_results }`. The spawned
  `Child` is held in `abort_flag`-checked loop; `cancel()` calls `child.kill().await`
  and drops the handle. Non-zero exit from `rg` (binary files, permission errors)
  is treated as end-of-stream: send `FindEvent::Done { truncated: false }` with
  whatever results were collected so far (never panics).
  **Note (L2)**: `rg --files-with-matches` deduplicates inherently (one path per matched
  file regardless of how many lines match); no additional dedup step is needed in the
  result accumulation loop. Make T012 pass.

- [X] T014 [US2] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a
  failing test for Tab-toggle when `content_available=false`: pressing Tab
  does NOT change mode and sets `notice` to a string containing "Content
  search unavailable" (contract §3a).

- [X] T015 [US2] (green) Wire the Tab key in `FindFileDialog::handle_key`
  (`crates/cargonaut-ui-tui/src/dialog.rs`): toggle `mode` between `Name`
  and `Content`; if toggling to `Content` and `!content_available`, keep mode
  as `Name` and set `notice = Some("Content search unavailable: rg not found")`.
  Make T014 pass.

**Checkpoint**: Content search fully functional; Tab-toggle with graceful degradation.

---

## Phase 5: User Story 3 — Cancel and abort in-progress walk (Priority: P3)

**Goal**: `Esc` at any dialog phase aborts the walk within ≤300 ms and returns
the active panel to its previous listing unchanged.

**Independent Test**: Start a walk on a large directory tree; press `Esc` within
2 s; confirm panel unchanged and app is immediately responsive.

- [X] T016 [US3] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a
  failing test for abort behaviour: start a name-search walk on a `tempfile` dir;
  call `cancel()` immediately after; assert `walk_rx` is `None` and `abort_flag`
  holds `true` (or is dropped); assert `phase` is `InputFocused` and `results`
  is empty. For the ≤300 ms abort timing (SC-006), use a test-only helper:
  expose a `#[cfg(test)] pub(crate) fn start_walk_with_delay(root, config, delay_per_entry: Duration)`
  that inserts `std::thread::sleep(delay_per_entry)` per BFS entry (test-only code path,
  not production). Start the delayed walk, call `cancel()`, measure elapsed time,
  assert < 300ms. This isolates the sleep to the test helper — production `start_walk`
  has no sleep. Do NOT inject `thread::sleep` into the production walk loop.

- [X] T017 [US3] (green) Wire the Esc path in the event-loop
  `ActiveDialog::FindFile` arm in `crates/cargonaut-ui-tui/src/lib.rs`:
  when `FindFileDialog::handle_key` returns `FindOutcome::Cancelled`, call
  `widget.cancel()` (which sets the abort flag, drops walk_rx, resets phase)
  **before** setting `active_dialog = None`. This is new wiring in the event-loop
  arm (T009 scope) that must be explicitly verified to ensure cancel + dismiss
  are atomic from the caller's perspective. Make T016 pass. (This task adds the
  event-loop-level Esc wiring; the dialog-level cancel logic is from T007.)

- [X] T018 [US3] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a failing
  test asserting that after the dialog is dismissed via `Esc` (cancelled),
  `ui.find_label` is NOT set and the active pane's listing is unchanged.

- [X] T019 [US3] (green) Confirm the `ActiveDialog::FindFile` Esc path in
  `crates/cargonaut-ui-tui/src/lib.rs` does NOT call the panelize helper and
  does NOT set `find_label`. Make T018 pass.

**Checkpoint**: All three user stories independently testable and green.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [X] T020 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  failing test asserting the help-overlay text contains both `M-?` and `Find`
  (contract §8, FR-019 — help-overlay discoverability).

- [X] T021 [POLISH] (green) Update the help-overlay in
  `crates/cargonaut-ui-tui/src/dialog.rs` (HELP_SECTIONS constant, Navigation
  or Search section) to add an entry for `M-?` → `Find file (name glob or
  ripgrep content search, then panelize)`. Make T020 pass.

- [X] T022 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  regression test asserting `navigate_to(real_dir)` clears `ui.find_label`
  (contract §6 — find_label lifecycle). Assert that after panelizing (setting
  `find_label = Some(pattern)`) and calling `navigate_to` with a real directory,
  `find_label` is `None`.

- [X] T023 [POLISH] (green) Confirm `navigate_to` in
  `crates/cargonaut-ui-tui/src/lib.rs` resets `ui.find_label = None` when a
  real directory is loaded (implemented in T009 — verify and lock with the test).
  Make T022 pass.

- [X] T024 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add
  a failing test for the result truncation path (contract §4): walk a `tempfile`
  dir with `max_results = 3` and 5 files; assert `results.len() == 3` and
  `truncated == true` after `poll_results` drains `Done`.

- [X] T025 [POLISH] (green) Verify the truncation guard in `start_walk` (walk
  task stops after `max_results` items and sends `Done { truncated: true }`)
  and in `poll_results` (sets `self.truncated = true`). Verify `render` shows
  `N matches (truncated)` when `truncated`. Make T024 pass.

- [X] T026 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add
  a failing test for the no-results path (contract §3b, NoResults row): after
  walk completes with 0 matches, `phase == NoResults` and `notice` contains
  "No files found"; pressing `Enter` in `NoResults` does NOT return `Panelize`
  outcome (returns `Consumed` or stays).

- [X] T027 [POLISH] (green) Implement the `NoResults` phase handling in
  `FindFileDialog::handle_key` and `poll_results` in `crates/cargonaut-ui-tui/src/dialog.rs`:
  on `Done` with empty results, set `phase = NoResults` and `notice = Some(format!("No files found matching `{}`", input))`.
  Confirm `Enter` in `NoResults` is a no-op. Make T026 pass.

- [X] T028 [POLISH] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a
  failing test for the scroll behavior: `ResultsFocused` with 20 results and a
  visible window of 5; pressing `Down` 7 times updates `scroll_offset` so the
  cursor stays visible; pressing `PgDn` advances by ~window height.

- [X] T029 [POLISH] (green) Implement scroll navigation in `FindFileDialog::handle_key`
  (`crates/cargonaut-ui-tui/src/dialog.rs`): `Up`/`Down` move `cursor` (the
  highlighted-result index from data-model.md — H2 addition) within results, clamped to
  `results.len()-1`; `PgUp`/`PgDn` move cursor by visible window height; update
  `scroll_offset` after each cursor move to keep cursor in the visible window
  (`scroll_offset ≤ cursor ≤ scroll_offset + window_height - 1`). Make T028 pass.

- [X] T030 [POLISH] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add a
  failing test for `rg` non-zero exit path (FR-012/FR-018 graceful degradation):
  set up a mock `rg` script (a temp shell script that exits with code 1) as
  `ripgrep_path`; call `start_walk` in Content mode; poll results; assert
  `results.len() == 0`, `phase == NoResults`, and `notice` contains a message
  (e.g. "Content search failed" or "No files found"). Confirms rg non-zero exit
  never panics the event loop and is surfaced as an empty result set.

- [X] T030B [POLISH] (green) Implement the rg non-zero exit handling in
  `FindFileDialog::start_walk` (Content mode, `crates/cargonaut-ui-tui/src/dialog.rs`):
  when the `tokio::process::Command` child exits with non-zero status, send
  `FindEvent::Done { truncated: false }` with whatever results were accumulated
  (may be zero). In `poll_results`, when `Done` is received with empty results
  and the walk was Content mode, set `notice = Some("Content search returned no results (check pattern or rg exit code)")`.
  Make T030 pass.

- [X] T031 [POLISH] Run `make ci-local` (fmt → clippy `-D warnings` → `cargo test --workspace`
  [SC-008 regression gate] → `cargo build --release` → `scripts/check-binary-size.sh`
  [SC-007 ≤8 MiB gate] → docs-gate). Run `cargo run -p cargonaut-bin` and walk
  the quickstart.md manual steps 1–7. Fix any clippy/fmt issues. All steps must pass
  with zero failures before T032.

- [X] T032 [POLISH] Docs (CLAUDE.md MANDATORY — must be committed BEFORE T033/PR merge):
  update `README.md` ("At a Glance" metrics — test count, feature count, binary size;
  + Feature History one-liner for Feature 052) and append a Feature 052 section to
  `Learnings.md` (≥3 bullets: the BFS abort pattern via AtomicBool, the
  synthetic DirListing panelize design, the tokio::process::Command streaming approach
  for ripgrep, the display-relative / store-absolute path split). Verify with
  `docs-gate` as part of T031 above.

- [X] T033 [POLISH] Run `cargo tarpaulin --package cargonaut-ui-tui --lcov --out Lcov`
  and confirm coverage for `cargonaut-ui-tui` remains ≥80% (NFR-007, Constitution §II).
  If coverage drops below 80%, add tests to recover the threshold before proceeding
  to T034. (This is a constitution MUST gate — not skippable.)

- [ ] T034 [POLISH] Close GitHub issue #41: reference the merged PR. Update
  `ROADMAP.md` to mark the issue as resolved (remove it from the open deferred
  items list). No new deferral needed — this feature resolves the tracked gap.
  Run after T032 and T033 are committed; this is the final step before PR merge.

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T003)** → User Stories.
- **US1 (T004–T011)**: depends on Foundational. **MVP.**
- **US2 (T012–T015)**: depends on Foundational; `start_walk` Content mode (T013)
  builds on the channel infrastructure from T007. Can be started in parallel
  with US1 polish once T007 is green.
- **US3 (T016–T019)**: depends on `cancel()` from T007 and the event-loop arm
  from T009. Begin after T009 is green.
- **Polish (T020–T034)**: after US1–US3. T031 (CI) gates T032 (docs). T032 committed before T034 (issue close). T033 (tarpaulin) runs after T031. T034 (issue close) is the final step before PR merge.

## Parallel Opportunities

- T004 (US1 pure-fn / phase tests) ∥ — only if broken into sub-commits; otherwise sequential within US1.
- T020 (help test) ∥ T022 (navigate_to test) ∥ T024 (truncation test) ∥ T026 (no-results test) ∥ T028 (scroll test) — all target different files/regions in Polish phase.

## Independent Test Criteria

- **US1**: Name-glob search finds expected files; panelize puts them in active panel; tag + F5 copy works.
- **US2**: Content search matches `rg --files-with-matches` output; Tab-toggle with `rg` absent shows notice, keeps Name mode.
- **US3**: `Esc` during walk aborts within ≤300 ms; panel unchanged after cancel.

## Suggested MVP Scope

**Phase 1 + Phase 2 + Phase 3 (US1)** — working `Alt-?` glob search with
incremental results, panelize, and bulk ops. US2 (ripgrep) and US3 (explicit
cancel test) are incremental enhancements that build directly on US1's walk
infrastructure.

## Format Validation

All tasks use `- [ ] TNNN [P?] [Story?] description + exact path`. Setup/
Foundational/Polish carry SETUP/FOUND/POLISH; user-story tasks carry US1/US2/US3.
Total tasks after analysis remediation: 36 (T001–T030B/T031–T034).

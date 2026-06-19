# Tasks: Persistent Subshell (Ctrl-o) — Feature 054

**Input**: Design documents from `specs/054-persistent-subshell/`

**Branch**: `054-persistent-subshell`

**Constitution**: TDD mandatory — every functional task MUST have a red (failing) commit BEFORE the green (passing) commit. Task IDs with `(red)` suffix denote the failing-test commit; the next task is the green implementation.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks)
- **[Story]**: Which user story this task belongs to
- Include exact file paths in descriptions

---

## Phase 1: Setup (Dependencies & Config)

**Purpose**: Wire new crates into the workspace and add the config field. No behaviour changes yet — this phase must compile cleanly.

- [ ] T001 Add `vt100 = "0.16"` to `[workspace.dependencies]` in `Cargo.toml`; add `portable-pty = { workspace = true }`, `vt100 = { workspace = true }`, `tui-term = "0.3"` to `crates/cargonaut-ui-tui/Cargo.toml` [dependencies]; verify `cargo check --workspace` passes
- [ ] T002 (red) Add failing test `subshell_height_pct_defaults_to_33` in `crates/cargonaut-config/src/lib.rs` asserting `Config::default().ui.subshell_height_pct == 33`
- [ ] T003 Add `subshell_height_pct: u8` (serde default 33, range-clamped 10–60) to `UiConfig` struct in `crates/cargonaut-config/src/lib.rs`; add clamping logic in `Config::load`; add `subshell_height_pct_clamped_below_10` and `subshell_height_pct_clamped_above_60` tests; make T002 green

**Checkpoint**: `cargo test -p cargonaut-config` passes; `cargo check --workspace` clean.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Core data structures and layout extension. No runtime behaviour yet — only types that compile.

**⚠️ CRITICAL**: All of Phase 3–5 tasks depend on this phase being complete.

- [ ] T004 (red) Add failing compile-test `subshell_phase_enum_exists` in `crates/cargonaut-ui-tui/src/lib.rs` (or a new test module) asserting `SubshellPhase::Hidden` variant exists
- [ ] T005 [P] Create `crates/cargonaut-ui-tui/src/subshell.rs`; define `pub(crate) enum SubshellPhase { Hidden, VisibleFmFocus, VisibleShellFocus }` with `Default = Hidden`; declare `pub(crate) mod subshell;` in `lib.rs`; make T004 green
- [ ] T006 [P] (red) Add failing compile-test `subshell_state_struct_fields` in `crates/cargonaut-ui-tui/src/subshell.rs` asserting `SubshellState { dead, scroll_offset }` fields exist
- [ ] T007 Add `SubshellState` struct skeleton to `crates/cargonaut-ui-tui/src/subshell.rs` with fields: `master: Box<dyn portable_pty::MasterPty + Send>`, `writer: Box<dyn std::io::Write + Send>`, `parser: vt100::Parser`, `pty_rx: tokio::sync::mpsc::Receiver<Vec<u8>>`, `scroll_offset: u16`, `dead: bool`, `current_size: portable_pty::PtySize`; make T006 green
- [ ] T008 [P] (red) Add failing test `frame_layout_has_subshell_field` in `crates/cargonaut-ui-tui/src/chrome.rs` asserting `FrameLayout { subshell: None }` compiles
- [ ] T009 Add `pub(crate) subshell: Option<ratatui::layout::Rect>` to `FrameLayout` struct in `crates/cargonaut-ui-tui/src/chrome.rs`; default to `None`; ensure existing `FrameLayout::default()` usage still compiles; make T008 green

**Checkpoint**: `cargo check --workspace` compiles cleanly. `cargo test -p cargonaut-ui-tui` still passes.

---

## Phase 3: User Story 1 — Toggle Subshell Open and Closed (Priority: P1) 🎯 MVP

**Goal**: `Ctrl-o` cycles the subshell panel through Hidden → VisibleFmFocus → VisibleShellFocus → Hidden. PTY-backed shell process spawns on first advance. Panel renders shell output (VT100). Shell exit is detected and restart offered.

**Independent Test**: `cargo test -p cargonaut-ui-tui subshell_phase` passes; manual: press `Ctrl-o` three times and confirm each state transition.

- [ ] T010 [US1] (red) Add failing unit tests in `crates/cargonaut-ui-tui/src/subshell.rs`: `advance_hidden_to_visible_fm_focus`, `advance_visible_fm_to_shell_focus`, `advance_shell_focus_to_hidden` — each asserts the correct `SubshellPhase` after calling a `advance_phase()` fn
- [ ] T011 [US1] Implement `SubshellState::spawn(shell: &str, cwd: &Path, rows: u16, cols: u16) -> anyhow::Result<SubshellState>` in `crates/cargonaut-ui-tui/src/subshell.rs`: open PTY via `portable_pty::native_pty_system().openpty(PtySize { rows, cols, .. })`, build `CommandBuilder::new(shell)` with `TERM=xterm-256color`, spawn into slave, drop slave, `try_clone_reader()` → `spawn_blocking` loop sending `Vec<u8>` chunks over `mpsc::channel(64)`, EOF sends empty `vec![]` sentinel; store `master`, `writer`, `parser = vt100::Parser::new(rows, cols, 200)`, `pty_rx`
- [ ] T012 [US1] Implement `SubshellState::poll_output(&mut self)` in `crates/cargonaut-ui-tui/src/subshell.rs`: drain all pending bytes from `pty_rx` via `try_recv()` loop; for non-empty chunks call `parser.process(&bytes)`; for empty sentinel set `dead = true`
- [ ] T013 [US1] Implement `SubshellState::advance_phase()` helper in `crates/cargonaut-ui-tui/src/subshell.rs` that cycles `SubshellPhase` as per the three-state contract; make T010 green
- [ ] T014 [US1] Add `subshell: Option<SubshellState>` and `subshell_phase: SubshellPhase` fields to `UiState` struct in `crates/cargonaut-ui-tui/src/lib.rs`; initialise both to `None`/`Hidden` in the `UiState { .. }` literal
- [ ] T015 [US1] Handle `Command::OpenSubshell` in `handle_key` in `crates/cargonaut-ui-tui/src/lib.rs`: when `Hidden → VisibleFmFocus`, lazily spawn `SubshellState::spawn($SHELL or /bin/sh, active_cwd, subshell_rows, terminal_cols)` (store in `ui.subshell`); guard against terminal-too-small (content_height < 8): set status notice and return without advancing; when `VisibleFmFocus → VisibleShellFocus`, advance phase; when `VisibleShellFocus → Hidden`, advance phase; any modal active → no-op (FR-012)
- [ ] T015b [US1] Implement Ctrl-o debounce guard in `handle_key` in `crates/cargonaut-ui-tui/src/lib.rs`: add `last_ctrl_o_at: Option<std::time::Instant>` field to `UiState`; when `OpenSubshell` is dispatched, compare `Instant::now()` with `last_ctrl_o_at`; if elapsed < 50 ms, silently discard the command (no state advance); otherwise update `last_ctrl_o_at` and proceed (spec.md edge case: rapid Ctrl-o bursts); add unit test `ctrl_o_debounce_ignores_rapid_press` in `crates/cargonaut-ui-tui/src/lib.rs`
- [ ] T016 [US1] Extend `draw_frame` signature in `crates/cargonaut-ui-tui/src/lib.rs` with `subshell_phase: SubshellPhase` and `subshell_screen: Option<&vt100::Screen>` parameters; in the vertical layout constraint computation inject `Constraint::Length(subshell_rows)` when phase != `Hidden` (between pane band and status bar); set `layout.subshell = Some(subshell_rect)` when visible; render `tui_term::widget::PseudoTerminal::new(screen)` into `subshell_rect` when screen is `Some`; render a bordered block with "Shell exited — press Ctrl-o to restart" text when `dead == true`; add a 1-row header line above the panel showing `[Shell]` and the current phase indicator
- [ ] T017 [US1] In `run_loop` in `crates/cargonaut-ui-tui/src/lib.rs`: each iteration call `ui.subshell.as_mut().map(|s| s.poll_output())` before the `term.draw` call; pass `ui.subshell_phase` and `ui.subshell.as_ref().map(|s| s.screen())` to `draw_frame`; on terminal resize event, if subshell exists and phase != `Hidden`, call `subshell.resize(new_rows, new_cols)` (implement `SubshellState::resize` in `subshell.rs`: calls `master.resize()` + `parser.screen_mut().set_size()`)
- [ ] T018 [US1] Implement `SubshellState::respawn(&mut self, shell: &str, cwd: &Path, rows: u16, cols: u16)` in `crates/cargonaut-ui-tui/src/subshell.rs`: drop existing `master`/`writer`, spawn fresh PTY + reader task; reset `dead = false`, `scroll_offset = 0`; call from `handle_key` OpenSubshell when `dead == true` and phase is `Hidden → VisibleFmFocus`
- [ ] T019 [US1] (green) Add integration tests in `crates/cargonaut-ui-tui/src/lib.rs`: `subshell_phase_cycles_correctly` (advance Hidden→VFM→VSH→Hidden without spawning real PTY — mock the shell path); `subshell_dead_shows_restart_notice`; `ctrl_o_noop_when_modal_active`; make T010 green

**Checkpoint**: `cargo test -p cargonaut-ui-tui subshell` passes. Manual: `Ctrl-o` three times cycles states; panel renders; `Ctrl-o` restores full layout.

---

## Phase 4: User Story 2 — Shell cwd Stays in Sync with the Active Panel (Priority: P1)

**Goal**: Every panel directory change (navigate, focus-swap, tab-switch) sends a shell-quoted `cd <path>\r` to the PTY, whether the panel is visible or hidden.

**Independent Test**: `cargo test -p cargonaut-ui-tui subshell_cwd_sync` passes; manual: navigate to a new dir → run `pwd` in shell → output matches panel dir.

- [ ] T020 [US2] (red) Add failing unit tests in `crates/cargonaut-ui-tui/src/subshell.rs`: `sync_cwd_sends_quoted_cd` (path with space: `"/home/user/my docs"` → shell receives `cd '/home/user/my docs'\r`); `sync_cwd_noop_when_dead`
- [ ] T021 [US2] Implement `SubshellState::sync_cwd(&mut self, path: &Path)` in `crates/cargonaut-ui-tui/src/subshell.rs`: if `dead`, return early; shell-quote via `shell_words::quote(path.to_str().unwrap_or(""))`, write `format!("cd {quoted}\r")` bytes to `writer`; make T020 green
- [ ] T022 [US2] In `run_loop` in `crates/cargonaut-ui-tui/src/lib.rs`, track `last_synced_cwd: Option<PathBuf>` (init `None`); after every `app.dispatch()` that can change the active pane's cwd, read `app.pane(app.active()).cwd()` and compare to `last_synced_cwd`; if different, call `ui.subshell.as_mut().map(|s| s.sync_cwd(&new_cwd))` and update `last_synced_cwd`; apply to these commands: `Descend`, `Ascend`, `QuickCdPopup` (on `complete_cd` completion), bookmark navigation (`Hotlist` confirm)
- [ ] T023 [US2] Extend cwd-sync in `run_loop` in `crates/cargonaut-ui-tui/src/lib.rs` to also fire after `FocusSwap`, `FocusLeft`, `FocusRight` dispatch — read the newly focused pane's cwd and sync
- [ ] T024 [US2] Extend cwd-sync in `run_loop` in `crates/cargonaut-ui-tui/src/lib.rs` to fire after `TabNext` and `TabPrev` dispatch — the active tab's cwd may have changed even though the focused side didn't change
- [ ] T025 [US2] Handle the edge case where the synced directory no longer exists: wrap the `sync_cwd` call in a check; if `path.exists()` is false, walk ancestors with `path.ancestors()` until finding an existing one and sync to that; if none found, sync to `"/"` (always safe fallback)
- [ ] T026 [US2] (green) Add integration tests in `crates/cargonaut-ui-tui/src/lib.rs`: `cwd_sync_fires_on_descend`, `cwd_sync_fires_on_focus_swap`, `cwd_sync_fires_on_tab_next`, `cwd_sync_hidden_subshell_still_syncs` — use a fake PTY writer (`Vec<u8>`) to capture written bytes and assert `cd` commands; make T020 green

**Checkpoint**: `cargo test -p cargonaut-ui-tui subshell_cwd_sync` passes. Manual: navigate + switch tabs; `pwd` in shell always matches.

---

## Phase 5: User Story 3 — Keyboard Focus and Input Routing (Priority: P2)

**Goal**: When `SubshellPhase::VisibleShellFocus`, all keystrokes (except `Ctrl-o`) are forwarded verbatim to the PTY. Mouse clicks route focus. Shell exits cleanly restore FM focus.

**Independent Test**: `cargo test -p cargonaut-ui-tui subshell_key_routing` passes; manual: press `Ctrl-o` twice → type text → appears in shell; press `Ctrl-o` → FM cursor responds to arrow keys again.

- [ ] T027 [US3] (red) Add failing unit tests in `crates/cargonaut-ui-tui/src/subshell.rs`: `key_to_pty_bytes_char`, `key_to_pty_bytes_enter`, `key_to_pty_bytes_ctrl_c`, `key_to_pty_bytes_arrow_up_normal_cursor`, `key_to_pty_bytes_arrow_up_application_cursor`, `key_to_pty_bytes_backspace`
- [ ] T028 [US3] Implement `pub(crate) fn key_to_pty_bytes(key: crossterm::event::KeyEvent, app_cursor: bool) -> Vec<u8>` in `crates/cargonaut-ui-tui/src/subshell.rs` using the mapping table from `research.md` R-009: `Char(c)` + CONTROL modifier → `[0x1f & c as u8]`; printable chars → UTF-8 bytes; Enter → `\r`; Backspace → `\x7f`; arrows → ANSI or application sequences based on `app_cursor`; special keys per R-009 table; make T027 green
- [ ] T029 [US3] Implement `SubshellState::write_key(&mut self, key: crossterm::event::KeyEvent)` in `crates/cargonaut-ui-tui/src/subshell.rs`: if `dead`, no-op; call `key_to_pty_bytes(key, self.parser.screen().application_cursor())`; write to `writer`; flush
- [ ] T030 [US3] In `handle_key` in `crates/cargonaut-ui-tui/src/lib.rs`, when `subshell_phase == VisibleShellFocus`: intercept key event BEFORE normal Command dispatch; if key is `Ctrl-o`, advance phase to `Hidden` (normal OpenSubshell path); otherwise call `ui.subshell.as_mut().map(|s| s.write_key(key_event))` and return (do NOT pass event to the file-manager command dispatch)
- [ ] T031 [US3] In `handle_mouse` in `crates/cargonaut-ui-tui/src/lib.rs`: if a click/drag event lands within `ui.layout.subshell` (the `Option<Rect>` from T009) and the subshell panel is visible, set `ui.subshell_phase = VisibleShellFocus`; if a click lands outside the subshell rect while `subshell_phase == VisibleShellFocus`, set `ui.subshell_phase = VisibleFmFocus`
- [ ] T032 [US3] Add subshell-mode `C-o` binding to `design/contracts/keymap.toml`: `[[binding]]\nmode = "subshell"\nkey = "C-o"\naction = "open-subshell"`; add `HELP_SECTIONS` entry for subshell keys in `crates/cargonaut-ui-tui/src/dialog.rs` so the help overlay covers the binding (verified by `help_covers_all_keymap_bindings` test)
- [ ] T033 [US3] (green) Add integration tests in `crates/cargonaut-ui-tui/src/lib.rs`: `key_forwarded_to_pty_in_shell_focus`, `ctrl_o_in_shell_focus_returns_to_fm`, `mouse_click_in_shell_rect_transfers_focus`, `mouse_click_outside_shell_rect_returns_fm_focus`; make T027 green

**Checkpoint**: `cargo test -p cargonaut-ui-tui subshell_key_routing` passes. Manual: full three-state cycle with real typing into shell works end-to-end.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Performance gates, lint gates, documentation, and PR-readiness.

- [ ] T034 [P] Run `cargo clippy --workspace --all-targets -- -D warnings`; fix every warning in `subshell.rs`, `lib.rs`, `chrome.rs`; add `#[allow]` only where the lint is a known false-positive with a comment explaining why
- [ ] T035 [P] Run `cargo fmt --check --workspace`; apply `cargo fmt` to all modified files; verify `cargo fmt --check` passes with no diff
- [ ] T036 Run `cargo test --workspace`; confirm all existing tests still pass (regression gate — specifically run `help_covers_all_keymap_bindings` after T032's keymap addition)
- [ ] T037 Run `cargo bench -p cargonaut-ui-tui --bench keypress_latency`; confirm ≤16 ms p99 result (NFR-002) — the PTY `try_recv` path must not block the keypress→first-paint hot path
- [ ] T038 Run `cargo bench -p cargonaut-ui-tui --bench rss_headroom`; confirm RSS ≤64 MiB with subshell panel open and a shell running (NFR / SC-003) — ring buffer size and `vt100::Parser` scrollback (200 lines) stay within budget
- [ ] T039 Run `scripts/check-binary-size.sh`; confirm stripped release binary ≤8 MiB (NFR-001); if over budget, reduce `vt100::Parser` scrollback or audit new dep sizes
- [ ] T040 [P] Update `README.md`: increment test count in "At a Glance" table; add one-line entry for Feature 054 in "Feature History" section
- [ ] T041 [P] Append Feature 054 section to `Learnings.md` with ≥3 bullets covering: PTY async read pattern chosen (`spawn_blocking` + `mpsc`); three-state Ctrl-o design rationale vs. MC-classic; VT100 emulation scope decision (`vt100` + `tui-term` vs. own parser); any non-obvious hitches discovered during implementation
- [ ] T042 Close GitHub issue #44 in the PR description with "Closes #44"

---

## Implementation Strategy

**MVP scope**: Phases 1–3 (US1: toggle + PTY panel rendering). Ship a working toggle first. cwd-sync (US2) and keyboard routing (US3) are additive and do not require any US1 redesign.

**Parallel opportunities within phases**:
- T008 and T004 are parallelisable (different files: `chrome.rs` vs `subshell.rs`).
- T034 and T035 (clippy + fmt) can run in parallel.
- T037, T038, T039 (bench + size check) can run in parallel after tests pass.
- T040 and T041 (README + Learnings) are independent parallel doc tasks.

**Story completion order** (dependency graph):
```
Phase 1 (T001-T003) → Phase 2 (T004-T009) → Phase 3 (T010-T019) [MVP complete]
                                            → Phase 4 (T020-T026) [US2 adds cwd-sync]
                                            → Phase 5 (T027-T033) [US3 adds key routing]
Phase 3 + Phase 4 + Phase 5 → Phase 6 (T034-T042) [polish + docs]
```

**TDD discipline**: Every task pair that is marked `(red)` followed by its green implementation task MUST be committed in that order. The red commit message format: `T054.NN (red): failing test for <description>`. The green commit message format: `T054.NN (green): <description> — tests passing`.

---

## Task Summary

| Phase | Tasks | Story | Status |
|---|---|---|---|
| Phase 1: Setup | T001–T003 | — | pending |
| Phase 2: Foundational | T004–T009 | — | pending |
| Phase 3: US1 Toggle | T010–T015b, T016–T019 | US1 | pending |
| Phase 4: US2 cwd-sync | T020–T026 | US2 | pending |
| Phase 5: US3 Key routing | T027–T033 | US3 | pending |
| Phase 6: Polish | T034–T042 | — | pending |
| **Total** | **43 tasks** | | |

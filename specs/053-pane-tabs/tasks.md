# Tasks: Pane Tabs — Multiple Panels Per Side (Feature 053)

**Input**: Design documents from `specs/053-pane-tabs/`

**Branch**: `053-pane-tabs` | **Spec**: spec.md | **Plan**: plan.md

**TDD mandate** (Constitution §II): Every task pair is **red → green**. The red commit contains a failing test; the green commit contains the implementation that makes it pass. Commit messages follow `T0NN (red): …` / `T0NN (green): …` pattern.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency conflicts)
- **[Story]**: User story label (US1, US2, US3)
- All file paths are repo-relative

---

## Phase 1: Setup

**Purpose**: Verify the environment is clean and branch is correct before any code changes.

- [ ] T001 Confirm tmpfs symlink is active (`make tmpfs-status`) and existing test suite is green (`make test`)

---

## Phase 2: Foundational — Core State Machine Refactor

**Purpose**: Replace `App.panes: [PaneState; 2]` with `App.sides: [SideState; 2]` and introduce all new types. This is the central structural change; US1–US3 all depend on it being complete and correct.

**⚠️ CRITICAL**: All US phases depend on this phase. Existing tests MUST remain green throughout every step.

### 2a — New types (red → green)

- [ ] T002 (red) Write failing tests for `SideState` and `TabBarEntry` types in `crates/cargonaut-core/src/lib.rs` — assert `SideState` has `tabs: Vec<PaneState>` + `active_tab: usize`; assert `TabBarEntry` has `index: usize`, `label: String`, `is_active: bool` fields accessible from tests (compile-fail tests or type-shape tests that fail until the types exist)
- [ ] T003 (green) Add private `SideState { tabs: Vec<PaneState>, active_tab: usize }` and public `TabBarEntry { index: usize, label: String, is_active: bool }` to `crates/cargonaut-core/src/lib.rs`; derive `Debug, Clone` on both; add `#[doc]` on all public items

### 2b — App struct refactor (red → green)

- [ ] T004 (regression guard — no red commit needed) Write tests in `crates/cargonaut-core/src/lib.rs` that call `app.pane(PaneId::Left)` and `app.active_pane_state()` and assert returned `PaneState` matches the single starting tab — these test API stability through the T005 refactoring (they pass before AND after; their purpose is to catch any regression introduced by the rename, not to start failing). Commit these before T005 with message `T004: API stability regression guard`. [H2 fix: these are not TDD-red tests; the existing test suite already serves as the red baseline before this refactoring]
- [ ] T005 (green) Refactor `App` in `crates/cargonaut-core/src/lib.rs`: rename `panes: [PaneState; 2]` → `sides: [SideState; 2]`; update `App::new()` to construct two single-element `SideState`s; update `pane()`, `pane_mut()`, `active_pane_mut()` to dereference `sides[idx].tabs[active_tab]`; run existing test suite + T004 guard to confirm zero regressions

### 2c — New Commands (red → green)

- [ ] T006 (red) Write failing tests in `crates/cargonaut-core/src/lib.rs` that dispatch `Command::TabNew`, `Command::TabClose`, `Command::TabNext`, `Command::TabPrev` and assert each returns `Ok(_)` without panicking (tests fail with "no variant" compile error until variants are added)
- [ ] T007 (green) Add four variants to `Command` enum in `crates/cargonaut-core/src/lib.rs`: `TabNew`, `TabClose`, `TabNext`, `TabPrev`; add stub arms in `App::dispatch()` that return `Ok(vec![])` (stubs turn T006 tests green; feature tests in Phase 3 will still fail)

**Checkpoint**: `make test` green. All existing tests pass. `App` struct uses `SideState`. Four new Command variants exist. Two new types exist.

---

## Phase 3: User Story 1 — Open and Switch Between Multiple Tabs (Priority: P1) 🎯 MVP

**Goal**: Users can open new tabs (`Ctrl-t`), switch with `[`/`]`, and close with `Ctrl-w`. A tab bar is always visible above each pane. Tab state (cursor, filter, selection) is independent per tab.

**Independent Test**: `cargo test -p cargonaut-core tab` — all tab operation tests pass. `cargo test -p cargonaut-ui-tui tab_bar` — tab bar renders correctly.

### 3a — Core tab operations (red → green)

- [ ] T008 (red) Write failing tests in `crates/cargonaut-core/src/lib.rs` for all tab operations (tests compile and run but fail at assertions because stubs return `Ok(vec![])`):
  - `tab_new_opens_in_same_cwd` — after dispatch TabNew, left side has 2 tabs, second tab cwd == first tab cwd
  - `tab_new_inherits_no_filter_or_selection` — new tab has `filter == None`, `selected.is_empty()`
  - `tab_new_becomes_active` — `sides[0].active_tab == 1` after TabNew on left pane
  - `tab_close_noop_on_single_tab` — dispatch TabClose with 1 tab → still 1 tab, no panic
  - `tab_close_selects_right_successor` — close tab 1 of [0,1,2] → active_tab == 1 (former index 2)
  - `tab_close_wraps_to_last_when_rightmost` — close tab 2 of [0,1,2] → active_tab == 1
  - `tab_next_advances_and_wraps` — TabNext cycles forward, wraps from last to first
  - `tab_prev_recedes_and_wraps` — TabPrev cycles backward, wraps from first to last
  - `tab_next_noop_with_one_tab` — single tab stays at index 0 after TabNext
  in `crates/cargonaut-core/src/lib.rs`
- [ ] T009 (green) Implement `tab_new()`, `tab_close()`, `tab_next()`, `tab_prev()` methods on `App` in `crates/cargonaut-core/src/lib.rs`:
  - `tab_new`: clones active tab's `cwd` + `listing`; new PaneState starts with `filter=None, selected=BTreeSet::new(), sort=Sort::NameAsc, show_hidden=config.ui.show_hidden`; cursor = `default_cursor()`; history empty; appends to `sides[active_idx].tabs`; sets `active_tab` to new last index; returns `vec![Event::PaneUpdated(self.active)]`
  - `tab_close`: no-op if `tabs.len() == 1`, returning `Ok(vec![])` (NOT `PaneUpdated` — single-tab close is a true no-op per contracts/core-api.md); else remove `active_tab`; new active = `min(closed_idx, tabs.len()-1)`; returns `vec![Event::PaneUpdated(self.active)]` [L2 fix: explicit `Ok(vec![])` for single-tab case]
  - `tab_next` / `tab_prev`: modular arithmetic on `active_tab`; returns `vec![Event::PaneUpdated(self.active)]`
  - Wire all four in `App::dispatch()` match arms (replace stubs)

### 3b — tab_bar_view (red → green)

- [ ] T010 (red) Write failing tests in `crates/cargonaut-core/src/lib.rs`:
  - `tab_bar_view_single_tab` — single tab returns 1 entry; `is_active = true`; label = basename of cwd
  - `tab_bar_view_multiple_tabs` — after 2 TabNews, `tab_bar_view` returns 3 entries; exactly one `is_active`; `index` is 1-based
  - `tab_bar_view_label_truncates_long_name` — cwd with 30-char basename → label is ≤20 chars with `…` suffix
  - `tab_bar_view_active_marker_on_correct_tab` — after TabNext, is_active moves to new active index
- [ ] T011 (green) Implement `App::tab_bar_view(&self, id: PaneId) -> Vec<TabBarEntry>` in `crates/cargonaut-core/src/lib.rs`: iterate `sides[idx].tabs`; label = last path segment (or `/` when at root) truncated to **max 20 UTF-8 chars** (hard maximum per contracts/tui-rendering.md; the `~20` in spec.md FR-004 is interpreted as a hard cap of 20, not approximate) with `…` suffix when truncation occurs; `index` is 1-based; `is_active = i == active_tab` [CHK007 fix: resolve "~20 chars" ambiguity in favor of hard max 20]

### 3c — Keymap bindings (parallel-eligible)

- [ ] T012 [P] Add `TabNext` and `TabPrev` variants to `Command` enum in `crates/cargonaut-ui-tui/src/keymap.rs` with `#[doc]` comments and `#[serde(rename_all = "kebab-case")]` (action strings `"tab-next"` / `"tab-prev"`)
- [ ] T013 [P] Add `]` → `tab-next` and `[` → `tab-prev` bindings to `design/contracts/keymap.toml` under the "Power features" section (Constitution §III: keymap-first)
- [ ] T014 Add all four tab commands to `ui_command_to_core()` in `crates/cargonaut-ui-tui/src/lib.rs`: `NewTab → AppCommand::TabNew`, `CloseTab → AppCommand::TabClose`, `TabNext → AppCommand::TabNext`, `TabPrev → AppCommand::TabPrev`

### 3d — Tab bar rendering (red → green)

- [ ] T015 (red) Write failing tests in `crates/cargonaut-ui-tui/src/lib.rs` (or `crates/cargonaut-ui-tui/src/pane.rs`):
  - `tab_bar_line_renders_single_tab` — `tab_bar_line(&[TabBarEntry{index:1,label:"foo".into(),is_active:true}], 40, &Theme::default())` returns a `Line` containing `"[1*]foo"` text [H1 fix: 3-arg signature per contracts/tui-rendering.md]
  - `tab_bar_line_renders_multiple_tabs_with_active_marker` — `tab_bar_line(&[entry1, entry2], 40, &Theme::default())` where entry2 is active: line contains `"[1]"` and `"[2*]"` spans
  - `draw_pane_tab_bar_occupies_first_row` — render `draw_pane` into a 40×6 `TestBackend`; assert first rendered row contains `"[1"` (tab bar visible above list border)
- [ ] T016 (green) Implement `pub fn tab_bar_line(entries: &[cargonaut_core::TabBarEntry], width: u16, theme: &Theme) -> ratatui::text::Line<'_>` as a free function in `crates/cargonaut-ui-tui/src/lib.rs` (3-arg signature, `theme` used for active-tab styling):
  - Per entry: `format!("[{}{}]{}", entry.index, if entry.is_active {"*"} else {""}, entry.label)` — **no space before basename** (format is `[N]basename` not `[N] basename`; the spec FR-012 example `[1] src` uses a space, but contracts/tui-rendering.md and the format string above do not; the contracts/tui-rendering.md definition is authoritative for implementation) [CHK015 fix: resolve format inconsistency in favor of compact `[N]basename` format]
  - Separate entries with `"  "` (two spaces) per contracts/tui-rendering.md
  - Compute cumulative widths; find scroll offset so active tab is always visible
  - Return `Line` of `Span`s: active tab span uses `theme.cursor_style()`, inactive uses default style
- [ ] T017 (green) Update `draw_pane()` in `crates/cargonaut-ui-tui/src/lib.rs`:
  - Change layout split from 2 constraints to 3: `[Length(1), Min(2), Length(1)]` (tab bar, list+border, mini-status)
  - Render `tab_bar_line(tab_bar, col[0].width, theme)` as a `Paragraph` into `col[0]`
  - Move existing list+border into `col[1]`; move mini-status into `col[2]`
  - Return `col[1]`'s inner rect (unchanged external contract — inner list rect for mouse hit-testing)
  - Add `tab_bar: &[cargonaut_core::TabBarEntry]` parameter to `draw_pane`
- [ ] T018 (green) Update `draw_frame()` signature in `crates/cargonaut-ui-tui/src/lib.rs` to accept `tab_bar_left: &[cargonaut_core::TabBarEntry]` and `tab_bar_right: &[cargonaut_core::TabBarEntry]`; pass them through to `draw_pane` calls; update the single `draw_frame` call site in `run_loop()` to pass `app.tab_bar_view(PaneId::Left)` and `app.tab_bar_view(PaneId::Right)` (computed before the `term.draw` closure)

**Checkpoint**: `cargo test --workspace` fully green. Tab open/switch/close working in core. Tab bar renders in TUI. Keymap bindings wired.

---

## Phase 4: User Story 2 — File Operations Act Between Active Tabs (Priority: P1)

**Goal**: Cross-pane copy, move, compare, and sync-path always use each side's **active** tab as source or destination, preserving existing two-pane semantics.

**Independent Test**: `cargo test -p cargonaut-core cross_pane` — cross-pane ops use active tab cwd.

**Note**: Because `confirm_copy`, `confirm_move`, `sync_other_panel_path` all call `self.pane(id).cwd`, and `pane()` already returns the active tab after Phase 2, the implementation is complete by construction. This phase adds tests to *verify* the semantics explicitly.

- [ ] T019 (red) Write failing tests in `crates/cargonaut-core/src/lib.rs`:
  - `cross_pane_copy_dest_is_active_tab_cwd` — create app; open a second left tab; navigate tab 2 to a different directory; keep tab 1 active; dispatch Copy from right pane; assert the copy job's `dst` path matches the active left tab's cwd (tab 1's cwd), not tab 2's cwd
  - `cross_pane_copy_after_tab_switch_uses_new_active` — same setup; switch left to tab 2 via TabNext; dispatch Copy; assert dst is tab 2's cwd
  - `sync_other_panel_uses_active_tab_cwd` — open 2 left tabs; switch to tab 2 (different dir); from right pane dispatch SyncOtherPanelPath; assert right pane's cwd becomes left tab 2's cwd
  - `dialog_dest_captured_at_open_time` — open confirm-copy dialog (capturing dst = active left tab cwd); switch active left tab via TabNext (while dialog is open, tab state MUST NOT change per FR-013, but verify even if it did that the dialog's captured dst is unchanged); assert dialog dst path == the left tab that was active when dialog opened [M1 fix: covers US2 AC3]
- [ ] T020 (green) Confirm tests pass without implementation changes — `pane()` already returns active tab through `sides[idx].tabs[active_tab]`. If any test reveals a code path using `panes` directly (bypassing the abstraction), fix that path in `crates/cargonaut-core/src/lib.rs`

**Checkpoint**: `cargo test -p cargonaut-core cross_pane` green. Cross-pane semantics verified.

---

## Phase 5: User Story 3 — All Existing Pane Features Work Per-Tab (Priority: P2)

**Goal**: Filter, sort, cursor, selection, show-hidden, history, hex viewer, find-file, panelize all work independently within each tab. FR-013 (modal swallows tab keys) verified.

**Independent Test**: `cargo test --workspace` — all existing tests pass unchanged.

- [ ] T021 (red) Write failing tests in `crates/cargonaut-core/src/lib.rs`:
  - `tab_state_filter_is_isolated` — set filter on tab 0; dispatch TabNew; assert tab 1 has `filter == None`; set filter on tab 1; dispatch TabPrev to switch back; assert tab 0's filter is unchanged
  - `tab_state_sort_is_isolated` — set sort on tab 0; dispatch TabNew; assert tab 1 has `sort == Sort::NameAsc` (default); cycle sort on tab 1; assert tab 0's sort unchanged
  - `tab_state_cursor_is_isolated` — move cursor in tab 0; dispatch TabNew (tab 1 cursor at default); assert they differ
  - `tab_state_selection_is_isolated` — toggle selection in tab 0; dispatch TabNew; assert tab 1 `selected.is_empty()`
  - `tab_new_does_not_inherit_filter` — apply filter; TabNew; new tab filter is None (FR-011)
  - `tab_state_show_hidden_is_isolated` — toggle show_hidden in tab 0; dispatch TabNew; assert tab 1 show_hidden == config default (not tab 0's toggled value) [M2 fix: FR-006 show_hidden isolation]
  - `tab_state_history_is_isolated` — navigate to 3 dirs in tab 0 (building history); dispatch TabNew; assert tab 1 history is empty [M2 fix: FR-006 history isolation]
  - `panelize_result_is_tab_local` — panelize a find-file result in tab 0; dispatch TabNew; assert tab 1 shows a fresh listing from its cwd, not the panelized result [M3 fix: US3 AC4]
  - `focus_swap_key_does_not_change_tabs` — dispatch FocusOtherPane (Tab key); assert `active_tab` on both sides is unchanged; dispatch M1/M2 focus commands; assert tabs unchanged [M4 fix: FR-010 dedicated test]
- [ ] T022 (red) Write failing tests in `crates/cargonaut-ui-tui/src/lib.rs`:
  - `modal_guard_tab_keys_swallowed` — test approach: construct an `AppState` with an active dialog injected (`active_dialog = Some(ActiveDialog::ConfirmDelete(...))`); call `handle_key(KeyEvent::new(KeyCode::Char(']'), Modifiers::empty()), &mut state)`; assert tab `active_tab` is unchanged and the function returned before reaching `dispatch_ui_command`; repeat for `[`, `Ctrl-t`, `Ctrl-w`; confirm tab count is unchanged after each key. [M6 fix: concrete test approach — create app with injected dialog state, invoke handle_key directly, check tab_count via app.sides[idx].tabs.len()]
- [ ] T023 (green) Confirm all T021/T022 tests pass through structural isolation (no new code needed beyond what Phase 2–3 produced). If any isolation path is broken, patch `tab_new()` in `crates/cargonaut-core/src/lib.rs` to correctly zero-initialize fields
- [ ] T024 Run the full regression suite to confirm all pre-existing tests pass: `cargo test --workspace`

**Checkpoint**: `cargo test --workspace` fully green. All FR-006/FR-008/FR-011/FR-013 tests pass.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Performance gates, quality, documentation. All stories functionally complete before this phase.

### Performance verification

- [ ] T025 [P] Update `crates/cargonaut-ui-tui/benches/keypress_latency.rs` (file verified present) to include a 5-tab-per-side scenario; run `cargo bench keypress` and assert **p99 latency ≤16ms** (spec SC-003 / Constitution NFR-002; percentile is p99 per plan.md §Performance — note: constitution §IV also defines SC-001..SC-004 for global Phase-1 perf; "spec SC-003" here means the keypress latency criterion from spec.md, distinct from constitution §IV SC-001) [M5 fix: clarify SC namespace; CHK019 fix: p99 percentile explicit]
- [ ] T026 [P] Update `crates/cargonaut-core/benches/rss_headroom.rs` (file verified present; also see `crates/cargonaut-ui-tui/benches/rss_headroom.rs` for the UI-side RSS gate) to open 5 tabs per side and assert total RSS ≤64 MiB (spec SC-004 = max RSS including tabs; constitution §IV SC-003 = same ≤64 MiB global gate) [M5 fix: clarify SC namespace; L1 fix: bench file paths verified]

### Code quality

- [ ] T027 Run `cargo clippy --workspace --all-targets -- -D warnings` and fix all warnings in `crates/cargonaut-core/src/lib.rs` and `crates/cargonaut-ui-tui/src/lib.rs`
- [ ] T028 Run `cargo fmt --check`; apply `cargo fmt` if needed; commit formatting fixes
- [ ] T029 [P] Verify `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc --workspace --no-deps` builds clean (Constitution §I)

### Coverage

- [ ] T030 Run `make ci-local` (full CI pipeline locally); verify tarpaulin reports ≥80% coverage on `cargonaut-core`; add targeted tests for any uncovered tab-ops paths

### Documentation

- [ ] T031 [P] Update `README.md`: increment test count and feature count in the "At a Glance" table; add one-line entry in "Feature History" for Feature 053
- [ ] T032 [P] Update `Learnings.md`: append section for Feature 053 with ≥3 bullets covering the `panes→sides` refactor approach, TDD red→green discipline on tab ops, and the tab bar horizontal scroll algorithm
- [ ] T033 Close GitHub issue #45 with a comment referencing the merged PR

**Checkpoint**: `make ci-local` fully green. README + Learnings updated. Ready for PR.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — **BLOCKS all user-story phases**
- **Phase 3 (US1)**: Depends on Phase 2 — core tab ops + TUI rendering
- **Phase 4 (US2)**: Depends on Phase 2 — cross-pane ops verification
- **Phase 5 (US3)**: Depends on Phase 3 (tab state must exist to test isolation)
- **Phase 6 (Polish)**: Depends on Phases 3–5 all green

### User Story Dependencies

- **US1 (P1)**: After Phase 2 — no dependency on US2/US3
- **US2 (P1)**: After Phase 2 — independent of US1 (but can share Phase 3 context)
- **US3 (P2)**: After US1 — tab state must exist to test isolation

### Within Each Phase

- `(red)` tasks MUST be committed failing before `(green)` implementation
- `[P]` tasks within a phase touch different files and can run in parallel
- T012, T013 (keymap) are independent of T008–T011 (core ops) and can proceed in parallel
- T025, T026 (benches) are independent of T027–T032 (quality/docs) and can proceed in parallel

### Critical Path

```
T001 → T002→T003 → T004→T005 → T006→T007        [Foundational]
       → T008→T009→T010 → T011 → T014            [US1 core]
       → T012, T013 (parallel)                    [US1 keymap]
       → T015→T016→T017→T018                      [US1 rendering]
       → T019→T020                                [US2]
       → T021→T022→T023→T024                      [US3]
       → T025..T033                               [Polish]
```

---

## Parallel Opportunities

### Within Phase 3 (after T007 is green):
```
# Parallel track A — core tab ops:
T008 (red) → T009/T010/T011 (green)

# Parallel track B — keymap (independent files):
T012 (keymap.rs Command enum)
T013 (keymap.toml bindings)
```

### Within Phase 6 (after T024 is green):
```
# Parallel:
T025 (keypress bench) | T026 (RSS bench)
T027 (clippy)         | T029 (rustdoc)
T031 (README)         | T032 (Learnings)
```

---

## Implementation Strategy

### MVP First (US1 only — Phases 1–3)

1. Complete Phase 1 (verify environment)
2. Complete Phase 2 (foundational refactor) — **CRITICAL BLOCKER**
3. Complete Phase 3 (US1 — tab open/switch/close + tab bar)
4. **STOP and VALIDATE**: `cargo test --workspace` and manual TUI smoke test
5. US1 alone is feature-complete and shippable as a standalone increment

### Incremental Delivery

1. Phase 1 + Phase 2 → stable base
2. Phase 3 → tabs work + tab bar visible (MVP)
3. Phase 4 → cross-pane ops verified
4. Phase 5 → state isolation verified
5. Phase 6 → CI green, docs done → ready for PR

---

## Notes

- Red commits: `git commit -m "T00N (red): failing test for …"`
- Green commits: `git commit -m "T00N (green): implement … to pass tests"`
- `[P]` tasks operate on different files with no shared state — safe to parallelize
- Every US label maps to exactly one user story in spec.md
- `make ci-local` = `clippy → cargo test → cargo build --release → check-pr-body → docs-gate`
- Skip docs-gate guard: any commit on this branch with `[no-docs]` in message disables the docs check (not needed here — README + Learnings will be updated in T031/T032)

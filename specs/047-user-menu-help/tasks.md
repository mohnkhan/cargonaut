# Tasks: User Menu (F2) + Scrollable Hypertext Help (F1)

**Input**: Design documents from `specs/047-user-menu-help/`

**Branch**: `047-user-menu-help`

**Prerequisites**: plan.md ✓ · spec.md ✓ · research.md ✓ · data-model.md ✓ · contracts/ ✓ · quickstart.md ✓

**Constitution §II — TDD REQUIRED**: Tests MUST be written in a failing state (red commit) before the implementation that makes them pass (green commit). Per-task git history must show `T###(red)` → `T###(green)` pairs for every FR task.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no incomplete dependencies)
- **[Story]**: User story this task belongs to (US1, US2, US3)
- Exact file paths are in every description

---

## Phase 1: Setup — Dependency & Workspace

**Purpose**: Add the one new dependency (`shell-words`) so all later compilation targets exist.

- [ ] T001 Add `shell-words = "1.1"` to `[workspace.dependencies]` in `Cargo.toml` and add it to `[dependencies]` in `crates/cargonaut-ui-tui/Cargo.toml`; run `cargo check -p cargonaut-ui-tui` to confirm the workspace resolves cleanly

**Checkpoint**: `cargo check --workspace` passes — no new warnings, no missing dependency errors.

---

## Phase 2: Foundational — `cargonaut-config` Types

**Purpose**: Core data types and config-path resolver that both US1 and US2 depend on. No UI changes yet.

**⚠️ CRITICAL**: US2 (user menu) cannot be implemented until this phase is complete.

- [ ] T002 (red) Write failing unit tests for `MenuItem` and `UserMenuConfig` TOML deserialization in `crates/cargonaut-config/src/lib.rs` `mod tests` — test: valid full item (label + command + only_if + key), item with only required fields, empty actions array, `only_if` absent defaults to `None`, `key` must be one char (invalid longer string fails); commit with message `T002(red): failing tests for MenuItem/UserMenuConfig deserialization`

- [ ] T003 Add `MenuItem` struct (fields: `label: String`, `command: String`, `only_if: Option<String>`, `key: Option<char>`) with `#[derive(Debug, Clone, serde::Deserialize)]` to `crates/cargonaut-config/src/lib.rs`; add `UserMenuConfig { actions: Vec<MenuItem> }` struct; run `cargo test -p cargonaut-config` — T002 tests must now pass; commit `T002/T003(green): MenuItem + UserMenuConfig types`

- [ ] T004 (red) Write failing unit tests for `menu_config_path()` in `crates/cargonaut-config/src/lib.rs` — test: when `XDG_CONFIG_HOME=/tmp/xdg` is set, path is `/tmp/xdg/cargonaut/menu.toml`; when unset and `HOME=/tmp/h`, path is `/tmp/h/.config/cargonaut/menu.toml`; commit `T004(red): failing test for menu_config_path`

- [ ] T005 Implement `pub fn menu_config_path() -> std::path::PathBuf` in `crates/cargonaut-config/src/lib.rs` following the same `XDG_CONFIG_HOME → $HOME/.config` pattern as `default_config_path()`; pub-export from the crate root; run `cargo test -p cargonaut-config` — T004 tests pass; commit `T004/T005(green): menu_config_path()`

- [ ] T006 (red) Write failing unit tests for `load_user_menu()` in `crates/cargonaut-config/src/lib.rs` — test: file not found returns `Ok(UserMenuConfig { actions: vec![] })`; valid TOML with two actions returns both items; TOML with syntax error returns `Err(MenuLoadError::Parse(...))` containing a line number; commit `T006(red): failing tests for load_user_menu`

- [ ] T007 Implement `pub fn load_user_menu(path: &std::path::Path) -> Result<UserMenuConfig, MenuLoadError>` in `crates/cargonaut-config/src/lib.rs` — `NotFound` → empty config (no error); `io::Error` other than NotFound → `Err(MenuLoadError::Io(...))`; TOML parse failure → `Err(MenuLoadError::Parse(format!("{path}: {err}")))` including file name; add `MenuLoadError` enum with `Io(std::io::Error)` and `Parse(String)` variants; run `cargo test -p cargonaut-config` — T006 tests pass; commit `T006/T007(green): load_user_menu + MenuLoadError`

**Checkpoint**: `cargo test -p cargonaut-config` fully green. All foundational types available for US1 and US2.

---

## Phase 3: User Story 1 — Scrollable Help Overlay (F1) (Priority: P1) 🎯 MVP

**Goal**: Replace the single-dismiss minimal F1 overlay with a full-screen, multi-section, scrollable keybinding reference that stays open until explicitly dismissed.

**Independent Test**: Press F1 → overlay opens with named sections and scroll indicators → Down/PageDown scrolls content → Home returns to top → Esc closes → underlying pane state is unchanged.

### Tests — US1 (write first, must FAIL before implementation)

- [ ] T008 (red) [US1] Write failing unit tests for `HelpSection` / `HELP_SECTIONS` in `crates/cargonaut-ui-tui/src/dialog.rs` — test: `HELP_SECTIONS` is non-empty; each section has a non-empty title; each section has at least one row; every row has non-empty key and desc; commit `T008(red): failing structural tests for HELP_SECTIONS`

- [ ] T009 (red) [US1] Write failing unit tests for `HelpOverlay` key handling in `crates/cargonaut-ui-tui/src/dialog.rs` — store `visible_height: u16` in the `HelpOverlay` struct (set at construction, e.g., 20 for tests); tests: `handle_key(Down)` increments scroll_offset when not at bottom; `handle_key(Up)` decrements when not at top, clamps at 0; `handle_key(Home)` resets to 0; `handle_key(End)` sets to `total_lines - visible_height`; `handle_key(F1)` / `handle_key(Esc)` returns `HelpAction::Close`; `handle_key(PageDown)` increments by `visible_height`; any other key returns `HelpAction::Swallow` (not `Close`); **`handle_key` takes no extra parameter** — `visible_height` is read from `self`; commit `T009(red): failing tests for HelpOverlay::handle_key`

- [ ] T010 (red) [US1] Write failing unit test `help_covers_all_keymap_bindings` in `crates/cargonaut-ui-tui/src/lib.rs` — parse `design/contracts/keymap.toml` at test time; for each `action` field value, assert that at least one row in `HELP_SECTIONS` has a `desc` or `key` field containing the action string (case-insensitive); commit `T010(red): failing SC-002 keymap coverage test`

### Implementation — US1

- [ ] T011 [P] [US1] Add `HelpRow` (`key: &'static str`, `desc: &'static str`) and `HelpSection` (`title: &'static str`, `rows: &'static [HelpRow]`) types to `crates/cargonaut-ui-tui/src/dialog.rs`; add `pub use` in the module to expose them to `lib.rs`

- [ ] T012 [P] [US1] Define `pub static HELP_SECTIONS: &[HelpSection]` in `crates/cargonaut-ui-tui/src/dialog.rs` with complete content covering all sections — Navigation, File Operations, Selection, Panels & Modes, Bookmarks, File Attributes, Transfers & Jobs, Theme & Config, About; **omit the "User Menu (F2)" section for now** (it will be added in T031 after US2 is complete, to avoid a placeholder that would mislead early-US1 readers); each section must include all live bindings from `design/contracts/keymap.toml` EXCEPT `show-user-menu` (covered by T031); run `cargo test -p cargonaut-ui-tui` — T008 tests pass; commit `T008/T011/T012(green): HelpSection types + HELP_SECTIONS data`

- [ ] T013 [P] [US1] Add `HelpAction { Close, Swallow }` enum and `HelpOverlay { scroll_offset: u16, total_lines: u16, visible_height: u16 }` struct with `new(visible_height: u16)` constructor (computes `total_lines` from `HELP_SECTIONS`, stores `visible_height`) to `crates/cargonaut-ui-tui/src/dialog.rs`; `visible_height` is set from the rendered area height at open time (or a default when the overlay is first constructed); update constructor in `dispatch_ui_command` call site to pass the current frame area height

- [ ] T014 [US1] Implement `HelpOverlay::handle_key(&mut self, code: KeyCode) -> HelpAction` in `crates/cargonaut-ui-tui/src/dialog.rs` — Up/Down (1 line), PageUp/PageDown (`self.visible_height` lines), Home (→0), End (→`total_lines.saturating_sub(visible_height)`), F1/Esc → `Close`, all others → `Swallow`; clamp `scroll_offset` to `[0, total_lines.saturating_sub(visible_height)]`; run `cargo test -p cargonaut-ui-tui` — T009 tests pass; commit `T009/T013/T014(green): HelpOverlay struct + handle_key`

- [ ] T015 [US1] Implement `HelpOverlay::render(&self, f: &mut Frame, area: Rect, theme: &Theme)` in `crates/cargonaut-ui-tui/src/dialog.rs` — clears the full `area` with `Clear`; renders a `Block::default().title("Help — Cargonaut").borders(Borders::ALL).style(theme.dialog_style())`; renders section titles as bold lines and `(key, desc)` pairs as indented rows into a `Text`; wraps in `Paragraph::new(text).scroll((self.scroll_offset, 0)).block(block)`; adds a right-aligned `[N/M]` line indicator in the title or as a right-aligned span; uses `theme.dialog_style()` throughout — no hardcoded ANSI

- [ ] T016 [US1] In `crates/cargonaut-ui-tui/src/lib.rs` — replace `help_open: bool` in `UiState` with `help_overlay: Option<HelpOverlay>`; update `fresh_ui()` in tests; update the `Command::ShowHelp` dispatch branch to `ui.help_overlay = Some(HelpOverlay::new())`; update the `if ui.help_open` key-swallow block to call `overlay.handle_key(key.code)` and close on `HelpAction::Close`; update `draw_frame` call to pass `ui.help_overlay.as_ref()`; update `if help_open { draw_help(...) }` in the draw fn to call `overlay.render(...)`; remove the old `HELP_BODY: &str` const and `draw_help` fn; migrate the existing HELP_BODY unit tests (hotlist, recursive keys, attribute keys, mouse toggle) to assert against `HELP_SECTIONS` instead

- [ ] T017 [US1] Run `cargo test -p cargonaut-ui-tui` — T010 `help_covers_all_keymap_bindings` test must now pass (all keymap actions present in HELP_SECTIONS); fix any missing entries until the test passes; commit `T010/T015/T016/T017(green): HelpOverlay wired into UI, all keymap bindings covered`

**Checkpoint**: `cargo test -p cargonaut-ui-tui` green. F1 opens a scrollable, multi-section overlay. Launch `./target/debug/cargonaut`, press F1, scroll with Down/PageDown, dismiss with Esc — verify vs. quickstart.md VS-1 and VS-2.

---

## Phase 4: User Story 2 — User Action Menu (F2) (Priority: P1)

**Goal**: F2 opens a live modal menu loaded from `~/.config/cargonaut/menu.toml`; selected actions run asynchronously with the highlighted entry's path safely substituted.

**Independent Test**: Create a `menu.toml` with one action containing `{path}`; press F2 on a highlighted file; select the action; verify the command ran with the correct shell-quoted path. Missing/broken `menu.toml` shows graceful fallback without crash.

### Tests — US2 (write first, must FAIL before implementation)

- [ ] T018 (red) [US2] Write failing unit tests for `UserMenuDialog` in `crates/cargonaut-ui-tui/src/dialog.rs` — test: `new(items)` selects row 0 when non-empty; `new([])` has no selection; `handle_key(Down)` moves selection; `handle_key(Up)` clamps at 0; `handle_key(Esc)` returns `UserMenuAction::Close`; `handle_key(Enter)` on item 0 returns `UserMenuAction::Execute(0)`; pressing shortcut char `'e'` for an item returns `UserMenuAction::Execute(idx)`; `new_error("msg")` constructor sets `error = Some("msg")`; commit `T018(red): failing tests for UserMenuDialog`

- [ ] T019 (red) [US2] Write failing unit tests for `build_action_command()` in `crates/cargonaut-ui-tui/src/lib.rs` — test: command `"echo {path}"` + path `/tmp/a` → `Command::new("echo").arg("/tmp/a")`-equivalent (check argv vec); command `"cat {path} | wc"` → falls through to `sh -c` with quoted path; path with spaces `/tmp/my file` is properly quoted so shell sees it as one arg; `{path}` absent → command runs as-is; commit `T019(red): failing tests for build_action_command`

### Implementation — US2

- [ ] T020 [P] [US2] Add `UserMenuAction { Close, Execute(usize) }` enum and `UserMenuDialog { items: Vec<cargonaut_config::MenuItem>, state: ListState, error: Option<String> }` struct with `new(items)` and `new_error(msg)` constructors to `crates/cargonaut-ui-tui/src/dialog.rs`

- [ ] T021 [US2] Implement `UserMenuDialog::handle_key(&mut self, code: KeyCode) -> Option<UserMenuAction>` in `crates/cargonaut-ui-tui/src/dialog.rs` — Esc → `Close`; Down/Up navigate the list; Enter → `Execute(focused_index)`; `KeyCode::Char(c)` checks items for matching `item.key == Some(c)` and returns `Execute(idx)`; navigation returns `None`; run `cargo test -p cargonaut-ui-tui` — T018 tests pass; commit `T018/T020/T021(green): UserMenuDialog struct + handle_key`

- [ ] T022 [US2] Implement `UserMenuDialog::render(&mut self, f: &mut Frame, area: Rect, theme: &Theme)` in `crates/cargonaut-ui-tui/src/dialog.rs` — `centered_rect(50, 60, area)`; `Clear` first; `Block` with title `"User Menu (F2)"` and `theme.dialog_style()`; if `error` is `Some`: render a `Paragraph` with the error text in a warning style + "Esc to close" hint; if `items` is empty: render placeholder row `"No actions defined — see ~/.config/cargonaut/menu.toml"`; otherwise render a `List` of items with `item.label` + right-aligned `item.key` hint, using `theme.dialog_style()` for normal rows and highlight style for focused row

- [ ] T023 [P] [US2] Implement `build_action_command(command: &str, path: &std::path::Path) -> (String, Vec<String>)` free function in `crates/cargonaut-ui-tui/src/lib.rs` — use `shell_words::quote(path_str)` to produce the quoted path; substitute `{path}` in `command` with the quoted path string; check for shell metacharacters (`|`, `;`, `&&`, `||`, `$`, `` ` ``, `>`, `<`); if none found: `shell_words::split(substituted)` → `(tokens[0].clone(), tokens[1..].to_vec())`; if found: `("sh".into(), vec!["-c".into(), substituted])`; run `cargo test -p cargonaut-ui-tui` — T019 tests pass; commit `T019/T023(green): build_action_command`

- [ ] T024 [US2] Implement `evaluate_only_if(condition: &str, path: &std::path::Path) -> bool` async fn in `crates/cargonaut-ui-tui/src/lib.rs` — substitute `{path}` in condition with `shell_words::quote(path_str)`; spawn `tokio::task::spawn_blocking(|| std::process::Command::new("sh").arg("-c").arg(cond).status())`; wrap with `tokio::time::timeout(Duration::from_millis(200), ...)`; return `true` only if the future resolves to `Ok(Ok(status))` with `status.success()`; timeout or error → `false`

- [ ] T025 [US2] Add `ActiveDialog::UserMenu { widget: UserMenuDialog }` variant to the `ActiveDialog` enum in `crates/cargonaut-ui-tui/src/lib.rs`

- [ ] T026 [US2] Wire `Command::ShowUserMenu` in `dispatch_ui_command` in `crates/cargonaut-ui-tui/src/lib.rs` — guard with `if active_dialog.is_some() { return Ok(()); }` (FR-021); call `menu_config_path()` from `cargonaut_config`; call `load_user_menu(&path)` — on parse error build `UserMenuDialog::new_error(msg)` directly; on ok: for each item with `only_if = Some(cond)`, call `evaluate_only_if(&cond, &active_path).await`, filter out hidden items; build `UserMenuDialog::new(visible_items)`; set `*active_dialog = Some(ActiveDialog::UserMenu { widget })`; set `*mode = Mode::Dialog`

- [ ] T027 [US2] Handle `ActiveDialog::UserMenu { widget }` in the key-dispatch block in `crates/cargonaut-ui-tui/src/lib.rs` — call `widget.handle_key(key.code)`; on `UserMenuAction::Close`: clear active_dialog, reset mode; on `UserMenuAction::Execute(idx)`: get item, get active path from app, close dialog, spawn `tokio::task::spawn_blocking` running `build_action_command`-derived `Command`; on completion update `*status` with `"Done."` or `"[exit N] <stderr_first_line>"`; **also handle `KeyCode::F(1)` in `UserMenuDialog::handle_key` as `UserMenuAction::Close`** — pressing F1 while F2 is open closes F2 (does NOT simultaneously open F1, per spec Edge Cases); the user can press F1 again after dismissal

- [ ] T028 [US2] Add `ActiveDialog::UserMenu { widget }` branch to the `draw_frame` function render path in `crates/cargonaut-ui-tui/src/lib.rs` (alongside other `ActiveDialog` variants in the match block) — call `widget.render(f, area, theme)`

- [ ] T029 [US2] Run `cargo test -p cargonaut-ui-tui` — all tests pass; then launch `./target/debug/cargonaut` and test VS-3 through VS-9 from quickstart.md manually; commit `T025/T026/T027/T028/T029(green): ShowUserMenu fully wired`

**Checkpoint**: F2 opens a live menu from `menu.toml`. A path with spaces is passed safely. An invalid `menu.toml` shows an error, not a crash. F2 while another dialog is open does nothing.

---

## Phase 5: User Story 3 — Config Format Documentation (Priority: P2)

**Goal**: `examples/menu.toml` exists and parses cleanly; F1 overlay includes a section describing F2 and the config path.

**Independent Test**: `load_user_menu(Path::new("examples/menu.toml"))` returns `Ok` with ≥3 items. The F1 overlay shows a section mentioning "F2" and "menu.toml".

- [ ] T030 [P] [US3] Create `examples/menu.toml` with ≥5 commented example actions demonstrating: label+command only; label+command+key; label+command+only_if; a command using shell operators (pipe); a command without `{path}`; validate it parses cleanly by running a quick `cargo test -p cargonaut-config -- load_user_menu` test pointing at this file path

- [ ] T031 [US3] Ensure `HELP_SECTIONS` in `crates/cargonaut-ui-tui/src/dialog.rs` includes a "User Menu (F2)" section that mentions: F2 key, `~/.config/cargonaut/menu.toml` path, `{path}` placeholder, pointer to `examples/menu.toml`; run the `help_covers_all_keymap_bindings` test to confirm `show-user-menu` action is covered; confirm by pressing F1 in the running app and scrolling to the F2 section

**Checkpoint**: `cargo test --workspace` green. `examples/menu.toml` exists and parses. F1 help mentions F2.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: CI gate validation, documentation updates, binary size check.

- [ ] T032 **[G1 — SC-001/SC-003 CI bench]** Extend `crates/cargonaut-ui-tui/benches/keypress_latency.rs` with two new criterion benchmarks: (a) `help_overlay_render_time` — construct a `HelpOverlay`, call `render()` into a `TestBackend` 80×24 frame, assert median <100 ms (satisfies SC-001 CI gate per constitution §II); (b) `build_action_command_latency` — call `build_action_command("echo {path}", Path::new("/tmp/test"))` in a tight loop, assert median <1 ms (sanity gate; actual launch latency validated in T036); commit bench as green; run `make bench` to confirm baseline

- [ ] T033 **[G2 — Mouse F2 deferral]** Open a GitHub issue: "Mouse support for F2 user menu overlay (deferred from Feature 047)" with problem statement (clicking items to select/activate), reason deferred (complexity of registering new overlay rect in handle_mouse; out of scope for this feature), suggested approach (add `UserMenu` variant to `ClickTarget` region map), effort estimate M, `follow-up` label; add a ROADMAP.md row in the appropriate tier referencing this issue; commit the ROADMAP.md update

- [ ] T034 Run `make ci-local` and fix any clippy warnings, test failures, or doc-link errors introduced by this feature; pay special attention to `#![warn(missing_docs)]` — all new public items in `cargonaut-config` and `cargonaut-ui-tui` must have doc comments

- [ ] T035 [P] Update `README.md` — increment feature count in the "At a Glance" table; add a one-line "Feature 047" entry to the Feature History section; update test count if it changed; update binary size from `scripts/check-binary-size.sh` output

- [ ] T036 [P] Update `Learnings.md` — append a `## Feature 047: User Menu (F2) + Scrollable Help (F1)` section with ≥3 bullet points covering: the `shell-words` tiered-execution decision, the `HELP_BODY → HELP_SECTIONS` migration, the `only_if` timeout strategy, and any surprises encountered during implementation

- [ ] T037 Run `scripts/check-binary-size.sh` after `make build` — verify the release binary did not grow by more than 32 KiB above the Feature 046 baseline (SC-007); record the new size in README.md

- [ ] T038 Run all quickstart.md validation scenarios VS-1 through VS-9 end-to-end in the running TUI; confirm each scenario passes; note any deviations

**Checkpoint**: `make ci-local` fully green. README.md and Learnings.md updated. Binary size within budget.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 — BLOCKS Phase 4 (US2 needs config types)
- **Phase 3 (US1 — F1 Help)**: Depends on Phase 1 only — can start in parallel with Phase 2
- **Phase 4 (US2 — F2 Menu)**: Depends on Phase 2 — cannot start until `MenuItem`/`load_user_menu` exist
- **Phase 5 (US3 — Docs)**: Depends on Phase 3 (HELP_SECTIONS must exist) and Phase 4 (F2 section content)
- **Phase 6 (Polish)**: Depends on all prior phases complete

### User Story Dependencies

- **US1 (F1 Help)**: Independent of US2 — can proceed after Phase 1
- **US2 (F2 Menu)**: Depends on Phase 2 types; independent of US1
- **US3 (Docs)**: Depends on US1 (HELP_SECTIONS) and US2 (examples/menu.toml)

### Within Each Phase — TDD Order

For every `(red)` task: write failing test → commit `T###(red)` → implement → commit `T###(green)`

### Parallel Opportunities

- T011, T012 (HelpSection types + data) can run in parallel
- T013 (HelpOverlay struct) can run in parallel with T011/T012
- T020 (UserMenuDialog struct) can run in parallel with T023 (build_action_command)
- T030 (examples/menu.toml) can run in parallel with T031 (HELP_SECTIONS F2 section)
- T033 (README update) and T034 (Learnings update) can run in parallel

---

## Parallel Example: User Story 1 (F1 Help)

```
# Parallel within Phase 3:
T011: HelpRow/HelpSection types in dialog.rs
T012: HELP_SECTIONS static data in dialog.rs
T013: HelpOverlay struct in dialog.rs

# Sequential after T011/T012/T013 complete:
T014 → T015 → T016 → T017
```

## Parallel Example: User Story 2 (F2 Menu)

```
# Parallel after Phase 2 complete:
T020: UserMenuDialog struct in dialog.rs
T023: build_action_command fn in lib.rs

# Sequential after T020:
T021 → T022 (handle_key → render)

# Sequential after T023:
T024 (evaluate_only_if)

# Sequential after T020/T021/T022/T023/T024:
T025 → T026 → T027 → T028 → T029
```

---

## Implementation Strategy

### MVP First (US1 only — F1 Scrollable Help)

1. Complete Phase 1: Setup (T001) — 5 min
2. Complete Phase 3: US1 — F1 Help (T008–T017) — estimated 2–3 hrs
3. **STOP and VALIDATE**: F1 opens, scrolls, all keymap coverage test passes
4. Ship MVP: scrollable help is independently valuable

### Incremental Delivery

1. Phase 1 (T001) → Phase 2 (T002–T007) → Phase 3 (T008–T017) → US1 done and tested
2. Phase 4 (T018–T029) → US2 done and tested independently
3. Phase 5 (T030–T031) → US3 done
4. Phase 6 (T032–T036) → CI green, docs updated, PR ready

---

## Notes

- **TDD is mandatory** (Constitution §II): every `(red)` task must be a separate commit that fails before its `(green)` partner
- `[P]` = different files or genuinely independent of in-progress tasks — safe to parallelize
- `shell_words::quote()` replaces `{path}` for FR-014 macro-safety compliance
- `centered_rect` and `theme.dialog_style()` are existing helpers in `lib.rs` — reuse them
- The `UiState` field rename (`help_open: bool` → `help_overlay: Option<HelpOverlay>`) will require updating `fresh_ui()` in tests and the `draw_frame` call site — do these atomically in T016
- `HELP_SECTIONS` is `'static` data; `total_lines` is computed once in `HelpOverlay::new()` by iterating it
- The `only_if` evaluation uses `tokio::time::timeout` + `spawn_blocking` — both already in scope via `tokio` workspace dep

# Tasks: Internal F4 Editor (Feature 056)

**Input**: Design documents from `specs/056-f4-internal-editor/`

**Branch**: `056-f4-internal-editor` | **Closes**: #40

**Constitution §II — TDD required**: Every FR has a red commit (failing test) before the green commit (implementation). Red/green commit pairs are noted on each task pair.

**Organization**: 3 user stories + foundational keymap additions. Stories are mostly sequential (US2 and US3 depend on US1 infrastructure).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story this task belongs to (US1, US2, US3)

---

## Phase 1: Setup

**Purpose**: No new files, no new crates, no new modules. All changes land in existing files. Branch already created. Skip to Phase 2.

---

## Phase 2: Foundational — Keymap Additions

**Purpose**: Add `Mode::Editor`, `Command::SaveFile`, `Command::EditorQuit` to `keymap.rs` and bind them in `keymap.toml`. These enum variants must exist before any `lib.rs` changes compile.

**⚠️ CRITICAL**: Phases 3–5 CANNOT start until T001–T003 are complete.

- [X] T001 Add `Mode::Editor` variant to the `Mode` enum in `crates/cargonaut-ui-tui/src/keymap.rs` (after `Preview`, with a doc comment "Internal text editor has focus.")
- [X] T002 Add `Command::SaveFile` and `Command::EditorQuit` variants to the `Command` enum in `crates/cargonaut-ui-tui/src/keymap.rs` (under a `// Editor` subsection; add doc comments)
- [X] T003 Add five editor-mode bindings to `design/contracts/keymap.toml` under a `# ── Internal editor (Feature 056) ──` section:
  - `mode="editor", key="F2", action="save-file"`
  - `mode="editor", key="C-s", action="save-file"`
  - `mode="editor", key="F10", action="editor-quit"`
  - `mode="editor", key="Esc", action="editor-quit"`
  - `mode="editor", key="q", action="editor-quit"`

**Checkpoint**: `cargo check -p cargonaut-ui-tui` passes; `parses_full_default_keymap_without_error` test passes with `cargo test -p cargonaut-ui-tui keymap`.

---

## Phase 3: User Story 1 — Edit and Save a Plain-Text File (Priority: P1) 🎯 MVP

**Goal**: F4 on a plain-text file opens a full-screen built-in editor. Arrow keys navigate, typing inserts text, F2/Ctrl-S saves to disk, F10/Esc/Q exits (cleanly — US2 guard not yet required here). Pane refreshes after close.

**Independent Test**: Open a text file with F4; type text; press F2; press F10; confirm `cat <file>` shows the edit. No external editor launched.

### Tests for User Story 1 (TDD — red commits before implementation)

> **Write these tests FIRST with `todo!()` bodies; ensure `cargo test` panics before implementing.**

- [X] T004 [US1] Add failing test stub `editor_insert_and_save_writes_correct_content` (body: `todo!()`) in the `#[cfg(test)] mod tests` block of `crates/cargonaut-ui-tui/src/dialog.rs` — **red commit: `T004 (red): editor insert+save test stub`**
- [X] T004b [US1] Add failing test stub `editor_utf8_roundtrip_no_edits` (body: `todo!()`) in same mod — verifies SC-005: open file, make no edits, save, assert file content unchanged — **red commit: `T004b (red): UTF-8 round-trip test stub`**
- [X] T004c [US1] Add failing test stub `editor_save_failure_keeps_dirty_and_shows_error` (body: `todo!()`) in same mod — verifies FR-005 edge case: save to a read-only path; assert `is_dirty()` still true and `status_msg` is Some — **red commit: `T004c (red): save-failure test stub`**
- [X] T005 [US1] Add failing test stub `editor_cursor_navigation_stays_in_bounds` (body: `todo!()`) in same mod — **red commit: `T005 (red): editor cursor bounds test stub`**
- [X] T006 [US1] Add failing test stub `editor_render_shows_modified_indicator` (body: `todo!()`) in same mod — **red commit: `T006 (red): editor modified-indicator render test stub`**

### Implementation for User Story 1

- [X] T007 [US1] Add `LineEnding` enum (`Lf`, `Crlf`) with `fn join(&self, lines: &[String]) -> String` helper to `crates/cargonaut-ui-tui/src/dialog.rs` (new section: `// Internal Editor (Feature 056)`) — **green for data-model LineEnding**
- [X] T008 [US1] Add `UnsavedChangesChoice` enum and `UnsavedChangesDialog` struct with `new()`, `handle_key()`, `render()` to `dialog.rs` — focus defaults to `2` (Cancel); Tab/Left/Right cycle; Enter confirms; Esc → Cancel; add `///` doc comments on all `pub` items (Constitution §I: `#![warn(missing_docs)]` is already active) — **green for FR-007**
- [X] T009 [US1] Add `FileEditorAction` enum (`Swallow`, `Close`, `UnsavedPromptShowing`, `SaveAndClose`, `DiscardAndClose`) to `dialog.rs` — **green for return-type contract**
- [X] T010 [US1] Add `FileEditorDialog` struct with fields: `path`, `display_name`, `lines: Vec<String>`, `cursor_line`, `cursor_col`, `scroll_offset`, `dirty`, `line_ending`, `unsaved_dlg: Option<UnsavedChangesDialog>`, `status_msg: Option<String>` — and `new(path, display_name, content: String, line_ending: LineEnding) -> Self` constructor that splits content on `\n` (stripping trailing `\r`) into `self.lines`; add `///` doc comments on all `pub` struct fields and `impl` methods (Constitution §I) — to `dialog.rs` — **green for FR-001 data model**
- [X] T011 [US1] Implement private cursor/scroll helpers on `FileEditorDialog` in `dialog.rs`: `clamp_cursor()`, `scroll_to_cursor(viewport_height: u16)`, `insert_char(ch: char)`, `delete_left()`, `delete_right()`, `split_line()`, `move_up()`, `move_down()`, `move_left()`, `move_right()`, `goto_start()` (cursor_line=0, cursor_col=0), `goto_end()` (cursor_line=lines.len()-1, cursor_col=last_line.len()); also add dispatch in `handle_key` for `KeyCode::Home` with `Ctrl` modifier → `goto_start()`, `KeyCode::End` with `Ctrl` → `goto_end()` — **green for FR-003/FR-004 navigation and editing (including Ctrl-Home/Ctrl-End per FR-003)**
- [X] T012 [US1] Implement `pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> FileEditorAction` on `FileEditorDialog` in `dialog.rs`: if `unsaved_dlg` is Some → route to sub-modal; otherwise dispatch raw navigation/editing keys (arrow, home, end, pgup, pgdn, printable char, backspace, delete, enter); `SaveFile`/`EditorQuit` commands are dispatched by the lib.rs caller before calling this method — **green for FR-003/FR-004**
- [X] T013 [US1] Implement `pub fn save(&mut self) -> std::io::Result<()>` on `FileEditorDialog` in `dialog.rs`: join `self.lines` using `self.line_ending.join()`, write via `std::fs::write(&self.path, content)`, clear `self.dirty` on success — **green for FR-005**
- [X] T014 [US1] Implement `pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme)` on `FileEditorDialog` in `dialog.rs`: 3-row layout (header with `*` if dirty, content area with cursor cell `Modifier::REVERSED`, footer with `Ln N, Col N` and `F2=Save  F10=Quit`); if `unsaved_dlg` is Some, overlay it centered over the content area — **green for FR-001/FR-002**
- [X] T015 [US1] Implement `pub fn is_dirty(&self) -> bool` accessor on `FileEditorDialog` in `dialog.rs` — **green trivial**
- [X] T016 [US1] Add `ActiveDialog::FileEditor { widget: dialog::FileEditorDialog }` variant to the `ActiveDialog` enum in `crates/cargonaut-ui-tui/src/lib.rs` — **green for dispatch infrastructure**
- [X] T017 [US1] Add `open_file_editor(raw_path: PathBuf, display_name: String) -> Result<dialog::FileEditorDialog, OpenEditorError>` async fn and `OpenEditorError` enum (`Binary`, `TooLarge { size_bytes: u64 }`, `Io(std::io::Error)`) to `lib.rs`: resolve symlink → read sample → UTF-8 check → size check → read full file → detect line ending (presence of `\r\n` in first 4 KiB → Crlf, else Lf) → `FileEditorDialog::new()` — **green for FR-001/FR-008 open path**
- [X] T018 [US1] Replace the `Command::Edit` arm in `handle_ui_event()` in `lib.rs` (currently calls `queue_external`): resolve focused file path (same as F3 viewer path resolution); call `open_file_editor().await`; on `Ok(widget)` → `*active_dialog = Some(ActiveDialog::FileEditor { widget }); *mode = Mode::Editor;`; on `Err(OpenEditorError::Binary)` → status "Cannot edit: binary file"; on `Err(OpenEditorError::TooLarge { size_bytes })` → status "Cannot edit: file too large (>10 MiB)"; on `Err(OpenEditorError::Io(e))` → status format — **green for FR-001/FR-008/FR-010**
- [X] T019 [US1] Add `ActiveDialog::FileEditor { widget }` key-dispatch arm in `handle_ui_event()` in `lib.rs` (after the FileViewer arm, mirroring its structure exactly): (1) accumulate chord into `chord_buf`; (2) keymap lookup via `keymap.lookup_sequence(Mode::Editor, chord_buf)` — if `SeqLookup::Command(cmd)`: clear chord_buf, then match: `Command::SaveFile` → call `widget.save()`, on error set `widget.status_msg` and keep editor open; `Command::EditorQuit` + `widget.is_dirty()` → `widget.handle_key(EditorQuit event)` which returns `UnsavedPromptShowing`; `Command::EditorQuit` + clean → `FileEditorAction::Close`; if `SeqLookup::Pending` → show chord in status; if `SeqLookup::NoMatch` → clear chord_buf, call `widget.handle_key(raw_key_event)`; (3) dispatch `FileEditorAction` return value: `Close`/`DiscardAndClose` → `*active_dialog = None; *mode = Mode::Pane; refresh_active_pane(app, ui)?`; `SaveAndClose` → `widget.save()` then close + refresh; `UnsavedPromptShowing`/`Swallow` → no-op — **green for FR-001/FR-005/FR-006/FR-009**
- [X] T020 [US1] Add `ActiveDialog::FileEditor { widget }` render arm in `draw_frame()` in `lib.rs`: call `widget.render(f, area, theme)` where `area` is the full terminal area — **green for FR-001 full-screen**
- [X] T021 [US1] Delete `ExternalTool` enum and `queue_external` fn from `lib.rs` (now dead code) — **green for FR-010**
- [X] T022 [US1] Implement `editor_insert_and_save_writes_correct_content` test body in `dialog.rs`: construct `FileEditorDialog::new(tmp_path, "test.txt", "hello".into(), LineEnding::Lf)`; call `insert_char('!')` 5× via public API then `save()`; assert `std::fs::read_to_string(tmp_path)` == `"hello!!!!!"` — **green commit: `T022 (green): editor insert+save test`**
- [X] T022b [US1] Implement `editor_utf8_roundtrip_no_edits` test body in `dialog.rs` (SC-005): write `"hello\nworld\n"` to a temp file; construct dialog from that content; call `save()` without edits; assert file content == `"hello\nworld\n"` — **green commit: `T022b (green): UTF-8 round-trip test`**
- [X] T022c [US1] Implement `editor_save_failure_keeps_dirty_and_shows_error` test body in `dialog.rs` (FR-005 write-failure edge case): construct dialog with `path` pointing to a non-existent read-only directory child (e.g. `/root/no_permission_test.txt`); call `insert_char('x')`; call `save()` directly and assert it returns `Err`; assert `is_dirty()` still true; in the lib.rs dispatch arm test: set `widget.status_msg` on save error — test indirectly via the dialog's `save()` return value — **green commit: `T022c (green): save-failure keeps dirty test`**
- [X] T023 [US1] Implement `editor_cursor_navigation_stays_in_bounds` test body in `dialog.rs`: construct single-line dialog; call `move_right()` 999× and `move_left()` 999× via `handle_key`; assert cursor never exceeds line length — **green commit: `T023 (green): cursor bounds test`**
- [X] T024 [US1] Implement `editor_render_shows_modified_indicator` test body in `dialog.rs`: construct dialog, render into `Buffer::empty(Rect { ... })`, assert `*` is absent; call `insert_char('x')`, render again, assert `*` is present in header area — **green commit: `T024 (green): modified indicator render test`**

**Checkpoint**: `cargo test -p cargonaut-ui-tui -- editor` passes. Manual: `make run`, F4 on a text file, type, F2 saves, F10 exits, `cat` confirms change on disk.

---

## Phase 4: User Story 2 — Unsaved-Changes Guard on Exit (Priority: P2)

**Goal**: F10/Esc/Q with unsaved changes shows a modal prompt (Save / Discard / Cancel). Discard exits without saving; Save saves then exits; Cancel returns to editing.

**Independent Test**: Open editor, type a char, press F10 → dialog appears. Press Tab→Discard→Enter → editor closes; file unchanged on disk.

### Tests for User Story 2 (TDD — red commits before implementation)

- [X] T025 [US2] Add failing test stub `editor_unsaved_changes_guard_triggered_on_quit` (body: `todo!()`) in `dialog.rs` mod tests — **red commit: `T025 (red): unsaved guard test stub`**
- [X] T026 [US2] Add failing test stub `editor_discard_does_not_save` (body: `todo!()`) in `dialog.rs` mod tests — **red commit: `T026 (red): discard-no-save test stub`**
- [X] T027 [US2] Add failing test stub `unsaved_dialog_cancel_resumes_editing` (body: `todo!()`) in `dialog.rs` mod tests — **red commit: `T027 (red): cancel-resume test stub`**

### Implementation for User Story 2

> **Note**: `UnsavedChangesDialog` struct is created in T008 (Phase 3). This phase completes its integration and verifies via green tests.

- [X] T028 [US2] Wire `UnsavedChangesDialog` into `FileEditorDialog::handle_key` `EditorQuit` path: if `is_dirty()` → `self.unsaved_dlg = Some(UnsavedChangesDialog::new()); FileEditorAction::UnsavedPromptShowing`; if `unsaved_dlg.is_some()` and key resolves to `UnsavedChangesChoice::Save` → `FileEditorAction::SaveAndClose`; `Discard` → `FileEditorAction::DiscardAndClose`; `Cancel` → `self.unsaved_dlg = None; FileEditorAction::Swallow` — in `crates/cargonaut-ui-tui/src/dialog.rs` — **green for FR-006/FR-007**
- [X] T029 [US2] Implement `editor_unsaved_changes_guard_triggered_on_quit` test body in `dialog.rs`: construct dirty dialog (after `insert_char`), call `handle_key(EditorQuit key event)`, assert result is `UnsavedPromptShowing`, assert `dialog.unsaved_dlg.is_some()` — **green commit: `T029 (green): unsaved guard triggered`**
- [X] T030 [US2] Implement `editor_discard_does_not_save` test body in `dialog.rs`: open dialog with temp file, insert char, trigger quit (→ guard shows), send Discard key, assert `FileEditorAction::DiscardAndClose`, assert file on disk is unchanged — **green commit: `T030 (green): discard leaves file unchanged`**
- [X] T031 [US2] Implement `unsaved_dialog_cancel_resumes_editing` test body in `dialog.rs`: trigger guard, send Cancel key (Esc), assert `Swallow` action, assert `unsaved_dlg` is None, assert `is_dirty()` still true — **green commit: `T031 (green): cancel resumes editing`**

**Checkpoint**: `cargo test -p cargonaut-ui-tui -- editor` passes. Manual: F10 with unsaved changes → 3-choice dialog → Tab to Discard → Enter → file unchanged.

---

## Phase 5: User Story 3 — Safety Limits for Uneditable Files (Priority: P3)

**Goal**: F4 on binary file shows "Cannot edit: binary file"; F4 on file >10 MiB shows "Cannot edit: file too large (>10 MiB)"; F4 on directory does nothing. No editor widget is created.

**Independent Test**: Press F4 on `/usr/bin/ls` → status message visible; F4 on an 11 MiB text file → status message visible.

### Tests for User Story 3 (TDD — red commits before implementation)

- [X] T032 [US3] Add failing test stub `open_file_editor_declines_binary` (body: `todo!()`) in the `#[cfg(test)] mod tests` block of `lib.rs` — **red commit: `T032 (red): binary decline test stub`**
- [X] T033 [US3] Add failing test stub `open_file_editor_declines_too_large` (body: `todo!()`) in the same mod — **red commit: `T033 (red): large file decline test stub`**

### Implementation for User Story 3

> **Note**: The binary/size checks in `open_file_editor()` are wired in T017 (Phase 3). This phase adds the green tests that validate those code paths.

- [X] T034 [US3] Implement `open_file_editor_declines_binary` test body in `lib.rs` tests: write a temp file containing null bytes; call `open_file_editor()` in a `tokio::test`; assert `Err(OpenEditorError::Binary)` — **green commit: `T034 (green): binary decline test`**
- [X] T035 [US3] Implement `open_file_editor_declines_too_large` test body in `lib.rs` tests: write a temp file of `STREAMING_THRESHOLD_BYTES + 1` bytes (all `b'a'`); call `open_file_editor()`; assert `Err(OpenEditorError::TooLarge { .. })` — **green commit: `T035 (green): large file decline test`**

**Checkpoint**: `cargo test -p cargonaut-ui-tui -- open_file_editor` passes. Manual: F4 on binary → status message only; F4 on >10 MiB → status message only.

---

## Phase 6: Polish & Documentation

**Purpose**: Mandatory docs (CLAUDE.md: README.md + Learnings.md on every feature merge) + final CI gate.

- [X] T036 [P] Update `README.md`: increment test count in "At a Glance" table; add one-line entry in "Feature History" for Feature 056
- [X] T037 [P] Update `Learnings.md`: append Feature 056 section (minimum 3 bullets covering: Vec<String> vs rope rationale, Mode::Editor isolation from Preview mode, UnsavedChangesDialog 3-choice design, synchronous save rationale for ≤10 MiB, ExternalTool deletion)
- [X] T038 Run `make ci-local` and confirm all steps pass (clippy, test, build, check-pr-body, docs-gate); additionally run `cargo bench -p cargonaut-ui-tui keypress_latency` and verify the median latency is still ≤16 ms (SC-001/SC-002 gate — the editor render path in draw_frame is now exercised by this bench)

> **PR note**: Reference "Closes #40" in the PR description body.

**Checkpoint**: `make ci-local` passes all steps. PR ready to merge.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 2 (Foundational)**: No dependencies — start immediately
- **Phase 3 (US1)**: Depends on Phase 2 (T001–T003) — `Mode::Editor` and `Command::*` must exist before lib.rs changes compile
- **Phase 4 (US2)**: Depends on Phase 3 (T008–T015) — `UnsavedChangesDialog` is created in Phase 3; Phase 4 completes its wiring and adds green tests
- **Phase 5 (US3)**: Depends on Phase 3 (T017) — `open_file_editor()` must exist before decline tests can reference it
- **Phase 6 (Polish)**: Depends on Phases 3–5 being code-complete

### Within User Story 1

```
T004–T006 (red stubs) ──► T007–T015 (implementation) ──► T016–T021 (lib.rs wiring) ──► T022–T024 (green tests)
```

T007 through T015 (dialog.rs additions) can be committed in any order — they are in the same file but distinct sections. T016–T021 (lib.rs changes) must follow T007–T015 to compile.

### Within User Story 2

```
T025–T027 (red stubs) ──► T028 (wiring) ──► T029–T031 (green tests)
```

### Within User Story 3

```
T032–T033 (red stubs) ──► T034–T035 (green tests)
```

T034 and T035 are independent (different test functions, both async); can be committed in either order.

### Parallel Opportunities

- T036 and T037 (docs in different files) can be done simultaneously
- T004, T005, T006 red stubs are in the same file but different test functions; write sequentially
- T032, T033 red stubs: same

---

## Implementation Strategy

### MVP (User Story 1 Only)

1. T001–T003: Keymap additions
2. T004–T006: Red test stubs
3. T007–T015: `dialog.rs` widget implementation
4. T016–T021: `lib.rs` wiring + dead code deletion
5. T022–T024: Green tests
6. **STOP and VALIDATE**: `cargo test -p cargonaut-ui-tui -- editor` passes; manual F4 edit+save works

### Incremental Delivery

1. MVP (US1) — file editing works, exits cleanly without save guard
2. US2 — unsaved-changes guard added; no accidental data loss
3. US3 — binary/large file protection; no garbled output
4. Polish — docs gate passes, PR ready

---

## Notes

- All `lib.rs` changes are in existing functions (`handle_ui_event`, `draw_frame`, `open_file_editor` is new)
- `ExternalTool` and `queue_external` are deleted in T021 — no callers remain after T018
- The `save()` method is synchronous (no thread); safe for ≤10 MiB per R-009
- Tab character display (4 spaces visually) is purely a render concern — `insert_char('\t')` stores the literal byte; `render()` expands it
- `unsafe` is not needed anywhere (Constitution §I satisfied)
- Cursor rendering: cell at `(cursor_col, cursor_line - scroll_offset)` gets `Modifier::REVERSED` (same technique as `render_vt100_screen` in `subshell.rs`)

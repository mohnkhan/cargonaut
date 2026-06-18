# Tasks: Bulk Rename via Editor + Undo of File Operations

**Input**: Design documents from `specs/050-bulk-rename-undo/`

**Prerequisites**: plan.md ✓ spec.md ✓ research.md ✓ data-model.md ✓ contracts/ ✓

**TDD**: Constitution §II requires failing tests committed before implementation. Each red→green pair follows this discipline.

**Organization**: Tasks are grouped by phase and user story. US1 (bulk rename) is the MVP and must be fully working before US2 (undo) begins.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies on incomplete tasks in the same phase)
- **[Story]**: Which user story this task belongs to (US1 / US2)
- Exact file paths are included in all descriptions

---

## Phase 1: Setup — Data Model + Command Types

**Purpose**: Add all new types and enum variants that every subsequent phase depends on. No behavior yet — stubs only.

- [X] T001 Add `UndoEntry` enum (`Rename`, `Copy`, `Move`, `Delete` variants) to `crates/cargonaut-core/src/lib.rs` before the `App` struct definition
- [X] T002 Add `undo_log: Option<UndoEntry>` field to `App` struct and initialise it to `None` in `App::new()` in `crates/cargonaut-core/src/lib.rs`
- [X] T003 Add `Command::BulkRenameApply(Vec<(String, String)>)` and `Command::UndoLastOp` variants to `Command` enum with stub dispatch arms (`BulkRenameApply(_) => Ok(vec![])` and `UndoLastOp => Ok(vec![])`) in `crates/cargonaut-core/src/lib.rs`
- [X] T004 [P] Add `PendingExternalKind` enum (`FileOpen` and `BulkRename { temp_path: PathBuf, original_names: Vec<String> }` variants) to `crates/cargonaut-ui-tui/src/lib.rs`; add `kind: PendingExternalKind` field to `PendingExternal`; update `queue_external()` and `queue_diff()` to set `kind: PendingExternalKind::FileOpen`; update `run_loop` post-action block to match on `ext.kind` (FileOpen arm = existing behavior) in `crates/cargonaut-ui-tui/src/lib.rs`

> T004 touches a different crate from T001–T003 and can be worked in parallel. T001 must precede T002 must precede T003 (same file, sequential).

**Checkpoint**: `cargo check --workspace` passes with no errors.

---

## Phase 2: Foundational — Rename Validation (Pure Logic)

**Purpose**: The `validate_rename_proposals()` function is a pure, filesystem-free function used by US1. It must be complete and tested before `apply_bulk_rename()` is built.

- [X] T005 (red) Write 7 failing unit tests for `validate_rename_proposals()` covering: all-unchanged (returns empty vec), 2-of-3 changed (correct output pairs), line-count mismatch, empty name on any line, name containing `/`, duplicate proposed names, and correct output pairs ordering — in the `#[cfg(test)]` module of `crates/cargonaut-core/src/lib.rs`
- [X] T006 (green) Implement `pub(crate) fn validate_rename_proposals(originals: &[String], edited: &[String]) -> Result<Vec<(String, String)>, String>` in `crates/cargonaut-core/src/lib.rs` to pass all T005 tests

> T005 must fail (`cargo test` reports these tests fail) before T006 is committed. Commit T005 with message `T005 (red): validate_rename_proposals — 7 failing tests`.

**Checkpoint**: `cargo test -p cargonaut-core -- validate_rename_proposals` passes (all 7 tests green).

---

## Phase 3: User Story 1 — Bulk Rename via Editor (Priority: P1) 🎯 MVP

**Goal**: Tag one or more files, press `C-x r`, edit names in `$EDITOR`, validated renames are applied atomically.

**Independent Test** (from quickstart.md Scenario 1): Tag 3 files, rename 2 of them in the editor, confirm only those 2 are renamed on disk and the temp file is deleted.

### Core: apply_bulk_rename

- [X] T007 [US1] (red) Write 7 failing tests for `App::apply_bulk_rename()` covering: no-change no-op returns "No changes — nothing renamed", 2-of-3 renamed on disk (third unchanged), collision with existing non-tagged entry → no renames applied, requires file:// scheme (non-file pane returns status error), records `UndoEntry::Rename` on success with pairs reversed, partial failure mid-batch records partial undo entry for completed renames, temp file absent from disk after validation failure (SC-005 failure path) — in `#[cfg(test)]` module of `crates/cargonaut-core/src/lib.rs`
- [X] T008 [US1] (green) Implement `pub async fn apply_bulk_rename(&mut self, pairs: Vec<(String, String)>) -> Result<Vec<Event>, AppError>` in `crates/cargonaut-core/src/lib.rs`: call `validate_rename_proposals()`, perform collision check against active pane directory listing, call `std::fs::rename()` for each changed pair, record `UndoEntry::Rename { pairs: reversed }` on success, refresh active pane, return `[PaneUpdated(active), Status("N entries renamed")]`
- [X] T009 [US1] Replace stub dispatch arm for `BulkRenameApply(pairs)` with `self.apply_bulk_rename(pairs).await` in `App::dispatch()` in `crates/cargonaut-core/src/lib.rs`

### UI: queue_bulk_rename + editor launch

- [X] T010 [P] [US1] (red) Write 4 failing unit tests for `queue_bulk_rename()` covering: no tagged entries → status `"Tag at least one entry to bulk rename"`, tagged entries → `pending_external` is `Some`, `pending_external.kind` is `PendingExternalKind::BulkRename`, `original_names` match the tagged entry basenames in listing order — in `#[cfg(test)]` module of `crates/cargonaut-ui-tui/src/lib.rs`
- [X] T011 [US1] (green) Implement `fn queue_bulk_rename(app: &App, ui: &mut UiState, status: &mut String)` in `crates/cargonaut-ui-tui/src/lib.rs`: collect tagged entry basenames from active pane in listing order, warn and exclude any entry with `\n` in name, write to a temp file in `std::env::temp_dir()` with name `cargonaut-rename-<pid>-<counter>.txt`, get `$EDITOR` (fallback `"vi"`), set `ui.pending_external = Some(PendingExternal { program: editor, args: vec![temp_path_str.clone()], kind: PendingExternalKind::BulkRename { temp_path, original_names } })`
- [X] T012 [US1] Handle `Command::BulkRenameViaEditor` in `dispatch_ui_command()` by calling `queue_bulk_rename(app, ui, status); return Ok(());` in `crates/cargonaut-ui-tui/src/lib.rs`
- [X] T013 [US1] Implement `async fn apply_bulk_rename_from_temp(app: &mut App, temp_path: &std::path::Path, original_names: &[String], status: &mut String)` and add its call in the `run_loop` post-action block for `PendingExternalKind::BulkRename`: read temp file lines, call `let _ = std::fs::remove_file(temp_path)` **unconditionally before any early return** (FR-009/SC-005 — temp file deleted on both success and failure paths), call `validate_rename_proposals(original_names, &edited)` → on Err set status and return, call `app.apply_bulk_rename(pairs).await`, handle events (refresh + status) in `crates/cargonaut-ui-tui/src/lib.rs`

> T007 and T010 [P] can be started simultaneously (different crates). T008 depends on T007; T011 depends on T010. T009 depends on T008. T012 depends on T011. T013 depends on T012 and T008.

**Checkpoint**: Quickstart Scenario 1–5 pass manually. `cargo test -p cargonaut-core -- apply_bulk_rename` green. `cargo test -p cargonaut-ui-tui -- queue_bulk_rename` green.

---

## Phase 4: User Story 2 — Undo Last File Operation (Priority: P2)

**Goal**: Press `C-z` after any file operation to reverse the most recent one; single-level; Delete is non-reversible.

**Independent Test** (from quickstart.md Scenario 6): After a successful bulk rename, press `C-z` — all files are restored to original names; second `C-z` → "Nothing to undo".

### Core: undo_last_operation

- [X] T014 [US2] (red) Write 7 failing tests for `App::undo_last_operation()` covering: `None` log → "Nothing to undo", `Rename` undo → all renamed files restored on disk, `Copy` undo → destination copies deleted, `Delete` undo → status "cannot be undone", second undo after first → "Nothing to undo" (log cleared), undo clears selection on both panes, `Move` undo (scaffold only — log is never populated, verify it returns events without crashing) — in `#[cfg(test)]` module of `crates/cargonaut-core/src/lib.rs`
- [X] T015 [US2] (green) Implement `pub async fn undo_last_operation(&mut self) -> Result<Vec<Event>, AppError>` in `crates/cargonaut-core/src/lib.rs`: match `self.undo_log.take()` (always clears), handle each `UndoEntry` variant — Rename: reverse-rename each pair; Copy: delete each copy path; Move: reverse-move each pair; Delete: emit warning status — refresh both panes and clear selection on success
- [X] T016 [US2] Replace stub dispatch arm for `UndoLastOp` with `self.undo_last_operation().await` in `App::dispatch()` in `crates/cargonaut-core/src/lib.rs`
- [X] T017 [US2] Update `App::confirm_copy()` to record `UndoEntry::Copy { copies }` after successful copy submission in `crates/cargonaut-core/src/lib.rs` (collect destination paths for all submitted entries)

### UI: wire UndoLastOp

- [X] T018 [P] [US2] Add `U::UndoLastOp => AppCommand::UndoLastOp` to `ui_command_to_core()` in `crates/cargonaut-ui-tui/src/lib.rs`

> T014 can start after Phase 1 (UndoEntry type exists). T015 depends on T014. T016 depends on T015. T017 is independent of T014–T016 (only needs UndoEntry type from T001). T018 [P] with T016/T017 since it's a different file; T018 needs `AppCommand::UndoLastOp` which was added in T003.

**Checkpoint**: Quickstart Scenarios 6–9 pass manually. `cargo test -p cargonaut-core -- undo_last_operation` green.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: CI gates for SC-001/SC-004, help overlay verification, docs, and final validation.

- [X] T019 Add `[[bench]] name = "bulk_rename" harness = false test = false` entry to `crates/cargonaut-core/Cargo.toml`
- [X] T020 [US1] Create benchmark file `crates/cargonaut-core/benches/bulk_rename.rs` with two benches: `bulk_rename_50` (create 50 temp files, tag all, call `apply_bulk_rename()` 50-pair rename, measure 100 iterations, compute p95, `assert!(p95_ns <= 500_000_000u128, "SC-001 breach: ...")`) and `undo_rename_50` (undo the 50-file rename, same assertion pattern, `assert!(p95_ns <= 500_000_000u128, "SC-004 breach: ...")`) — inline `assert!` makes `cargo bench` fail on regression, matching the Feature 049 compare_dirs bench pattern
- [X] T021 [P] Verify the help overlay mentions `C-x r` for bulk rename and `C-z` for undo — check `crates/cargonaut-ui-tui/src/dialog.rs` line ~1088; update if the description is missing or incorrect
- [X] T022 Run `make ci-local` and fix any clippy warnings, fmt failures, or test failures in `crates/cargonaut-core/src/lib.rs` and `crates/cargonaut-ui-tui/src/lib.rs`
- [X] T023 [P] Update `README.md`: increment test count, increment feature count to 16, add Feature 050 one-liner to the Feature History section
- [X] T024 [P] Append Feature 050 section to `Learnings.md` (minimum 3 bullets: PendingExternalKind generalization, validate_rename_proposals pure-function approach, undo log single-level design)
- [X] T025 [P] Update `ROADMAP.md`: strike through #47 row as `~~[#47]~~ | ~~Bulk rename via editor + undo~~ | **Closed — Feature 050**` and update "Last updated" line

> T020 depends on T019 (Cargo.toml entry must exist first). T021/T023/T024/T025 [P] with each other. T022 must be last in this phase.

**Checkpoint**: `make ci-local` passes all steps. Quickstart Scenarios 1–9 verified.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1** (Setup): No dependencies — start immediately
- **Phase 2** (Foundational): Depends on Phase 1 completion (UndoEntry type, Command variants)
- **Phase 3** (US1): Depends on Phase 2 completion (`validate_rename_proposals` must be green)
- **Phase 4** (US2): Depends on Phase 1 completion (UndoEntry type); may start concurrently with Phase 3 for the red test tasks (T014) but green implementation (T015+) should follow Phase 3 green for cleaner CI
- **Phase 5** (Polish): Depends on Phase 3 + Phase 4 completion

### Task Dependencies Within Phase 3

```
T007 (red, core) ──► T008 (green) ──► T009 (dispatch arm)
                                           ↓
T010 (red, UI) ──► T011 (green) ──► T012 ──► T013 ◄──────────┘
```

T007 and T010 [P] (different crates, both depend only on Phase 1 + 2 complete).

### Task Dependencies Within Phase 4

```
T014 (red) ──► T015 (green) ──► T016 (dispatch arm)
                    └──────────► T017 (confirm_copy update)
T018 [P] — needs T003 (AppCommand::UndoLastOp) + Phase 3 T009 merged
```

### Parallel Opportunities

- **Phase 1**: T001→T002→T003 sequential (same file); T004 [P] (different crate)
- **Phase 3**: T007 and T010 can be written simultaneously
- **Phase 4**: T018 can be done after T003 is complete (independent of T014–T017)
- **Phase 5**: T021, T023, T024, T025 all [P] with each other

---

## Parallel Example: Phase 3

```
# Both red-test tasks can be started at the same time:
Task T007: Write failing tests for apply_bulk_rename — cargonaut-core
Task T010: Write failing tests for queue_bulk_rename — cargonaut-ui-tui

# After T007 is committed, implement in cargonaut-core:
Task T008: Implement apply_bulk_rename
Task T009: Wire dispatch arm

# After T010 is committed, implement in cargonaut-ui-tui:
Task T011: Implement queue_bulk_rename
Task T012: Handle BulkRenameViaEditor in dispatch_ui_command
Task T013: Implement post-action in run_loop
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Complete Phase 1: Setup (types + stubs)
2. Complete Phase 2: Foundational (validate_rename_proposals green)
3. Complete Phase 3: US1 (apply_bulk_rename + queue_bulk_rename + run_loop post-action)
4. **STOP and VALIDATE**: Run Quickstart Scenarios 1–5 manually
5. Only then proceed to Phase 4 (US2 undo)

### TDD Discipline (per constitution §II)

For every red→green pair:
1. Write the test(s) — verify `cargo test` FAILS
2. `git commit -m "T00X (red): <function-name> — N failing tests"`
3. Implement the function — verify `cargo test` PASSES
4. `git commit -m "T00X (green): <function-name>"`

### Incremental Delivery

- After Phase 3: US1 is fully functional and independently testable
- After Phase 4: US2 adds undo capability without breaking US1
- Each phase is a complete, verifiable increment

---

## Notes

- `validate_rename_proposals()` is `pub(crate)` — accessible from tests in the same file, not exposed publicly
- `apply_bulk_rename()` is `pub` — called from the TUI after editor exits
- `undo_last_operation()` is `pub` — dispatched via `Command::UndoLastOp`
- All temp files go to `std::env::temp_dir()` — on dev host this is tmpfs (no SSD writes)
- The `PendingExternalKind::FileOpen` arm in `run_loop` preserves existing F3/F4 behavior exactly
- Move undo (`UndoEntry::Move`) is scaffolded in T015 but never populated in Feature 050; covered by a test verifying it doesn't crash

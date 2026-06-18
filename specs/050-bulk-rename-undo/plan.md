# Implementation Plan: Bulk Rename via Editor + Undo of File Operations

**Branch**: `050-bulk-rename-undo` | **Date**: 2026-06-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/050-bulk-rename-undo/spec.md`

## Summary

Two user stories: (1) bulk rename tagged entries by opening `$EDITOR` on a temp file of basenames, reading back edits, validating, and atomically renaming; (2) undo the most recent reversible file operation (rename, copy, move) in the session.

Both stories reuse existing machinery: the F4 `PendingExternal` suspend/resume mechanism for editor launch, `std::fs::rename` for atomic in-directory renames, and the `App` struct in `cargonaut-core` as the home for the new `undo_log` field. The `Command::BulkRenameViaEditor` keymap variant already exists in `cargonaut-keymap`. The `C-z` → `undo-last-op` binding and `Command::UndoLastOp` variant also already exist. Both just need to be wired to implementation.

## Technical Context

**Language/Version**: Rust (stable, workspace toolchain — same as all other crates)

**Primary Dependencies** (all already in workspace):
- `ratatui 0.27` + `crossterm 0.28` for TUI suspend/resume
- `std::fs::{rename, remove_file, remove_dir_all}` for filesystem operations
- `std::env::temp_dir()` for temp-file location
- No new crate dependencies required

**Storage**: Local filesystem only (`file://` scheme). No persistence — undo log is session-scoped, in-memory only.

**Testing**: `cargo test --workspace` (TDD red→green per constitution §II). Pure-logic validation functions are unit-tested; async core operations use the existing in-memory VFS test harness from other features.

**Target Platform**: Linux (POSIX `rename(2)` guarantees atomicity within the same filesystem; acceptable to note this in code)

**Project Type**: TUI application (orthodox file manager). Feature adds into the existing `cargonaut-core` + `cargonaut-ui-tui` crates.

**Performance Goals**:
- SC-001: Bulk rename of 50 files completes within 500 ms of editor exit
- SC-004: Undo of 50-file rename completes within 500 ms of keypress

**Constraints**:
- Undo log: max 1 entry, session-only, non-persistent
- No new crate deps
- TDD: failing tests committed before green implementation
- Constitution §III: new keymap bindings already defined in `design/contracts/keymap.toml`

**Scale/Scope**: Up to 50 tagged files (spec SC-001/SC-004 benchmarks). No hard limit enforced in code — 50 is the performance-tested ceiling.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| §I Code Quality: `clippy -D warnings`, `fmt --check` | ✅ Pass | No new unsafe code. All new fns documented. |
| §II Test-First: TDD red→green, SC-### CI gates | ✅ Pass | Plan includes failing test commits before green. SC-001 and SC-004 get CI bench gates per §II. |
| §III UX Consistency: keymap in one source of truth | ✅ Pass | `C-x r` and `C-z` bindings already in `design/contracts/keymap.toml`. No new bindings needed. |
| §IV Performance: benches in CI for SC gates | ✅ Pass | SC-001/SC-004 get criterion bench (or integration timing test) per §IV pattern. |
| §V SSD Preservation: target/ is tmpfs symlink | ✅ Pass | CI exempt; dev host uses `make tmpfs-setup`. |
| Dev Workflow: confirmation for destructive ops | ✅ Pass | Undo of copy warns before removing copies (FR-011 spec note). No pre-undo confirmation dialog needed for rename. |

*Post-design re-check*: No violations. Data model introduces no new persistence; undo log is in-memory. Temp file is in `/tmp` (tmpfs on dev host — no SSD writes).

## Project Structure

### Documentation (this feature)

```text
specs/050-bulk-rename-undo/
├── plan.md              ← this file
├── research.md          ← Phase 0 output
├── data-model.md        ← Phase 1 output
├── contracts/           ← Phase 1 output (keymap additions, core API)
│   ├── core-api.md
│   └── keymap-additions.md
├── quickstart.md        ← Phase 1 output
└── tasks.md             ← Phase 2 output (speckit-tasks)
```

### Source Code (modified files)

```text
crates/cargonaut-core/src/lib.rs
│   ← New: Command::BulkRenameApply(Vec<(String, String)>)
│   ← New: Command::UndoLastOp
│   ← New: UndoEntry enum (Rename/Copy/Move/Delete)
│   ← New: App.undo_log: Option<UndoEntry>
│   ← New: App::apply_bulk_rename()
│   ← New: App::undo_last_operation()
│   ← Mod: confirm_copy() records UndoEntry::Copy

crates/cargonaut-ui-tui/src/lib.rs
│   ← New: PendingExternalKind enum (FileOpen | BulkRename { temp_path, original_names })
│   ← Mod: PendingExternal gets kind: PendingExternalKind
│   ← Mod: run_external / run_loop post-action handling
│   ← Mod: dispatch_ui_command handles BulkRenameViaEditor, UndoLastOp
│   ← Mod: ui_command_to_core adds UndoLastOp → AppCommand::UndoLastOp

design/contracts/keymap.toml    ← No changes needed (bindings already exist)
```

## Complexity Tracking

No constitution violations to justify. The single complexity note:

| Item | Decision | Rationale |
|---|---|---|
| `PendingExternalKind` on `PendingExternal` | Added a `kind` field to carry post-action context | The run_loop needs to know what to do after editor exits: for F3/F4 just refresh the pane; for bulk rename, read the temp file and apply renames. A single struct with a `kind` field is cleaner than two separate `Option<…>` fields on UiState. |
| Move undo scaffold | `UndoEntry::Move` defined but never populated | Move is not yet truly implemented (the confirmed Move just re-dispatches, doing nothing). The data model names it for completeness; when Move is implemented in a future feature, it will record the entry. Spec FR-011 names Move as reversible — the structure is in place. |

## Phase 0: Research

See [research.md](research.md).

## Phase 1: Design

### Data Model

See [data-model.md](data-model.md).

### Contracts

See [contracts/core-api.md](contracts/core-api.md) and [contracts/keymap-additions.md](contracts/keymap-additions.md).

### Quickstart

See [quickstart.md](quickstart.md).

## Implementation Strategy

### MVP First (US1 only)

Ship bulk rename first, independently testable. After US1 is green and benched, add the undo log plumbing (US2). The spec allows this ordering because undo is listed as P2 (lower priority).

### TDD Discipline

Per constitution §II:
1. Red commit: write failing tests for a function
2. Green commit: implement the function to pass those tests
3. Each task in tasks.md follows this pattern

### Core vs UI split

- **Core** owns: filesystem rename, temp file I/O, validation logic, undo log, undo application
- **UI** owns: editor launch (PendingExternal), temp file path generation, reading back editor output, calling core after editor exits

This split keeps core testable without a terminal.

## Risks and Mitigations

| Risk | Likelihood | Mitigation |
|---|---|---|
| `$EDITOR` not set | Low (most users have it) | Use `vi` as fallback (same as F4). Report error to status bar. |
| Rename target collides with existing file | Medium | validate before applying; abort entire batch on any collision |
| Temp file left on disk after crash | Low | Use RAII or `defer`-style drop (scopeguard crate — already in workspace if used, otherwise `std` pattern); not a safety issue, just cosmetic |
| `std::fs::rename` across filesystems | Impossible | Renames are within active pane directory, by design. Cross-directory rename requires a move (separate op). |
| Undo of copy deletes user-modified copies | Medium | Emit warning status before removing; spec allows this |

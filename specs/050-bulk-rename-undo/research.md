# Research: Bulk Rename via Editor + Undo of File Operations

**Feature**: 050-bulk-rename-undo | **Date**: 2026-06-18

## R-001 — Editor suspend/resume pattern (F3/F4 precedent)

**Decision**: Reuse the existing `PendingExternal` mechanism from Feature 031 (F3/F4 shell-out).

**Rationale**: The mechanism is proven and covers all edge cases: disable raw mode → leave alternate screen → run editor → re-enable raw mode → enter alternate screen → refresh. The `run_loop` consumes `ui.pending_external` at the top of each iteration after event handling, ensuring a clean single-shot execution.

**Alternatives considered**: Running the editor in a `tokio::task::spawn_blocking` was considered but rejected — it complicates terminal state management and doesn't match the existing pattern.

**Code reference**: `cargonaut-ui-tui/src/lib.rs:355` (`if let Some(ext) = ui.pending_external.take()`) and `run_external()` at line 1270.

**Extension needed**: Add `kind: PendingExternalKind` to `PendingExternal` so `run_loop` knows whether to just `refresh_active_pane()` (F3/F4) or read the temp file and call `apply_bulk_rename()` (bulk rename).

## R-002 — Temp file creation and lifecycle

**Decision**: Use `std::env::temp_dir()` + a process-ID + counter-based unique name (e.g. `cargonaut-rename-<pid>-<counter>.txt`). Delete the temp file in a cleanup step after the rename attempt (success or failure). Use a `scopeguard`-style RAII wrapper or a `finally`-style explicit delete to guarantee cleanup even on error paths.

**Rationale**: `tempfile` crate creates files that are automatically deleted on drop. However, `tempfile` is not currently a workspace dependency. A simple name derived from PID + atomic counter is sufficient and avoids a new dependency.

**Alternative**: Use the `tempfile` crate (already considered; not in workspace; adding a crate for a one-use wrapper is over-engineered for this feature). The manual approach is fine.

**Cleanup guarantee**: After `run_external` returns, the UI reads the file and calls the rename. Then deletes it. If deletion fails, log to status bar but don't abort (it's just a temp file in `/tmp`).

## R-003 — Rename validation logic placement

**Decision**: Two-level validation:
1. **Structural validation** (pure, no filesystem I/O): line count match, no empty names, no `/` in names, no duplicates within the proposed set. Implemented as a pure function `validate_rename_proposals()` in core or as a standalone function in the UI layer before calling core. Testable without a filesystem.
2. **Collision check** (requires filesystem I/O): check that no proposed new name collides with an existing entry that is NOT in the tagged set. Performed inside `App::apply_bulk_rename()` before the first `std::fs::rename` call.

**Rationale**: Separating structural from filesystem validation makes the pure logic trivially unit-testable. The collision check needs the pane listing and the filesystem, so it lives in the apply function.

**Error behavior**: Any validation failure → abort all renames, return `Err` (or `Ok(vec![Event::Status(...)])`) with a descriptive message. No partial rename.

## R-004 — Atomicity of rename within directory

**Decision**: Call `std::fs::rename(old, new)` sequentially for each rename pair. On POSIX/Linux, `rename(2)` is atomic for same-filesystem operations (which is guaranteed here — both paths are in the same directory). If any rename fails mid-batch, stop immediately and report which entry failed.

**Rationale**: POSIX guarantees `rename(2)` atomicity. We don't need two-phase commit or rollback — if a rename fails, the partial state is reported via the status bar and the undo log records the completed portion (allowing undo of what was done).

**Alternative**: Stage all renames via intermediate names to allow rollback. Rejected — too complex, not required by spec (FR-007 says "error message identifies which rename failed and no further renames are attempted", not "roll back previous ones").

## R-005 — Undo log design

**Decision**: Single `Option<UndoEntry>` field on `App`, where `UndoEntry` is an enum:
```
enum UndoEntry {
    Rename { pairs: Vec<(PathBuf, PathBuf)> }, // (new_path, original_path) — reversed for undo
    Copy   { copies: Vec<PathBuf> },           // paths to delete on undo
    Move   { pairs: Vec<(PathBuf, PathBuf)> }, // (dst_path, src_path) — reversed for undo
    Delete,                                     // sentinel — "not undoable"
}
```

Each new completed operation overwrites the previous entry. Undo clears the entry; subsequent undo presses return "Nothing to undo".

**Rationale**: Single-level undo is what the spec requires (FR-013). The `Option<UndoEntry>` is the smallest possible structure. Session-scoped, no persistence needed.

**Alternative**: A stack of entries (multi-level undo). Rejected — spec explicitly says single-level.

## R-006 — Undo of copy: deleting modified copies

**Decision**: Undo of `Copy` deletes all destination copies unconditionally using `std::fs::remove_file()` (files) or `std::fs::remove_dir_all()` (directories). Emit a status warning "Undo: removing destination copies — any post-copy edits will be lost."

**Rationale**: Spec clarification (Clarifications §Session 2026-06-18): undo removes copies unconditionally. The user is responsible for understanding that post-copy edits to copies will be lost.

## R-007 — Move undo scaffold (future-proofing)

**Decision**: `UndoEntry::Move` is defined in the data model but never populated in Feature 050. The `Command::Move` dispatch in core currently re-shows the confirmation dialog (effectively a no-op for actual file movement). When Move is properly implemented in a future feature, it will record `UndoEntry::Move` at that time.

**Rationale**: Keeping the type in the enum costs nothing and makes the undo dispatcher complete — `undo_last_operation()` will handle `UndoEntry::Move` correctly when a Move undo entry is ever set. Spec FR-011 lists Move as reversible.

## R-008 — Existing keymap bindings (no changes needed)

**Finding**: Both key bindings are already defined in `design/contracts/keymap.toml`:
- `C-x r` → `bulk-rename-via-editor` (line ~302)
- `C-z` → `undo-last-op` (line ~178)

And both command variants are in `cargonaut-ui-tui/src/keymap.rs`:
- `Command::BulkRenameViaEditor` (line 178)
- `Command::UndoLastOp` (line 122)

Neither is yet wired in `dispatch_ui_command()`. No keymap.toml changes are required.

## R-009 — `scopeguard` crate availability

**Finding**: The `scopeguard` crate is not currently in the workspace. Temp file cleanup will use an explicit `let _ = std::fs::remove_file(&temp_path)` after the rename attempt, in both the success and error branches. This is equivalent to a defer/finally for this use case and avoids a new dependency.

## R-010 — Line ending conventions for temp file

**Decision**: Write the temp file using `\n` (Unix newlines). Read back with `lines()` which handles both `\n` and `\r\n`. This is robust on all supported platforms while being maximally simple.

**Edge case**: Files with `\n` in their names cannot be bulk renamed via this mechanism. They are excluded from the tagged set with a status warning if any such entry is found (matching spec Assumptions section).

# Data Model: Bulk Rename via Editor + Undo of File Operations

**Feature**: 050-bulk-rename-undo | **Date**: 2026-06-18

## Entities

### UndoEntry (new — cargonaut-core)

Session-scoped undo log entry stored as `Option<UndoEntry>` on `App`.

```rust
pub enum UndoEntry {
    /// Single or bulk rename completed. `pairs` is (new_path, original_path) —
    /// already reversed so undo_last_operation() can apply them directly.
    Rename { pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> },
    /// Copy completed. `copies` is the list of destination paths that were
    /// created; undo removes them.
    Copy { copies: Vec<std::path::PathBuf> },
    /// Move completed. `pairs` is (current_path, original_path) — reversed for
    /// undo. Never populated in Feature 050 (Move is not yet implemented);
    /// scaffold only.
    Move { pairs: Vec<(std::path::PathBuf, std::path::PathBuf)> },
    /// Delete completed. Not reversible — undo emits a warning status.
    Delete,
}
```

**Fields**:
- `Rename::pairs` — each tuple is `(new_path, original_path)` where `original_path` is what it was before the rename. Undo calls `rename(new_path, original_path)` for each.
- `Copy::copies` — each is an absolute `PathBuf` to a destination file or directory. Undo calls `remove_file` or `remove_dir_all` for each.
- `Move::pairs` — same reversal convention as `Rename::pairs` but for cross-directory moves.
- `Delete` — sentinel variant. Undo displays "Delete cannot be undone" and clears the log.

**Lifecycle**:
- Created / overwritten: after any completed destructive operation
- Cleared: after a successful undo, or when overwritten by a newer operation
- Scope: session-only; not persisted across restarts

### App.undo_log (new field — cargonaut-core)

```rust
pub struct App {
    // ... existing fields ...
    /// FR-010/FR-013: session-scoped, single-level undo log.
    /// None = "nothing to undo"; Some(entry) = one undoable operation.
    pub(crate) undo_log: Option<UndoEntry>,
}
```

**Invariants**:
- At most one entry at any time
- Cleared to `None` after each `undo_last_operation()` call (whether or not the undo succeeded)
- Overwritten (not stacked) by each new completed operation

### PendingExternalKind (new — cargonaut-ui-tui)

Discriminates what action to take after `run_external()` returns.

```rust
enum PendingExternalKind {
    /// F3/F4: just refresh the active pane after the editor exits.
    FileOpen,
    /// C-x r bulk rename: read back the temp file and apply renames.
    BulkRename {
        /// Path to the temp file containing original names (one per line).
        temp_path: std::path::PathBuf,
        /// Original basenames in panel listing order (one per line of temp file).
        original_names: Vec<String>,
    },
}
```

**Lifecycle**:
- Created in `dispatch_ui_command` when `BulkRenameViaEditor` is handled
- Consumed in `run_loop` after `run_external()` returns
- `temp_path` is deleted immediately after processing (success or failure)

### PendingExternal (modified — cargonaut-ui-tui)

```rust
struct PendingExternal {
    /// Resolved program (e.g. "$EDITOR" or "vi").
    program: String,
    /// Arguments passed after program. For F3/F4: [file_path]. For diff: [argv[1..], left, right].
    /// For bulk rename: [temp_file_path].
    args: Vec<String>,
    /// What to do after the external process exits.
    kind: PendingExternalKind,
}
```

**Default kind for F3/F4**: `PendingExternalKind::FileOpen`. Existing callers (`queue_external`, `queue_diff`) are updated to set `kind: PendingExternalKind::FileOpen`.

### RenamePair

Transient — not a persistent type, used only during bulk rename processing.

```text
struct RenamePair {
    original: String,   // original basename
    proposed: String,   // proposed new basename (from edited temp file)
}
// Only pairs where original != proposed are acted upon.
```

Not a named Rust type — represented as `Vec<(String, String)>` in the function signature for simplicity.

## State Transitions

### Bulk Rename

```
User presses C-x r
  → dispatch_ui_command(BulkRenameViaEditor)
  → [check tagged entries; if none → status "Tag at least one file to bulk rename"]
  → write temp file (original names, one per line)
  → set pending_external = Some(PendingExternal { kind: BulkRename { temp_path, original_names }, ... })
  → run_loop sees pending_external → run_external (editor opens)
  → editor exits
  → run_loop post-action: read temp file, validate, call app.apply_bulk_rename()
  → delete temp file
  → refresh active pane
  → if success: record UndoEntry::Rename on app.undo_log
  → if validation failure: emit Status(error_msg) — no UndoEntry recorded
```

### Undo Last Operation

```
User presses C-z
  → dispatch_ui_command(UndoLastOp)
  → ui_command_to_core → AppCommand::UndoLastOp
  → app.dispatch(UndoLastOp) → app.undo_last_operation()
  → match app.undo_log:
      None → Status("Nothing to undo")
      Some(Rename) → rename(new → orig) for each pair; emit PaneUpdated(Both); Status("Undone")
      Some(Copy) → warn + remove each copy; emit PaneUpdated(Both); Status("Undone")
      Some(Move) → move(dst → src) for each pair; emit PaneUpdated(Both); Status("Undone")
      Some(Delete) → Status("Delete cannot be undone — no in-session recovery available")
  → app.undo_log = None (always cleared)
```

## Validation Rules

For `validate_rename_proposals(originals: &[String], edited: &[String]) -> Result<Vec<(String, String)>, String>`:

| Rule | Check | Error message |
|---|---|---|
| Line count | `edited.len() == originals.len()` | "Line count changed: expected N, got M — do not add or delete lines" |
| No empty names | every `edited[i]` is non-empty | "Empty name on line N" |
| No path separator | `!edited[i].contains('/')` | "Name on line N contains '/' — must be a basename only" |
| No duplicates | all `edited` names are distinct | "Duplicate name 'X' on lines N and M" |
| No collision with existing | proposed name not in dir listing for non-tagged entries | "Name 'X' already exists and is not in the rename set" (checked in apply_bulk_rename) |

**Note**: The collision check requires the filesystem and is performed inside `apply_bulk_rename()`, not in `validate_rename_proposals()` which is pure.

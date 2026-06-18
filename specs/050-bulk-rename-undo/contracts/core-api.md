# Contract: Core API — Bulk Rename + Undo

**Feature**: 050-bulk-rename-undo | **Date**: 2026-06-18
**Crate**: `cargonaut-core` (`crates/cargonaut-core/src/lib.rs`)

## New Command Variants

```rust
pub enum Command {
    // ... existing variants ...

    /// C-x r — apply a set of validated bulk renames in the active pane directory.
    /// Each pair is (original_basename, new_basename). Pairs where original == new
    /// are silently skipped (FR-003/FR-006).
    BulkRenameApply(Vec<(String, String)>),

    /// C-z — undo the last recorded reversible file operation (FR-010/FR-013).
    UndoLastOp,
}
```

**Note on naming**: `BulkRenameApply` (not `BulkRenameViaEditor`) because the editor interaction is UI-side only. Core receives the already-validated rename pairs. This keeps core testable without a terminal.

## New Public Methods on App

### `App::apply_bulk_rename`

```rust
/// Apply a set of bulk renames in the active pane's directory.
///
/// # Arguments
/// `pairs` — (original_basename, new_basename). Only pairs where original != new
/// are acted upon. All pairs are validated before any rename is applied (FR-004).
///
/// # Returns
/// On success: `[PaneUpdated(active), Status("N entries renamed")]` + records UndoEntry.
/// On validation failure: `[Status(error_msg)]` — no filesystem changes made.
/// On partial failure: `[PaneUpdated(active), Status("Renamed M/N; failed: 'X': <os-error>")]`.
pub async fn apply_bulk_rename(
    &mut self,
    pairs: Vec<(String, String)>,
) -> Result<Vec<Event>, AppError>
```

**Preconditions**:
- Active pane must be a `file://` directory
- `pairs` has already passed structural validation (line count, no empty names, no `/`, no duplicates)
- Collision check (against existing non-renamed entries in the directory) is performed here

**Postconditions (success)**:
- All files in `pairs` (where `original != new`) are renamed on disk
- `self.undo_log = Some(UndoEntry::Rename { pairs: reversed })` is set
- Active pane listing is refreshed

**Error behavior (validation)**:
- Collision detected → returns `Status(error_msg)`, no filesystem changes
- Empty `pairs` (all unchanged) → returns `Status("No changes — nothing renamed")`

**Error behavior (filesystem)**:
- `rename()` fails mid-batch → stops, records partial undo entry for completed renames, returns `Status(...)`

### `App::undo_last_operation`

```rust
/// Undo the most recent reversible file operation in this session (FR-010/FR-013).
///
/// # Returns
/// - Nothing to undo: `[Status("Nothing to undo")]`
/// - Rename undone: `[PaneUpdated(Left), PaneUpdated(Right), Status("Undo: N renames reversed")]`
/// - Copy undone: `[PaneUpdated(Left), PaneUpdated(Right), Status("Undo: N copies removed")]`
/// - Move undone: `[PaneUpdated(Left), PaneUpdated(Right), Status("Undo: N files moved back")]`
/// - Delete: `[Status("Undo: delete cannot be reversed — no in-session recovery available")]`
///
/// Always clears `self.undo_log` after execution, regardless of success or failure.
pub async fn undo_last_operation(&mut self) -> Result<Vec<Event>, AppError>
```

**Postconditions**:
- `self.undo_log = None` after every call (FR-013 single-level)
- Both panes refreshed on success (FR-015)
- Selection/tag state cleared after undo (FR-015)

## Updated Method Signature

### `App::confirm_copy` (modified)

After existing logic, before returning:
```rust
// Record undo entry for copies (FR-011 US2)
let dst_cwd = self.pane(dst_pane).cwd.clone();
let copies: Vec<PathBuf> = entries.iter().map(|n| dst_cwd.path().join(n)).collect();
self.undo_log = Some(UndoEntry::Copy { copies });
```

## Validation Contract

```rust
/// Pure validation — no filesystem I/O.
///
/// Returns: the subset of pairs where `original != new`, or `Err(message)` on invalid input.
pub(crate) fn validate_rename_proposals(
    originals: &[String],
    edited: &[String],
) -> Result<Vec<(String, String)>, String>
```

**Input contract**:
- `originals.len()` == number of tagged entries (in listing order)
- `edited` is parsed from the temp file, one entry per line

**Output contract**:
- `Ok(pairs)` → pairs are all `(o, e)` where `o != e`; each `e` is non-empty, has no `/`, is unique within `edited`
- `Err(msg)` → human-readable error message for status bar display

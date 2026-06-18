# Contract: Keymap Additions — Bulk Rename + Undo

**Feature**: 050-bulk-rename-undo | **Date**: 2026-06-18

## No Changes to keymap.toml

Both bindings already exist in `design/contracts/keymap.toml`:

| Key | Action | FR |
|---|---|---|
| `C-x r` | `bulk-rename-via-editor` | FR-001/FR-208 |
| `C-z` | `undo-last-op` | FR-010 |

## Additions to TUI Dispatch

The following changes are needed in `cargonaut-ui-tui/src/lib.rs`:

### `ui_command_to_core` additions

```rust
fn ui_command_to_core(cmd: Command) -> Option<AppCommand> {
    // ... existing mappings ...
    U::UndoLastOp => AppCommand::UndoLastOp,
    // Note: BulkRenameViaEditor is NOT mapped here — it's handled
    // directly in dispatch_ui_command (editor launch is UI-side)
    _ => return None,
}
```

### `dispatch_ui_command` additions

```rust
// US1 (FR-001 through FR-009): C-x r bulk rename via $EDITOR.
Command::BulkRenameViaEditor => {
    queue_bulk_rename(app, ui, status);
    return Ok(());
}
```

Where `queue_bulk_rename(app, ui, status)` is a new function (analogous to `queue_external` and `queue_diff`):
1. Collect tagged entries from active pane (all kinds — files, dirs, symlinks)
2. Filter out entries with `\n` in name; warn in status
3. If no entries remain → `*status = "Tag at least one entry to bulk rename"; return`
4. Write `original_names` to a temp file in `std::env::temp_dir()`
5. Get `$EDITOR` (fallback to `"vi"`)
6. Set `ui.pending_external = Some(PendingExternal { program: editor, args: vec![temp_path_str], kind: PendingExternalKind::BulkRename { temp_path, original_names } })`

### `run_loop` post-action dispatch

After `run_external(term, &ext, ...)`:

```rust
match ext.kind {
    PendingExternalKind::FileOpen => {
        // existing behavior: refresh + status
        let _ = app.refresh_active_pane().await...;
        status = format!("Returned from {}", ext.program);
    }
    PendingExternalKind::BulkRename { ref temp_path, ref original_names } => {
        apply_bulk_rename_from_temp(app, temp_path, original_names, status).await;
    }
}
```

Where `apply_bulk_rename_from_temp` is a new async helper:
1. Read temp file → split into lines
2. Delete temp file (best-effort)
3. Call `validate_rename_proposals(original_names, &edited_names)` → on Err, set status and return
4. Call `app.apply_bulk_rename(pairs).await` → handle events
5. Refresh both panes

## Help Text Update

`dialog.rs` line 1088 already mentions `bulk-rename-via-editor` in the help overlay — verify it renders correctly.

## F-key Bar (optional)

No function-key bar label change required for this feature. `C-x r` and `C-z` are chord shortcuts that don't appear in the F-key bar row. The existing bar labels remain unchanged.

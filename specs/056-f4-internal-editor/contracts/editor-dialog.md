# Contract: FileEditorDialog

**Module**: `cargonaut_ui_tui::dialog`

**Callers**: `lib.rs` run loop — `ActiveDialog::FileEditor` dispatch arm + `open_file_editor()`.

---

## Construction

```rust
// async helper in lib.rs — opens + validates file, constructs widget
pub async fn open_file_editor(
    raw_path: std::path::PathBuf,
    display_name: String,
) -> Result<dialog::FileEditorDialog, OpenEditorError>
```

`OpenEditorError` variants:
- `Binary` — file failed UTF-8 sample check
- `TooLarge { size_bytes: u64 }` — file exceeds size limit
- `Io(std::io::Error)` — any other I/O failure

On success, `FileEditorDialog::new(path, display_name, content_str, line_ending)` is called internally.

---

## Key Handling

```rust
pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> FileEditorAction
```

**Key routing table** (raw key, resolved BEFORE keymap lookup for printable chars):

| Input | Action |
|-------|--------|
| Printable char | Insert at cursor; set dirty |
| Backspace | Delete char left of cursor; set dirty |
| Delete | Delete char at cursor; set dirty |
| Enter | Split line at cursor; set dirty |
| Arrow Up/Down | Move cursor by line |
| Arrow Left/Right | Move cursor by column (with line wrap) |
| Home | cursor_col = 0 |
| End | cursor_col = line.len() |
| Page Up | scroll_offset -= viewport_height; clamp |
| Page Down | scroll_offset += viewport_height; clamp |

Keymap-resolved commands:

| `Command` | Behaviour |
|-----------|-----------|
| `SaveFile` | Save to disk; clear dirty; set status_msg on error |
| `EditorQuit` | If clean → `FileEditorAction::Close`; if dirty → show `UnsavedChangesDialog` → `UnsavedPromptShowing` |

When `unsaved_dlg` is `Some`, all key events route to `unsaved_dlg.handle_key(key)` first:

| `UnsavedChangesChoice` | Result |
|------------------------|--------|
| `Save` | `FileEditorAction::SaveAndClose` |
| `Discard` | `FileEditorAction::DiscardAndClose` |
| `Cancel` | dismiss sub-modal; resume editing |

---

## Rendering

```rust
pub fn render(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme)
```

Layout (3 rows):
1. **Header** (1 line): filename + `*` if dirty
2. **Content area** (`area.height - 2` lines): visible lines with left gutter (line numbers optional), cursor highlighted
3. **Footer** (1 line): `Ln {cursor_line+1}, Col {cursor_col+1}` on left; `F2=Save  F10=Quit` hints on right; `status_msg` overrides the left side when non-empty

When `unsaved_dlg` is `Some`, render the sub-modal overlay centered over the content area.

---

## Save

```rust
pub fn save(&mut self) -> std::io::Result<()>
```

Joins `self.lines` with the original `line_ending`, writes to `self.path` via `std::fs::write`, clears `self.dirty`. On error: does NOT modify `self.dirty`; the caller should set `status_msg` and keep the editor open.

---

## UnsavedChangesDialog Contract

```rust
pub fn handle_key(&mut self, key: crossterm::event::KeyEvent) -> Option<UnsavedChangesChoice>
```

Returns `None` when the key cycles focus without dismissing. Returns `Some(choice)` when the user confirms.

Tab / Left / Right cycle `focus` through {0=Save, 1=Discard, 2=Cancel}. Enter resolves to `focus`. Esc always resolves to `Cancel`.

---

## Integration Points

- `open_file_editor()` in `lib.rs` — mirrors `open_file_viewer()`; called from `Command::Edit` handler
- `ActiveDialog::FileEditor { widget }` arm in `lib.rs` — mirrors `ActiveDialog::FileViewer` arm
- `Mode::Editor` in `keymap.rs` and `keymap.toml` — editor-mode bindings
- `refresh_active_pane()` in `lib.rs` — called after close (as with external F4 return today)
- `BINARY_DETECT_BYTES`, `STREAMING_THRESHOLD_BYTES` from `dialog.rs` — reused for file-open guards

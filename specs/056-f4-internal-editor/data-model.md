# Data Model: Internal F4 Editor (Feature 056)

## LineEnding

```
enum LineEnding { Lf, Crlf }
```

Detected from the first line break in the file. LF is the default if the file is empty. All lines are stripped of their terminator on load and joined with the original terminator on save, preserving the file's native style.

---

## FileEditorDialog

The top-level widget stored in `ActiveDialog::FileEditor`.

```
struct FileEditorDialog {
    path:          PathBuf,       // absolute, resolved path for saving
    display_name:  String,        // shown in the header bar
    lines:         Vec<String>,   // line buffer, no terminators
    cursor_line:   usize,         // 0-based row index into `lines`
    cursor_col:    usize,         // 0-based column (byte offset in the UTF-8 string)
    scroll_offset: usize,         // first visible line index (0 = top of file)
    dirty:         bool,          // unsaved changes exist
    line_ending:   LineEnding,    // preserve original LF or CRLF
    unsaved_dlg:   Option<UnsavedChangesDialog>,  // sub-modal; Some when exit-guard is showing
    status_msg:    Option<String>,                // transient footer message (save error, etc.)
}
```

**Invariants**:
- `cursor_line` < `lines.len()` at all times (guaranteed by `clamp_cursor()`).
- `cursor_col` ≤ `lines[cursor_line].len()` (also clamped after every mutation).
- An empty file is represented as `lines = vec!["".to_string()]`.
- `scroll_offset` ≤ `cursor_line` ≤ `scroll_offset + viewport_height - 1` after every `scroll_to_cursor()` call.

---

## FileEditorAction

Return type of `FileEditorDialog::handle_key`.

```
enum FileEditorAction {
    /// Key consumed; redraw needed but no structural change.
    Swallow,
    /// User requested quit; dialog has no unsaved changes — close immediately.
    Close,
    /// User requested quit with unsaved changes; sub-modal is now showing.
    UnsavedPromptShowing,
    /// Sub-modal resolved: save then close.
    SaveAndClose,
    /// Sub-modal resolved: discard and close.
    DiscardAndClose,
}
```

The `lib.rs` dispatch arm handles `SaveAndClose` by calling `dialog.save()` and then removing the `ActiveDialog`. `DiscardAndClose` removes it without saving. `UnsavedPromptShowing` is a no-op in the dispatcher (the sub-modal now owns key events).

---

## UnsavedChangesDialog

```
enum UnsavedChangesChoice { Save, Discard, Cancel }

struct UnsavedChangesDialog {
    focus: usize,   // 0 = Save, 1 = Discard, 2 = Cancel
}
```

Defaults `focus` to `2` (Cancel) — the safe choice. Tab / arrow cycle through buttons; Enter confirms the focused choice; Esc resolves to Cancel.

---

## Relationships

```
ActiveDialog::FileEditor
  └── FileEditorDialog
        ├── unsaved_dlg: Option<UnsavedChangesDialog>
        └── (on save) writes to `path` on disk
```

The editor lives entirely in the TUI event loop. No background threads. No shared state with core or VFS beyond:
- reading `path` at open time (via `open_file_editor()`)
- writing `path` at save time (via `dialog.save()`)
- triggering pane refresh after close (via `refresh_active_pane()`)

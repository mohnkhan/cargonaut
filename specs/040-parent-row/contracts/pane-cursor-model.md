# Contract: virtual-row cursor model + `..` rendering

**Feature**: 040-parent-row

## Core (`cargonaut-core`, `PaneState` / dispatch)

### New public API on `PaneState`
```rust
/// True when the current directory has a parent (a `..` row is shown).
pub fn has_parent(&self) -> bool;            // cwd.parent().is_some()

/// 0 at a filesystem root, 1 otherwise — the virtual↔real index shift.
pub fn parent_offset(&self) -> usize;        // has_parent() as usize

/// Number of addressable rows: parent_offset() + visible real entries.
pub fn row_count(&self) -> usize;

/// True when the cursor is on the synthetic `..` row.
pub fn on_parent_row(&self) -> bool;

/// What the cursor currently points at.
pub fn focused_row(&self) -> FocusedRow;     // Parent | Entry(real_index)
```
```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedRow { Parent, Entry(usize) }
```

### Updated behavior (existing items)
- `focused_entry_index() -> Option<usize>`: returns `None` on the `..` row,
  else `visible_indices()[cursor - parent_offset()]`. (Unchanged signature.)
- `CursorDown` / `CursorTo(n)`: clamp `cursor` to `row_count()-1` (was visible
  len). `CursorUp`: `saturating_sub(1)` (lower bound 0 = `..` when present).
- Cursor-reset sites (descend/ascend `navigate_to`, `relist_active`,
  `refresh_active_pane`, `toggle_hidden`, `set_filter`, mkdir relist): set
  `cursor = default_cursor()` (private helper) instead of `0`.
- `Descend` dispatch: `if active pane on_parent_row() → ascend_to_parent() else
  descend_into_focused()`.
- `SelectionToggle`, `SelectionInvert`, `select_by_pattern`,
  `selection_or_focused`, `recursive_dir_size`, `show_focused_in_other_panel`:
  **no source change** — they already gate on `focused_entry_index()` /
  `visible_indices()` / `selected`, all of which exclude the `..` row.

### Guarantees
- `0 <= cursor < row_count()` after any command (FR-005).
- The `..` row is present iff `has_parent()`, independent of filter/hidden (FR-009).
- `selected` never contains the `..` row; copy/move/delete never target it
  (FR-006/007/008/010).
- Root: `parent_offset()==0`, behavior identical to today (FR-002, SC-002).

## TUI (`cargonaut-ui-tui`, `PaneView` / mouse)

### `PaneView`
- `has_parent()` / `row_count()` mirrors core (computed from synced `cwd`).
- `sync_from`: select `state.cursor` directly into `ListState`, clamped to
  `row_count()-1` (cursor is already the virtual index).
- `render`: when `has_parent()`, prepend a single `..` `ListItem` (themed, label
  `..`) before the visible real-entry items; the `ListState` index aligns so row
  0 highlights `..`.
- `focused_entry_index()`: mirror the core offset (used by quick-view/preview).

### Mouse (`handle_mouse` in `lib.rs`)
- Single click: `CursorTo(viewport_top() + row)` — already a virtual-row index;
  no change needed beyond the core clamp.
- Double click: dispatch `Descend` — which now ascends when the clicked row is the
  `..` row. No special-case needed.

## Test contract

**Core** (`cargonaut-core/src/lib.rs`):
- `parent_row_present_non_root` / `absent_at_root`: `row_count`/`has_parent`/
  `on_parent_row` per cwd.
- `default_cursor_is_first_real_entry`: fresh non-root listing → `cursor ==
  parent_offset()`, `focused_entry_index() == Some(first visible)`.
- `cursor_up_from_first_entry_lands_on_parent_then_clamps`.
- `descend_on_parent_row_ascends`: cursor on `..`, `Descend` → cwd == parent.
- `selection_toggle_on_parent_row_is_noop`; `selection/copy never include ..`.
- `parent_row_present_regardless_of_filter_or_hidden`.
- Updated: the ~existing tests asserting raw `cursor`/positions shift by
  `parent_offset`.

**TUI** (`pane.rs` / `lib.rs`):
- `render_prepends_parent_row_non_root` (TestBackend: first row shows `..`).
- `render_no_parent_row_at_root`.
- `sync_cursor_maps_to_virtual_row`.
- `double_click_parent_row_ascends` (mouse path).
- Updated: existing pane/mouse tests asserting positions shift by `parent_offset`.

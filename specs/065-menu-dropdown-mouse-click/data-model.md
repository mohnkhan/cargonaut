# Data Model: Click-on-dropdown-item support

**Feature**: 065-menu-dropdown-mouse-click | **Date**: 2026-06-22

No new persisted entities or config keys. This feature operates entirely on the existing
in-memory `MenuBar` and on transient mouse events. The "model" here is the relationship
between menu state and rendered geometry that hit-testing relies on.

## Entities

### MenuBar (existing — `chrome.rs`)

| Field | Type | Meaning |
|-------|------|---------|
| `menus` | `Vec<Menu>` | Ordered titles, each with `items: Vec<(&'static str, Command)>`. Unchanged. |
| `open` | `Option<usize>` | Index of the open menu, or `None`. Unchanged. |
| `item_sel` | `usize` | Highlighted item within the open menu. Now also set by mouse click and hover (was keyboard-only). |

No fields added or removed.

### Mouse event (existing — crossterm)

| Field | Type | Use in this feature |
|-------|------|---------------------|
| `kind` | `MouseEventKind` | `Down(MouseButton::Left)` → invoke/close/switch; `Moved` → hover-highlight. Others ignored. |
| `column` / `row` | `u16` | Screen `(x, y)` tested against title rects and the dropdown rect. |

### Mouse-capture state (existing — `lib.rs`)

`config.ui.mouse` (session) AND `UiState.mouse_enabled` (runtime) gate whether
`EnableMouseCapture` is active. When off, no event reaches `handle_mouse`. No change; relied
upon for FR-009.

### FrameLayout (existing — `lib.rs`, one field added)

| Field | Type | Meaning |
|-------|------|---------|
| `menu`, `left`, `right`, `fkeys`, `subshell` | `Rect`/`Option<Rect>` | Existing hit-test rects. Unchanged. |
| `full` | `Rect` | **NEW** — the full terminal rectangle (`f.size()`), set in `draw_frame`. Passed as the buffer arg to `item_at`/`in_dropdown` so hit-test height-clamping matches `render`. (finding U1) |

## Geometry relationship (the hit-test contract)

For an open menu at title index `i`, with menu-bar `area` and buffer area `buf`:

```
dropdown.x      = title_rects(area)[i].x
dropdown.y      = area.y + 1
dropdown.width  = (max item-label len + 4), clamped to buf width from dropdown.x
dropdown.height = (item_count + 2), clamped to buf height from dropdown.y   // +2 = top+bottom border
item row i      => screen y = dropdown.y + 1 + i      // +1 skips the top border
visible items   => those with (dropdown.y + 1 + i) < (dropdown.y + dropdown.height - 1)
```

`item_at(area, buf, x, y)` returns `Some(i)` iff:
- a menu is open, AND
- `x` within `[dropdown.x+1, dropdown.x + dropdown.width - 1)` (inside left/right border), AND
- `y == dropdown.y + 1 + i` for some visible item `i`.

Clicks on the border rows/columns, or on rows beyond the clamped height, return `None`
(→ FR-003 no-op inside frame; → FR-004 close when fully outside).

## Validation rules

- `select(idx)`: clamp `item_sel` to `0..items.len()-1` of the open menu; no-op if no menu open
  or the menu is empty. (All current menus are non-empty.)
- `item_at`: never returns an index ≥ the open menu's item count; never returns an index for a
  row clipped away by buffer-height clamping.

## State transitions (open menu, left-click)

```
click on a DIFFERENT title      → open(that title)        [switch]      (FR-005)
click on the OPEN menu's title  → close()                 [toggle off]  (FR-006)
click on an item row            → select+selected_command → close → dispatch  (FR-001)
click inside frame, not an item → (no change, stays open)               (FR-003)
click fully outside             → close(), then panel-hit logic runs    (FR-004 pass-through)
```

## State transitions (open menu, pointer moved)

```
move over item row i            → select(i)               (FR-007)
move over border / outside rows → (highlight unchanged)   (FR-008)
```

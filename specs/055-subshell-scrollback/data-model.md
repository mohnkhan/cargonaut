# Data Model: Subshell Scrollback Rendering (Feature 055)

No new entities or schema changes. All relevant state already exists.

## Existing Entities (unchanged)

### `SubshellState` (crates/cargonaut-ui-tui/src/subshell.rs)

| Field | Type | Role |
|-------|------|------|
| `parser` | `vt100::Parser` | VT100 state machine; exposes `screen()` and `screen_mut()` |
| `scroll_offset` | `u16` | Rows scrolled up from live view (0 = live bottom) |
| `dead` | `bool` | Shell process exited |
| `scroll_offset` | `u16` | **Already present**; used as input to `set_scrollback` |

New method added (crate-private):
```
screen_mut(&mut self) -> &mut vt100::Screen
```
Mirrors existing `screen(&self) -> &vt100::Screen`. Exposes `set_scrollback` at call sites in lib.rs.

### `vt100::Screen` (external crate)

| API | Mutability | Effect |
|-----|-----------|--------|
| `cell(row, col)` | `&self` | Returns cell from `visible_rows()` — scrollback-offset-aware |
| `cursor_position()` | `&self` | Returns **live** cursor row (NOT scrollback-adjusted) |
| `scrollback()` | `&self` | Returns current `scrollback_offset` (0 = live view) |
| `set_scrollback(rows)` | `&mut self` | Shifts view; clamps to actual scrollback length |

## State Transitions

```
scroll_offset = 0  ──ScrollUp──▶  scroll_offset += 1  (older content visible)
scroll_offset = n  ──ScrollDown──▶  scroll_offset = n-1 (toward live)
scroll_offset = 0  ──ScrollDown──▶  scroll_offset = 0  (clamped by saturating_sub)

Before each draw:
  screen.set_scrollback(scroll_offset)  →  screen renders scrollback content

After each draw:
  screen.set_scrollback(0)             →  screen back to live view for other accesses
```

## No New Persistence or Config

- No new config fields
- No disk writes
- No new crate dependencies

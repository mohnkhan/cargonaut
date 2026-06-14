# Contract: Mouse Interaction

Defines event→action mapping and hit-test regions. Satisfies FR-013..FR-018. Mouse is **on by default** (clarified).

## Enablement

- On start, if `config.ui.mouse` (default now `true`) is on → `EnableMouseCapture`; else do not capture.
- Teardown always `DisableMouseCapture` (best-effort, symmetric with screen teardown).
- Runtime toggle key suspends/resumes capture; terminal hold-modifier (commonly Shift) bypasses capture for native selection (FR-013).

## Event mapping

| Event | Hit region | Action |
|-------|-----------|--------|
| Left down (single) | panel row | focus that panel + `CursorTo(row→index)` (FR-014) |
| Left down (double, same row ≤400 ms) | panel row | `Descend` (enter dir / open file) (FR-015) |
| Left down | function-key button | dispatch that button's `Command` (FR-017) |
| Left down | menu title | open that menu; click item → dispatch (FR-017) |
| Scroll up/down | panel | `CursorUp`/`CursorDown` (FR-016) |
| Left down | empty area below last row | focus panel only; cursor unchanged (edge case) |
| any | outside all regions / border gap | ignored (FR-018) |

## Hit-test

- The loop stores the latest `FrameLayout` (panel/status/menu/fkey rects + per-button/-title sub-rects).
- `point_in(rect, col, row)` via `Rect::contains`.
- Row→entry index: `clicked_row - panel.inner.top + scroll_offset`, clamped to the visible (filter+hidden-masked) subset; out-of-range → no cursor move.
- A click arriving between a resize and the next render uses the last stored layout (acceptable; spec edge case).

## Invariants (testable)

- T-MOUSE-1: with capture disabled, no mouse event changes state (FR-013/SC: keyboard build parity).
- T-MOUSE-2: a left-click at a known (col,row) inside the right panel sets active=Right and cursor=expected index.
- T-MOUSE-3: two left-downs on the same row within the window → `Descend`; on different rows → two cursor moves, no descend.
- T-MOUSE-4: scroll over a panel moves the cursor in the scroll direction.
- T-MOUSE-5: click on fkey button #7 dispatches the Mkdir command; click on a menu title opens it.
- T-MOUSE-6: click outside all regions is a no-op.

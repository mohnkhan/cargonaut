# Research: Subshell Scrollback Rendering (Feature 055)

## R-001: vt100 0.16.2 API — `set_scrollback` and `screen_mut`

**Decision**: Use `vt100::Parser::screen_mut() -> &mut Screen` combined with `Screen::set_scrollback(rows: usize)`.

**Rationale**: Both APIs are present in vt100 0.16.2 (verified in `~/.cargo/registry/src/.../vt100-0.16.2/src/`):
- `Parser::screen_mut(&mut self) -> &mut crate::Screen` (parser.rs:62)
- `Screen::set_scrollback(&mut self, rows: usize)` (screen.rs:113) — clamps internally to `rows.min(scrollback.len())`

**Alternatives considered**: Extending the scrollback capacity (parser created with 200 rows), passing `&mut Screen` through `draw_frame`, using `unsafe` transmute for interior mutability. All rejected as unnecessary given the two-step approach (set before draw, reset after).

## R-002: Apply Scrollback — Where to Call `set_scrollback`

**Decision**: Apply `set_scrollback(scroll_offset)` BEFORE extracting `subshell_screen: Option<&vt100::Screen>` in lib.rs, and reset to 0 AFTER `term.draw(...)` returns.

**Rationale**: 
1. Borrow checker: `set_scrollback` requires `&mut Screen`; the subsequent `screen()` call returns `&Screen`. Rust allows sequential mutable-then-immutable borrows on the same value when the mutable borrow ends first.
2. Draw correctness: the immutable `&Screen` obtained AFTER `set_scrollback` has `scrollback_offset` already set; `Screen::cell(row, col)` uses `visible_rows()` which respects `scrollback_offset`. No changes to `render_vt100_screen` internals are needed to see scrollback content.
3. No `draw_frame` signature change needed: `subshell_screen: Option<&vt100::Screen>` passes through unchanged.
4. Thread safety: `poll_output()` is called before the set_scrollback call and before `term.draw()`, all in the same async task. No concurrent PTY writes occur during the draw.

**Alternatives considered**:
- Pass `&mut vt100::Screen` through `draw_frame` and `render_vt100_screen`. Rejected: large signature churn for no benefit.
- Use `RefCell<vt100::Parser>`. Rejected: adds runtime overhead and complexity.
- Apply offset after draw (set_scrollback after rendering, before reset). Rejected: the screen ref is extracted before draw; the only window to apply the offset is before extraction.

## R-003: Cursor Rendering During Scrollback

**Decision**: Skip cursor rendering in `render_vt100_screen` when `screen.scrollback() > 0`.

**Rationale**: `Screen::cursor_position()` returns `grid().pos()` — the live cursor row in the live terminal coordinate system (row 0..height). After `set_scrollback(n)`, `Screen::cell(row, col)` returns content from the scrollback view, but `cursor_position()` still returns the live cursor row. Drawing the cursor at the live row would overlap a scrollback cell with no relation to the actual cursor position. The cleanest fix is `if screen.scrollback() == 0 { ... render cursor ... }`.

**Alternatives considered**:
- Compute the scrollback-adjusted cursor row (`live_row - n`). Rejected: `u16` underflow risk; `cursor_position()` returns unsigned values that could wrap.
- Pass `scroll_offset` as a separate parameter to `render_vt100_screen`. Rejected: redundant; `screen.scrollback()` returns the same information.

## R-004: Scroll Direction Bug

**Decision**: Fix `ScrollDown`/`ScrollUp` direction — swap `saturating_add`/`saturating_sub` so that scrolling UP with the mouse wheel increases `scroll_offset` (older content) and scrolling DOWN decreases it (newer content, toward live view).

**Rationale**: Standard terminal emulator convention (xterm, gnome-terminal, iTerm2, alacritty): mouse wheel UP = see older content = `scroll_offset++`; mouse wheel DOWN = return to live = `scroll_offset--`. The current code has `ScrollDown → saturating_add(1)` (older) and `ScrollUp → saturating_sub(1)` (newer), which is inverted. Since issue #79 says "the plumbing is in place," this inversion would have produced visually backward scrolling had the render been wired up.

**Alternatives considered**: Leave as-is and let users discover the inverted convention. Rejected: inverted scroll is a usability defect that would generate immediate bug reports.

## R-005: `SubshellState::screen_mut` Accessor

**Decision**: Add `pub(crate) fn screen_mut(&mut self) -> &mut vt100::Screen { self.parser.screen_mut() }` to mirror the existing `pub(crate) fn screen(&self) -> &vt100::Screen`.

**Rationale**: Keeps call sites in lib.rs symmetrical (`s.screen()` / `s.screen_mut()`); encapsulates parser field access. The method is `pub(crate)` since `SubshellState` is crate-private.

## R-006: Clamping of `scroll_offset`

**Decision**: Rely on vt100's internal clamping; do not add explicit clamping in the scroll event handler.

**Rationale**: `Screen::set_scrollback` clamps `rows.min(self.scrollback.len())`. If a user scrolls up 300 rows in a buffer with only 200 rows, `scroll_offset` holds 300 but `set_scrollback(300)` applies only 200. The next frame repeats correctly. The `scroll_offset` `u16` type has `u16::MAX = 65535` headroom — far above the 200-row parser buffer — so no real overflow risk. Adding explicit clamping would require reading the scrollback buffer size, which is not exposed by the vt100 API on `Screen`.

## R-007: TDD — Test for SC-004 (New Automated Test)

**Decision**: Add a unit test `render_vt100_screen_scrollback_offset_changes_content` in `subshell.rs::tests` that:
1. Creates a `vt100::Parser::new(5, 10, 20)` (5 visible rows, 20 scrollback rows)
2. Feeds it 25 lines (line 0..24) to fill the visible area and push 20 lines into scrollback
3. Calls `render_vt100_screen` with `set_scrollback(0)` → captures buffer A
4. Calls `render_vt100_screen` with `set_scrollback(5)` → captures buffer B
5. Asserts A ≠ B

**Rationale**: Directly verifies SC-004 ("at least one automated test demonstrates that a non-zero `scroll_offset` changes rendered output when scrollback content exists"). The test uses the same `render_vt100_screen` function and a real `vt100::Parser` so it exercises the actual render path.

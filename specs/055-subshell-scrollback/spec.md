# Feature Specification: Subshell Scrollback Rendering

**Feature Branch**: `055-subshell-scrollback`

**Created**: 2026-06-19

**Status**: Draft

**Input**: Wire scroll_offset into render_vt100_screen for subshell scrollback (closes #79). SubshellState::scroll_offset is already tracked and adjusted on mouse scroll events (lib.rs) but render_vt100_screen in subshell.rs always renders from row 0 regardless of the offset. Scrolling the mouse wheel inside the subshell panel changes scroll_offset but has no visible effect. Fix by passing scroll_offset into render_vt100_screen, calling screen.set_scrollback(scroll_offset) before rendering and restoring to 0 after. The storage and event plumbing are already in place — only the render wiring is missing.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Scroll Up Through Subshell History (Priority: P1)

A user running commands in the subshell panel (e.g., a long `ls -la` or build output) scrolls the mouse wheel upward inside the panel. Previously output lines that had scrolled off the top of the panel become visible. Scrolling back down returns to the live terminal view.

**Why this priority**: This is the sole functional gap: the scroll offset is already captured and updated on every wheel event, but the render function ignores it. Without this wiring, the feature shipped in Feature 054 is essentially broken for users who need to review prior output.

**Independent Test**: Open the subshell, run a command that produces more output than the panel height (e.g., `seq 1 200`), scroll the mouse wheel up inside the panel. Lines that scrolled off the top must now appear.

**Acceptance Scenarios**:

1. **Given** the subshell panel is open and has produced more output than its visible height, **When** the user scrolls the mouse wheel up inside the panel, **Then** earlier output lines scroll into view — one row per wheel-click event (proportional = 1 scroll unit per `MouseEventKind::ScrollUp` event).
2. **Given** the user has scrolled up N rows into scrollback history, **When** the user scrolls the mouse wheel down, **Then** the view moves toward the live screen bottom; scrolling fully down restores the live terminal view.
3. **Given** the user is at the live bottom of the terminal, **When** the user scrolls down further, **Then** the view remains at the bottom (`scroll_offset` clamps to 0 via `saturating_sub`; no cell content changes across consecutive frames at boundary).
4. **Given** the user has scrolled up, **When** the subshell produces new output, **Then** the historical view is preserved (`scroll_offset` value is unchanged — it is only modified by mouse scroll events, not by PTY output) until the user scrolls back down.

---

### User Story 2 — Scrollback at Maximum History Boundary (Priority: P2)

A user scrolls up until there is no more history available in the vt100 scrollback buffer. The view stops moving; subsequent upward scrolls have no additional effect.

**Why this priority**: Boundary correctness prevents visual artefacts and crashes at the edge of the scrollback buffer.

**Independent Test**: Fill the subshell with output exceeding the scrollback buffer capacity (200 rows), then scroll up until the oldest available line is visible. Additional upward scrolls must not panic; all rendered cells must contain valid content (no empty or garbage symbols — i.e., every cell is either a space or a printable character from the command output).

**Acceptance Scenarios**:

1. **Given** `scroll_offset` equals the total available scrollback rows (up to 200 rows — the fixed scrollback capacity of the vt100 parser), **When** the user scrolls up, **Then** `scroll_offset` does not increase beyond the available buffer (clamped internally by `vt100::Screen::set_scrollback`) and the display is stable.
2. **Given** an empty scrollback buffer (terminal just opened), **When** the user scrolls up, **Then** nothing happens and `scroll_offset` stays at 0.

---

### Edge Cases

- What happens when the panel is resized while the user has scrolled up? `SubshellState::resize()` replaces the parser with a fresh one (empty scrollback); `scroll_offset` MUST be reset to 0 at the same time to prevent a stale offset from referencing an empty buffer (addressed by T017).
- How does the system behave if `set_scrollback` is called on an already-mutably-borrowed screen? The implementation must ensure no aliased mutable access.
- What if the subshell process exits while the user is scrolled into history? When `dead = true`, the panel renders a restart notice ("Shell exited — press Ctrl-o to restart") instead of the scrollback; scrollback readability is deferred until the shell is restarted (out of scope for this feature).
- What happens when `scroll_offset` exceeds the actual scrollback rows available from the vt100 parser? The offset must be clamped before calling `set_scrollback`.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The subshell render function MUST apply `scroll_offset` when painting the terminal screen so that the displayed rows correspond to the scrollback position chosen by the user.
- **FR-002**: After rendering, the scrollback position of the internal vt100 screen MUST be restored to 0 (live bottom) so that subsequent non-render accesses to the screen are unaffected.
- **FR-003**: The `scroll_offset` value passed to the render function MUST be clamped to the range `[0, available_scrollback_rows]` to prevent out-of-bounds access.
- **FR-004**: Scrolling up MUST display earlier terminal output; scrolling back down MUST return to the live terminal view, with no visible artefacts at either boundary. The terminal cursor MUST NOT be rendered when `scroll_offset > 0` (scrollback mode), since `cursor_position()` returns live coordinates that do not correspond to the scrollback view.
- **FR-005**: The rendering change MUST NOT introduce frame-rate regression; the subshell panel MUST continue to redraw within the existing 16 ms keypress-to-first-paint budget.
- **FR-006**: The render wiring MUST be covered by at least one automated test that verifies a non-zero `scroll_offset` produces a different rendered frame than offset 0 when scrollback content exists.

### Key Entities

- **SubshellState**: Holds `scroll_offset: u16` (already present). No schema change needed.
- **render_vt100_screen**: The render function in `subshell.rs` that paints the vt100 screen into a Ratatui buffer. Painted content will reflect the scrollback position chosen by the user; no signature change is needed.
- **vt100::Screen**: The vt100 library's screen type. Provides `set_scrollback(&mut self, rows: usize)` to shift the view into scrollback history.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Mouse scroll up in the subshell panel makes earlier output visible within one rendered frame (≤16 ms after the scroll event).
- **SC-002**: Mouse scroll down from any scrollback position returns fully to the live terminal view within one rendered frame.
- **SC-003**: All existing tests continue to pass; no new test failures introduced.
- **SC-004**: At least one new automated test demonstrates that a non-zero `scroll_offset` changes the rendered output when scrollback content exists.
- **SC-005**: No panic or crash occurs when `scroll_offset` equals or exceeds the available scrollback row count.

## Assumptions

- `SubshellState::scroll_offset` is already updated on mouse wheel events (Feature 054 shipped this plumbing); only the render path is missing. Note: the scroll direction was found to be inverted at implementation time (`ScrollDown` incremented instead of decrementing); corrected by this feature as part of the render wiring work.
- The vt100 0.16 `Screen::set_scrollback(&mut self, rows: usize)` API is available and behaves as documented: shifts the visible window into history by the given number of rows, silently clamping to the available buffer depth.
- The subshell draw path holds exclusive access to the `vt100::Parser` (and thus `&mut Screen`) during the render call; no concurrent mutation by the PTY reader thread occurs during drawing (draw is driven by the event loop, PTY writes arrive via channel and are applied before or after draw, not during).
- Mobile / non-mouse input paths (keyboard scroll) are out of scope for this feature; the existing mouse wheel event handling is sufficient.
- The change is self-contained to `subshell.rs` and the draw call site in `lib.rs`; no new dependencies or crate-level changes are required.

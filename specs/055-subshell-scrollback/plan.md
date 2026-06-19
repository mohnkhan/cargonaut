# Implementation Plan: Subshell Scrollback Rendering

**Branch**: `055-subshell-scrollback` | **Date**: 2026-06-19 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/055-subshell-scrollback/spec.md`

## Summary

Wire `SubshellState::scroll_offset` into the subshell render path. The `scroll_offset` field is already updated on mouse wheel events (lib.rs:1899, 1914) but `render_vt100_screen` (subshell.rs:346) always paints from vt100 row 0. The fix applies `vt100::Screen::set_scrollback(scroll_offset)` before extracting the immutable screen reference for drawing, then resets to 0 afterward. No changes to `draw_frame` or `render_vt100_screen` signatures are required.

Also fixes an inverted scroll direction bug: `MouseEventKind::ScrollDown` currently calls `saturating_add(1)` (shows older content) when standard terminal convention requires `saturating_sub(1)` (returns toward live), and vice versa for `ScrollUp`.

## Technical Context

**Language/Version**: Rust (stable, edition 2021)

**Primary Dependencies**: vt100 0.16.2 (Parser, Screen, set_scrollback), ratatui (Buffer, Rect), crossterm (MouseEventKind)

**Storage**: N/A — in-memory VT100 parser state only

**Testing**: cargo test (unit tests in subshell.rs mod tests)

**Target Platform**: Linux TUI (same as parent project)

**Project Type**: TUI desktop application

**Performance Goals**: ≤16 ms keypress→first-paint (NFR-002, Constitution §IV). No extra allocation per frame — `set_scrollback` mutates in-place.

**Constraints**: `scroll_offset` is `u16`; vt100 parser created with 200 scrollback rows; clamping is handled by `Screen::set_scrollback` internally.

**Scale/Scope**: Two file changes — `crates/cargonaut-ui-tui/src/subshell.rs` and `crates/cargonaut-ui-tui/src/lib.rs`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| §I Code Quality — clippy clean | PASS | No unsafe; all changes are idiomatic Rust |
| §I Code Quality — `#![warn(missing_docs)]` | PASS | No new public APIs; `screen_mut` is `pub(crate)` |
| §II Test-First (TDD) | PASS | Red test before green per task plan |
| §II SC CI gates | PARTIAL | SC-004 added to test suite. SC-001/SC-002 rely on the existing `benches/keypress-latency.rs` bench — but `ci-local` (scripts/ci/ci-local.sh) does NOT run `cargo bench` in the per-PR pipeline (intentional: bench runs are release-mode only). T016 covers manual bench validation before merging; this gap is pre-existing and not introduced by this feature. |
| §III UX Consistency | PASS | No new keymap entries or dialogs |
| §IV Performance ≤16 ms | PASS | `set_scrollback` is O(1) in-place mutation; no heap allocation |
| §V SSD Preservation | N/A (CI-only concern) | Dev builds via `make build` (tmpfs-guarded) |

No violations. Complexity table omitted.

## Project Structure

### Documentation (this feature)

```text
specs/055-subshell-scrollback/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── checklists/
│   └── requirements.md  # Spec quality checklist
└── tasks.md             # Phase 2 output (speckit-tasks)
```

### Source Code (repository root)

```text
crates/cargonaut-ui-tui/src/
├── subshell.rs          # screen_mut() accessor; render_vt100_screen cursor-skip; tests
└── lib.rs               # set_scrollback before draw; reset after draw; scroll-direction fix
```

## Phase 0: Research

See [research.md](research.md).

## Phase 1: Design

See [data-model.md](data-model.md) and [quickstart.md](quickstart.md).

## Implementation Approach

### T1: Scroll Direction Fix (lib.rs)

Lines 1899 and 1914 in `lib.rs` have inverted scroll semantics:
- `ScrollDown` → `saturating_add(1)` moves into OLDER history (wrong; standard is scroll down = newer)
- `ScrollUp` → `saturating_sub(1)` moves toward NEWER content (wrong; standard is scroll up = older)

Fix: swap the operations so `ScrollUp` → `saturating_add(1)` and `ScrollDown` → `saturating_sub(1)`.

### T2: `screen_mut` accessor (subshell.rs)

Add `pub(crate) fn screen_mut(&mut self) -> &mut vt100::Screen { self.parser.screen_mut() }` alongside the existing `screen(&self)` method.

### T3: Apply scrollback before draw, reset after (lib.rs)

```rust
// Before subshell_screen extraction (after poll_output):
if let Some(s) = ui.subshell.as_mut() {
    let offset = s.scroll_offset as usize;
    s.screen_mut().set_scrollback(offset);
}
// Existing line stays unchanged:
let subshell_screen: Option<&vt100::Screen> = ui.subshell.as_ref().map(|s| s.screen());
// ... draw ...
term.draw(|f| { ... })?;
// After draw — reset so non-render screen accesses see live view:
if let Some(s) = ui.subshell.as_mut() {
    s.screen_mut().set_scrollback(0);
}
```

No changes to `draw_frame` or `render_vt100_screen` signatures.

### T4: Hide cursor in scrollback mode (subshell.rs)

In `render_vt100_screen`, `cursor_position()` always returns the **live** cursor row (not scrollback-adjusted). Drawing it during scrollback would place the cursor in the wrong cell. Guard the cursor block:

```rust
// Only render cursor when at live bottom (scrollback == 0)
if screen.scrollback() == 0 {
    let (cur_row, cur_col) = screen.cursor_position();
    // ... existing cursor rendering ...
}
```

### T5: Test (subshell.rs)

Red test (committed before T2–T4):
```rust
#[test]
fn render_vt100_screen_scrollback_offset_changes_content() {
    // Feed parser with 30 lines into a 10-row terminal to generate scrollback
    // Apply set_scrollback(5) → render → compare buffer to live render
    // Expect them to differ
    todo!()  // red: compile passes, test panics
}
```

Green test: replace `todo!()` with the actual implementation.

## Complexity Tracking

No constitution violations.

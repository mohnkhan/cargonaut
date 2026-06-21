# Research: Click-on-dropdown-item support

**Feature**: 065-menu-dropdown-mouse-click | **Date**: 2026-06-22

All Technical Context items were resolvable from the existing codebase; no external research
or NEEDS CLARIFICATION markers remained after `/speckit-clarify`. This document records the
design decisions and the code facts they rest on.

## Code facts (current state)

- `MenuBar` lives in `crates/cargonaut-ui-tui/src/chrome.rs` with fields `menus`, `open:
  Option<usize>`, `item_sel: usize`. Public API: `open`, `open_first`, `close`, `select_down`,
  `select_up`, `next_menu`, `prev_menu`, `selected_command`, `title_at`, `titles`, `is_open`,
  `render`.
- `render()` (chrome.rs ~376-411) computes the dropdown rect inline: `x = title rect.x`,
  `y = area.y + 1`, `width = max item label len + 4`, `height = (items + 2)` clamped to the
  buffer, then draws a bordered `List` with `highlight` on `item_sel`. Items are inside a
  one-cell border, so item row *i* renders at `y + 1 + i`.
- Mouse handling is `handle_mouse` in `crates/cargonaut-ui-tui/src/lib.rs` (~2227-2353),
  matching `ScrollDown`, `ScrollUp`, `Down(MouseButton::Left)`, and `_ => {}`. The left-click
  arm already: (1) subshell focus, (2) `fkeybar.command_at`, (3) `menu.title_at` → `open(idx)`,
  (4) panel row → focus + cursor + double-click descend.
- Keyboard menu dispatch (lib.rs ~688-706): when `menu.is_open()`, Enter does
  `menu.selected_command()` → `menu.close()` → `dispatch_ui_command(...)`. This is the
  canonical dispatch path the mouse path must reuse (FR-012).
- Mouse capture is gated upstream: `EnableMouseCapture` is only issued when `config.ui.mouse`
  and runtime `mouse_enabled` are on; Alt-m toggles it. So when mouse is off, no `MouseEvent`
  reaches `handle_mouse` at all (FR-009 falls out for free).

## Decisions

### D1 — Single source of dropdown geometry

**Decision**: Extract the rect math from `render()` into `MenuBar::dropdown_rect(&self, area:
Rect, buf_area: Rect) -> Option<Rect>` and call it from both `render()` and the new
`item_at()`.

**Rationale**: FR-002 demands the clickable rows match the rendered rows exactly. Two
independent copies of the geometry would drift the first time the border, width, or clamping
logic changes. One function = one truth.

**Alternatives considered**: (a) Store the last-rendered rect in a field and read it in
`item_at`. Rejected — couples hit-testing to a prior render call, brittle in tests and on the
first event of a frame. (b) Duplicate the math in `item_at`. Rejected — drift risk, exactly
the bug FR-002 calls out.

### D2 — Hover via `MouseEventKind::Moved`

**Decision**: Add a `Moved` arm to `handle_mouse`. If a menu is open and the position maps to
an item via `item_at`, call `menu.select(idx)`. Otherwise do nothing.

**Rationale**: crossterm reports motion as `Moved` when capture is active. Terminals that do
not report motion simply never emit `Moved`, so click-to-invoke keeps working with no special
casing — that is the graceful degradation FR-010 requires. `Drag` (button-held motion) is not
needed; menus have no drag semantics.

**Alternatives considered**: tracking a separate "hovered" index distinct from `item_sel`.
Rejected — the spec wants hover to *update selection* (so a later Enter/click acts on it),
which is exactly what reusing `item_sel` gives; a second field would have to be reconciled.

### D3 — Close-and-pass-through for outside clicks

**Decision**: For a `Down(Left)` when a menu is open: check title → switch/toggle; else check
`item_at` → invoke + close; else `menu.close()` and **fall through** to the existing panel-hit
logic so the same click also focuses/moves/descends.

**Rationale**: The 2026-06-22 clarification chose pass-through (one click closes the menu *and*
acts on the panel). Closing first then running the unchanged panel branch yields this with no
new panel code.

**Alternatives considered**: swallow the closing click (return early after `close()`).
Rejected by clarification — would force a second click to act on the panel.

### D4 — `select(idx)` setter, no new state

**Decision**: Add `pub fn select(&mut self, idx: usize)` that clamps to the open menu's item
count and sets `item_sel`; used by both click and hover. No new struct fields.

**Rationale**: Hover and click both need to set an arbitrary item index; the existing API only
has relative `select_down`/`select_up`. A clamped absolute setter is the minimal addition and
keeps `item_sel` the single selection source.

### D5 — Buffer area via a new `FrameLayout.full` field (resolves analysis finding U1)

**Decision**: `item_at`/`dropdown_rect` take the full terminal `Rect` to clamp height the same
way `render` does. `handle_mouse` has no such rect today (`FrameLayout` holds only menu/panes/
fkeys/subshell). Add a `full: Rect` field to `FrameLayout`, set in `draw_frame` from the
already-computed `let area = f.size();` (lib.rs:2940), and pass `ui.layout.full` into
`item_at`.

**Rationale**: `f.size()` is exactly the buffer area `render` clamps against, so hit-test
clamping and render clamping agree by construction. Reuses an existing value; no new
computation.

**Alternatives considered**: (a) Re-query terminal size inside `handle_mouse`. Rejected —
duplicate source, can drift from the frame the user actually sees. (b) Skip height clamping in
`item_at`. Rejected — would let clicks on clipped-away rows select hidden items (the short-
terminal edge case).

### D6 — `in_dropdown` predicate instead of exposing the rect (resolves finding I1)

**Decision**: Keep `dropdown_rect` private; add a public `in_dropdown(area, buf, x, y) -> bool`
so `handle_mouse` (in `lib.rs`) can distinguish a click inside the dropdown frame but off the
items (FR-003 no-op) from a click fully outside (FR-004 close + pass-through).

**Rationale**: `handle_mouse` cannot reach a private method; the only thing it needs is the
boolean "is this point within the dropdown", not the geometry. A predicate is the minimal
public surface and keeps the rect encapsulated.

**Alternatives considered**: make `dropdown_rect` `pub(crate)`. Rejected — exposes geometry the
caller would have to re-interpret; the predicate is clearer and harder to misuse.

## Open questions

None. Both clarifications (pass-through; hover in-scope) are recorded in spec.md §Clarifications.
All `/speckit-analyze` findings (U1, C1, A1, I1) have concrete remediations applied across
contracts/data-model/plan/tasks.

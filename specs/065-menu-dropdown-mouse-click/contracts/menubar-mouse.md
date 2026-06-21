# Contract: MenuBar mouse interaction

**Feature**: 065-menu-dropdown-mouse-click | **Date**: 2026-06-22

This is the UI contract for mouse interaction with the pull-down menu bar. It defines the new
`MenuBar` methods and the `handle_mouse` integration order. Signatures are indicative (Rust);
the binding obligation is the *behavior*, enforced by the tests listed under each item.

## New `MenuBar` methods (`chrome.rs`)

### `dropdown_rect`

```rust
/// The rectangle the open dropdown occupies for the given bar `area` and
/// buffer area, or `None` if no menu is open. Mirrors `render`'s geometry so
/// hit-testing and drawing can never disagree.
fn dropdown_rect(&self, area: Rect, buf: Rect) -> Option<Rect>;
```

- MUST return the same rect (origin, clamped width, clamped height) that `render` draws.
- MUST return `None` when `self.open` is `None`.
- `render` MUST be refactored to consume this method (single source of truth, FR-002).
- **Visibility (resolved, finding I1)**: private `fn`. Only `item_at`, `render`, and the
  in-module `chrome.rs` unit tests use it — all reachable without `pub`. Integration tests in
  `lib.rs` go through the public `item_at`, never `dropdown_rect` directly, so it stays private.
- **Buffer source (resolved, finding U1)**: `buf` is the full terminal rectangle (= `f.size()`
  in `draw_frame`, = `buf.area()` in `render`). It is needed to clamp the dropdown height
  exactly as `render` does for short terminals. See the `FrameLayout.full` contract below.

### `item_at`

```rust
/// Hit-test a point against the open dropdown's item rows. Returns the item
/// index under `(x, y)`, or `None` for clicks on the border, outside the
/// dropdown, on rows clipped by a short terminal, or when no menu is open.
pub fn item_at(&self, area: Rect, buf: Rect, x: u16, y: u16) -> Option<usize>;
```

- `Some(i)` ⟺ a menu is open AND `(x,y)` lies on the rendered row of visible item `i` (interior,
  not border). See data-model.md geometry.
- MUST NOT return an index ≥ open menu item count.
- MUST NOT return an index whose row was clamped away (short terminal).
- Pure; no `&mut self`; no side effects.
- **Tests**: `menu_bar_item_hit_test` (first row, last row, border row → None, outside → None,
  closed → None), `menu_bar_item_hit_test_clamped` (short buffer: clipped row → None).

### `in_dropdown`

```rust
/// Whether `(x, y)` falls anywhere within the open dropdown's rectangle
/// (including its border), used to tell a "click inside the frame but not on
/// an item" (no-op, FR-003) apart from a "fully outside" click (close +
/// pass-through, FR-004). Returns `false` when no menu is open.
pub fn in_dropdown(&self, area: Rect, buf: Rect, x: u16, y: u16) -> bool;
```

- Pure; `&self`. Equivalent to `dropdown_rect(area, buf).is_some_and(|r| rect_contains(r, x, y))`.
- Public because the caller is `handle_mouse` in `lib.rs` (cannot reach the private
  `dropdown_rect`); this is the minimal public surface that keeps the rect itself encapsulated
  (finding I1 resolution).
- **Tests**: `menu_bar_in_dropdown` (point on border → true, point on item → true, point just
  outside → false, closed → false).

### `select`

```rust
/// Set the highlighted item directly (used by mouse click and hover).
/// Clamps to the open menu's item range; no-op if no menu is open.
pub fn select(&mut self, idx: usize);
```

- After `select(i)` on an open non-empty menu, `selected_command()` returns item `i`'s command
  (clamped if `i` ≥ len).
- No-op when `self.open` is `None`.
- **Tests**: `menu_bar_select_sets_item` (select + `selected_command` agree; out-of-range
  clamps; closed → no panic / no change).

## `FrameLayout.full` (`lib.rs`) — buffer-area plumbing (finding U1)

`handle_mouse` hit-tests against `ui.layout` rects but `FrameLayout` (lib.rs:105) currently has
no full-screen rect, while `item_at`/`dropdown_rect` need the terminal size to clamp height.

**Contract**: add a `full: Rect` field to `FrameLayout`, populated in `draw_frame` from the
existing `let area = f.size();` (the value already computed at lib.rs:2940). `handle_mouse`
then passes `ui.layout.full` as the `buf` argument to `item_at`.

- MUST be set on every real frame (the `draw_frame` return at lib.rs:3112).
- The two test-only `FrameLayout { … }` literals (lib.rs:3396, 5708) MUST also set `full`
  (to the test's terminal rect) so they compile and exercise realistic clamping.
- No behavior change to existing fields.

## `handle_mouse` integration (`lib.rs`)

### `Down(MouseButton::Left)` — ordering obligation

When the left button goes down, evaluation order MUST be:

1. Subshell focus (existing) — unchanged.
2. Function-key bar `command_at` (existing) — unchanged.
3. Menu-bar `title_at` (existing) → `open(idx)` (switch/toggle). Unchanged; already handles
   switching to a different menu (FR-005) and toggling the open one (FR-006).
4. **NEW** — if `menu.is_open()` and `item_at(ui.layout.menu, ui.layout.full, x, y) == Some(i)`:
   `menu.select(i)` → take `selected_command()` → `menu.close()` →
   `dispatch_ui_command(cmd, …)` → `return Ok(())`. (FR-001, FR-012)
5. **NEW** — else if `menu.is_open()` and `in_dropdown(ui.layout.menu, ui.layout.full, x, y)`
   (inside the frame but not on an item, e.g. the border): `return Ok(())` — no-op, menu stays
   open (FR-003).
6. **NEW** — else if `menu.is_open()` (the click is fully outside title + dropdown):
   `menu.close()` and **continue** (do NOT return) so the existing panel-hit logic runs for the
   same event (FR-004 close-and-pass-through).
7. Panel rows: focus + cursor + double-click descend (existing) — unchanged, now also reached
   via step 6.

> Ordering note: step 4 (`item_at`) must be checked before step 5 (`in_dropdown`), since an item
> row is also inside the dropdown rect. Item-hit wins; border/interior-non-item falls to the
> no-op; everything else closes and passes through.

- **Tests** (integration, `lib.rs` test module, `T-MENU-MOUSE-*`):
  - click item dispatches its command and closes the menu (FR-001).
  - click first/last item maps correctly (FR-002 — no off-by-one).
  - click on dropdown border leaves menu open, dispatches nothing (FR-003).
  - click outside on a panel closes the menu AND focuses pane + moves cursor (FR-004).
  - click a different title switches; click same title closes (FR-005/006).
  - mouse disabled/suspended → no menu mouse effect (FR-009) — exercised via the existing
    disabled-mouse harness.

### `Moved` — new arm

```rust
MouseEventKind::Moved => {
    if ui.menu.is_open() {
        if let Some(i) = ui.menu.item_at(ui.layout.menu, ui.layout.full, x, y) {
            ui.menu.select(i);
        }
    }
}
```

- MUST be O(1) and allocation-free (FR-007 perf; NFR-002).
- Movement over border/outside rows MUST leave `item_sel` unchanged (FR-008) — guaranteed by
  `item_at` returning `None` there.
- MUST dispatch no command.
- Absence of `Moved` events (terminal without motion reporting) ⇒ feature still works via
  clicks (FR-010).
- **Tests**: `menu_hover_moves_highlight` (move over item 2 → `selected_command` is item 2, no
  dispatch), `menu_hover_border_no_change` (move over border → highlight unchanged).

## Non-goals (contract boundaries)

- No right/middle button handling.
- No scroll-wheel navigation inside the dropdown.
- No change to keyboard menu handling (lib.rs ~688-706) or to `theme.menu_*`.
- No new public types; no changes to the `Command` enum or keymap.toml.

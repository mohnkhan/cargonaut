# Research: Pane Tabs (Feature 053)

## R-001: Tab Bar Widget — Custom vs. ratatui Built-in

**Decision**: Build a custom inline tab bar renderer, not ratatui's `Tabs` widget.

**Rationale**: ratatui's `Tabs` widget does not support horizontal scrolling to keep the active tab in view (FR-005). It also imposes specific styling (dividers between tabs) that doesn't match the orthodox-FM aesthetic (`[1]dir1  [2*]dir2`). A custom renderer is ~20 lines of Span arithmetic and gives full control over truncation, scroll offset, and style.

**Implementation sketch**:
```
tab_bar_line(entries: &[TabBarEntry], width: u16, theme: &Theme) -> Line
```
- Label per tab: `[N]basename` or `[N*]basename` (active)
- Truncate `basename` to min(20, available) chars
- Compute cumulative widths; find start offset that keeps active tab in view
- Return a `Line` of `Span`s with appropriate styles

**Alternatives considered**:
- `ratatui::widgets::Tabs` — no horizontal scroll, fixed divider style, harder to style per-tab width
- A full `StatefulWidget` — overkill for a 1-row bar; inline `Line` construction is simpler

---

## R-002: Horizontal Scroll Algorithm for Tab Bar (FR-005)

**Decision**: Per-frame computed scroll offset (not persistent state).

**Algorithm**:
1. For each tab, compute label width: `len("[N]") + min(basename_len, 20) + 2` (separator)
2. Compute cumulative left-edge positions
3. Find the range `[scroll_start, scroll_start + pane_width)` that contains `active_tab`'s left edge
4. Render only tabs whose label overlaps the visible range, slicing the first/last label if needed

**Why no persistent state**: The user sees the active tab at all times (the scroll adjusts to it). There's no "scroll independently of active tab" interaction. Stateless computation is simpler and avoids bugs when tabs are added/removed.

**Alternatives considered**:
- Store `tab_scroll_offset: usize` per `SideState` — adds state that must be maintained on tab create/close/switch, no user benefit over computed scroll since active tab is always centered

---

## R-003: `[` and `]` Key Codes in crossterm

**Decision**: `[` maps to `KeyCode::Char('[')` with `KeyModifiers::NONE`; `]` maps to `KeyCode::Char(']')` with `KeyModifiers::NONE`. These are standard printable characters with no modifier complications. The keymap parser handles them as `parse_key_chord("[") → KeyChord { code: Char('['), modifiers: NONE }`.

**Verification**: The existing keymap handles `~`, `/`, `:`, `*`, `+`, `-`, `<` as plain char keys — `[` and `]` follow the same pattern. No terminal compatibility concern.

**Conflict check**: `[` was previously unbound (only `<` is `open-fuzzy-filter`); `]` was previously unbound. No existing bindings displaced.

---

## R-004: Tab Operations — Sync vs. Async

**Decision**: `tab_new()`, `tab_close()`, `tab_next()`, `tab_prev()` are all **synchronous** (`fn` not `async fn`), returning `Vec<Event>`.

**Rationale**:
- `tab_new`: clones the active tab's `listing` snapshot (no VFS call). The new tab shows the same directory contents without a re-list; the user can navigate to trigger a fresh list. This avoids an `async fn` for the most common operation.
- `tab_close`, `tab_next`, `tab_prev`: pure state mutations with no I/O.

**Tradeoff**: The cloned listing may be stale if the directory changed since last listed. Acceptable — the user can press the reload mechanism (or navigate away and back) to refresh.

**Alternatives considered**:
- `async fn tab_new` + fresh `local_fs.list()` call — accurate but adds latency (~1-10ms) and requires the caller to `await`, complicating test structure
- "Refresh on tab switch" — could call `relist_active()` on every `tab_next`/`tab_prev`; not required by spec and adds I/O

---

## R-005: State Isolation Between Tabs

**Decision**: Each `PaneState` in `SideState.tabs` is fully independent. Tab operations that create a new tab clone only `cwd` and `listing` from the source; `cursor`, `selected`, `filter`, `show_hidden`, `sort`, and both history vectors start fresh.

**Spec mapping**:
- FR-006: cursor, selection, sort, filter, show-hidden, history all per-tab → each `PaneState` carries these independently
- FR-011: new tab must not inherit filter or selection → enforced at `tab_new` construction

**How `navigate_to` keeps isolation**: `navigate_to(id, path)` calls `self.pane_mut(id)` which now dereferences `sides[idx].tabs[active_tab]`. Only the active tab's state is modified. Inactive tabs' `PaneState`s are untouched.

**How cross-pane ops preserve semantics**: `confirm_copy` reads `self.pane(src_pane).cwd` and `self.pane(dst_pane).cwd`, which both resolve through their respective `active_tab`. The same two-pane semantics hold (FR-007).

---

## R-006: Tab Bar Height — Always 1 Row (FR-004)

**Decision**: The tab bar always occupies exactly 1 terminal row per pane column, regardless of tab count. `draw_pane()` layout changes from:
```
col[0]: Min(2) → list with border
col[1]: Length(1) → mini-status
```
to:
```
col[0]: Length(1) → tab bar
col[1]: Min(2) → list with border
col[2]: Length(1) → mini-status
```

**Constant height rationale (FR-004)**: The pane content area (visible listing rows) is constant whether there is 1 or 10 tabs. No layout jump on first `Ctrl-t`. The cost is 1 terminal row always — typical terminals are 24+ rows, so the trade-off is reasonable.

**Single-tab appearance**: With 1 tab, the bar shows `[1*]dirname` — a single active entry. This is the same as always having the tab bar (per spec clarification).

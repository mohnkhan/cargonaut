# Research: Panel `..` Parent Entry as First Row

**Feature**: 040-parent-row | **Date**: 2026-06-15

No unknown external technologies. The research resolves the *index-model* design
against the existing code (read directly, not external sources).

## R-001 — Where the synthetic `..` row lives: core virtual-row cursor

**Decision**: Make the `..` row a property of the **core cursor model**, not a
TUI-only render trick. Redefine `PaneState.cursor` to index the *virtual row
list* `[.. (when cwd has a parent)] ++ visible_indices()`. A `parent_offset()`
(0 at a root, 1 otherwise) converts between virtual rows and real-entry indices.

**Rationale**:
- Keyboard activation is a **core** command (`Descend`, dispatched from the
  keymap). If the `..` row existed only in the TUI, the TUI key handler would
  have to intercept Enter and decide ascend-vs-descend, diverging from the core
  cursor — two cursors to keep in sync. A core-owned virtual cursor keeps one
  source of truth that `sync_from` copies verbatim (FR-011).
- Mouse hit-testing already computes `index = viewport_top() + row` and dispatches
  `CursorTo(index)` / `Descend` (`crates/cargonaut-ui-tui/src/lib.rs`). With the
  rendered rows including the `..` row, those indices are *already* virtual-row
  indices — so the mouse path needs no special case once `Descend` is parent-row
  aware.
- The issue explicitly asks for "an explicit `has_parent` + index offset in
  PaneState/PaneView".

**Alternatives considered**:
- *TUI render-time synthesis only* (Explore's Option C). Rejected: splits cursor
  semantics across core/TUI and forces the key handler to special-case Enter.
- *Add `..` to `DirListing::entries`*. Rejected outright: shifts every real-entry
  index, breaks the selection set's meaning, and pollutes a VFS type with a UI
  concern.

## R-002 — Selection stays real-indexed; parent-row no-ops fall out for free

**Decision**: Keep `PaneState.selected: BTreeSet<usize>` referring to indices into
`listing.entries` (real entries), unchanged. Update only `focused_entry_index()`
to return `None` when the cursor is on the `..` row (`cursor < parent_offset()`).

**Rationale**: The selection/op paths already gate on `focused_entry_index()`:
- `SelectionToggle` does `if let Some(idx) = focused_entry_index()` → on the
  parent row it's `None`, so toggle is already a no-op (FR-006). **Zero change.**
- `selection_or_focused` (copy/move/delete) uses `selected` (real) or
  `focused_entry_index()` → never yields `..` (FR-008). **Zero change.**
- `SelectionInvert` / `select_by_pattern` iterate `visible_indices()` (real
  entries only); `..` is not among them (FR-007). **Zero change.**
- `recursive_dir_size` / `show_focused_in_other_panel` use `focused_entry_index()`
  → safe no-op on the parent row.

So the only core edits are: the `parent_offset`/`row_count`/`focused_row` helpers,
the `focused_entry_index()` offset, the cursor clamp/reset sites, and the
`Descend` branch. This is why "selection stays stable" (FR-010) is structural.

**Alternatives considered**: A separate `on_parent_row: bool` field alongside a
real-relative cursor — rejected as two-variable cursor state that every command
must keep consistent; the single virtual cursor is simpler.

## R-003 — Root detection drives presence

**Decision**: `parent_offset()` = `self.cwd.parent().is_some() as usize`. At a
filesystem root `VfsPath::parent()` returns `None` (`segments.is_empty()`), so
`parent_offset()` is 0, no `..` row is shown, and the cursor model is identical to
today's (FR-002, SC-002). The same `parent()` check already guards
`ascend_to_parent`, so ascent above root remains impossible.

**Rationale**: Reuses the existing, tested root detection; presence and ascent
share one predicate, so they can't disagree.

## R-004 — Default cursor = first real entry (clarified)

**Decision**: On any fresh listing (descend, ascend, refresh, filter/hidden
toggle, mkdir relist), set `cursor = default_cursor()` =
`parent_offset().min(row_count().saturating_sub(1))`. For a non-root directory
with ≥1 real entry that is `1` (first real entry); for an empty non-root directory
it clamps to `0` (the `..` row); at a root it is `0` (first real entry).

**Rationale**: Implements the clarified decision (cursor starts on the first real
entry, FR-014) and keeps "cursor on first content entry" muscle memory. The
existing reset sites currently assign `cursor = 0`; they become
`cursor = default_cursor()`.

**Affected reset sites** (today `p.cursor = 0`): `toggle_hidden`, `navigate_to`,
`set_filter`, `refresh_active_pane`, `relist_active`, and the mkdir/relist paths
(`crates/cargonaut-core/src/lib.rs` ~lines 799, 881, 1143, 1253, 1330/1343, 1416,
1432). Cursor clamps `CursorDown`/`CursorUp`/`CursorTo` move from
`visible.len()` bounds to `row_count()` bounds.

## R-005 — TUI render + mouse

**Decision**: `PaneView` derives `has_parent()` from its synced `cwd`
(`cwd.parent().is_some()`), prepends a single `..` `ListItem` when true, and uses
`state.cursor` directly as the `ListState` selection (it is already the virtual
index). `focused_entry_index()` in `PaneView` mirrors the core offset. Mouse
double-click keeps dispatching `Descend`; because `Descend` now ascends when the
cursor is on the parent row, double-clicking `..` ascends with no special case.

**Rationale**: One model end-to-end (FR-011). The `..` row is a normal themed
`ListItem` (Constitution §III), highlightable like any row.

## R-006 — Test blast radius

**Decision**: Update the ~20 existing tests that assert raw `cursor` values or
listing positions to the virtual-row model in the same change that introduces it
(red step). Tests asserting `focused_entry_index()` by **real** index mostly stay
valid (the offset cancels: at startup in a non-root temp dir, `cursor` defaults to
1 and `focused_entry_index()` is still `Some(0)`). Tests asserting raw `cursor ==
N` after navigation shift by `+parent_offset`.

**Rationale**: The issue predicted "~10 tests" break; the real figure including
`pane.rs` and mouse tests is ~20. Updating them is mechanical and is the
red-before-green authoring step, not scope creep.

## Summary

| ID | Decision | Touches |
|----|----------|---------|
| R-001 | Core virtual-row cursor (`parent_offset`) | core |
| R-002 | Selection stays real-indexed; parent-row no-ops free | core (minimal) |
| R-003 | `cwd.parent()` drives presence + root suppression | core |
| R-004 | Default cursor = first real entry (FR-014) | core reset sites |
| R-005 | TUI prepends `..` ListItem; mouse uses virtual idx | ui-tui |
| R-006 | Update ~20 index-coupled tests in the red step | core + ui-tui tests |

No `NEEDS CLARIFICATION` remains.

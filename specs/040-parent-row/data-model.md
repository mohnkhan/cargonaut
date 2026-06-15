# Data Model: Panel `..` Parent Entry as First Row

**Feature**: 040-parent-row | **Date**: 2026-06-15

No persisted data, no new stored fields, and **no change to `DirListing`** (the
`..` row is never an entry). The change is a reinterpretation of the existing
`PaneState.cursor` plus derived helpers.

## Reinterpreted field

### `PaneState.cursor` (existing `usize`) — now a virtual-row index

| Before | After |
|--------|-------|
| Index into the **visible real entries** (`visible_indices()[cursor]`) | Index into the **virtual row list**: `[.. (when has_parent)] ++ visible_indices()` |

- `cursor == 0` && `has_parent` → the `..` row.
- `cursor >= parent_offset()` → the real entry `visible_indices()[cursor - parent_offset()]`.
- Always clamped to `0..row_count()`.

`selected: BTreeSet<usize>` is **unchanged** — still indices into
`listing.entries` (real entries only). It never contains the `..` row.

## Derived helpers (new, on `PaneState`)

| Helper | Type | Meaning |
|--------|------|---------|
| `has_parent()` | `bool` | `cwd.parent().is_some()` — is there a `..` row? |
| `parent_offset()` | `usize` | `has_parent() as usize` (0 or 1) — virtual↔real shift |
| `row_count()` | `usize` | `parent_offset() + visible_indices().len()` — addressable rows |
| `on_parent_row()` | `bool` | `has_parent() && cursor < parent_offset()` |
| `focused_row()` | `FocusedRow` | `Parent` or `Entry(real_index)` (see below) |
| `default_cursor()` | `usize` | `parent_offset().min(row_count().saturating_sub(1))` — first real entry, or `..` in an empty non-root dir, or `0` at root |

`focused_entry_index()` (existing) is updated:
```
if cursor < parent_offset() { None }            // on the .. row
else { visible_indices().get(cursor - parent_offset()).copied() }
```

### `FocusedRow` (new enum, core)
```
enum FocusedRow {
    Parent,        // cursor on the synthetic `..` row
    Entry(usize),  // cursor on a real entry (index into listing.entries)
}
```
(Returned by `focused_row()`; `Descend` matches on it. `focused_entry_index()`
remains the `Option<usize>` convenience used by selection/op paths.)

## Behavior table (cursor → action)

| Cursor position | `focused_entry_index()` | `Descend`/double-click | `SelectionToggle` | copy/move/delete target |
|---|---|---|---|---|
| `..` row (non-root, cursor 0) | `None` | **ascend to parent** | no-op | never includes `..` |
| real entry (dir) | `Some(real)` | descend into it | toggles tag | as selected/focused |
| real entry (file) | `Some(real)` | open (existing behavior) | toggles tag | as selected/focused |
| root dir (no `..`) | as today | as today | as today | as today |

## Cursor lifecycle

```text
enter non-root dir (descend/ascend/refresh/filter/hidden/mkdir)
        │  cursor = default_cursor()  ──►  first real entry (cursor = parent_offset)
        ▼                                   (empty non-root dir → cursor = 0 = `..`)
   [ .. ]   row 0           ◄── CursorUp from first entry lands here; CursorUp again = no-op
   [ entry0 ] row 1  ◄── default focus
   [ entry1 ] row 2
     ...
   row_count()-1            ◄── CursorDown clamps here
```

## Invariants

- `0 <= cursor < row_count()` after every command (clamped).
- `selected` only ever holds indices `< listing.entries.len()`; never the `..` row.
- The `..` row is present **iff** `cwd.parent().is_some()`, independent of filter
  and hidden-file state.
- `visible_indices()`, `listing.entries`, and any entry count are computed over
  real entries only (the `..` row is not counted — FR-013).
- TUI `PaneView` and core `PaneState` compute `has_parent()` from the same
  `VfsPath::parent()` predicate on the same (synced) `cwd`, so they never disagree.

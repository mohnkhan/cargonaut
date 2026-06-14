# Phase 1 Data Model: Visual & Interactive Parity Layer

Entities are described by responsibility, fields, and relationships — not final Rust signatures (those land in implementation). Types reference existing code where reused.

## Theme (new — `cargonaut-ui-tui/src/theme.rs`)

Typed palette applied throughout rendering (constitution §III: typed, no hardcoded ANSI).

| Field | Meaning |
|-------|---------|
| `panel_bg`, `panel_fg` | default panel background / foreground |
| `dir_fg` | directory entries |
| `exec_fg` | executable entries (mode has any execute bit) |
| `symlink_fg` | symlink entries |
| `hidden_fg` | hidden (dotfile) entries |
| `cursor_bg`, `cursor_fg` | the highlight (cursor) row bar |
| `marked_fg` | tagged/selected entries |
| `marked_cursor_*` | tagged AND under cursor |
| `border_focused`, `border_unfocused` | panel border by focus |
| `menu_bg`, `menu_fg`, `menu_sel_*` | menu bar + open dropdown |
| `fkey_label_*`, `fkey_num_*` | function-key bar number vs label |
| `status_bg`, `status_fg` | status line |
| `dialog_bg`, `dialog_fg`, `dialog_sel_*` | modal dialogs |

- **Construction**: `Theme::builtin(name) -> Option<Theme>`; `Theme::resolve(name) -> Theme` (falls back to default on unknown — FR-006). Built-ins: `commander-dark` (default), `monochrome`.
- **Relationships**: held once in the event loop; borrowed (`&Theme`) by every render fn.
- **Validation**: unknown name → default + non-fatal notice (FR-006). Colors degrade on low-depth terminals (FR-007).

## FrameLayout (new — `cargonaut-ui-tui/src/lib.rs`)

Snapshot of on-screen regions for the most recent frame; enables mouse hit-testing (US3).

| Field | Meaning |
|-------|---------|
| `left`, `right` | the two panel `Rect`s (list inner area) |
| `status` | status line `Rect` |
| `menu` | menu bar `Rect` + per-title sub-rects |
| `fkeys` | function-key bar `Rect` + per-button sub-rects |
| `ministatus_left/right` | mini-status `Rect`s |

- **Lifecycle**: produced by `draw_frame` each frame, stored in the loop, read by `handle_mouse`. A click arriving between resize and next render hit-tests the last stored layout (edge case in spec).

## FunctionKeyBinding (new — `chrome.rs`)

Associates a function key with its label and the command it invokes in the current context.

| Field | Meaning |
|-------|---------|
| `n` | 1..=10 |
| `label` | display text (e.g. "Copy") |
| `command` | the `keymap::Command` it dispatches |
| `available` | whether implemented this feature (false → "not yet available", FR-011) |

## Listing column set & parent entry (extends `pane.rs` render + `core` listing)

- **Columns** per visible entry: `name`, `size` (or `<DIR>` / recursive size), `mtime` (formatted via `config.ui.date_format`), `perms` (from `mode.bits`). Source: existing `VfsMetadata { size, mtime, mode, kind, is_hidden }` (types.rs).
- **Parent entry**: synthetic `..` as row 0 of every listing except at a filesystem root; activating it ascends (FR-020). Modeled as a listing-level flag/prefixed row, not a real `DirEntry`.
- **ListingMode**: reuse existing config enum (`Brief`, `Standard`, `Long`, `User`); this feature wires Brief (names, multi-column) and Full (= `Standard`/`Long`, name+size+mtime+perms). Cycled by `CycleListingMode`.

## QuickView (new panel view-kind — `core` + `pane.rs`)

When a pane is in quick-view, the *passive* panel shows a bounded preview of the active pane's highlighted file.

| Field | Meaning |
|-------|---------|
| `path` | file being previewed |
| `preview` | bounded UTF-8 text (≤256 KiB / ≤1000 lines) |
| `kind` | `Text` \| `BinaryPlaceholder` \| `TooLarge` \| `Unreadable` |

- **Validation**: non-text/binary/oversized → placeholder, never garbage (FR-022 edge case). Read off the frame path.

## SortOrder (reuse `cargonaut-vfs` `Sort`)

Existing enum: `NameAsc`, `NameDesc`, `SizeDesc`, `MtimeDesc`, `ExtAsc` (types.rs:200-213). Promote from the hardcoded `NameAsc` (core lib.rs:270) to a per-pane mutable field. `CycleSortKey` rotates the key; a reverse toggle flips direction. Active order is surfaced in the mini-status / header (FR-021).

## ProgressView (new — projected in `core`, rendered by `dialog.rs`)

User-facing projection of an in-flight transfer, derived from the engine's existing `Running` events (job.rs already emits bytes/throughput/ETA).

| Field | Meaning |
|-------|---------|
| `current_item` | file currently being transferred |
| `bytes_done`, `bytes_total` | overall progress |
| `throughput` | bytes/sec |
| `eta` | estimated time remaining |
| `cancelable` | always true (routes to existing `CancellationToken`) |

- **Lifecycle**: appears while ≥1 transfer is Running; dismisses on completion/cancel; target panel refreshes (FR-026/027).

## AppCommand additions (core `Command` enum)

New variants wired this feature (currently `None`/absent): `CursorTo(usize)`, `Mkdir(String)`, `SelectByPattern(String)`, `UnselectByPattern(String)`, `CycleSortKey`, `ToggleSortReverse`, `CycleListingMode`, `RecursiveDirSize`, `ToggleQuickView`, `ViewExternal`, `EditExternal`, `OpenMenuBar`, `ShowHelp`. Each maps from an existing `keymap::Command` (see contracts/commands-delta.md).

## State transitions (panel)

```
Pane(list) --CycleListingMode--> Pane(brief) --> Pane(full) --> Pane(quickview-passive) --> ...
Pane --ToggleSortReverse--> Pane(reversed)
Pane --F3/F4--> [suspend TUI] -> external pager/editor -> [restore TUI] --> Pane(refreshed)
Transfer running --> ProgressView shown --(complete|cancel)--> ProgressView dismissed + panel refresh
```

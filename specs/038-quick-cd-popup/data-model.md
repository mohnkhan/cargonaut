# Data Model: Quick-CD Popup with Tab-Completion

**Feature**: 038-quick-cd-popup | **Date**: 2026-06-15

This feature is interaction state, not persistent data. The "entities" are the
transient prompt state and the completion results that flow across the core↔TUI
seam. No serialization, no storage.

## Existing types consumed (unchanged)

- **`VfsPath`** (`cargonaut-vfs/src/types.rs`): `{ scheme, authority, segments }`.
  Target of resolution and completion. Built from active pane `cwd` + typed text.
- **`PaneState`** (`cargonaut-core/src/lib.rs:74`): provides
  `cwd: VfsPath`, `dir_history_back: Vec<VfsPath>` (recent dirs, most-recent
  last), `listing: DirListing`.
- **`DirEntry` / `DirListing` / `VfsKind`** (`cargonaut-vfs/src/traits.rs`):
  completion reads `entry.name` and `entry.meta.kind == VfsKind::Dir`.
- **`Command::QuickCdPopup`** (`cargonaut-core/src/lib.rs`): existing enum
  variant; keeps its identity but its *handling* changes from "status stub" to
  "the TUI opens the dialog." (It no longer needs to emit a placeholder status.)
- **`App::navigate_to(id, VfsPath)`** (`cargonaut-core/src/lib.rs:982`): the
  reused navigation primitive (lists target, swaps cwd, records history).

## New core types

### `CdCompletions` (returned by `App::complete_cd`)

```text
CdCompletions {
    /// Full path strings, ready to place in the input buffer, ordered:
    /// recent-visited matches first (most-recent first), then filesystem
    /// children in backend sort order; de-duplicated by string.
    candidates: Vec<String>,
}
```

- Empty `candidates` ⇒ "nothing to complete" (drives FR-009).
- Always directories (FR-008); files are filtered out before this is built.
- May be a plain `Vec<String>` if a wrapper struct earns no keep; the contract is
  the ordering + dir-only invariant, not the wrapper.

## New TUI types

### `PathInputDialog` (shared widget, `cargonaut-ui-tui::dialog`)

State:

```text
PathInputDialog {
    title: String,
    prompt: String,
    buffer: String,            // current text; prefilled on open (FR-014)
    cursor: usize,             // edit position (end on open)
    completions: Vec<String>,  // cached candidate cycle (R-005)
    completion_for: String,    // buffer value the cache was computed for
    cycle_idx: usize,          // current position in `completions`
    error: Option<String>,     // inline error line (FR-006); cleared on edit
    note: Option<String>,      // transient "(no matches)" hint (FR-009)
}
```

Invariants:
- `cursor <= buffer.len()`.
- Editing (Char/Backspace) clears `error` and invalidates the completion cache
  (`completions` considered stale when `completion_for != buffer`).
- `cycle_idx < completions.len()` whenever `completions` is non-empty.

### `PathInputAction` (return of `PathInputDialog::handle_key`)

```text
enum PathInputAction {
    Consumed,                      // key handled, nothing for the loop to do
    Edited,                        // buffer changed (cache now stale)
    RequestCompletions { text },   // Tab on a stale cache — loop must fetch
    Submit(String),                // Enter — accept this text
    Cancel,                        // Esc
}
```

### `ActiveDialog::QuickCd` (new variant, `cargonaut-ui-tui::lib`)

```text
ActiveDialog::QuickCd { widget: PathInputDialog }
```

Added alongside `Confirm`/`Resume`/`Input`. No `kind` needed — quick-cd is its
own variant with a fixed accept action (`App::quick_cd`).

## State transitions (prompt lifecycle)

```text
            Alt-c (Command::QuickCdPopup)
[no dialog] ─────────────────────────────▶ [QuickCd open]
                                            buffer = active cwd display, cursor=end

[QuickCd open] --Char/Backspace-->          [QuickCd open]  (buffer edits, cache stale, error cleared)

[QuickCd open] --Tab, cache stale-->        loop: complete_cd(buffer)
                                            └─ apply_completions:
                                                 some -> buffer = candidate[0], cache fresh
                                                 none -> note="(no matches)"
[QuickCd open] --Tab, cache fresh-->        cycle_idx=(idx+1)%len; buffer = candidate[idx]

[QuickCd open] --Enter-->                   loop: quick_cd(buffer)
                                            ├─ Ok  -> navigate active pane; close; Mode::Pane
                                            └─ Err -> set_error(e); STAY open (FR-006)

[QuickCd open] --Enter, buffer empty-->     no-op; stay open (US3 #3)

[QuickCd open] --Esc-->                     close; no navigation; Mode::Pane (FR-010)
```

## Relationships

- `PathInputDialog` is owned transiently by `ActiveDialog::QuickCd`; created on
  open, dropped on close. Nothing outlives the prompt.
- `complete_cd` reads `App` (active pane cwd, history, backend) but does not
  mutate it. `quick_cd` mutates only via `navigate_to` on the active pane.
- The inactive pane is never read or written by any quick-cd operation (FR-013).

# Data Model: cargonaut-core God-File Split

**Feature**: 059-cargonaut-core-split | **Date**: 2026-06-21

This refactor introduces **no runtime data**. The "entities" are code-organization units and the invariants that bind them. This document is the authoritative module map that `tasks.md` follows.

## Entities

### Module root — `lib.rs`

The thin entry point after the split.

- **Contains**: crate-level `//!` docs; external `use` imports; `pub use` re-export surface (`TransferId`, `TransferMode`, `Bookmark`, `Hotlist`, and `pub use` of every public type/fn moved to submodules); `mod` declarations; the `App` struct definition; the `SideState` struct definition.
- **Must NOT contain** (post-split): any `impl App` method body, any free fn body other than trivial glue, any `#[cfg(test)]` block.
- **Invariant**: every name in `contracts/public-api-baseline.txt` is reachable at exactly the same `cargonaut_core::<path>` after the split, via re-export.
- **Size target**: ≤ ~230 lines (SC-001).

### Central state types (stay at root)

| Type | Field visibility | Why it stays in `lib.rs` |
|------|------------------|--------------------------|
| `App` | 12 **private** fields | Methods scattered across submodules must read its private fields; descendant-module privacy makes this work only if `App` is defined at the crate root. |
| `SideState` (private type) | 2 **private** fields (`tabs`, `active_tab`) | Accessed by `App` methods now living in submodules; same descendant-privacy reason. |

These two definitions remaining at root is what makes the whole split require **zero visibility widening** (research.md R-002).

### Movable value types (relocate + re-export)

| Type | Target module | Field/variant visibility | Notes |
|------|---------------|--------------------------|-------|
| `PaneId` | `pane` | public variants | + `impl PaneId::other` |
| `PaneFilter` | `pane` | private field (own-module access only) | + `compile`/`is_match`/`pattern` |
| `PaneState` | `pane` | **all fields `pub`** | moves freely; + 7 methods |
| `FocusedRow` | `pane` | public variants | |
| `TabBarEntry` | `pane` | public fields | view model |
| `ViewMode` | `pane` | public variants | + `next` |
| `SplitOrient` | `pane` | public variants | + `toggle` |
| `Command` | `command` | public variants | dispatch vocabulary |
| `Event` | `command` | public variants | |
| `DialogKind` | `command` | public variants | |
| `AppError` | `error` | public variants | `thiserror` enum |
| `UndoEntry` | `error` | public variants | |
| `JobStatus` | `jobs` | public variants | |
| `JobView` | `jobs` | public fields | |
| `ProgressView` | `jobs` | public fields | |
| `ResumeOfferView` | `jobs` | public fields | |

### Behavior modules (`impl App` blocks — no type defs)

| Module | Responsibility (one sentence) | Public methods owned |
|--------|-------------------------------|----------------------|
| `app` | Construct the App and route a `Command` to its handler. | `new`, `dispatch`, `registry`, `view_mode`, `active_progress`, `split_orient`, `config`, `active_pane`, `pane`, `active_pane_state`, `status` |
| `nav` | Move panes through the VFS and apply name filters. | `navigate_into`, `refresh_active_pane`, `quick_cd`, `complete_cd`, `set_filter` (+ private nav helpers) |
| `history` | Step a pane through its back/forward directory history. | (private `dispatch` targets; no new public surface) |
| `fsops` | Create directories, pattern-select, sum recursive sizes. | (private `dispatch` targets) |
| `attrs` | Change permissions/ownership (incl. recursive) and create links. | `chmod_selection`, `chown_selection`, `chmod_recursive`, `chown_recursive`, `create_symlink`, `create_hard_link` |
| `compare` | Mark differing entries across the two panes. | (private `dispatch` target) |
| `rename` | Apply bulk renames and undo the last reversible op. | `apply_bulk_rename`, `undo_last_operation` |
| `hotlist` | Manage the directory bookmark hotlist. | `bookmarks`, `add_bookmark`, `remove_bookmark`, `jump_to_bookmark` |
| `tabs` | Create/close/cycle per-side directory tabs. | `tab_bar_view` (+ private tab ops) |
| `transfers` | Submit/cancel/pause/resume transfers; build job views; confirm. | `transfer_ids`, `transfer`, `job_views`, `cancel_transfer`, `pause_transfer`, `resume_paused`, `confirm_copy`, `scan_resume_offers`, `pending_resume_views`, `resume_offer`, `start_over_offer`, `skip_offer` |

### Free functions (relocate + re-export the public ones)

| Fn | Visibility | Target module |
|----|-----------|---------------|
| `validate_rename_proposals` | **pub** → re-export | `rename` |
| `glob_match` | **pub** → re-export | `pane` |
| `transfer_state_snapshot` | **pub** → re-export | `jobs` |
| `pane_idx` | private (maybe `pub(crate)`) | `pane` |
| `parse_path` | private (maybe `pub(crate)`) | `nav` |
| `next_sort_key`, `sort_label` | private | `nav` |
| `recursive_status`, `attr_status`, `RECURSE_NODE_CAP` | private | `attrs` |
| `resume_offer_view`, `job_status_from`, `crc32_partial` | private | `jobs` |

### Test units

| Entity | Form | Placement |
|--------|------|-----------|
| Shared fixtures (`make_app`, `app_with_three`, `mode_of`, `entry_index`, `submit_one_copy`, …) | `#[cfg(test)] pub(crate) mod test_support` | `src/test_support.rs` |
| Per-feature test groups | `#[cfg(test)] mod tests` | inside each owning submodule |

## Cross-cutting invariants (validation rules)

1. **API identity**: `diff baseline.txt post-split.txt` is empty (FR-003 / SC-003).
2. **Consumer compile**: workspace + benches build & test with no `src/` edits outside `cargonaut-core` (FR-004 / SC-004).
3. **No behavior change**: full pre-existing test set passes, same count (FR-005/FR-006).
4. **No widening to public**: no item private today becomes `pub`; at most `pub(crate)` (FR-007).
5. **Docs clean**: `#![warn(missing_docs)]` + `-D broken-intra-doc-links` produce no new warnings (FR-008).
6. **No residual god-file**: `lib.rs` ≤ ~230 lines; no production submodule larger than the largest pre-existing sibling-crate module; test block partitioned (FR-010 / SC-006).

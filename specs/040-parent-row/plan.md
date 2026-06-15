# Implementation Plan: Panel `..` Parent Entry as First Row

**Branch**: `040-parent-row` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/040-parent-row/spec.md`

## Summary

Add a synthetic `..` row as the first row of every non-root pane listing;
activating it (Enter when focused, or double-click) ascends to the parent — the
clickable/visible affordance FR-020 has always called for, alongside the existing
ascend key. At a filesystem root no `..` row is shown. The row is not a real
entry: it can't be tagged, is excluded from bulk selection and copy/move/delete,
and is immune to the hidden/filter toggles.

Technical approach: a **core-owned virtual-row cursor model**. Today
`PaneState.cursor` is an index into the visible real entries. We redefine it to
index the *virtual row list* `[.. (when the cwd has a parent)] ++ visible real
entries`. A `parent_offset()` (0 or 1) shifts between virtual rows and real
entries; `focused_entry_index()` returns `None` when the cursor is on the `..`
row; `Descend` ascends when on that row. The selection set keeps referring to
real entries unchanged, so tagging/copy/move/delete are unaffected (and a no-op
on the parent row falls out for free). The TUI prepends one `..` `ListItem` and
mouse hit-testing maps screen rows straight onto virtual-row indices — so
keyboard, mouse, and render all share the single model. No keymap change (reuses
Enter/Descend + double-click).

## Technical Context

**Language/Version**: Rust (edition 2021; workspace toolchain pinned)

**Primary Dependencies**: `ratatui` + `crossterm` (TUI: `List`/`ListState`,
mouse), internal `cargonaut-core` (`PaneState`, dispatch), `cargonaut-vfs`
(`VfsPath::parent`, `DirListing`). No new dependencies.

**Storage**: None. The `..` row is synthetic and never persisted; it is not added
to `DirListing::entries`.

**Testing**: `cargo test --workspace`. Core dispatch/state unit tests
(`#[tokio::test]` with temp dirs via the existing `make_app`); TUI `PaneView`
render + cursor tests via `ratatui::backend::TestBackend`; mouse hit-test unit
tests in `cargonaut-ui-tui`.

**Target Platform**: Linux terminal (TUI); local filesystem this phase.

**Project Type**: Single multi-crate Rust workspace; terminal application.

**Performance Goals**: Not on a tracked SC bench. Cost is one extra `ListItem`
per frame and a `VfsPath::parent()` (small segment-vec clone) per cursor op /
frame — negligible against the NFR-002 16 ms keypress budget.

**Constraints**: No `unsafe`; `clippy -D warnings`; `cargo fmt`;
`#![warn(missing_docs)]` on new public items. The selection set MUST keep
referring to the same real entries (FR-010). One model across key/mouse/render
(FR-011).

**Scale/Scope**: Edits concentrated in `PaneState` (a handful of helper methods +
cursor-reset/clamp sites + the `Descend` branch) and `PaneView` (render + sync +
mouse). ~20 existing tests need index updates (the blast radius the issue
predicted); plus new tests for the `..` behavior.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality (NON-NEGOTIABLE)**: PASS (planned). New public helpers on
  `PaneState`/`PaneView` carry doc comments; no `unsafe`; formatted; clippy clean.
- **II. Test-First (NON-NEGOTIABLE)**: PASS (planned). New behavior authored
  red→green; the ~20 existing index-coupled tests are updated to the virtual-row
  model in the same red step that introduces it. Each FR/SC gets a test.
- **III. UX Consistency**: PASS. No new keymap binding — the `..` row reuses the
  existing Descend/Enter activation and the existing mouse double-click→Descend
  path; `design/contracts/keymap.toml` is unchanged. Rendering uses the typed
  `Theme`; no hardcoded ANSI; the `..` row is a normal `ListItem`, not an ad-hoc
  layout.
- **IV. Performance (NON-NEGOTIABLE)**: PASS. Not one of the four tracked SC
  benches; one extra `ListItem` and a cheap `parent()` check per frame — no
  regression risk to copy throughput, resume, RSS, or startup.
- **V. SSD Preservation (NON-NEGOTIABLE — dev-host)**: PASS. Build/test via
  `make` targets; no `cargo clean` / `rm -rf target`.

**Result**: No violations. Complexity Tracking table not required.

## Project Structure

### Documentation (this feature)

```text
specs/040-parent-row/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── pane-cursor-model.md   # the virtual-row cursor contract
├── checklists/
│   └── requirements.md
└── tasks.md             # /speckit-tasks output (next phase)
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   └── src/lib.rs        # PaneState: parent_offset()/has_parent()/row_count()/
│                         #   on_parent_row()/focused_row(); update
│                         #   focused_entry_index(); cursor-reset sites →
│                         #   default cursor = parent_offset; CursorDown/Up/To
│                         #   clamp to row_count; Descend ascends on parent row.
│                         #   (selection/copy paths unchanged — no-op falls out)
└── cargonaut-ui-tui/
    └── src/
        ├── pane.rs       # PaneView: has_parent()/row_count(); sync_from cursor
        │                 #   = virtual index; render prepends the `..` ListItem;
        │                 #   focused_entry_index() offset.
        └── lib.rs        # handle_mouse: click/double-click already dispatch
                          #   CursorTo(virtual idx)/Descend — verify they map to
                          #   the parent row correctly (Descend ascends).

design/contracts/keymap.toml   # UNCHANGED (no new binding)
```

**Structure Decision**: Existing workspace. The model lives in **core**
(`PaneState`) so keyboard, mouse, and render all read one cursor; the TUI only
renders the synthetic row and forwards virtual-row indices. This is the issue's
"index offset in PaneState/PaneView" approach and avoids splitting cursor
semantics across layers.

## Complexity Tracking

> No Constitution violations — table intentionally omitted.

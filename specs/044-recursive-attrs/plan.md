# Implementation Plan: Recursive chmod / chown into Subtrees

**Branch**: `044-recursive-attrs` | **Date**: 2026-06-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/044-recursive-attrs/spec.md`

## Summary

Extend Feature 043's file-attribute operations with **recursive** variants
(issue #65). New dedicated chords `C-x C` (recursive chmod) and `C-x O`
(recursive chown) — plus File-menu entries — reuse the existing mode/owner input
dialog, then always chain a `ConfirmDialog` (FR-002) before applying the change
to a selected directory's **entire subtree**.

The recursion is a **core-level bounded walk** — no new VFS methods (the existing
per-path `chmod`/`chown` from Feature 043 are reused). A subtree collector
enumerates every entry under each selected directory via a bounded BFS (the
`recursive_dir_size` pattern: a `NODE_CAP` so a huge tree can't wedge the UI),
descending only into `VfsKind::Dir` entries — `VfsKind::Symlink` is a distinct
variant (and `list` uses `symlink_metadata`), so symlinked directories are
**never followed** (FR-006) for free. Collected paths are then changed
**deepest-first** so a restrictive mode/owner on a parent can't lock the apply
out of a not-yet-processed child (FR-011). Symbolic chmod is applied per entry
relative to its current bits (the existing `ModeSpec::apply`). Per-entry failures
are aggregated via the existing `attr_status` helper; truncation (cap hit) is
reported.

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest).

**Primary Dependencies**: existing `cargonaut-vfs` (`VfsBackend::chmod`/`chown`,
`list`, `VfsKind`, `ModeSpec`, `parse_owner` — all from Feature 043),
`cargonaut-core` (`App`, `selection_or_focused`, `refresh_active_pane`,
`attr_status`, the `recursive_dir_size` bounded-walk pattern),
`cargonaut-ui-tui` (`TextInputDialog`, `ConfirmDialog`, dispatch, menu, keymap).
**No new crates; no new VFS methods.**

**Storage**: N/A — mutates the filesystem directly; no persisted state.

**Testing**: `cargo test --workspace`. Core recursion tests (`#[tokio::test]`,
nested tempdir trees): chmod/chown applied at depth (SC-001/002), confirm gating
(SC-003 — at the UI dispatch layer), truncation under a lowered cap (SC-004),
symlinked-dir not descended (SC-005), partial failure aggregated (SC-006),
deepest-first apply doesn't lock out (FR-011), file-only selection = shallow
(FR-009). UI dispatch/confirm-chain tests mirror Feature 043's chown flow.

**Target Platform**: Linux terminal, local filesystem.

**Project Type**: Single Rust workspace. Touches core (the walk + recursive App
methods + two core commands) and ui-tui (two keymap commands + bindings + menu +
dispatch/confirm + help). **No vfs changes.**

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). The walk +
apply run in the async dispatch path (a confirmed, user-initiated batch), off the
render loop; the `NODE_CAP` bound guarantees termination.

**Constraints**: `unsafe`-free. `#![warn(missing_docs)]` on new public items.
New bindings land in `design/contracts/keymap.toml` first (Constitution III).
NFR-001 (≤8 MiB) — no new deps, negligible code growth.

**Scale/Scope**: A shared `collect_subtree` walk + 2 recursive App methods +
2 core `Command` variants in core; 2 keymap `Command` variants + 2 keymap
bindings + 2 `InputKind` variants + 2 dispatch arms (chain confirm) + 2 File-menu
entries + help text in ui-tui; plus tests. Everything else (mode/owner parsing,
selection, per-entry status, ConfirmDialog flow) is reused from Feature 043.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe`. New public items (`App::chmod_recursive`/
  `chown_recursive`, `Command::ChmodRecursive`/`ChownRecursive`) documented.
  clippy `-D warnings` + `cargo fmt --check` enforced. ✅
- **II. Test-First**: Every FR/SC gets a failing test first then green, with
  `(red)`→`(green)` history. Depth/confirm/truncation/symlink/partial-failure/
  lock-out each map to a cargo test. ✅
- **III. UX Consistency**: Recursive chords added to `keymap.toml` first; reuse
  `TextInputDialog` + `ConfirmDialog` (no ad-hoc UI); File menu + help updated.
  Typed theme only. ✅
- **IV. Performance (NFR-001/002)**: bounded walk (NODE_CAP), off the render
  path; no new deps. ✅
- **V. SSD Preservation**: dev build/test via `make`. ✅

**Result**: PASS — no violations, Complexity Tracking empty.

## Project Structure

### Documentation (this feature)

```text
specs/044-recursive-attrs/
├── plan.md · research.md · data-model.md · quickstart.md
├── contracts/recursive-attrs-seam.md
└── tasks.md   (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/cargonaut-core/src/
└── lib.rs   # + collect_subtree() bounded walk; chmod_recursive/chown_recursive;
            #   Command::{ChmodRecursive,ChownRecursive} + dispatch arms + tests

crates/cargonaut-ui-tui/src/
├── keymap.rs   # + Command::{ChmodRecursive,ChownRecursive}
└── lib.rs      # + InputKind::{ChmodRecursive,ChownRecursive}; C-x C / C-x O
                #   dispatch arms (input → confirm chain); File-menu entries; help
design/contracts/keymap.toml   # + C-x C / C-x O bindings (pane mode)
```

**Structure Decision**: No vfs changes — recursion is orchestration over the
existing per-path ops, so it lives in core (the walk) + ui-tui (the trigger),
mirroring exactly how Feature 043's chown routes input → `ConfirmDialog` →
core `Command` → App method. The recursive App methods sit beside
`chmod_selection`/`chown_selection`; the bounded walk reuses the
`recursive_dir_size` convention.

## Complexity Tracking

> No constitution violations — section intentionally empty.

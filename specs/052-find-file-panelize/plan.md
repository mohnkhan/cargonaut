# Implementation Plan: Find-File and Panelize

**Branch**: `052-find-file-panelize` | **Date**: 2026-06-19 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/052-find-file-panelize/spec.md`

## Summary

Add `Alt-?` find-file popup: a modal overlay that searches the active panel's
directory tree by filename glob (tokio + std::fs BFS + globset) or by file
content (ripgrep `--files-with-matches`), streams results incrementally, and
on confirmation panelizes the result set into the active panel as a synthetic
flat `DirListing` — enabling all existing bulk operations (tag, copy, move,
delete, F3 view, F4 edit) on the found files. Closes GitHub issue #41.

The dialog follows the established `ActiveDialog` + `dialog.rs` pattern (same
as HotlistDialog, FileViewerDialog). The walk runs in `tokio::task::spawn_blocking`,
streams results via `mpsc::unbounded_channel`, and is aborted via an
`Arc<AtomicBool>` flag on `Esc`. No new external crates; `globset` (already
in the workspace, used by `cargonaut-core`) is added to `cargonaut-ui-tui`.

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest of Cargonaut).

**Primary Dependencies**: `ratatui` + `crossterm` (TUI/keys), `tokio` (async
event loop + `spawn_blocking` + `mpsc`), `globset` (workspace dep, added to
`cargonaut-ui-tui`), existing `cargonaut-ui-tui` (`dialog.rs`, `lib.rs`,
`keymap.rs`, `pane.rs`), `cargonaut-config` (`config.search`). **No new
external crates.** `globset = { workspace = true }` added to `cargonaut-ui-tui/Cargo.toml`.

**Storage**: N/A — find results are transient session state; no persistence.

**Testing**: `cargo test --workspace`. Unit tests for:
- `plan_content_available` (pure fn: rg path → bool),
- `FindFileDialog` phase transitions (via direct method calls),
- `walk_name_search` sync helper (temp-dir fixture),
- `FindFileDialog::render` (TestBackend assertions),
- `Keymap` binding (`M-?` → `FindFilePopup`),
- Panelize integration (temp-dir + synthetic listing entry count).
**No new criterion bench** — feature introduces no new performance-sensitive path
(walk is off-thread; drain is O(n) per 100ms tick already budgeted).

**Target Platform**: Linux terminal (TUI).

**Project Type**: Single Rust workspace (multi-crate). Front-end change confined
to `cargonaut-ui-tui`; one binding line in `design/contracts/keymap.toml`; no
core/vfs/config behavior changes.

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). Walk is
off-thread; result drain is a `try_recv` loop in the existing 100ms tick —
negligible frame budget impact. SC-001 (≤5 s for 10k-file name search) is
trivially achieved by async off-thread walk. SC-006 (≤300 ms abort) enforced
by the `AtomicBool` abort flag checked per directory.

**Constraints**: NFR-001 (≤8 MiB stripped binary) — no new external deps.
`unsafe`-free. `#![warn(missing_docs)]` on all new public items. Binding in
`design/contracts/keymap.toml` first (Constitution §III).

**Scale/Scope**: 1 new `Command` variant, 1 keymap binding, 1 dialog struct
(`FindFileDialog`) + supporting enums (`SearchMode`, `DialogPhase`, `FindEvent`,
`FindOutcome`), 1 `ActiveDialog` variant, 1 `dispatch_ui_command` arm, 1
`UiState` field (`find_label`), status-bar update, help-text update. 1 dep
line added to `cargonaut-ui-tui/Cargo.toml`. No new modules (adds to `dialog.rs`).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe`. All new public items carry doc comments.
  `clippy -D warnings` and `cargo fmt --check` enforced by CI. `globset` dep
  addition: workspace-managed version, no pinning divergence. ✅
- **II. Test-First**: Every FR gets a failing test (red) before implementation
  (green). Pure decision fns (`plan_content_available`, phase transition truth
  tables from contracts §3) are unit-testable without I/O. Walk tested with
  a `tempfile` fixture. Panelize tested via a `TestBackend` render assert on
  entry count. Each SC maps to a CI-run test (SC-001–SC-008). ✅
- **III. UX Consistency**: `FindFileDialog` in `dialog.rs` (shared widget
  infrastructure — FR-014). `ActiveDialog::FindFile` in `lib.rs`. Binding in
  `design/contracts/keymap.toml` first (FR-013). No hardcoded ANSI — theme-colored.
  Help overlay updated (F1 → M-? entry — FR-019). ✅
- **IV. Performance**: Walk off-thread (`spawn_blocking`). Drain is `try_recv`
  loop in existing 100ms tick. No criterion bench required. ✅
- **V. SSD Preservation**: Dev build/test via `make` wrappers (tmpfs-guarded).
  No `cargo clean` / `rm -rf target`. ✅

**Result**: PASS — no violations. Complexity Tracking intentionally empty.

## Project Structure

### Documentation (this feature)

```text
specs/052-find-file-panelize/
├── plan.md                      # This file
├── research.md                  # Phase 0 output (completed)
├── data-model.md                # Phase 1 output (completed)
├── contracts/
│   └── find-file-seam.md        # Phase 1 output (completed)
├── quickstart.md                # Phase 1 output (completed)
└── tasks.md                     # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
design/contracts/
└── keymap.toml                  # + [[binding]] pane M-? → find-file-popup

crates/cargonaut-ui-tui/
├── Cargo.toml                   # + globset = { workspace = true }
└── src/
    ├── dialog.rs                # + SearchMode, DialogPhase, FindEvent, FindOutcome,
    │                            #   FindFileDialog (struct + impl + tests)
    ├── keymap.rs                # + Command::FindFilePopup variant + keymap test
    └── lib.rs                   # + ActiveDialog::FindFile variant,
                                 #   dispatch_ui_command arm (open dialog),
                                 #   event-loop: FindFile dialog handling arm,
                                 #   tick: poll_results() call when walking,
                                 #   panelize helper (build DirListing from paths),
                                 #   UiState.find_label field,
                                 #   status-bar/chrome: [Find: label] display,
                                 #   help-text: M-? entry,
                                 #   integration tests
```

**Structure Decision**: Single Rust workspace; all changes confined to
`cargonaut-ui-tui` plus the shared keymap contract. No core/vfs/config
behavior changes. Follows the established pattern for keymap-driven modal
dialogs (Feature 038 Quick-CD, Feature 042 Hotlist, Feature 051 FileViewer).

## Complexity Tracking

> No constitution violations — section intentionally empty.

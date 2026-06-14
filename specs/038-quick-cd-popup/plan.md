# Implementation Plan: Quick-CD Popup with Tab-Completion

**Branch**: `038-quick-cd-popup` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/038-quick-cd-popup/spec.md`

## Summary

Replace the Alt-c placeholder (`Command::QuickCdPopup` → status string at
`crates/cargonaut-core/src/lib.rs`) with a working inline "quick cd" prompt. The
prompt is a modal text-input dialog, prefilled with the active pane's current
directory, that completes directory paths against the focused pane's VFS plus the
pane's recent-directory history (`dir_history_back`, T1.24) and, on Enter,
navigates the active pane through the existing `navigate_to` path.

The completion + accept logic lives in `cargonaut-core` (pure, async, unit-
testable via `App::dispatch`/helper methods — the "injected-input" test surface).
The modal widget lives in the shared `cargonaut-ui-tui::dialog` module alongside
`ConfirmDialog`/`TextInputDialog`/`ResumePromptDialog`, built so the same widget
serves the deferred tasks panel (#32) and panel filter prompt (#33).

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest of
Cargonaut).

**Primary Dependencies**: `ratatui` + `crossterm` (TUI/keys), `tokio` (async
event loop), existing `cargonaut-vfs` (`VfsBackend::list`, `VfsPath`),
`cargonaut-core` (`App`, `PaneState`, `navigate_to`). No new crates.

**Storage**: N/A (no persistence; quick-cd input is transient).

**Testing**: `cargo test --workspace`. Core logic via `cargonaut-core` unit
tests (`#[tokio::test]`, tempdir fixtures, `App` driven directly). Widget logic
via `cargonaut-ui-tui::dialog` unit tests (synthetic `KeyCode` sequences +
`TestBackend` render assertions), mirroring the existing dialog tests.

**Target Platform**: Linux terminal (TUI).

**Project Type**: Single Rust workspace (multi-crate). Core engine +
TUI front-end.

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). Completion's
single `VfsBackend::list` call is async and runs in the already-async
`handle_key`, so it never blocks the render loop's frame budget.

**Constraints**: Honor NFR-001 (≤8 MiB stripped binary) — no new heavy deps;
prefix matching uses std string ops. `unsafe`-free. `#![warn(missing_docs)]` on
all new public items.

**Scale/Scope**: ~1 new core command path + 2 async `App` helpers, 1 new shared
dialog widget, 1 `ActiveDialog` variant + event-loop wiring, plus tests. Single
filesystem backend exercised (local).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe`. New public items carry doc comments. clippy
  `-D warnings` and `cargo fmt --check` enforced by CI. ✅ (no waivers)
- **II. Test-First**: Each FR gets a red→green pair. SC-006 (the injected-input
  E2E behavior) is gated by a core unit test that drives open→type→complete→
  accept and open→cancel. Widget key-handling gets dialog-level tests. ✅
- **III. UX Consistency**:
  - Dialog reuses the shared `dialog` module — the new `PathInputDialog` lives
    beside the existing widgets, no ad-hoc layout in feature code. The
    constitution's aspirational `dialog!` macro does not exist yet; the
    established shared-widget pattern in `dialog.rs` is the current realization
    and we extend it (documented in research R-006). ✅
  - Keymap single source of truth: `M-c → quick-cd-popup` is **already** in
    `design/contracts/keymap.toml`; no new binding is introduced. ✅
  - Theme: render via `theme.dialog_style()` like the sibling widgets — no
    hardcoded ANSI. ✅
- **IV. Performance**: No new tracked bench needed; completion is one async
  `list` on the active backend, off the synchronous render path. Binary-size and
  keypress-latency gates remain green (no heavy deps). ✅
- **V. SSD Preservation**: Dev-host only; `make` wrappers run `check-tmpfs`. No
  change to build-artifact handling. ✅

**Result**: PASS. No Complexity Tracking entries required.

## Project Structure

### Documentation (this feature)

```text
specs/038-quick-cd-popup/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions & rationale
├── data-model.md        # Phase 1 — entities & state
├── quickstart.md        # Phase 1 — how to exercise the feature
├── contracts/
│   └── quick-cd-seam.md # Phase 1 — core↔TUI contract
├── checklists/
│   ├── requirements.md  # from /speckit-specify
│   └── implementation.md # from /speckit-checklist
└── tasks.md             # from /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   └── src/lib.rs
│       • Command::QuickCdPopup            (exists — keep; opens the dialog UI-side)
│       • App::complete_cd(partial) -> CdCompletions      (NEW, async)
│       • App::quick_cd(path_text) -> Result<Vec<Event>>  (NEW, async; routes navigate_to)
│       • path-resolution helper: resolve typed text → VfsPath relative to active cwd
│       • unit tests: completion ordering, dir-only filter, recent-dir merge,
│         relative/absolute resolve, accept-valid, accept-invalid (no nav), cancel
│
├── cargonaut-vfs/            # unchanged (consumes VfsBackend::list, VfsPath)
│
└── cargonaut-ui-tui/
    └── src/
        ├── dialog.rs
        │   • PathInputDialog        (NEW shared widget: prefill, cursor, edit,
        │     completion-cycle, inline error) + PathInputAction enum
        │   • dialog unit tests (key sequences + TestBackend render)
        └── lib.rs
            • ActiveDialog::QuickCd { widget: PathInputDialog }   (NEW variant)
            • dispatch_ui_command: Command::QuickCdPopup opens the dialog
              prefilled with active pane cwd
            • handle_key dialog branch: Tab → app.complete_cd (async) →
              widget.apply_completions; Enter → app.quick_cd → on Err
              widget.set_error (stay open); Esc → close
            • render: ActiveDialog::QuickCd → widget.render

design/contracts/keymap.toml  # unchanged — M-c already bound
```

**Structure Decision**: Single workspace, existing crates. Engine logic
(completion + navigation) in `cargonaut-core`; presentation + key routing in
`cargonaut-ui-tui`. This split keeps the testable behavior off the TTY (the
injected-input tests run against `App`), matching the project's established
pattern where core unit tests verify engine behavior and the bin-level PTY
driver stays deferred (#30).

## Complexity Tracking

No constitution violations — section intentionally empty.

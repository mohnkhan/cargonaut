# Implementation Plan: Tasks/Jobs Panel Popup

**Branch**: `039-tasks-jobs-panel` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/039-tasks-jobs-panel/spec.md`

## Summary

Replace the F12 / `:jobs` status-bar placeholder with a real modal **tasks
panel**: a shared list dialog rendered over a read-only projection of the App's
existing transfer registry. Each row shows a transfer's source → destination and
its current state/progress; the selected row can be cancelled, paused, or
resumed. Pause reuses the engine's cancellation+checkpoint machinery (cancel the
token, leave the checkpoint sidecar, mark the registry entry user-paused); resume
re-arms the transfer through the existing, tested `resume_transfer` checkpoint
path with a fresh cancellation token. The headline acceptance test submits three
throttled transfers, pauses one, and asserts the other two continue to
completion while the paused one resumes and finishes.

Technical approach: add a UI-agnostic `JobView` projection + a `paused` marker
set to `cargonaut-core` (mirroring the existing `ProgressView` / `ResumeOfferView`
seam), expose `pause` / `resume` / `cancel`-by-id methods, and build a
`TasksPanelDialog` widget in `cargonaut-ui-tui` modeled on the existing
`ResumePromptDialog`. No new transfer-crate API is required.

## Technical Context

**Language/Version**: Rust (edition 2021; workspace toolchain pinned in repo)

**Primary Dependencies**: `tokio` (tasks + `sync::watch`), `tokio-util`
(`CancellationToken`), `ratatui` + `crossterm` (TUI), `uuid`. Internal crates:
`cargonaut-core` (App + registry), `cargonaut-transfer` (`submit_transfer` /
`resume_transfer` / `scan_resumable` / `TransferState`), `cargonaut-ui-tui`
(dialogs, event loop), `cargonaut-vfs` (`VfsBackend`).

**Storage**: No new persistence. Pause/resume reuses the existing on-disk
checkpoint sidecar (`<dst-parent>/.cargonaut-transfer-<id>.json`). The panel
reflects only the in-memory session registry.

**Testing**: `cargo test --workspace`. Unit tests colocated in modules; widget
render tests via `ratatui::backend::TestBackend`; async integration tests with
`#[tokio::test]`; the three-job pause scenario driven through `App` with two temp
directories and the `CARGONAUT_TRANSFER_THROTTLE_MIBPS` throttle to keep copies
in flight deterministically.

**Target Platform**: Linux terminal (TUI). Local filesystem backend in this
phase.

**Project Type**: Single Rust workspace (multi-crate); terminal application.

**Performance Goals**: Not on a tracked SC bench path. The panel is a modal over
≤ a handful of session jobs; render and per-row actions are O(n) in job count.
Keypress→paint stays within the existing NFR-002 16 ms budget (small list, no
I/O on the render path).

**Constraints**: No `unsafe`. `clippy -D warnings`, `cargo fmt`, and
`#![warn(missing_docs)]` on all touched public items. Exactly one modal active at
a time. Per-row actions must be safe no-ops on terminal/ineligible states.

**Scale/Scope**: Two new public projection types + three App methods in core; one
new dialog widget + one `ActiveDialog` variant + wiring in the TUI. Tens of jobs
at most in practice.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality (NON-NEGOTIABLE)**: PASS (planned). New public items in
  `cargonaut-core` and `cargonaut-ui-tui` carry doc comments; no `unsafe`
  introduced; code formatted; clippy clean. Pure read-only projections and
  small state mutations.
- **II. Test-First (NON-NEGOTIABLE)**: PASS (planned). Every FR gets a test
  authored red before green (per-task `(red)`→`(green)` commits). SC-003 (the
  three-job pause scenario) and SC-007 (end-to-end open→act→close) get dedicated
  CI gates: a core integration test and TUI dispatch/widget tests.
- **III. UX Consistency**: PASS. The panel is a shared `dialog.rs` widget
  (`TasksPanelDialog`) built like `ResumePromptDialog` — no ad-hoc layout in
  feature code; uses the typed `Theme` (`dialog_style()`), no hardcoded ANSI.
  The F12 binding already exists in `design/contracts/keymap.toml` (source of
  truth); per-row keys (`c`/`p`/`r`, arrows/`jk`, Esc) are dialog-internal,
  consistent with how `ResumePromptDialog` and `ConfirmDialog` own their keys.
- **IV. Performance (NON-NEGOTIABLE)**: PASS. Not one of the four tracked SC
  benches; no regression risk to copy throughput, resume, RSS, or startup. The
  modal renders a short list and does no work on the hot navigation path.
- **V. SSD Preservation (NON-NEGOTIABLE — dev-host)**: PASS. Build/test via
  `make` targets (which run `check-tmpfs`); no `cargo clean` / `rm -rf target`.
  `make tmpfs-status` confirms the `target/` symlink before iterating.

**Result**: No violations. Complexity Tracking table not required.

## Project Structure

### Documentation (this feature)

```text
specs/039-tasks-jobs-panel/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output
│   ├── core-api.md      # core projection + action method contracts
│   └── tasks-panel-widget.md  # dialog widget contract
├── checklists/
│   └── requirements.md  # spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   └── src/lib.rs           # + JobView, JobStatus projections;
│                            #   + `paused: HashSet<TransferId>` field;
│                            #   + job_views(), pause_transfer(),
│                            #     resume_paused(), cancel_transfer(id);
│                            #   replace ShowTasksPanel stub semantics
│   └── (tests colocated in lib.rs #[cfg(test)] + optional tests/jobs_panel.rs)
├── cargonaut-transfer/      # UNCHANGED (reuses submit/resume/scan_resumable)
└── cargonaut-ui-tui/
    └── src/
        ├── dialog.rs        # + TasksPanelDialog widget + TasksAction enum
        └── lib.rs           # + ActiveDialog::TasksPanel; dispatch open;
                             #   handle_key routing; draw_frame arm;
                             #   per-frame row refresh (FR-008)

design/contracts/keymap.toml # F12 binding already present (verify `:jobs`)
```

**Structure Decision**: Existing multi-crate workspace. Core gets the
UI-agnostic projection + action seam (consistent with `ProgressView` /
`ResumeOfferView` / the Feature-037 resume seam, which keeps all transfer-crate
types out of the UI). The UI gets one shared dialog widget and the modal wiring.
The transfer crate is untouched — pause = existing cancel; resume = existing
`resume_transfer`.

## Complexity Tracking

> No Constitution violations — table intentionally omitted.

# Implementation Plan: Visual & Interactive Parity Layer

**Branch**: `031-visual-interactive-parity` | **Date**: 2026-06-14 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/031-visual-interactive-parity/spec.md`

## Summary

Deliver the interactive surface that makes Cargonaut look and feel like the reference orthodox dual-pane file manager, on top of the existing engine (VFS, resumable transfer, keymap, dispatch, dir-history). Five prioritized slices: (US1) a typed color **Theme** applied throughout rendering; (US2) screen **chrome** — top menu bar, bottom function-key bar, per-panel mini-status; (US3) **mouse** support (default on) via lifted layout rects + hit-testing; (US4) richer **panel listing** — mtime/perms columns, `..` entry, sort cycling, brief/full/quick-view modes, recursive dir-size; (US5) **operation parity** — mkdir, pattern select/unselect, a live transfer progress dialog, plus F3/F4 shelling out to `$PAGER`/`$EDITOR`. The work is almost entirely in `cargonaut-ui-tui` and `cargonaut-core`, with a small `cargonaut-config` extension and a one-line `cargonaut-bin` flag-merge fix. No new crates.

## Technical Context

**Language/Version**: Rust (workspace edition; toolchain per repo `rust-toolchain`/CI).

**Primary Dependencies**: `ratatui` (widgets + `style::Color`, already a dep), `crossterm` (terminal + **mouse capture**, already a dep), `tokio` (async event loop), `globset` (pattern selection — already the planned glob engine), `thiserror`, `serde` (config). No new heavyweight deps.

**Storage**: N/A (config is TOML files via existing `cargonaut-config`). Themes are compiled-in.

**Testing**: `cargo test --workspace` (unit + integration); `ratatui::backend::TestBackend` for render assertions (already used in `pane.rs`/`dialog.rs`); `criterion` benches for the perf gates (existing).

**Target Platform**: Linux terminal (primary); crossterm keeps it portable.

**Project Type**: Single Rust workspace, multiple crates (desktop/CLI TUI).

**Performance Goals**: Preserve NFR-002 (≤16 ms keypress→first-paint, 60 Hz). Per-frame theming and column formatting must stay allocation-light; quick-view preview and recursive dir-size MUST be bounded/async so they never block the frame loop.

**Constraints**: NFR-001 (≤8 MiB stripped binary — currently 1.91 MiB; built-in themes + chrome must not blow this), constitution §III (typed theme vars, no hardcoded ANSI; keymap single source of truth), §I (clippy -D warnings, `#![warn(missing_docs)]`, no undocumented `unsafe`), §II (TDD red→green per FR), §V (tmpfs target — use `make` wrappers).

**Scale/Scope**: ~5 user stories, FR-001..FR-031. Listings already virtual-scroll to 1M entries (existing bench); quick-view preview capped (e.g. ≤256 KiB / ≤1000 lines).

## Constitution Check

*GATE: must pass before Phase 0 and re-checked after Phase 1.*

| Principle | Impact & Compliance |
|-----------|---------------------|
| **I. Code Quality** | All new modules carry `#![warn(missing_docs)]` doc comments; clippy `-D warnings`; `cargo fmt`. No `unsafe` introduced. F3/F4 external-process spawn uses `Command::new(prog).arg(path)` (no `sh -c`) per §Development-Workflow macro-safety rule. |
| **II. Test-First** | Each FR gets a failing test first (red commit) then implementation (green). Theme resolution, hit-testing math, column formatting, sort cycling, pattern matching, mkdir, quick-view bounding all unit-testable; chrome/pane render via `TestBackend`; progress-dialog via core event projection tests. |
| **III. UX Consistency** | **Typed `Theme`** struct — no hardcoded ANSI (directly satisfies "Theme variables are typed"). New dialogs (mkdir prompt, pattern prompt, progress) reuse the shared dialog widgets/pattern in `dialog.rs`. New/changed bindings land in `design/contracts/keymap.toml` first (menu/F-key bar invoke the same `Command`s). |
| **IV. Performance** | Theming is per-cell style assignment (no extra passes). Quick-view + recursive-dir-size run off the frame path (bounded read / async task) to protect NFR-002. New keypress-latency assertions stay under budget. Binary-size gate re-checked. |
| **V. SSD Preservation** | Dev-host only; use `make build`/`make test` (tmpfs-guarded). No change to build layout. |

**Gate result**: PASS — no violations; no Complexity Tracking entries required. Quick-view and external-tool shell-out are the only notable design risks and are bounded by the rules above. Re-checked after Phase 1 design: still PASS.

## Project Structure

### Documentation (this feature)

```text
specs/031-visual-interactive-parity/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions & rationale
├── data-model.md        # Phase 1 — entities (Theme, FrameLayout, columns, …)
├── quickstart.md        # Phase 1 — manual + automated validation guide
├── contracts/           # Phase 1 — themes.md, mouse-interaction.md, commands-delta.md
│   ├── themes.md
│   ├── mouse-interaction.md
│   └── commands-delta.md
└── tasks.md             # Phase 2 (/speckit-tasks — not created here)
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   └── src/lib.rs            # + AppCommand::CursorTo(usize), Mkdir, SelectByPattern/UnselectByPattern,
│                             #   CycleSortKey, CycleListingMode, RecursiveDirSize, QuickView state;
│                             #   sort application; transfer-progress projection (ProgressView);
│                             #   parent `..` synthesis; pattern selection over listing.
├── cargonaut-ui-tui/
│   └── src/
│       ├── lib.rs            # event loop: enable/disable mouse (default on), handle_mouse(),
│       │                     #   return FrameLayout from draw_frame, thread &Theme, suspend/resume
│       │                     #   terminal around F3/F4 external process, menu/fkey dispatch.
│       ├── theme.rs          # NEW: Theme struct, built-in palettes, name→Theme, color-depth degrade.
│       ├── chrome.rs         # NEW: MenuBar + FunctionKeyBar + per-pane mini-status widgets + hit-test.
│       ├── pane.rs           # per-entry colored ListItem (kind/mode), columns (mtime/perms),
│       │                     #   `..` row, brief/full layouts, quick-view render, absolute cursor.
│       ├── dialog.rs         # + MkdirPromptDialog, PatternPromptDialog, TransferProgressDialog.
│       └── keymap.rs         # (parser unchanged) ensure new commands map in ui_command_to_core.
├── cargonaut-config/
│   └── src/lib.rs            # UiConfig.mouse default → true; ListingMode + QuickView wiring; theme name.
└── cargonaut-bin/
    └── src/main.rs           # merge cli.theme / cli.mc_keys into config before App::new (dead flags fix).

design/contracts/keymap.toml # add/confirm bindings: +, -, F7, C-s, M-t, C-Space, F3, F4, F9, menu.
```

**Structure Decision**: Reuse the existing 6-crate workspace. The presentation layer (`cargonaut-ui-tui`) absorbs the bulk (theme, chrome, mouse, columns, quick-view, new dialogs); `cargonaut-core` gains the new `Command` variants + sort/selection/parent/progress-projection logic so the UI stays a thin renderer over `App` state (preserves the existing `PaneState`/`sync_from` separation the codebase already enforces). `cargonaut-config` gets minimal field/default changes. `cargonaut-bin` gets the flag-merge bug fix.

## Phase 0 — Research

See [research.md](./research.md). All Technical Context items are resolved (no remaining NEEDS CLARIFICATION); the four spec clarifications already fixed the open product decisions (mouse default-on, brief/full/quick-view, F3/F4 external, built-in themes only).

## Phase 1 — Design & Contracts

- [data-model.md](./data-model.md) — `Theme`, `FrameLayout`, `FunctionKeyBinding`, listing column set + `..` entry, `SortOrder`, `ListingMode`/`QuickView`, `ProgressView`.
- [contracts/themes.md](./contracts/themes.md) — themable element list + built-in palette values + degrade rules.
- [contracts/mouse-interaction.md](./contracts/mouse-interaction.md) — event→action mapping + hit-test regions + double-click rule.
- [contracts/commands-delta.md](./contracts/commands-delta.md) — keymap bindings and `Command`→core-`Command` wiring delta (the currently-`None` set this feature lights up).
- [quickstart.md](./quickstart.md) — how to run and validate each user story (manual + automated).

**Post-Design Constitution re-check**: PASS. Design keeps the renderer thin, theme typed, dialogs shared, keymap authoritative, and the two risky features (quick-view, external tools) bounded.

## Complexity Tracking

No constitutional violations — section intentionally empty.

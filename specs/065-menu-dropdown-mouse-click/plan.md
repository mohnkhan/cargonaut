# Implementation Plan: Click-on-dropdown-item support for the pull-down menu bar

**Branch**: `065-menu-dropdown-mouse-click` | **Date**: 2026-06-22 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/065-menu-dropdown-mouse-click/spec.md`

## Summary

The pull-down menu bar already opens on a title click but its dropdown items are
keyboard-only. This feature makes the open dropdown fully mouse-operable: left-click an item
to invoke it (then close), hover to move the highlight, click a different title to switch,
and click outside to close (passing the click through to the underlying panel). All behavior
is gated by the existing mouse-capture state and leaves the keyboard path untouched.

Technical approach: add pure hit-test/selection methods to `MenuBar` in `chrome.rs`
(`dropdown_rect` [private], `item_at`, `in_dropdown`, `select`) that recompute the *same*
geometry `render()` already uses, then wire them into `handle_mouse` in `lib.rs` for the
`Down(Left)` and `Moved` mouse event kinds. One field (`full: Rect`, the terminal area) is
added to `FrameLayout` so the handler can pass the buffer area for height-clamping. No new
config, no new keybindings.

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest of cargonaut)

**Primary Dependencies**: `ratatui` (geometry: `Rect`, widgets), `crossterm`
(`MouseEvent`, `MouseEventKind::{Down, Moved}`, `MouseButton::Left`) — all already in use.

**Storage**: N/A

**Testing**: `cargo test --workspace`; unit tests in `chrome.rs` (`#[cfg(test)]`), integration
tests in `lib.rs` test module (existing `T-MOUSE-*` harness style).

**Target Platform**: Terminal (Linux/macOS); TUI binary.

**Project Type**: Single Rust workspace, desktop-class TUI application.

**Performance Goals**: Hover (`Moved`) events can arrive at high frequency; the per-event
handler MUST be O(1) and allocation-free, well within NFR-002's 16 ms keypress→paint budget.

**Constraints**: No `unsafe`. `clippy -D warnings` clean. `#![warn(missing_docs)]` — every new
public method documented. No hardcoded ANSI (reuse `Theme`). Build artifacts in tmpfs (§V).

**Scale/Scope**: Small. 4 new methods on `MenuBar` (`dropdown_rect`, `item_at`, `in_dropdown`,
`select`), one new `FrameLayout.full` field, one new `Moved` arm + item-click/close branches in
`handle_mouse`. ~6 unit tests + ~6 integration tests. Two source files touched (`chrome.rs`,
`lib.rs`), plus mandatory `README.md` / `Learnings.md`.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I. Code Quality (NON-NEGOTIABLE) | PASS | No `unsafe`. New `pub fn`s on `MenuBar` get `///` docs (crate has `#![warn(missing_docs)]`). `cargo fmt`/`clippy -D warnings` enforced by `make ci-local`. |
| II. Test-First (NON-NEGOTIABLE) | PASS | Each FR gets a failing test first; git history shows `red` → `green` per task. SCs map to integration tests (click invokes, click closes+passes through, hover highlights, disabled-mouse no-op). |
| III. UX Consistency | PASS | No new keymap bindings (mouse-only; keymap.toml untouched). No new dialogs. Highlight reuses existing `theme.menu_sel_*`; no hardcoded escapes. |
| IV. Performance (NON-NEGOTIABLE) | PASS | `Moved` handler is O(1) selection update; no new allocations, no forced extra repaint beyond the normal frame. Within NFR-002 16 ms budget. |
| V. SSD Preservation (NON-NEGOTIABLE) | PASS | Use `make build`/`make test` (tmpfs-guarded). No `cargo clean`/`rm -rf target`. |

**Result**: No violations. No entries required in Complexity Tracking.

## Project Structure

### Documentation (this feature)

```text
specs/065-menu-dropdown-mouse-click/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── menubar-mouse.md # Phase 1 output — MenuBar mouse method contracts
├── checklists/
│   └── requirements.md  # Spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
crates/cargonaut-ui-tui/src/
├── chrome.rs   # MenuBar: add dropdown_rect() [private], item_at(), in_dropdown(), select(); unit tests
└── lib.rs      # FrameLayout: add full: Rect (set in draw_frame). handle_mouse(): item-click +
                #   in_dropdown no-op + close/pass-through branches in Down(Left); new Moved arm; tests

README.md       # MANDATORY: At-a-Glance metrics + Feature History row
Learnings.md    # MANDATORY: ≥3 bullets on what was hard / root causes / decisions
```

No changes outside `cargonaut-ui-tui`. No new files in the source tree.

## Phase 0: Research

See [research.md](./research.md). Key decisions:

- **Geometry single-source**: extract the dropdown rectangle math into `MenuBar::dropdown_rect`
  and have both `render()` and `item_at()` use it, so the clickable rows can never drift from
  the rendered rows (prevents the off-by-one FR-002 guards against).
- **Hover via `MouseEventKind::Moved`**: crossterm delivers motion events when mouse capture is
  on; terminals that don't report motion simply never fire `Moved`, giving free graceful
  degradation (FR-010). No `Drag` handling needed.
- **Close-and-pass-through**: implement by closing the menu first, then letting the existing
  panel-click code path run for the same event (FR-004 clarified to pass-through).
- **No new menu state**: reuse `MenuBar.open` + `MenuBar.item_sel`; add a `select(idx)` setter.
- **Buffer area (finding U1)**: add `FrameLayout.full: Rect` from `f.size()`; pass it to
  `item_at` so hit-test clamping matches `render`.
- **Inside-vs-outside (finding I1)**: keep `dropdown_rect` private; expose `in_dropdown(...)
  -> bool` so `handle_mouse` can separate FR-003 (no-op) from FR-004 (close + pass-through).

## Phase 1: Design & Contracts

- **Data model**: [data-model.md](./data-model.md). No new entities; documents the existing
  `MenuBar` state and the geometry relationship used for hit-testing.
- **Contracts**: [contracts/menubar-mouse.md](./contracts/menubar-mouse.md). Signatures and
  behavioral contracts for `dropdown_rect`, `item_at`, `select`, and the `handle_mouse`
  integration order.
- **Quickstart**: [quickstart.md](./quickstart.md). Manual + automated validation steps.

### Post-Design Constitution Re-Check

Re-evaluated after design: still PASS on all five principles. The design adds only pure
functions + one event arm; no architectural complexity, no new dependencies, no keymap or
theme changes. No Complexity Tracking entries needed.

## Complexity Tracking

No constitutional violations — table intentionally empty.

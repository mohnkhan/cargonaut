# Implementation Plan: In-Session Mouse Capture Toggle

**Branch**: `041-mouse-capture-toggle` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/041-mouse-capture-toggle/spec.md`

## Summary

Add a runtime keymap binding — `M-m` (Alt-m) — that toggles mouse capture
on/off without restarting, implementing the deferred FR-013 from Feature 031
(issue #38). The existing TUI already tracks a single runtime capture flag
(`UiState.mouse_enabled`, initialized from `config.ui.mouse`) and already
honors it in `handle_mouse` (early-return when off) and `run_external`
(re-enables capture to match the flag after shelling out). This feature wires
a new `Command::ToggleMouseCapture` into the existing `dispatch_ui_command`
path to flip that flag and call `EnableMouseCapture`/`DisableMouseCapture`,
gated so that a session launched with mouse support disabled (`--no-mouse` /
`ui.mouse=false`) reports "disabled for this session" instead of capturing.

State is surfaced two ways (per clarification): a transient status-line
message on each toggle, plus a persistent indicator drawn in the top menu-bar
row showing captured vs. suspended.

The toggle decision is factored into a **pure function**
(`plan_mouse_toggle(supported, currently) -> MouseToggleOutcome`) so the
FR/SC behavior is unit-testable without terminal I/O; `dispatch_ui_command`
performs the thin `execute!` + status wiring from the returned outcome.

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest of
Cargonaut).

**Primary Dependencies**: `ratatui` + `crossterm` (TUI/keys/`EnableMouseCapture`
/`DisableMouseCapture`), `tokio` (async event loop), existing
`cargonaut-ui-tui` (`UiState`, `dispatch_ui_command`, `draw_frame`, `chrome`),
`cargonaut-ui-tui::keymap` (`Command`, `Keymap`), `cargonaut-config`
(`config.ui.mouse`). **No new crates.**

**Storage**: N/A — the toggle is transient session state; nothing is persisted
to the config file (relaunch starts from the config/flag default).

**Testing**: `cargo test --workspace`. The pure `plan_mouse_toggle` outcome
logic via plain unit tests (no I/O). The binding via `keymap.rs` unit tests
(`DEFAULT_KEYMAP_TOML` parse + `lookup`). The persistent indicator via a
`TestBackend` render assertion in `chrome`/`lib` tests (mirrors existing
`render_to_string` chrome tests). Help-text content via a string-contains test.

**Target Platform**: Linux terminal (TUI).

**Project Type**: Single Rust workspace (multi-crate). Front-end change is
confined to `cargonaut-ui-tui`; one binding line in the `design/contracts`
keymap.

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). The toggle
is an O(1) flag flip plus a single `execute!` of one control sequence; the
persistent indicator is a one-line `Paragraph` render in already-allocated
chrome. No measurable frame-budget impact.

**Constraints**: Honor NFR-001 (≤8 MiB stripped binary) — no new deps. `unsafe`
-free. `#![warn(missing_docs)]` on all new public items. New binding lands in
`design/contracts/keymap.toml` first (Constitution III).

**Scale/Scope**: 1 new `Command` variant, 1 keymap binding, 1 pure decision
function + outcome enum, 1 `dispatch_ui_command` arm, 1 persistent-indicator
render addition (threaded through `draw_frame`), help-text update, plus tests.
No new modules, no new dialog, no core-crate change.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe`. New public items (`Command::ToggleMouseCapture`,
  `MouseToggleOutcome`, `plan_mouse_toggle`) carry doc comments. clippy
  `-D warnings` and `cargo fmt --check` enforced by CI. ✅ (no waivers)
- **II. Test-First**: Every FR gets a failing test first (red) then implementation
  (green), per-task git history showing `(red)` → `(green)`. The pure
  `plan_mouse_toggle` covers FR-002/003/006 outcome logic; keymap test covers
  FR-001/009; render test covers FR-005 (persistent half); status assertions
  cover FR-005 (transient half). SC-001..006 each map to a unit/integration test
  run by `cargo test` in CI. No new criterion bench is required (this feature
  introduces no performance SC). ✅
- **III. UX Consistency**: New `M-m` binding is added to
  `design/contracts/keymap.toml` first (single source of truth). No ad-hoc
  dialog — the feature needs none. Indicator uses typed theme colors, no
  hardcoded ANSI. Help overlay updated so the binding is discoverable. ✅
- **IV. Performance (NFR-001/002)**: No new deps → no binary growth risk. Toggle
  + indicator are trivial-cost; no impact on the keypress-latency bench. ✅
- **V. SSD Preservation**: Dev build/test via `make` wrappers (tmpfs-guarded);
  no `cargo clean` / `rm -rf target`. ✅

**Result**: PASS — no violations, Complexity Tracking left empty.

## Project Structure

### Documentation (this feature)

```text
specs/041-mouse-capture-toggle/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (keymap + toggle-outcome contract)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
design/contracts/
└── keymap.toml                       # + [[binding]] global M-m → toggle-mouse-capture

crates/cargonaut-ui-tui/src/
├── keymap.rs                         # + Command::ToggleMouseCapture variant + tests
├── lib.rs                            # + plan_mouse_toggle()/MouseToggleOutcome,
│                                     #   dispatch_ui_command arm, draw_frame indicator
│                                     #   threading, help-text update + tests
└── chrome.rs                         # persistent mouse indicator render helper + tests
```

**Structure Decision**: Single Rust workspace, change isolated to the
`cargonaut-ui-tui` crate plus the shared keymap contract. No core/vfs/config
behavior changes (config is read-only here). This matches the established
pattern for keymap-driven UI features (e.g. Feature 038 `M-c` quick-cd, Feature
033 `M-!` filter) where a new `Command` variant flows through the shared
`dispatch_ui_command`.

## Complexity Tracking

> No constitution violations — section intentionally empty.

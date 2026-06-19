# Implementation Plan: Pane Tabs — Multiple Panels Per Side

**Branch**: `053-pane-tabs` | **Date**: 2026-06-19 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/053-pane-tabs/spec.md`

---

## Summary

Introduces per-side tab lists to the existing two-pane file manager. The internal `App.panes: [PaneState; 2]` is replaced by `App.sides: [SideState; 2]` where `SideState` holds a `Vec<PaneState>` + `active_tab: usize`. The public API (`pane(PaneId)`, `active_pane_state()`) is unchanged — they now return the active tab's `PaneState`. Four new commands (`TabNew`, `TabClose`, `TabNext`, `TabPrev`) map to `Ctrl-t`, `Ctrl-w`, `]`, `[`. A 1-row tab bar widget is added above each pane column and always rendered (FR-004). All existing features continue to work per-tab with zero call-site changes outside `cargonaut-core`.

---

## Technical Context

**Language/Version**: Rust 1.76 (MSRV per `Cargo.toml`)

**Primary Dependencies**: ratatui 0.27, crossterm 0.28, tokio 1.40 — no new crate dependencies for this feature

**Storage**: N/A — tab state is session-only, not persisted (per spec clarification)

**Testing**: `cargo test --workspace`, `cargo tarpaulin` (coverage gate ≥80%), `criterion` benches (keypress latency, RSS)

**Target Platform**: Linux terminal (primary), crossterm-compatible terminals

**Project Type**: TUI binary (`cargonaut-bin`) + library crates (`cargonaut-core`, `cargonaut-ui-tui`)

**Performance Goals**:
- ≤16ms keypress→first-paint (NFR-002, Constitution §IV) — tab bar adds one `Line` of `Span`s, negligible cost
- ≤64 MiB RSS (Constitution §IV SC-003) — 5 additional `PaneState`s add <1 MiB (SC-004)

**Constraints**: FR-009 — `PaneId` public API must not change; zero call-site modifications outside `cargonaut-core`

**Scale/Scope**: 4 source files modified (`lib.rs` ×2, `keymap.rs`, `keymap.toml`); ~200–300 lines net new

---

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-checked after Phase 1 design.*

| Principle | Status | Notes |
|---|---|---|
| §I Code Quality | ✅ PASS | All new public types carry `#[doc]`; clippy -D warnings satisfied; no `unsafe` code |
| §II Test-First | ✅ PASS | TDD applies: every FR gets a red test before green code; ≥80% coverage maintained |
| §III UX Consistency | ✅ PASS | New keys (`[`, `]`) land in `keymap.toml` first; `Ctrl-t`/`Ctrl-w` already there; tab bar uses theme variables (no hardcoded ANSI) |
| §IV Performance | ✅ PASS | 1-row `Line` render is O(n tabs) ~1µs; RSS increase <1 MiB for 5 extra tabs |
| §V SSD Preservation | ✅ PASS (CI exempt) | No artifact tree changes; tmpfs already set up |

**No constitution violations requiring justification.**

---

## Project Structure

### Documentation (this feature)

```text
specs/053-pane-tabs/
├── plan.md              # This file (/speckit-plan command output)
├── research.md          # Phase 0 output — R-001..R-006
├── data-model.md        # Phase 1 output — SideState, TabBarEntry, App changes
├── quickstart.md        # Phase 1 output — validation scenarios
├── contracts/
│   ├── core-api.md      # Public API additions + stable surface
│   ├── keymap-additions.toml  # New [ and ] bindings
│   └── tui-rendering.md # draw_frame/draw_pane contract
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code Changes (repository root)

```text
crates/cargonaut-core/src/lib.rs
  - New SideState struct (private)
  - New TabBarEntry struct + tab_bar_view() method (public)
  - App.panes → App.sides refactor
  - Four new Command variants: TabNew, TabClose, TabNext, TabPrev
  - Four new methods: tab_new(), tab_close(), tab_next(), tab_prev()
  - Dispatch arms for new Commands
  - New tests: ~15 new #[tokio::test] / #[test] cases

crates/cargonaut-ui-tui/src/keymap.rs
  - Two new Command variants: TabNext, TabPrev

crates/cargonaut-ui-tui/src/lib.rs
  - draw_frame() gains tab_bar_left/tab_bar_right parameters
  - draw_pane() gains tab_bar: &[TabBarEntry] parameter + 1-row tab bar render
  - run_loop() computes tab_bar_left/right per frame
  - ui_command_to_core() maps NewTab, CloseTab, TabNext, TabPrev
  - New tests: tab bar render tests

design/contracts/keymap.toml
  - Add [ → tab-prev binding
  - Add ] → tab-next binding
```

**Structure Decision**: All changes are additive to existing single-project crate layout. No new crates, no new modules (tab bar rendering is inlined in `draw_pane`).

---

## Phase 0: Research (complete — see research.md)

All NEEDS CLARIFICATION items resolved:

| Question | Decision | Ref |
|---|---|---|
| Ratatui Tabs widget vs. custom | Custom inline Line renderer (scroll + style control) | R-001 |
| Horizontal scroll algorithm | Per-frame stateless computation based on active tab position | R-002 |
| `[`/`]` key codes in crossterm | `KeyCode::Char('[')` / `KeyCode::Char(']')`, no conflicts | R-003 |
| Tab ops async or sync? | Synchronous — clone listing from source tab, no VFS call | R-004 |
| State isolation mechanism | Independent PaneState per tab; new tab starts clean | R-005 |
| Tab bar height constant? | Always 1 row; layout split to 3 constraints (tab bar + list + mini-status) | R-006 |

---

## Phase 1: Design (complete — see data-model.md, contracts/)

### Architecture decisions

**1. SideState wraps PaneState Vec (private)**

The refactor is surgical: only the 5 private accessors (`pane`, `pane_mut`, `active_pane_mut`, `active_pane_state`, `App::new`) touch `sides` directly. The 50+ dispatch arms in `App::dispatch()` all go through these accessors and require zero changes.

**2. tab_new() is synchronous — clone listing**

New tab clones source tab's `listing` snapshot. No VFS call, no `async`. Tradeoff: listing may be slightly stale if directory changed since last navigation. Acceptable per R-004.

**3. Tab bar renders in draw_pane() — not a separate widget struct**

A custom `tab_bar_line()` free function produces a `Line<'_>` from `&[TabBarEntry]` and `u16` width. `draw_pane()` renders it to a `Paragraph`. No `Widget` impl needed — keeps the widget count small.

**4. FR-013 modal guard is free**

The TUI's `handle_key()` returns early from the dialog arm before reaching `dispatch_ui_command()`. No additional guard needed at the tab dispatch level (though `dispatch_ui_command` may add defense-in-depth via `active_dialog.is_some()` check).

**5. FrameLayout.left/right updated to new inner rect**

After the layout change, `draw_pane()` returns `col[1]`'s inner rect (the list block's inner), not `col[0]`'s. Mouse hit-testing (`handle_mouse`) uses `layout.left`/`layout.right` to compute which row was clicked — it will automatically use the corrected rect.

### Keymap-first compliance (Constitution §III)

New keymap bindings (`[`, `]`) are added to `design/contracts/keymap.toml` before any implementation code ships. The existing `Ctrl-t` and `Ctrl-w` bindings were already in the keymap; the `Command` enum already has `NewTab` and `CloseTab`. The two new `Command` variants (`TabNext`, `TabPrev`) are added to `keymap.rs` alongside the TOML bindings.

---

## Implementation Order (for tasks.md generation)

The following user-story groupings map to task.md phases:

### US1: Core tab state machine (P1 — blocks everything)
1. Add `SideState`, `TabBarEntry` types to `cargonaut-core` (red tests first)
2. Refactor `App.panes → App.sides`; update `pane()`, `pane_mut()`, `active_pane_mut()`
3. Add `Command::TabNew/TabClose/TabNext/TabPrev`
4. Implement `tab_new()`, `tab_close()`, `tab_next()`, `tab_prev()`, `tab_bar_view()`
5. Wire dispatch arms in `App::dispatch()`
6. Green: all core tab tests pass

### US1+US3: Keymap bindings
7. Add `TabNext`, `TabPrev` to `keymap::Command` enum (`cargonaut-ui-tui`)
8. Add `[`/`]` bindings to `design/contracts/keymap.toml`
9. Wire `NewTab`, `CloseTab`, `TabNext`, `TabPrev` in `ui_command_to_core()`

### US1: Tab bar rendering
10. Implement `tab_bar_line()` in `cargonaut-ui-tui`
11. Update `draw_pane()` layout (3 constraints, render tab bar in col[0])
12. Update `draw_frame()` signature and call sites
13. Update `run_loop()` to compute `tab_bar_left`/`tab_bar_right` per frame

### US2: Cross-pane ops use active tab
14. Verify (test): `confirm_copy` uses correct active-tab cwd
15. Verify (test): `sync_other_panel_path` uses active-tab cwd

### US3: Per-tab state isolation
16. Verify (test): filter, sort, cursor, selection all isolated per tab
17. Verify (test): existing feature tests pass in single-tab configuration (regression gate)

### Performance gates
18. Run `cargo bench` — verify keypress latency ≤16ms with 5 tabs per side
19. Run RSS bench — verify ≤64 MiB total with 5 extra tabs
20. Coverage check ≥80%

---

## Complexity Tracking

No constitution violations to justify. The feature is additive:
- `SideState` is a natural wrapper over `Vec<PaneState>` — no complex abstraction
- The public API surface is strictly smaller post-refactor (4 new Commands, 1 new method, 1 new type)
- Rendering adds one `Line` + 3-constraint layout split — minimal UI complexity

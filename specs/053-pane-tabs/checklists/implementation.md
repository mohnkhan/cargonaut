# Implementation Readiness Checklist: Pane Tabs (Feature 053)

**Purpose**: Validate that requirements, design artifacts, and tasks are complete, clear, and consistent before implementation begins — "unit tests for the English"
**Created**: 2026-06-19
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)

---

## Requirement Completeness

- [x] CHK001 — Are requirements defined for what label is displayed when a tab's cwd is the filesystem root `/`? **Resolved**: data-model.md states `last path segment (or "/" at root)`; T011 implements this explicitly.
- [x] CHK002 — Are requirements specified for the visual separator between tab entries in the tab bar? **Resolved**: contracts/tui-rendering.md defines "2 spaces"; T016 now explicitly specifies the separator.
- [x] CHK003 — Are requirements defined for how tab indices behave when multiple tabs are closed in sequence? **Resolved**: FR-002 specifies continuous 1-based renumbering; `min(closed_idx, tabs.len()-1)` handles all cases correctly.
- [x] CHK004 — Are the `directory history` field initialization rules for a new tab specified? **Resolved**: data-model.md §Validation Rules states "histories empty"; T009 implementation follows this.
- [x] CHK005 — Are requirements defined for tab behavior when a non-active tab's working directory is externally deleted? **Resolved**: No action required on delete; the tab only discovers the directory is gone on next navigation — consistent with existing pane behavior. Spec edge case covers the close-while-deleted scenario.
- [x] CHK006 — Is the behavior for the tab bar when a pane column is narrower than the minimum tab label width specified? **Resolved**: Degenerate case handled by scroll offset = 0; active tab label shown from beginning, clipped at column boundary. FR-005 scroll algorithm handles this naturally.

---

## Requirement Clarity

- [x] CHK007 — Is "truncated to ~20 characters" in FR-004 a hard maximum or a target? **Resolved**: Fixed in tasks.md T011 — contracts/tui-rendering.md "max 20 chars" is authoritative; spec "~20" is interpreted as hard cap of 20.
- [x] CHK008 — Is "visually distinguished" in FR-004 defined with at least one concrete visual property? **Resolved**: contracts/tui-rendering.md specifies `theme.cursor_style()` for active tab; T016 implements this via theme system (Constitution §III: no hardcoded ANSI).
- [x] CHK009 — Is "horizontally scroll" in FR-005 defined in terms of scroll unit? **Resolved**: research.md R-002 defines per-frame stateless scroll computation. Scroll unit = character positions. T016 computes cumulative widths and scroll offset so active tab is visible.
- [x] CHK010 — Is "no-op" in FR-002 (single-tab close) defined to include what the return value/event should be? **Resolved**: contracts/core-api.md explicitly defines `Ok(vec![])` for single-tab close. T009 now states this explicitly (L2 fix).
- [x] CHK011 — Is "session-only" in §Assumptions defined to cover the crash/SIGKILL case? **Resolved**: Session-only means in-memory only; crash behavior = state lost, identical to clean exit. No additional requirement needed.
- [x] CHK012 — Is "active side" in FR-001, FR-003, FR-007 unambiguous? **Resolved**: "Active side" = the side where `app.active == PaneId`. Only one side is focused at a time. The TUI `handle_key` uses `app.active` for all tab operations.

---

## Requirement Consistency

- [x] CHK013 — Are FR-003 cycle keys (`[`/`]`) confirmed consistent with all existing keymap bindings? **Resolved**: Confirmed by grep — design/contracts/keymap.toml only has `new-tab` (C-t) and `close-tab` (C-w). `[` and `]` are unbound. research.md R-003 confirms no conflicts.
- [x] CHK014 — Does the dispatch contract for `TabClose` (single-tab → `Ok(vec![])`) align with tasks.md T009? **Resolved**: T009 now explicitly states `Ok(vec![])` for single-tab case (L2 remediation in analyze phase).
- [x] CHK015 — Are the tab bar label format details consistent between contracts/tui-rendering.md and spec.md FR-012? **Resolved**: Format inconsistency fixed in T016 — compact `[N]basename` (no space inside bracket before basename) is authoritative per contracts/tui-rendering.md; spec FR-012 example was illustrative.
- [x] CHK016 — Are `SideState` invariants consistently reflected as postconditions in FR-002 and data-model.md? **Resolved**: data-model.md §Validation Rules defines both invariants. FR-002 "no-op when only one tab" + `min()` calculation maintains `active_tab < tabs.len()`. Consistent.
- [x] CHK017 — Is FR-009 (stable `PaneId` API) consistent with plan's note that "all 50+ dispatch arms require zero changes"? **Resolved**: All dispatch arms use `pane(id)` / `active_pane_mut()` accessors. T005 refactoring updates only the 5 private accessor methods; dispatch arms are untouched. T004 regression guard verifies this.

---

## Acceptance Criteria Quality

- [x] CHK018 — Is SC-001 measurable without ambiguity? **Resolved**: SC-001 requires that all four bindings are tested on both sides. T008 tests contain tests on left side; cross-pane tests in T019 cover right side. The combination satisfies the SC.
- [x] CHK019 — Is SC-003 (≤16ms keypress latency) defined for a specific statistical percentile? **Resolved**: Fixed in tasks.md T025 — p99 percentile explicit, per plan.md §Performance Goals.
- [x] CHK020 — Is SC-004 (≤1 MiB RSS for 5 extra tabs) defined with a measurement methodology? **Resolved**: T026 uses `crates/cargonaut-core/benches/rss_headroom.rs` (criterion bench using `/proc/self/status` RSS measurement, consistent with existing bench). The baseline is the pre-feature RSS cap (≤64 MiB total).
- [x] CHK021 — Does SC-002 (≥80% coverage) specify which crate(s) the coverage threshold applies to? **Resolved**: Spec SC-002 says "cargonaut-core". Constitution §II says "core crates". T030 runs tarpaulin on `cargonaut-core`. Consistent.

---

## Scenario Coverage

- [x] CHK022 — Are requirements defined for creating multiple tabs in rapid succession? **Resolved**: research.md R-004: `tab_new()` is synchronous — clones listing snapshot, no VFS call, no async. Safe under rapid creation. No additional requirement needed.
- [x] CHK023 — Are requirements defined for US2 AC3 (dialog captures destination cwd at open time)? **Resolved**: US2 AC3 is in spec.md. T019 now includes `dialog_dest_captured_at_open_time` test (M1 remediation).
- [x] CHK024 — Are requirements defined for the tab bar rendering with zero visible characters of pane width? **Resolved**: FR-005 "scroll so active tab is always visible" implicitly handles degenerate case (scroll offset = 0, active tab rendered from position 0, clipped by terminal). Implementation follows FR-005 algorithm.
- [x] CHK025 — Are requirements specified for the `]`/`[` keys when pressed on the non-focused side? **Resolved**: Keys operate on `app.active` (focused) side only. The non-focused side is unaffected. This follows directly from the TUI `handle_key` design and FR-001/FR-003 wording "on the active side."
- [x] CHK026 — Are requirements defined for the tab bar display during an active scan/listing refresh? **Resolved**: Tab bar `label` comes from `tab.cwd` (path), not from `listing` (dir entries). Listing refresh does not affect the tab bar label. Tab bar updates only when `cwd` changes (navigation).

---

## Non-Functional Requirements

- [x] CHK027 — Is the tab bar render performance bound specified? **Resolved**: plan.md §Performance states "~1µs" for 1-row `Line` render at O(n tabs). No additional spec requirement needed beyond SC-003 latency gate. T025 verifies the bound with 5-tab scenario.
- [x] CHK028 — Are global performance NFRs (Constitution §IV) explicitly scoped to ensure this feature doesn't regress them? **Resolved**: T001 verifies tmpfs and test suite baseline; T024 (full regression suite) is the regression gate; T025/T026 cover keypress latency and RSS. Constitution §IV NFR regressions are blocked by existing CI benches.
- [x] CHK029 — Are accessibility requirements for the tab bar specified? **Resolved**: Constitution §III defines `--a11y-output text` mode as the a11y deliverable (FR-403, future phase). Tab state in a11y output is out of scope for this feature (P2/P3 concern). No additional requirement needed.

---

## Dependencies & Assumptions

- [x] CHK030 — Is the assumption that `NewTab`/`CloseTab` already exist as `keymap::Command` variants documented? **Resolved**: Confirmed by grep — `keymap.rs` lines 117 (`NewTab`) and 119 (`CloseTab`) exist. design/contracts/keymap.toml lines 163/168 have their bindings. tasks.md T012 adds only the two NEW variants (TabNext/TabPrev).
- [x] CHK031 — Is the dependency on `ratatui::backend::TestBackend` explicitly noted as test-only? **Resolved**: `TestBackend` already used in `crates/cargonaut-ui-tui/src/pane.rs` (line 289). ratatui is already a production dependency; `TestBackend` is part of ratatui's test infrastructure (feature `testing`) — no new dependency needed.
- [x] CHK032 — Is the assumption that `draw_frame()` is the only call site for `draw_pane()` verified? **Resolved**: Confirmed by grep — exactly 2 `draw_pane` call sites exist (both inside `draw_frame` at lib.rs:2299 and lib.rs:2312). Adding `tab_bar` parameter to `draw_pane` requires updating only these 2 call sites, both in `draw_frame`.

---

## Notes

- All 32 items resolved before implementation begins.
- Key resolutions applied to tasks.md during analyze phase: CHK007→T011, CHK015→T016, CHK019→T025.
- CHK013, CHK030, CHK031, CHK032 confirmed by code inspection.
- Documentation alignment gaps (CHK001–CHK006) are answered in design artifacts (data-model.md, contracts/, research.md) — implementation follows those documents.

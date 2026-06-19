# Implementation Readiness Checklist: Pane Tabs (Feature 053)

**Purpose**: Validate that requirements, design artifacts, and tasks are complete, clear, and consistent before implementation begins — "unit tests for the English"
**Created**: 2026-06-19
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)

---

## Requirement Completeness

- [ ] CHK001 — Are requirements defined for what label is displayed when a tab's cwd is the filesystem root `/`? [Completeness, Gap — data-model.md specifies `last path segment (or "/" at root)` but spec.md FR-004 does not mention this edge] [Spec §FR-004]
- [ ] CHK002 — Are requirements specified for the visual separator between tab entries in the tab bar (e.g., number of spaces, divider character)? [Completeness — contracts/tui-rendering.md defines "2 spaces" but spec FR-004/FR-012 do not specify this detail] [Spec §FR-012]
- [ ] CHK003 — Are requirements defined for how tab indices behave when multiple tabs are closed in sequence (e.g., close tab 1 then close new tab 1 of a two-tab set)? [Completeness, Spec §FR-002]
- [ ] CHK004 — Are the `directory history` field initialization rules for a new tab specified in the feature spec (not just in data-model.md)? [Completeness, Gap — data-model.md says "histories empty" but spec FR-006 lists history as an isolated field without specifying its initial value] [Spec §FR-006]
- [ ] CHK005 — Are requirements defined for tab behavior when a tab's working directory is externally deleted WHILE it is not the active tab (not just when it is the active tab being closed)? [Completeness, Gap — spec Edge Cases only addresses the close-while-deleted scenario] [Spec §Edge Cases]
- [ ] CHK006 — Is the behavior for the tab bar when a pane column is narrower than the minimum tab label width specified? [Completeness, Gap — spec §Edge Cases mentions minimum label width as "an implementation detail", but no requirement defines the fallback rendering]

---

## Requirement Clarity

- [ ] CHK007 — Is "truncated to ~20 characters" in FR-004 a hard maximum or a target? The tilde (~) introduces intentional ambiguity; does the spec intend to allow up to 20, exactly 20, or approximately 20? [Clarity, Spec §FR-004 — contracts/tui-rendering.md says "max 20 chars" (hard); spec uses "~20"; these should align]
- [ ] CHK008 — Is "visually distinguished" in FR-004 defined with at least one concrete visual property, or left entirely to implementation? The spec lists "e.g., bold, highlighted background, or underline" as examples — is one of these required or is the choice unconstrained? [Clarity, Spec §FR-004]
- [ ] CHK009 — Is "horizontally scroll" in FR-005 defined in terms of the scroll unit (whole tabs, pixels, or characters)? [Clarity, Spec §FR-005 — research.md R-002 says "per-frame stateless computation" but the scroll unit is not specified]
- [ ] CHK010 — Is "no-op" in FR-002 (single-tab close returns `Ok(vec![])`) defined in the spec, or only in contracts/core-api.md? A caller reading only the spec would not know whether an event is emitted. [Clarity, Spec §FR-002 — gap between spec prose and dispatch contract]
- [ ] CHK011 — Is "session-only" in §Assumptions defined to cover the crash/SIGKILL case (tabs are lost, no crash), or only the clean-exit case? [Clarity, Spec §Assumptions]
- [ ] CHK012 — Is the meaning of "active side" in FR-001, FR-003, and FR-007 unambiguous — does it mean the focused pane, or can a non-focused pane also be "active"? [Clarity, Spec §FR-001/FR-003/FR-007]

---

## Requirement Consistency

- [ ] CHK013 — Are the FR-003 cycle keys (`[`/`]`) confirmed consistent with all existing keymap bindings — specifically that `[` and `]` are currently unbound in `design/contracts/keymap.toml`? [Consistency, Spec §FR-003 ↔ research.md R-003 — R-003 confirms no conflicts; spec should reference this explicitly]
- [ ] CHK014 — Does the dispatch contract for `TabClose` (single-tab → `Ok(vec![])`) in contracts/core-api.md align with the tasks.md T009 description, which previously implied `PaneUpdated` on all paths? [Consistency, contracts/core-api.md ↔ tasks.md T009 — remediated in H2 fix; verify alignment holds]
- [ ] CHK015 — Are the tab bar label format details in contracts/tui-rendering.md (`[N]basename  [N*]basename`, 2-space separator) consistent with the examples shown in spec.md FR-012 (`[1] src  [2] tests  [3*] docs` — which uses space before basename, not in contracts format)? [Consistency, Spec §FR-012 ↔ contracts/tui-rendering.md — space inside bracket vs outside bracket differs]
- [ ] CHK016 — Are the `SideState` invariants (`tabs.len() >= 1`, `active_tab < tabs.len()`) consistently reflected as enforceable postconditions in FR-002 and in data-model.md §Validation Rules? [Consistency, data-model.md ↔ Spec §FR-002]
- [ ] CHK017 — Is FR-009 (stable `PaneId` API) consistent with the plan's note that "all 50+ dispatch arms require zero changes" — is there any dispatch arm that currently uses the `panes` field directly (bypassing the accessor) that would NOT be covered by the API stability guarantee? [Consistency, Spec §FR-009 ↔ plan.md §Architecture]

---

## Acceptance Criteria Quality

- [ ] CHK018 — Is SC-001 ("integration test exercising all four bindings on both sides") measurable without ambiguity — does "both sides" mean the test must exercise Left AND Right side independently, and does "all four bindings" mean a single combined test or individual tests are sufficient? [Measurability, Spec §SC-001]
- [ ] CHK019 — Is SC-003 (≤16ms keypress latency) defined for a specific statistical percentile (plan.md says "p99" — is this in the spec)? [Clarity, Spec §SC-003 — plan.md includes p99 but spec.md §SC-003 does not specify percentile]
- [ ] CHK020 — Is SC-004 (≤1 MiB RSS for 5 extra tabs) defined with a measurement methodology — what baseline RSS, what tool (tarpaulin, /proc/self/status), what warm-up? [Measurability, Spec §SC-004]
- [ ] CHK021 — Does SC-002 (≥80% coverage maintained) specify which crate(s) the coverage threshold applies to — is it `cargonaut-core` only, or all workspace crates? [Clarity, Spec §SC-002 — Constitution §II says "core crates"; spec SC-002 says "cargonaut-core"]

---

## Scenario Coverage

- [ ] CHK022 — Are requirements defined for creating multiple tabs in rapid succession (e.g., pressing Ctrl-t 10 times before any listing is refreshed)? [Coverage, Gap — research.md R-004 says "synchronous clone" but no spec requirement bounds rapid creation]
- [ ] CHK023 — Are requirements defined for US2 AC3 (copy dialog captures destination cwd at the time the dialog is opened, not at confirmation time)? [Coverage, Spec §US2 AC3 — present in spec; verify tasks.md T019 now covers it after M1 remediation]
- [ ] CHK024 — Are requirements defined for the tab bar rendering with zero visible characters of pane width (extremely narrow terminal)? [Coverage, Edge Case, Gap — FR-005 says "scrolls to keep active tab visible" but does not define degenerate minimum-width behavior]
- [ ] CHK025 — Are requirements specified for the `]`/`[` keys when pressed on the non-focused side? [Coverage, Gap — spec says keys affect "the active side" (focused pane), but does not explicitly confirm `[`/`]` are scoped to the focused side only]
- [ ] CHK026 — Are requirements defined for the tab bar display during an active scan/listing refresh (e.g., does the tab bar update immediately or wait for refresh)? [Coverage, Gap — tab bar uses `cwd` from PaneState, not listing; but not explicitly stated in spec]

---

## Non-Functional Requirements

- [ ] CHK027 — Is the tab bar render performance bound (O(n tabs) per frame) specified or bounded with a maximum-tabs scenario? [Completeness, Gap — plan.md §Performance says "~1µs" but spec has no NFR for tab count scaling]
- [ ] CHK028 — Are the global performance NFRs (Constitution §IV SC-001: ≥80% cp throughput; SC-002: SIGKILL resume; SC-004: ≤150ms startup) explicitly scoped to ensure this feature does not regress them even with 5 tabs per side? [Coverage, Constitution §IV — tasks address spec SC-003/SC-004 but not regression of constitution SC-002/SC-004]
- [ ] CHK029 — Are accessibility requirements for the tab bar specified — e.g., does a screen-reader user get any indication of which tab is active, given the TUI's `--a11y-output text` mode (Constitution §III)? [Coverage, Gap — Constitution §III defines a11y mode as the deliverable; no FR addresses tab state in a11y output]

---

## Dependencies & Assumptions

- [ ] CHK030 — Is the assumption that `NewTab`/`CloseTab` already exist as `keymap::Command` variants (and are wired in keymap.toml as `C-t`/`C-w`) documented in plan.md or tasks.md, or is it only derivable by reading keymap.rs? [Assumption, plan.md §Phase 1 — partially; tasks T012 references adding TabNext/TabPrev but assumes NewTab/CloseTab exist]
- [ ] CHK031 — Is the dependency on `ratatui::backend::TestBackend` for tab bar rendering unit tests (T015) explicitly noted as a test-only dependency — confirming it does not add a production dependency? [Dependency, tasks.md T015]
- [ ] CHK032 — Is the assumption that `draw_frame()` is the only call site for `draw_pane()` (no other callers that would break when the tab_bar parameter is added) explicitly verified in plan.md or tasks.md? [Assumption, plan.md §Architecture — "update the single draw_frame call site" implies yes, but not explicitly confirmed]

---

## Notes

- Items marked `[Gap]` indicate requirements absent from spec.md that exist only in design documents (data-model.md, contracts/, research.md) — these are acceptable if the implementation artifacts are sufficient, but the gap is worth noting for maintainability.
- CHK013, CHK030 are likely already satisfied — verify against source files during review.
- CHK015 (label format space placement) is a low-risk cosmetic inconsistency — resolve in T016 implementation with a definitive format choice.
- Mark items `[x]` as verified during implementation review; annotate with the resolution method if a gap was found.

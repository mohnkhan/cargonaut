# Checklist: Menu Dropdown Mouse-Interaction Requirements Quality

**Purpose**: Validate that the requirements for menu-dropdown mouse interaction are complete,
clear, consistent, and measurable BEFORE implementation. These are "unit tests for the spec",
not for the code.
**Created**: 2026-06-22
**Feature**: [spec.md](../spec.md)
**Focus**: hit-testing geometry, click-to-invoke + close, close-and-pass-through, switch/toggle,
hover highlight, graceful degradation (mouse disabled / no motion events).

## Hit-Testing Geometry

- [x] CHK001 Are the dropdown's clickable item rows defined relative to the rendered frame (border offset) so an off-by-one cannot occur? [Clarity, Spec §FR-002, data-model.md]
- [x] CHK002 Is the behavior of a click on the dropdown border (inside the frame, not an item) explicitly specified as a no-op that keeps the menu open? [Completeness, Spec §FR-003]
- [x] CHK003 Are requirements defined for which item rows are clickable when the dropdown is clamped by a short terminal (clipped rows not selectable)? [Edge Case, Spec §Edge Cases]
- [x] CHK004 Is the distinction between "inside the dropdown frame" and "fully outside" specified precisely enough to drive different behaviors (no-op vs close)? [Clarity, Spec §FR-003/FR-004]
- [x] CHK005 Is the source of the geometry used for hit-testing required to match the rendering geometry (single source of truth)? [Consistency, research.md D1]

## Click-to-Invoke + Close

- [x] CHK006 Is the end state after clicking an item specified (command dispatched AND menu closed)? [Completeness, Spec §FR-001]
- [x] CHK007 Is it required that a clicked item routes through the same dispatch path as keyboard Enter (identical behavior regardless of input)? [Consistency, Spec §FR-012]
- [x] CHK008 Are the first-row and last-row click outcomes specified to confirm no boundary mis-mapping? [Measurability, Spec §FR-002, US1 scenarios]

## Close-and-Pass-Through

- [x] CHK009 Is the click-outside behavior unambiguously specified as close-AND-act (pass-through), not swallow? [Clarity, Spec §FR-004, §Clarifications]
- [x] CHK010 Are the specific panel actions that the pass-through click triggers enumerated (focus pane + move cursor; double-click descends)? [Completeness, Spec §FR-004]
- [x] CHK011 Is it specified that no menu item is invoked by a close-and-pass-through click? [Consistency, Spec §FR-004]

## Switch / Toggle

- [x] CHK012 Is switching to a different menu by clicking its title specified while a menu is already open? [Completeness, Spec §FR-005]
- [x] CHK013 Is the toggle-closed behavior on clicking the already-open menu's own title specified? [Completeness, Spec §FR-006]
- [x] CHK014 Are switch (FR-005) and toggle (FR-006) consistent with the existing title-click open behavior (no conflicting rule)? [Consistency, Spec §FR-005/006]

## Hover Highlight

- [x] CHK015 Is hover-to-highlight specified to update the selection (so a later click/Enter acts on it) rather than a separate transient highlight? [Clarity, Spec §FR-007, research.md D2]
- [x] CHK016 Is it specified that hover dispatches no command? [Completeness, Spec §FR-007]
- [x] CHK017 Is the no-change behavior defined when the pointer moves over the border or off the item rows? [Edge Case, Spec §FR-008]
- [x] CHK018 Is a performance constraint stated for hover handling given high-frequency motion events? [Non-Functional, Spec §FR-007, plan.md Performance Goals]

## Graceful Degradation

- [x] CHK019 Is behavior specified when mouse is disabled for the session (--no-mouse / ui.mouse=false) — no menu mouse effect, keyboard unchanged? [Coverage, Spec §FR-009/FR-011]
- [x] CHK020 Is behavior specified when mouse is suspended at runtime (Alt-m)? [Coverage, Spec §Edge Cases]
- [x] CHK021 Is the fallback specified when the terminal delivers no motion events (clicks still work; hover forgone)? [Edge Case, Spec §FR-010]
- [x] CHK022 Is it stated that existing keyboard navigation (F9/arrows/hjkl/Enter/Esc) remains unchanged by this feature? [Consistency, Spec §FR-011]

## Cross-Cutting Requirements Quality

- [x] CHK023 Are all success criteria (SC-001…SC-005) measurable and traceable to specific requirements/scenarios? [Measurability, Spec §Success Criteria]
- [x] CHK024 Is scope explicitly bounded (right/middle-click, scroll-in-dropdown, user-menu dialog excluded)? [Coverage, Spec §Out of Scope]
- [x] CHK025 Are the assumptions (one-cell border, left-button-only, terminal-dependent motion) documented and validated against the codebase? [Assumption, Spec §Assumptions]

## Notes

- Pass = the requirement is present, unambiguous, and consistent in spec.md (and supporting
  design docs). Fail = ambiguous, missing, or conflicting → fix the spec before/while
  implementing.
- This checklist validates the *spec*. Behavioral verification of the code lives in
  [quickstart.md](../quickstart.md) and the test tasks in [tasks.md](../tasks.md).

# Implementation-Readiness Checklist: Panel `..` Parent Entry as First Row

**Purpose**: Validate that the requirements are complete, clear, consistent, and
measurable before implementation — focused on the virtual-row cursor model,
ascend-on-activate parity, the non-operable guarantees, presence rules, and
selection-index stability. (These items test the *requirements*, not the code.)
**Created**: 2026-06-15
**Feature**: [spec.md](../spec.md)

## Cursor / Index Model

- [ ] CHK001 Is the addressable cursor range defined precisely (the `..` row plus visible real entries) rather than left implicit? [Clarity, Data-model]
- [ ] CHK002 Is the mapping between cursor position and the real-entry index it refers to specified unambiguously, including when on the `..` row? [Completeness, Contract focused_entry_index]
- [ ] CHK003 Is the default cursor position on entering a directory specified for non-root, root, and empty-non-root cases? [Completeness, Spec §FR-014]
- [ ] CHK004 Are cursor clamping bounds defined for both ends (cannot go above `..`, cannot pass the last row)? [Edge Case, Spec §FR-005]
- [ ] CHK005 Is it stated that the selection set continues to reference real entries (not virtual rows), independent of the `..` row's presence? [Consistency, Spec §FR-010]
- [ ] CHK006 Is "real-entry identity stays stable" expressed measurably (same entry selected before/after the change for the same actions)? [Measurability, Spec §SC-005]

## Activation Parity (keyboard + mouse)

- [ ] CHK007 Is the activation that ascends from the `..` row defined for the keyboard path (which key)? [Completeness, Spec §FR-003]
- [ ] CHK008 Is the activation defined for the mouse path (double-click), and stated to use the same ascent operation? [Consistency, Spec §FR-004]
- [ ] CHK009 Does the spec require keyboard, mouse, and render to agree on which row is `..` (one model, no divergence)? [Consistency, Spec §FR-011]
- [ ] CHK010 Is it specified that ascent from the `..` row uses the same path/side effects as the existing ascend action (history, etc.)? [Clarity, Spec §FR-003, Assumptions]

## Non-Selectable / Non-Operable Guarantees

- [ ] CHK011 Is "the `..` row cannot be tagged" stated as a requirement with a defined outcome (no-op)? [Completeness, Spec §FR-006]
- [ ] CHK012 Are bulk operations (invert, select-by-pattern) explicitly required to exclude `..`, including a pattern that textually matches `..`? [Coverage, Spec §FR-007, Edge Cases]
- [ ] CHK013 Is it required that copy/move/delete can never include the parent, in every selection state? [Completeness, Spec §FR-008]
- [ ] CHK014 Is the behavior of activating copy/move/delete while focused on `..` with an empty selection defined (operates on nothing)? [Edge Case, Gap]

## Presence Rules (filter / hidden / root)

- [ ] CHK015 Is the `..` row required to be present regardless of the active name filter — even one matching zero real entries? [Coverage, Spec §FR-009, Edge Cases]
- [ ] CHK016 Is the `..` row required to be unaffected by the hidden-file toggle? [Coverage, Spec §FR-009]
- [ ] CHK017 Is suppression of the `..` row at a filesystem root specified, with ascent above root impossible? [Completeness, Spec §FR-002, §SC-002]
- [ ] CHK018 Is the empty-non-root-directory case defined (only the `..` row present, cursor rests on it)? [Edge Case, Spec §FR-014, Edge Cases]

## Rendering & Identity

- [ ] CHK019 Is the `..` row's required position (first row) and label specified? [Clarity, Spec §FR-001, §FR-012]
- [ ] CHK020 Is it specified that entry counts / status do not count `..` as a real entry? [Completeness, Spec §FR-013]
- [ ] CHK021 Is the `..` row required to be visually highlightable like any other row (so the cursor on it is visible)? [Gap, Spec §FR-012]
- [ ] CHK022 Is long-listing/viewport behavior consistent with the `..` row occupying a row (no off-by-one in scrolling)? [Edge Case, Gap]

## Consistency & Boundaries

- [ ] CHK023 Is the term "row" used consistently to mean a virtual row (vs "entry" for a real listing item) across spec/plan/contracts? [Consistency, Terminology]
- [ ] CHK024 Are the out-of-scope items (land-on-came-from, `.` row, `..` in non-pane lists, drag-to-parent) explicit so they aren't silently expected? [Coverage, Spec §Out of Scope]
- [ ] CHK025 Is it specified that the existing ascend key/menu/mouse behavior is unchanged (the `..` row is additive)? [Consistency, Spec §Out of Scope]
- [ ] CHK026 Is per-pane scoping stated (activating `..` in one pane does not affect the other)? [Clarity, Edge Cases]

## Acceptance & Process

- [ ] CHK027 Does each functional requirement (FR-001…FR-014) have at least one measurable success criterion or acceptance scenario? [Coverage, Spec §SC-001…SC-006]
- [ ] CHK028 Are the success criteria technology-agnostic and objectively verifiable? [Measurability, Spec §Success Criteria]
- [ ] CHK029 Do the tasks encode red→green ordering, including updating the ~20 existing index-coupled tests in the red step? [Traceability, tasks.md, Constitution §II]
- [ ] CHK030 Are the mandatory docs updates (README, Learnings) and the #37 issue/ROADMAP closure represented as tasks? [Completeness, tasks.md T016–T018, CLAUDE.md]

## Dependencies & Assumptions

- [ ] CHK031 Is the assumption that a single canonical ascent operation already exists documented and validated against the code? [Assumption, Spec §Assumptions, Research R-001/R-003]
- [ ] CHK032 Is the assumption that root detection already exists (and drives presence) documented? [Assumption, Spec §Assumptions, Research R-003]
- [ ] CHK033 Is it stated that no new keymap binding is introduced (reuses existing activation)? [Dependency, Plan Constitution §III]

## Notes

- Check items off as the requirements are confirmed: `[x]`.
- CHK006 corresponds to analyze finding C1: fold an explicit before/after
  selection-stability assertion into T011 during implementation.
- This checklist validates requirement quality; functional verification lives in
  the test tasks (T003–T013) and quickstart scenarios.

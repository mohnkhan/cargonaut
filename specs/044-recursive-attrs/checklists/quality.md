# Requirements Quality Checklist: Recursive chmod / chown

**Purpose**: Unit-test the *requirements* (clarity, completeness, consistency, coverage) before implementation
**Created**: 2026-06-17
**Feature**: [spec.md](../spec.md)
**Depth**: Standard · **Audience**: PR reviewer

## Requirement Completeness

- [x] CHK001 — Is the recursion opt-in mechanism (dedicated `C-x C` / `C-x O` + menu) fully specified? [Completeness, Spec §FR-001 / Clarifications]
- [x] CHK002 — Is the confirmation requirement defined for every recursive change? [Completeness, Spec §FR-002]
- [x] CHK003 — Are both file and directory entries within the subtree covered by the change? [Completeness, Spec §FR-003/004]
- [x] CHK004 — Is the symlink-handling rule (no descent into link directories) specified? [Completeness, Spec §FR-006]
- [x] CHK005 — Is the bound on traversal and its truncation reporting specified? [Completeness, Spec §FR-005]
- [x] CHK006 — Is per-entry partial-failure handling (aggregate, no rollback) specified? [Completeness, Spec §FR-007]

## Requirement Clarity

- [x] CHK007 — Is "applied recursively to the subtree" defined precisely (which entries, from which roots)? [Clarity, Spec §Key Entities "Subtree"]
- [x] CHK008 — Is the apply-order requirement (not locking out mid-traversal) stated unambiguously? [Clarity, Spec §FR-011 / Edge Cases]
- [x] CHK009 — Is symbolic-vs-octal behavior under recursion clarified (per-entry relative)? [Clarity, Spec §FR-003]
- [x] CHK010 — Is "the selection includes a directory" vs a file-only selection disambiguated? [Clarity, Spec §FR-009]

## Requirement Consistency

- [x] CHK011 — Are the recursive chords consistent with the existing shallow `C-x c`/`C-x o` and the `C-x` family (no collision)? [Consistency, Spec §FR-001]
- [x] CHK012 — Is the recursive flow consistent with Feature 043's input→confirm→apply model? [Consistency, Spec §Assumptions]
- [x] CHK013 — Does "non-recursive operations remain unchanged" align with adding recursion (opt-in)? [Consistency, Spec §FR-008]

## Acceptance Criteria Quality

- [x] CHK014 — Is "applied at depth" objectively verifiable (a nested entry has the new attribute)? [Measurability, Spec §SC-001/002]
- [x] CHK015 — Is "declining confirmation changes nothing" measurable? [Measurability, Spec §SC-003]
- [x] CHK016 — Is "completes without hanging + reports truncation" measurable? [Measurability, Spec §SC-004]
- [x] CHK017 — Is "symlink target outside the subtree unchanged" measurable? [Measurability, Spec §SC-005]

## Edge Case & Scenario Coverage

- [x] CHK018 — Is the unreadable-subdirectory case (skip + report, continue siblings) covered? [Coverage, Spec §Edge Cases]
- [x] CHK019 — Is the symlink-cycle / tree-escape risk addressed (via no-follow)? [Coverage, Spec §FR-006]
- [x] CHK020 — Is the huge-tree case covered (bound + truncation)? [Edge Case, Spec §FR-005/SC-004]
- [x] CHK021 — Is the `..`-row exclusion restated for recursion? [Coverage, Spec §Edge Cases]
- [x] CHK022 — Is the mixed selection (directory + loose files) behavior specified? [Coverage, Spec §Edge Cases]
- [x] CHK023 — Is recursion-requested-on-a-plain-file handled (acts as shallow)? [Edge Case, Spec §FR-009]

## Scope & Boundary

- [x] CHK024 — Is it stated that no new VFS operations / no new persisted state are introduced? [Boundary, Spec §Assumptions]
- [x] CHK025 — Is the feature scoped to the local backend? [Boundary, Spec §Assumptions]
- [x] CHK026 — Is directory-smart mode handling (e.g. `X`) explicitly out of scope (literal `chmod -R`)? [Boundary, research R-006]

## Notes

- All 26 items pass against the current spec (+ research for CHK026 / CHK008 apply-order rationale).
- This is a requirements-quality gate; implementation correctness is gated by the TDD tasks in tasks.md.

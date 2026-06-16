# Requirements Quality Checklist: File Attribute Operations

**Purpose**: Unit-test the *requirements* (clarity, completeness, consistency, coverage) before implementation
**Created**: 2026-06-17
**Feature**: [spec.md](../spec.md)
**Depth**: Standard · **Audience**: PR reviewer

## Requirement Completeness

- [x] CHK001 — Are requirements defined for both chmod input forms (octal AND symbolic)? [Completeness, Spec §FR-001]
- [x] CHK002 — Are requirements present for all four operations (chmod, chown, symlink, hardlink)? [Completeness, Spec §FR-001..004]
- [x] CHK003 — Is the behavior on a backend that lacks an operation specified? [Completeness, Spec §FR-006]
- [x] CHK004 — Are post-operation listing-refresh requirements stated? [Completeness, Spec §FR-008]
- [x] CHK005 — Is multi-file (batch) selection behavior, including partial failure, specified? [Completeness, Spec §FR-010, SC-005]
- [x] CHK006 — Is it specified how ownership change is *observed* given no owner column exists? [Completeness, research R-006 / SC-006]

## Requirement Clarity

- [x] CHK007 — Is the accepted symbolic-mode grammar defined precisely enough to validate input? [Clarity, Spec §Assumptions, data-model ModeSpec]
- [x] CHK008 — Is the chown owner-string format (`user` / `:group` / `user:group`, name or numeric) unambiguous? [Clarity, data-model]
- [x] CHK009 — Is "current selection" defined exactly (tagged, else focused, excluding `..`)? [Clarity, Spec §FR-005]
- [x] CHK010 — Is the link-creation location (active pane cwd, user-named) specified? [Clarity, Spec §Assumptions]
- [x] CHK011 — Is symbolic-vs-absolute application across multiple files (relative per-file vs absolute) made explicit? [Clarity, research R-002]

## Requirement Consistency

- [x] CHK012 — Are the keybindings (`C-x c/o/s/l`) consistent with the existing `C-x` chord family and free of collisions? [Consistency, Spec §FR-011]
- [x] CHK013 — Is the confirmation rule consistent (chown confirmed; single-file chmod not)? [Consistency, Spec §FR-007 / Assumptions]
- [x] CHK014 — Do the selection semantics match the existing copy/move/delete model (no new concept)? [Consistency, Spec §Assumptions]

## Acceptance Criteria Quality

- [x] CHK015 — Is the invalid-input outcome objectively verifiable (no change + error)? [Measurability, Spec §FR-009 / SC-004]
- [x] CHK016 — Is "reported without crashing" measurable (status surfaced, which items failed)? [Measurability, Spec §FR-010 / SC-005]
- [x] CHK017 — Are success criteria stated for each operation independently (chmod / links / chown)? [Acceptance Criteria, Spec §SC-001..006]

## Edge Case & Scenario Coverage

- [x] CHK018 — Is dangling-symlink (non-existent target) behavior specified? [Edge Case, Spec §Edge Cases]
- [x] CHK019 — Is hard-link-across-filesystems / hard-link-to-directory failure specified? [Edge Case, Spec §Edge Cases]
- [x] CHK020 — Is duplicate/existing link-name behavior (refuse, no overwrite) specified? [Edge Case, Spec §US2 #3]
- [x] CHK021 — Is unprivileged chown (permission denied) behavior specified? [Exception Flow, Spec §US3 #2 / SC-005]
- [x] CHK022 — Is unknown user/group behavior specified? [Edge Case, Spec §US3 #3 / FR-009]
- [x] CHK023 — Is `..`-row exclusion from all operations stated? [Coverage, Spec §Edge Cases / FR-005]
- [x] CHK024 — Is cancel (Esc) leaving state unchanged specified for every dialog? [Coverage, Spec §FR-012]

## Scope & Boundary

- [x] CHK025 — Is recursion explicitly declared out of scope (and tracked as a deferral)? [Boundary, Spec §Clarifications / Edge Cases]
- [x] CHK026 — Is the feature scoped to the local backend (remote/archive out of scope)? [Boundary, Spec §Assumptions]
- [x] CHK027 — Is it stated that no persisted state is introduced? [Assumption, Spec §Assumptions]

## Notes

- All 27 items pass against the current spec — every checked requirement is documented and unambiguous.
- CHK006/CHK011 are clarified in research.md (owner not a visible column → verified by re-stat; symbolic applied per-file). These live in research rather than spec body, which is acceptable as design rationale.
- This is a requirements-quality gate, not an implementation test; implementation correctness is gated by the TDD tasks in tasks.md.

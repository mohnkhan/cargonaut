# Implementation-Readiness Checklist: Tasks/Jobs Panel Popup

**Purpose**: Validate that the requirements for the tasks/jobs panel are complete,
clear, consistent, and measurable before implementation — focused on pause/resume
semantics, per-row action safety, modal lifecycle, UX consistency, and the
TDD/docs gates. (These items test the *requirements*, not the code.)
**Created**: 2026-06-15
**Feature**: [spec.md](../spec.md)

## Pause / Resume Semantics

- [ ] CHK001 Is "pause" defined precisely enough that its observable effect is unambiguous (stops progress, preserves resumability) rather than implying task termination? [Clarity, Spec §FR-010, §Clarifications]
- [ ] CHK002 Is checkpoint preservation on pause stated as a requirement, not left as an implementation side effect? [Completeness, Spec §FR-016]
- [ ] CHK003 Is the maximum acceptable progress loss on resume quantified ("at most one checkpoint interval")? [Measurability, Spec §FR-011, §SC-004]
- [ ] CHK004 Do the requirements specify that a resumed transfer retains its identity (no duplicate row / stable position)? [Completeness, Data-model invariants]
- [ ] CHK005 Is the "re-arm" requirement explicit — that a resumed transfer can be paused or cancelled again? [Completeness, Spec §FR-011]
- [ ] CHK006 Are requirements defined for pausing a transfer *before its first checkpoint exists* (what resume does then)? [Edge Case, Research R-002]
- [ ] CHK007 Is a user-paused transfer required to be distinguishable from a user-cancelled one, with the distinction's source of truth named? [Consistency, Spec §FR-017, Data-model]

## Per-Row Action Eligibility & No-Op Safety

- [ ] CHK008 Is the eligibility of each action (cancel/pause/resume) per transfer state explicitly enumerated? [Completeness, Data-model eligibility table]
- [ ] CHK009 Is "safe no-op" defined as a measurable outcome (no state change, no crash) for ineligible actions? [Measurability, Spec §FR-012, §SC-006]
- [ ] CHK010 Are requirements consistent on who enforces eligibility (the action method no-ops; the widget always reports the action)? [Consistency, Contracts core-api/widget]
- [ ] CHK011 Is the cancel action required to affect only the selected transfer, with siblings explicitly unaffected? [Clarity, Spec §FR-009, §SC-002]
- [ ] CHK012 Is "resume only applies to paused jobs" stated as a requirement (not just an eligibility hint)? [Completeness, Spec §FR-012]

## Modal Lifecycle

- [ ] CHK013 Is the single-modal-at-a-time rule stated, including that re-invoking the action does not stack a second panel? [Completeness, Spec §FR-013]
- [ ] CHK014 Is input-capture while the panel is open specified so panel keys don't trigger other shortcuts? [Clarity, Spec §FR-005]
- [ ] CHK015 Is live refresh ("progress/state update without reopening") an explicit requirement with a defined refresh trigger? [Completeness, Spec §FR-008, Research R-004]
- [ ] CHK016 Is "close has no side effects" expressed measurably (both panes and all transfers unchanged on close)? [Measurability, Spec §FR-007, §SC-005]
- [ ] CHK017 Are selection-bounds requirements defined for when the underlying list changes while open (selection never out of range)? [Edge Case, Spec §FR-006]
- [ ] CHK018 Is the empty-state requirement explicit (panel opens and indicates "no transfers" rather than appearing broken)? [Coverage, Spec §FR-014]

## UX Consistency (Shared Widget / Keymap)

- [ ] CHK019 Is the requirement to reuse shared dialog widgets (not ad-hoc layout) stated and traceable to the constitution? [Consistency, Spec §FR-015, Constitution §III]
- [ ] CHK020 Are the in-panel keys enumerated unambiguously (move / cancel / pause / resume / close) and free of conflicts? [Clarity, Spec §FR-018]
- [ ] CHK021 Is the relationship between the two triggers (F12 and `:jobs`) defined — both resolving to one action — including the case where the command surface doesn't yet exist? [Ambiguity, Spec §FR-001, Research R-006]
- [ ] CHK022 Is row content (identity + state + progress) specified consistently across spec, data-model, and the widget contract? [Consistency, Spec §FR-003, Data-model JobRow]
- [ ] CHK023 Is the long-path truncation behavior for rows specified so layout cannot break? [Edge Case, Spec §Edge Cases]

## State & Status Modeling

- [ ] CHK024 Are the distinguishable display states enumerated and mapped to underlying transfer states without gaps? [Completeness, Spec §FR-004, Data-model JobStatus]
- [ ] CHK025 Is the derivation rule for "Paused" (marker overrides raw snapshot) stated once as the single source of truth, avoiding contradictory definitions? [Consistency, Data-model derivation rule]
- [ ] CHK026 Is list ordering specified (submit order, stable across refresh) so behavior is deterministic? [Clarity, Contracts core-api]

## Acceptance Criteria & Testability

- [ ] CHK027 Is the headline three-job scenario specified precisely enough to write one deterministic test (counts, which one paused, expected end states)? [Measurability, Spec §SC-003]
- [ ] CHK028 Does every functional requirement (FR-001…FR-018) have at least one measurable success criterion or acceptance scenario? [Coverage, Spec §SC-001…SC-007]
- [ ] CHK029 Is the means of keeping transfers in flight for the test (throttle) identified so the scenario is reproducible, not timing-dependent? [Measurability, Quickstart, Research]
- [ ] CHK030 Is an explicit assertion that *panes are unchanged on close* present in the acceptance criteria (not only implied)? [Gap, Spec §SC-005 — flagged in analyze C1]

## Process Gates (TDD / Docs)

- [ ] CHK031 Do the tasks encode the red→green ordering (failing test before implementation) per the constitution's NON-NEGOTIABLE TDD rule? [Traceability, tasks.md, Constitution §II]
- [ ] CHK032 Is each success criterion that needs a CI gate mapped to a specific test task? [Coverage, tasks.md ↔ Spec §SC-###]
- [ ] CHK033 Are the mandatory docs updates (README "At a Glance" + Feature History, Learnings ≥3 bullets) represented as tasks for the docs-gate? [Completeness, tasks.md T023/T024, CLAUDE.md]
- [ ] CHK034 Is the issue-closure / ROADMAP paper-trail accounted for (close #32; ROADMAP only if anything descoped)? [Traceability, tasks.md T025, CLAUDE.md Deferrals]

## Dependencies & Assumptions

- [ ] CHK035 Is the assumption that a canonical cancellation path + checkpointing engine already exist documented and validated against the codebase? [Assumption, Spec §Assumptions, Research R-001/R-002]
- [ ] CHK036 Is it documented that the panel introduces no parallel job bookkeeping (reads the existing registry only)? [Consistency, Spec §FR-002]
- [ ] CHK037 Are out-of-scope boundaries (history persistence, multi-select, queue reordering, remote backends) explicit so they aren't silently expected? [Coverage, Spec §Out of Scope]

## Notes

- Check items off as the requirements are confirmed: `[x]`.
- CHK030 corresponds to analyze finding C1 (the one MEDIUM): fold a pane-unchanged
  assertion into the close test during implementation.
- This checklist validates requirement quality; functional verification lives in
  the test tasks (T003–T020) and quickstart scenarios.

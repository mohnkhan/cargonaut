# Cross-Artifact Analysis: Feature 037

**Date**: 2026-06-15 | **Scope**: read-only consistency check across spec.md, plan.md,
research.md, data-model.md, contracts/resume-seam.md, tasks.md. (Tasks lightly amended once,
T007, to close the one coverage gap found — noted below.)

## Requirement → Task coverage

| FR | Requirement (short) | Task(s) | Test gate |
|----|---------------------|---------|-----------|
| FR-001 | Scan both launch dirs on startup | T009, T011 | T007 |
| FR-002 | Show resume prompt listing offers | T011 | T007 + T018 (e2e) |
| FR-003 | No prompt / no delay when none found | T009, T011, T022 | T007 (C2) |
| FR-004 | Three actions (resume/start over/skip) | T012, T017 (widget exists) | T018 + unit |
| FR-005 | Resume continues from offset | T010 | T008, T018 (SC-002) |
| FR-006 | Verify + remove sidecar on completion | T010 (engine guarantee) | T018 |
| FR-007 | Start over discards + fresh copy | T015 | T013 |
| FR-008 | Skip leaves sidecar | T016 | T014 |
| FR-009 | Fail safe on mismatch | T010 | T008 (C8) |
| FR-010 | Malformed sidecar doesn't block launch | T009 (delegates to scan_resumable) | **T007 (added case)** |
| FR-011 | E2E SIGKILL→resume→verify test | T018 | T018 |
| FR-012 | Gated + CI-enabled | T020 | T019, CI |
| FR-013 | Reuse shared dialog + keymap | T011, T012, T017 | §III review (T021) |

**Result**: all 13 FRs mapped to at least one implementation task and one test. ✅

## Success Criteria → gate

| SC | Gate task | Notes |
|----|-----------|-------|
| SC-001 byte-identical resume | T018 (sha256 assert) | ✅ |
| SC-002 ≤ one checkpoint interval re-copied | T018 (resumed-bytes assert) | ✅ the central deliverable |
| SC-003 deterministic + CI | T019 (stability), T020 (CI) | ✅ throttle (T004) underpins determinism |
| SC-004 no startup regression | T022 | ✅ spot-check; scan = one list/pane dir |
| SC-005 fail safe, no corruption | T008, T010 | ✅ |

**Result**: every SC has a CI/automated gate (Constitution §IV). ✅

## User-story → task mapping

- US1 (P1): T007–T012 — self-contained MVP (resume works). ✅
- US2 (P2): T013–T017 — start over + skip; extends US1's UI dispatch arm (acceptable shared
  file; T017 explicitly extends T012). ✅
- US3 (P3): T018–T020 — the SC-002 gate; correctly depends on US1 existing. ✅

## Consistency findings

1. **Gap (resolved)**: FR-010 (malformed sidecar) originally had no dedicated core test —
   relied solely on `scan_resumable`'s upstream behavior. Amended T007 to add a malformed-
   sidecar case at the launch-scan level. ✅
2. **Known unknown (tracked)**: the copy-confirm key for the PTY sequence (T018) is determined
   empirically from `ConfirmDialog::handle_key` during implementation — flagged in T018 and
   tasks Notes, not a blocker. ✅
3. **Constitution §II (TDD)**: every implementation task has a preceding `(red)` test task;
   red-before-green commit ordering is specified in tasks "Dependencies". ✅
4. **Constitution §III**: no new keymap.toml bindings (launch-time modal reuses the existing
   widget's built-in `r`/`s`/`c` handling) — consistent with spec FR-013 and plan. ✅
5. **Constitution §V**: T002 checks tmpfs; test temp files use `tempfile` (TMPDIR). The PTY
   test could write a 128 MiB temp file — within tmpfs budget; CI exempt. ✅
6. **Terminology**: `ResumeOfferView` (core projection) vs `ResumableSummary` (UI widget) vs
   `ResumableTransfer` (engine) used consistently across data-model, contracts, and tasks. ✅
7. **Clarify alignment**: scan scope (both dirs), test sizing (modest+throttle), CI strategy
   (flag on existing step) — all three clarifications are reflected in plan R-004/R-006/R-008
   and tasks T009/T018/T020. ✅
8. **Scope discipline**: tasks introduce no work beyond spec FRs; deferred items from issue
   #29's broader area (e.g. #30 PTY nav test) are referenced only as a future beneficiary in
   T025, not implemented here. ✅

## Verdict

Artifacts are mutually consistent and complete. One coverage gap (FR-010) found and closed.
**Ready for `/speckit-implement`.**

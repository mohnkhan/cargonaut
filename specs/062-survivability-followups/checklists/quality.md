# Checklist: Survivability-Follow-ups Requirements Quality

**Created**: 2026-06-21 · **Feature**: [spec.md](../spec.md)

## Recovery correctness
- [ ] CHK001 Is the input-recovery boundary specified to mirror the render path (catch → status → bounded escalation), not invent new behavior? [Consistency, Spec §FR-001/FR-002]
- [ ] CHK002 Is "no crash file for a recovered input fault" stated and consistent with the fatal-only file rule (Feature 061 FR-002)? [Consistency, Spec §FR-001]
- [ ] CHK003 Is the input-recovery bound quantified (escalate after N) rather than vague? [Clarity, Spec §FR-002]
- [ ] CHK004 Is the "already terminal" guard for transfer-Failed specified so a real Completed/Cancelled is never overwritten? [Edge Case, Spec §Edge Cases]
- [ ] CHK005 Is task isolation's scope clear (only the faulting job becomes Failed; others unaffected)? [Coverage, Spec §FR-003/SC-002]

## About view
- [ ] CHK006 Are the About fields enumerated and required to come from the single `diag` source (no drift vs F1/CLI)? [Consistency, Spec §FR-004]
- [ ] CHK007 Is dismissal behavior (Esc/Enter → normal mode) specified? [Completeness, Spec §FR-005]
- [ ] CHK008 Is the no-new-keybinding decision (menu-only) explicit so keymap/help-coverage are untouched? [Clarity, Spec §Assumptions]

## Audit + regression
- [ ] CHK009 Is the unwrap audit bounded (reviewed shortlist, behavior-preserving) rather than "remove all unwraps"? [Clarity, Spec §FR-006]
- [ ] CHK010 Is non-regression of Feature 061 crash-safety stated as a requirement with a gate? [Coverage, Spec §FR-007/SC-004]

## Analyze result
- Cross-artifact: every FR maps to ≥1 task; every SC has a gate; 0 critical, 0
  duplication. Terminology consistent with Feature 061 ("fault"/"panic", `diag`).

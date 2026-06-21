# Checklist: Release/Versioning Requirements Quality

**Created**: 2026-06-21 · **Feature**: [spec.md](../spec.md)

- [ ] CHK001 Is the version policy explicit about pre-1.0 (0.y.z) breaking-change rules, not just "SemVer"? [Clarity, Spec §FR-001]
- [ ] CHK002 Is the single version source named (`[workspace.package] version`)? [Completeness, Spec §FR-001]
- [ ] CHK003 Is the release checklist ordered and unambiguous (bump→changelog→verify→tag→push)? [Clarity, Spec §FR-002]
- [ ] CHK004 Does the CHANGELOG restructure preserve all existing history? [Completeness, Spec §FR-003/R3]
- [ ] CHK005 Is the release trigger specified (push `v*` tag) and idempotent re-run behavior considered? [Edge Case, Spec §Edge Cases]
- [ ] CHK006 Is "fail loudly if no CHANGELOG section for the tag" a stated requirement? [Coverage, Spec §FR-006]
- [ ] CHK007 Is tag↔version consistency a checked precondition? [Measurability, Spec §FR-005/SC-002]
- [ ] CHK008 Is the artifact contents enumerated (tarball + checksum + notes)? [Completeness, Spec §FR-004]
- [ ] CHK009 Is actually-cutting-the-release scoped out pending explicit go-ahead? [Scope, Spec §Out of Scope]

## Analyze
Every FR → a task (FR-001→T001, 002→T006, 003→T002, 004→T005, 005→T003/T004,
006→T005, 007→T004). SCs gated by `make release-check` + the workflow. 0 critical.

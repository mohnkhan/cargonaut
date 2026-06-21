# Checklist: Fuzzing Requirements Quality

**Created**: 2026-06-21 · **Feature**: [spec.md](../spec.md)

- [ ] CHK001 Is the no-panic invariant stated for each named parser (not "the parsers")? [Clarity, Spec §FR-001]
- [ ] CHK002 Is the always-on gate distinguished from deep fuzzing (runs in stable CI vs nightly)? [Consistency, Spec §FR-002/FR-006]
- [ ] CHK003 Is the SSD-safety requirement concrete (tmpfs `CARGO_TARGET_DIR` via a make target), not aspirational? [Measurability, Spec §FR-005/SC-003]
- [ ] CHK004 Is the roundtrip requirement bounded to inputs that actually parse + have a rendering? [Clarity, Spec §FR-003]
- [ ] CHK005 Is reproducibility of a discovered crash required (corpus/artifact)? [Coverage, Spec §FR-007]
- [ ] CHK006 Is the `fuzz/` crate's exclusion from the default build stated so the binary/`cargo test` are unaffected? [Completeness, Spec §SC-004/Assumptions]
- [ ] CHK007 Are the fuzzed input shapes enumerated (empty, long, invalid UTF-8, NUL, control, huge octal)? [Edge Case, Spec §Edge Cases]
- [ ] CHK008 Is the minimum case count quantified for the gate (≥1000/parser)? [Measurability, Spec §SC-001]

## Analyze result
Every FR maps to a task (FR-001/002→T001, FR-003→T002, FR-004→T003/T004,
FR-005→T005, FR-006→T006, FR-007→T004/T006 corpora). Every SC has a gate.
0 critical. Terminology consistent ("parser", "invariant", "target").

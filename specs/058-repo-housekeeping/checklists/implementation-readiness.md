# Implementation Readiness Checklist: Repository Housekeeping

**Purpose**: Requirements-quality gate (PR reviewer) before `/speckit-implement`. Tests
whether the spec/plan/tasks are complete, clear, and consistent — not whether the
implementation works.
**Created**: 2026-06-21
**Focus**: surgical archiving · no-production-code boundary · honest docs-gate · deferral paper-trail

## Surgical Archiving Scope

- [ ] CHK001 — Is the exact set of files to be archived vs preserved in `design/contracts/` explicitly enumerated? [Completeness, Spec §FR-001, §FR-003]
- [ ] CHK002 — Is "live contract" defined with concrete criteria distinguishing it from "historical", rather than left to judgment? [Clarity, data-model.md §State definitions]
- [ ] CHK003 — Is the requirement that live files remain "byte-for-byte" unchanged stated measurably (e.g., a diff-scoped success criterion)? [Measurability, Spec §SC-002]
- [ ] CHK004 — Is the constitutional authority of `keymap.toml` (§III) explicitly called out as a preservation constraint, not just implied? [Consistency, plan.md §Constitution Check]
- [ ] CHK005 — Are the archive-vs-delete decision and its rationale recorded so a reviewer can judge it? [Traceability, research.md §R-001]

## No-Production-Code Boundary

- [ ] CHK006 — Is "no production code change" expressed as a checkable predicate (a path glob over `crates/*/src/`) rather than a vague promise? [Measurability, Spec §SC-006]
- [ ] CHK007 — Are the out-of-bounds areas (all `crates/*/src/`, all live `design/contracts/` files) explicitly listed in the plan? [Completeness, plan.md §Structure Decision]
- [ ] CHK008 — Is the `cargonaut-core` god-file split unambiguously declared out of scope for this feature? [Clarity, Spec §Assumptions, research.md §R-006]

## Docs-Gate & CI Honesty

- [ ] CHK009 — Does the spec state how the docs-gate is satisfied (update README + Learnings) vs bypassed (`[no-docs]`), and justify the choice? [Completeness, Spec §Edge Cases, research.md §R-005]
- [ ] CHK010 — Are the required doc updates (README feature history; Learnings ≥3 bullets) specified with enough precision to verify? [Clarity, Spec §FR-011]
- [ ] CHK011 — Is the regression gate (which CI steps must pass) named explicitly rather than "CI passes"? [Measurability, Spec §SC-005, plan.md §Testing]

## Deferral Paper-Trail

- [ ] CHK012 — Are all six CLAUDE.md-mandated issue fields (problem, why-deferred, suggested approach, decision pointer, effort, tier+`follow-up` label) enumerated as requirements? [Completeness, Spec §FR-009]
- [ ] CHK013 — Is the one-to-one linkage (exactly one ROADMAP row ↔ one issue) stated as a verifiable invariant? [Consistency, Spec §SC-007, data-model.md §INV-4]
- [ ] CHK014 — Is the location where the deferral was decided cited so the issue can point back to it? [Traceability, research.md §R-006]

## Edge Cases & Assumptions

- [ ] CHK015 — Is the git semantics of removing an untracked empty dir (`tests/integration/` yields no committed diff) called out so reviewers don't expect a diff? [Edge Case, Spec §Edge Cases]
- [ ] CHK016 — Is the claim "all real benches live per-crate" backed by evidence rather than asserted? [Assumption, research.md §R-004]
- [ ] CHK017 — Is reversibility/rollback for each change documented? [Coverage, quickstart.md §Rollback]
- [ ] CHK018 — Are the empirical claims (57/59 dead paths; zero CI references) recorded with how they were measured, so they can be re-checked? [Traceability, research.md §R-001]

## Notes

- All items interrogate the *written requirements*, not runtime behavior — appropriate
  for a docs/infra feature whose "behavior" is repository state.
- Pre-assessment: CHK001–CHK018 are all satisfiable from the current spec/plan/research
  (the artifacts were authored from measured evidence). This checklist is the reviewer's
  confirmation surface, not a list of known gaps.

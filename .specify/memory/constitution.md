<!--
SYNC IMPACT REPORT
==================
Version change:  0.0.0 (uninitialized placeholder) → 1.0.0
Bump type:       MAJOR — first substantive ratification; all placeholder tokens replaced.

Principles added:
  - I.  Code Quality              (new)
  - II. Test-First Discipline     (new)
  - III. User Experience Consistency (new)
  - IV. Performance Requirements  (new)

Sections added:
  - Quality Gates       (new)
  - Development Workflow (new)
  - Governance          (populated from template stub)

Templates requiring updates:
  ✅ .specify/templates/plan-template.md
       Constitution Check gates are now derivable from the four Core Principles and
       the Quality Gates section. No structural edits needed; agents fill the gate
       list at plan-generation time.
  ✅ .specify/templates/spec-template.md
       Success Criteria section already requires measurable metrics — aligns with
       Principle IV. No structural changes needed.
  ✅ .specify/templates/tasks-template.md
       TDD task ordering (tests before implementation) already reflected in template
       phase structure. No changes needed.

Deferred TODOs:
  None — all placeholders resolved.
-->

# Cargonaut Constitution

## Core Principles

### I. Code Quality

Every line of code MUST be readable, maintainable, and purposeful. Complexity MUST be
justified; if a simpler path exists, take it. Code MUST pass static analysis and linting
gates with zero warnings before peer review. Reviewers MUST reject code that introduces
unnecessary abstraction, duplicate logic, or opaque naming. Dead code MUST be deleted,
not commented out.

**Non-negotiables**:
- Lint and static analysis MUST pass with zero warnings before PR submission.
- Functions and modules MUST have a single, clear responsibility.
- No code is merged without at least one peer-review approval from a domain owner.
- All public APIs MUST carry type annotations or equivalent contract definitions.

### II. Test-First Discipline (NON-NEGOTIABLE)

Tests MUST be written before implementation. No implementation code is accepted without
a failing test that first justifies it. The Red-Green-Refactor cycle is mandatory on
every feature. Test coverage for critical paths MUST reach 80% or above; overall
coverage below 60% is a hard merge blocker.

**Non-negotiables**:
- Unit tests MUST be written and confirmed failing before implementation begins.
- Integration tests MUST cover all inter-component contracts.
- Contract tests MUST exist for every public API boundary.
- All tests MUST pass in CI before merge; flaky tests MUST be fixed, not skipped.
- Performance regression tests MUST be included when a feature touches a hot path.

### III. User Experience Consistency

All user-facing surfaces MUST adhere to the Cargonaut design system. Interaction
patterns, terminology, error messages, and visual hierarchy MUST remain uniform across
the OS. No new UX pattern is introduced without cross-team design review. Accessibility
to WCAG 2.1 AA standard MUST be met for every shipped UI component.

**Non-negotiables**:
- New UI components MUST reuse existing design-system primitives; new primitives require
  explicit design review sign-off before implementation begins.
- Error messages MUST be actionable: state what failed and what the user can do next.
- Keyboard navigation and screen-reader support MUST be verified before merge for all
  interactive components.
- UX copy (labels, tooltips, confirmations) MUST be reviewed for tone and consistency
  against the Cargonaut writing guide before merge.

### IV. Performance Requirements

Every feature MUST define measurable performance targets in its specification before
implementation begins. Features that degrade p95 response time, resident memory
footprint, or frame rate below defined thresholds MUST NOT be merged. Performance
budgets are set per-component in the feature spec and are enforced by automated
benchmarks in CI.

**Non-negotiables**:
- Features MUST declare performance goals in `spec.md` Success Criteria before planning
  begins (e.g., `<200ms p95 latency`, `<50MB resident memory`, `60 fps steady-state`).
- CI MUST run benchmark suites; a regression of >10% on any tracked metric is a merge
  blocker.
- Hot paths MUST be profiled before and after implementation; results MUST be attached
  to the PR.
- No polling loops, unbounded allocations, or synchronous I/O on the main/UI thread
  without explicit written justification reviewed by a domain owner.

## Quality Gates

Every feature branch MUST clear all gates below before merging to main:

1. **Lint & Static Analysis** — zero warnings, zero type errors.
2. **Test Suite** — all unit, integration, and contract tests pass in CI.
3. **Coverage** — critical-path coverage ≥ 80%; overall coverage ≥ 60%.
4. **Performance Benchmarks** — no regression >10% on any tracked metric.
5. **UX Review** — design-system compliance confirmed; accessibility verified.
6. **Peer Review** — at least one approval from a domain owner; no unresolved comments.
7. **Constitution Check** — plan.md and spec.md reviewed against all four Core Principles
   before Phase 0 research proceeds and again after Phase 1 design.

## Development Workflow

1. Write or update `spec.md` — define user stories, requirements, and performance
   targets (Principle IV requires targets before planning).
2. Run `/speckit-clarify` to resolve ambiguities before planning begins.
3. Run `/speckit-plan` — Constitution Check (Quality Gate 7) MUST pass before
   Phase 0 research proceeds.
4. Run `/speckit-tasks` — task ordering MUST reflect TDD discipline: tests written
   and confirmed failing before any implementation task for that story begins.
5. Implement via `/speckit-implement`, enforcing Red-Green-Refactor on every cycle.
6. Open PR; all seven Quality Gates MUST pass before merge.
7. Post-merge: run performance benchmark suite on main; alert on-call if any metric
   degrades from the pre-merge baseline.

## Governance

This constitution supersedes all other practices, style guides, and ad-hoc conventions.
Amendments require:

1. A written rationale identifying which principle(s) are affected and why the change
   is necessary.
2. A semantic version bump (see policy below) applied before the amendment is merged.
3. A migration plan for any in-flight features affected by the change.
4. Approval from at least two domain owners before the amended constitution is merged.

**Versioning policy**:
- **MAJOR**: A principle is removed, redefined, or made incompatible with prior guidance.
- **MINOR**: A new principle or section is added; existing guidance is materially expanded.
- **PATCH**: Clarifications, wording improvements, or typo fixes with no semantic change.

All PRs and reviews MUST verify compliance with the current version of this constitution.
Complexity violations MUST be documented in the plan's Complexity Tracking table with a
clear justification and an explanation of why a simpler alternative was rejected.

**Version**: 1.0.0 | **Ratified**: 2026-04-26 | **Last Amended**: 2026-04-26

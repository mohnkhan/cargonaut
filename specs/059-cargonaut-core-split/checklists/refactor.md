# Refactor-Quality Checklist: cargonaut-core God-File Split

**Purpose**: Validate that the *requirements* for this move-only refactor are complete, clear, consistent, and measurable — before implementation. This is a "unit test for the spec/plan", not a test of the code.
**Created**: 2026-06-21
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)
**Audience/Timing**: PR reviewer, release gate.

## API-Surface Stability

- [ ] CHK001 Is "public API unchanged" defined with an objective, reproducible measure rather than reviewer judgment? [Measurability, Spec §FR-003, Contracts]
- [ ] CHK002 Is the exact pre-refactor public surface captured as a versioned artifact the diff can run against? [Completeness, Contracts §public-api-baseline.txt]
- [ ] CHK003 Do the requirements specify that re-exports must preserve both the *name* and the *path* (`cargonaut_core::X`), not just the name's existence? [Clarity, Spec §FR-003/§US2.2]
- [ ] CHK004 Are cross-crate re-exports (`TransferId`, `TransferMode`, `Bookmark`, `Hotlist`) explicitly in scope of the stability requirement? [Coverage, Spec §FR-003]
- [ ] CHK005 Is the limitation of the name-level surface diff (cannot catch same-name signature drift) acknowledged, with a complementary consumer-compile proof required? [Consistency, Research §R-005]

## Behavior Preservation (move-only)

- [ ] CHK006 Is "move-only" defined by enumerating what may change (use paths, module plumbing, visibility-to-compile) vs. what may not (logic, signatures, control flow, messages)? [Clarity, Spec §FR-005]
- [ ] CHK007 Are user-facing status strings and error messages explicitly named as invariants that must not change? [Completeness, Spec §FR-005, Out of Scope]
- [ ] CHK008 Is "no behavior change" backed by a measurable acceptance criterion (existing suite green + identical executed-test count)? [Measurability, Spec §FR-006/§SC-004]
- [ ] CHK009 Are opportunistic cleanups explicitly excluded so the move stays auditable? [Boundary, Spec §Out of Scope, Research §R-008]

## Test Integrity

- [ ] CHK010 Do requirements state that no test may be removed, disabled, or silently skipped — and how "same count" is verified? [Completeness, Spec §FR-006, Tasks §T002/§T023]
- [ ] CHK011 Is the constraint that relocated unit tests must retain access to private items (methods/fields) captured as a requirement, not just an implementation note? [Coverage, Spec §Edge Cases, Research §R-004]
- [ ] CHK012 Is the placement of shared test fixtures (reachable by every dependent test) specified? [Completeness, Spec §Edge Cases, Tasks §T004]
- [ ] CHK013 Is the ordering dependency "shared fixtures extracted before per-module test relocation" documented as a prerequisite? [Consistency, Tasks §Phase 2 / §Dependencies]

## Visibility Hygiene

- [ ] CHK014 Is "no widening beyond what is strictly required to compile" stated with a measurable bound (no new `pub`; at most `pub(crate)`)? [Clarity, Spec §FR-007]
- [ ] CHK015 Is the rationale that lets the split avoid widening (descendant-module access to root-defined private fields) recorded, and the structs it forces to stay at root (`App`, `SideState`) identified? [Completeness, Research §R-002, Data-model]
- [ ] CHK016 Is there a check that any introduced `pub(crate)` does not leak to the public surface? [Measurability, Spec §FR-007, Analyze §C3]

## Thin lib.rs / No Residual God-File

- [ ] CHK017 Is "thin module root" quantified (a line-count target and an enumeration of what may remain in `lib.rs`)? [Measurability, Spec §SC-001, Plan §Structure]
- [ ] CHK018 Is the floor on submodule count (≥4) stated, and is the actual target count internally consistent across spec/plan/research/tasks? [Consistency, Spec §FR-002, Analyze §F1]
- [ ] CHK019 Does the requirement explicitly forbid leaving the large test block whole in `lib.rs` (no concern simply re-monolithized)? [Completeness, Spec §FR-010]
- [ ] CHK020 Is there a measurable ceiling so a submodule cannot become a *new* god-file? [Measurability, Spec §SC-006, Tasks §T019]
- [ ] CHK021 Is each module's responsibility stated tightly enough to be expressible in one sentence (cohesion criterion)? [Clarity, Spec §SC-002/§SC-007, Data-model]

## Downstream & Bench Compile-Clean

- [ ] CHK022 Are all downstream consumers enumerated (`cargonaut-ui-tui`, `cargonaut-transfer`, `cargonaut-bin`) with a zero-source-edit requirement? [Completeness, Spec §FR-004]
- [ ] CHK023 Are the `cargonaut-core` benches recognized as API consumers that must also compile unchanged? [Coverage, Research §R-005, Tasks §T022]
- [ ] CHK024 Is "zero downstream edits" verifiable objectively (e.g., empty diff vs. `origin/main`) rather than by assertion? [Measurability, Tasks §T021]

## Constitution Gates

- [ ] CHK025 Are all mandated gates named with their exact invocation (clippy `-D warnings`, `missing_docs`, `-D broken-intra-doc-links`, `fmt --check`)? [Completeness, Constitution §I, Spec §FR-008/§FR-009]
- [ ] CHK026 Is the pre-existing, unrelated rustdoc warning distinguished from the gated lint so "no *new* warnings" is the actual bar? [Clarity, Research §R-005, Quickstart §4]
- [ ] CHK027 Is the SSD/tmpfs constraint stated as a precondition with an audit step and the forbidden commands called out? [Completeness, Constitution §V, Tasks §T001]
- [ ] CHK028 Is the §II Test-First deviation explicitly justified against the constitution's own no-behavior clause (not silently ignored)? [Consistency, Plan §Constitution Check/§Complexity Tracking]

## Coverage, Traceability & Closeout

- [ ] CHK029 Does every FR and buildable SC map to at least one task? [Coverage, Tasks §Coverage, Analyze §Metrics]
- [ ] CHK030 Is the deferral paper-trail closeout specified (issue #86 closed, ROADMAP row reconciled) per the project rule? [Completeness, Spec §FR-011, Tasks §T029/§T032]
- [ ] CHK031 Are the mandatory docs updates (`README.md` metrics/history, `Learnings.md` ≥3 bullets) required on the branch before merge? [Completeness, Spec §FR-011, Tasks §T027/§T028]
- [ ] CHK032 Is the final merge gate identified as the same pipeline CI runs (`make ci-local`), so local green predicts remote green? [Consistency, Tasks §T030, Quickstart §6]

## Notes

- All items test whether the *requirements/plan* are well-written for a behavior-preserving refactor — not whether the code works (that is the suite + surface diff at implementation time).
- Traceability: every item carries a `[Spec §…]` / `[Tasks §…]` / `[Research §…]` / `[Constitution §…]` reference or a quality marker (≥80% target met).

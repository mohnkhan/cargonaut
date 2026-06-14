# Specification Quality Checklist: Cargonaut

**Purpose**: Validate specification completeness before Phase 1 begins.
**Created**: 2026-05-17

## Content quality

- [X] Vision statement: one paragraph, clear
- [X] User stories priority-ordered (P1 MVP → P2 → P3)
- [X] Each US has acceptance scenarios in Given/When/Then form
- [X] Edge cases enumerated (5+)
- [X] Out-of-scope items explicit (Section 15)

## Requirement completeness

- [X] Functional requirements use MUST/SHOULD/MAY consistently
- [X] Each FR has a measurable acceptance predicate
- [X] Non-functional requirements quantified (memory ceiling, startup, throughput)
- [X] Machine-readable mirror exists (`contracts/requirements.toml`)
- [X] Every FR has a `verification` field pointing at a test or check

## Success criteria

- [X] Each SC is measurable (numeric or pass/fail)
- [X] Each SC has a CI gate or benchmark file
- [X] Phased acceptance map (which SCs gate which phase)

## Architecture + design

- [X] High-level architecture diagram (ASCII OK)
- [X] Component diagrams (in `architecture/` — sequence-copy-resume.txt for now; more in Phase 2+)
- [X] Cargo workspace layout
- [X] Public API surfaces (in plan.md + scaffold/ source)

## Configuration + CLI

- [X] Config schema (JSON Schema in `contracts/config.schema.json`)
- [X] CLI surface (spec.md Section 8)
- [X] Keymap (`contracts/keymap.toml`)

## Plugin + security

- [X] Plugin interface (`contracts/plugin-api.md`)
- [X] Threat model (excerpt in plugin-api.md; full in `security/threat-model.md` Phase 3 deliverable)

## UX

- [X] Wireframes (ASCII OK): main-view, copy-dialog
- [X] Keyboard shortcut map (`contracts/keymap.toml`)

## Testing + CI

- [X] Test plan (`tests-plan.md`): unit, integration, fuzz, property
- [X] CI pipeline outline (YAML in `tests-plan.md`)

## Releases + migration

- [X] Phased release milestones (`milestones.md`)
- [X] orthodox-FM migration table (`milestones.md`)

## Implementation scaffold

- [X] Cargo workspace skeleton (`scaffold/Cargo.toml`)
- [X] Per-crate Cargo.toml files
- [X] Public type/trait sketches (`scaffold/crates/*/src/*.rs`)
- [X] Runnable prototype (cargonaut-bin/src/main.rs prints config + pane paths)

## Naming

- [X] 5 candidate names proposed (spec.md Section 16)
- [X] Top choice recommended (Cargonaut)
- [X] Rationale per name

## Backlog

- [X] Phase 1-3 task backlog with owner-week estimates (`tasks.md`)
- [X] Engineer-parallelism notes per phase
- [X] Dependency graph documented

## Notes

- Phase 1 SCOPE is locked. Phases 2-6 are sketched (FRs + milestones + high-level tasks) but each will run its own clarify+plan+tasks pass when picked up.
- This bundle is the "design tunnel" — implementation lives in a separate Cargonaut repo bootstrapped from `scaffold/`.

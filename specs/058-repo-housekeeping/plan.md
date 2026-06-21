# Implementation Plan: Repository Housekeeping — Stale Artifact Reconciliation

**Branch**: `058-repo-housekeeping` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/058-repo-housekeeping/spec.md`

## Summary

Reconcile three decayed metadata artifacts so the repository's self-description matches
its real structure, with **no production code changes**:

1. Archive `design/contracts/requirements.toml` (97% dead `verification=` links; falsely
   claims CI enforcement) with a historical banner + corrected header, and add a
   `design/`-level archive marker. Live contracts in the same dir (notably the
   constitutionally-authoritative `keymap.toml`, plus the JSON schemas, `commands.toml`,
   `plugin-api.md`) are left untouched.
2. Correct the stale `Cargo.toml` header comment ("Phase 1 in progress" / `design/plan.md`)
   to point at the current spec-kit workflow.
3. Delete orphaned root scaffolding: `tests/integration/` and `benches/`.
4. Record findings + the deferred `cargonaut-core` god-file split as a GitHub issue +
   `ROADMAP.md` row, and update `README.md`/`Learnings.md` per the docs rule.

The technical approach is a sequence of edits/deletions verified by re-running the
existing CI pipeline; correctness is observable repo state, not new runtime behavior.

## Technical Context

**Language/Version**: Rust 1.76 (workspace) — but this feature changes **no `.rs` source**.

**Primary Dependencies**: None added. Touches TOML/Markdown metadata + directory layout only.

**Storage**: N/A (filesystem metadata only).

**Testing**: Existing CI pipeline as the regression gate — `cargo clippy --workspace
--all-targets -D warnings`, `cargo test --workspace`, `cargo build --release`,
`check-pr-body`, `docs-gate`. No new tests (no new behavior to test); the "tests" for
this feature are the verifiable observations in spec §Success Criteria.

**Target Platform**: Repo/dev tooling (platform-agnostic).

**Project Type**: Rust workspace (6 crates) — housekeeping/infra change.

**Performance Goals**: N/A (no runtime code path touched).

**Constraints**: Must not modify `crates/*/src/`; must keep the workspace green; must
preserve live contract files byte-for-byte; archiving preferred over deletion for the
requirements manifest (preserve record).

**Scale/Scope**: 1 archived file + 1 archive marker + 1 Cargo.toml comment + 2 dir
deletions + 2 mandated doc updates + 1 issue + 1 ROADMAP row. ~6 files changed.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Applies? | Status / Justification |
|-----------|----------|------------------------|
| **I. Code Quality** (clippy -D warnings, fmt, missing_docs, no-unsafe) | Partial | No code changes, so no new clippy/fmt/doc surface. Pipeline still run to prove zero regression. PASS. |
| **II. Test-First** (TDD per FR; SC→CI gate) | N/A | FRs here are file-state/doc changes, not runtime behavior — there is no red/green code to author. The constitution's TDD clause is scoped to behavioral FR-###. This is the docs/infra class the repo explicitly supports (docs-gate + `[no-docs]` bypass exist for it). No new SC needs a bench. PASS (justified N/A). |
| **III. UX Consistency** (keymap single source = `design/contracts/keymap.toml`) | Guardrail | Directly relevant: `keymap.toml` is authoritative and lives in the same dir being touched. Plan explicitly preserves it (FR-003). PASS. |
| **IV. Performance** (Phase-1 benches, binary size) | N/A | No runtime path touched; no bench affected. The root `benches/` being deleted holds only `.gitkeep` — all enforced benches are per-crate and untouched. PASS. |
| **V. SSD Preservation** (target/ in tmpfs) | Applies to verification | Verification builds MUST run with `target/` symlinked to tmpfs. Plan runs `make tmpfs-status` before any build and uses `make`-wrapped targets (which run `check-tmpfs`). No waiver needed. PASS. |

**Gate result**: PASS. No violations → Complexity Tracking left empty.

## Project Structure

### Documentation (this feature)

```text
specs/058-repo-housekeeping/
├── plan.md              # This file
├── research.md          # Phase 0 — reconcile-vs-archive analysis, evidence
├── data-model.md        # Phase 1 — artifact classification (live vs historical)
├── quickstart.md        # Phase 1 — how to verify the housekeeping
├── checklists/
│   └── requirements.md  # Spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 — /speckit-tasks output
```

No `contracts/` directory: this feature exposes **no external interface** (no public
API, CLI surface, or schema). Per the plan workflow, contracts are skipped for purely
internal changes.

### Source Code (repository root) — files touched

```text
Cargo.toml                              # FR-005: correct stale header comment
design/contracts/requirements.toml      # FR-001,002: archive banner + remove false CI claim
design/README.md                         # FR-004: NEW — design/ archive marker
tests/integration/                       # FR-006: DELETE (untracked empty dir)
benches/.gitkeep + benches/              # FR-007: DELETE (root placeholder)
ROADMAP.md                               # FR-010: row for cargonaut-core split follow-up
README.md                                # FR-011: feature history + at-a-glance
Learnings.md                             # FR-011: feature 058 learnings entry
# GitHub issue (FR-009) created via gh — not a file in the tree
```

**Structure Decision**: Edit-in-place + targeted deletions on the existing tree. No new
crate, module, or directory beyond `design/README.md` and this spec dir. Explicitly
**out of bounds**: everything under `crates/*/src/` and every live file in
`design/contracts/` except `requirements.toml`.

## Complexity Tracking

> No constitution violations. Section intentionally empty.

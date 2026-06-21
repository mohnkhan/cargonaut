# Tasks: Repository Housekeeping — Stale Artifact Reconciliation

**Feature**: 058-repo-housekeeping | **Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)

**Note**: No automated test tasks — this feature changes no runtime behavior. Verification
is by the observable repo-state checks in [quickstart.md](./quickstart.md), each mapped to
a Success Criterion. The regression gate is the existing CI pipeline.

## Phase 1: Setup

- [X] T001 Confirm working state: on branch `058-repo-housekeeping`, `git status` clean except feature spec docs, and `make tmpfs-status` shows `target/` symlinked to tmpfs (Constitution §V)

## Phase 2: Foundational

_No blocking prerequisites — each user story below is independent and may proceed in any order. Setup (T001) is the only gate._

## Phase 3: User Story 1 — Trustworthy contracts manifest (P1)

**Goal**: `design/contracts/requirements.toml` self-identifies as historical and drops the false CI-enforcement claim, without touching co-located live contracts.

**Independent Test**: `head -15 design/contracts/requirements.toml` shows the banner + pointer to `specs/NNN/`; `grep "CI greps this file"` finds no live assertion; `git diff --name-only main...HEAD -- design/contracts/` lists only `requirements.toml`.

- [X] T002 [US1] Add a prominent HISTORICAL banner to the top of `design/contracts/requirements.toml` stating it is the original 6-phase master manifest, superseded by the per-feature `specs/NNN/` workflow (adopted at feature 031), and naming the current source of truth for requirement→test traceability
- [X] T003 [US1] In the same `design/contracts/requirements.toml` header, remove/correct the false claim that "CI greps this file to verify every requirement has a linked test" (no CI/script references it — research R-001)
- [X] T004 [P] [US1] Create `design/README.md` archive marker identifying the original master-plan docs (`plan.md`, `spec.md`, `tasks.md`, `research.md`, `requirements.toml`) as historical/superseded by `specs/NNN/`, while noting `design/contracts/` still holds LIVE authoritative files (`keymap.toml` per Constitution §III, the JSON schemas, `commands.toml`, `plugin-api.md`)
- [X] T005 [US1] Verify no collateral edits: `git diff --name-only main...HEAD -- design/contracts/` returns exactly `design/contracts/requirements.toml` (SC-002, INV-2)

## Phase 4: User Story 2 — Accurate build-manifest header (P2)

**Goal**: `Cargo.toml` header points at the live workflow, not the archived plan.

**Independent Test**: `sed -n '1,8p' Cargo.toml` shows a current-workflow pointer; `grep "Phase 1 in progress" Cargo.toml` finds nothing.

- [X] T006 [US2] Rewrite the top comment block of `Cargo.toml` to reference the current spec-kit workflow (`.specify/feature.json` → `specs/NNN/`, and/or `CLAUDE.md`), removing "Phase 1 in progress" and the presentation of `design/plan.md` as the live plan (FR-005, R-003). Comment-only change — do not alter any `[workspace]`/`[dependencies]`/`[profile]` keys

## Phase 5: User Story 3 — No orphaned scaffolding (P3)

**Goal**: Root `tests/integration/` and `benches/` no longer exist.

**Independent Test**: `test ! -e tests/integration && test ! -e benches`.

- [X] T007 [P] [US3] Remove the untracked empty `tests/integration/` directory (`rmdir tests/integration` and, if then empty, `tests/`); confirms nothing built from it (research R-004)
- [X] T008 [P] [US3] `git rm benches/.gitkeep` and remove the now-empty root `benches/` directory (all real benches live under `crates/*/benches/`)

## Phase 6: User Story 4 — Durable record of findings + deferral (P2)

**Goal**: The deferred `cargonaut-core` module-split is tracked per the constitution's paper-trail rule.

**Independent Test**: `grep cargonaut-core ROADMAP.md` shows one row referencing an issue #; `gh issue list --label follow-up` shows the open issue.

- [X] T009 [US4] Open a GitHub issue for the `cargonaut-core/src/lib.rs` god-file split (6.2k lines, ~300 fns, 1 module) with: problem statement, why deferred (out of scope for housekeeping; behavior-preserving refactor warrants its own spec), suggested approach (split into `fs`/`compare`/`rename`/`history`/`attrs` submodules, keep public re-exports stable), pointer to decision (this spec §US4 + research R-006), effort estimate (M), and the `follow-up` label + appropriate tier label
- [X] T010 [US4] Add a row to `ROADMAP.md` in the correct tier referencing the T009 issue with a one-line context note ("Feature 058 audit follow-up — cargonaut-core is a single 6.2k-line module; split into submodules")

## Phase 7: Polish & Cross-Cutting (docs gate + verification)

- [X] T011 Update `README.md`: add a Feature 058 entry to Feature History and refresh the "At a Glance" metrics if affected (FR-011)
- [X] T012 Append a Feature 058 section to `Learnings.md` with ≥3 bullets (the false-CI-claim discovery, the live-vs-historical surgical-archive decision, the untracked-empty-dir git nuance) (FR-011)
- [X] T013 Run `make ci-local` and confirm green: clippy `-D warnings` → `cargo test --workspace` → `cargo build --release` → check-pr-body → docs-gate (SC-005)
- [X] T014 Run the full [quickstart.md](./quickstart.md) checklist (SC-001..SC-007) and confirm every check passes, including `git diff --name-only main...HEAD -- 'crates/*/src/'` is empty (SC-006)

## Dependencies & Execution Order

- **T001** (setup) gates everything.
- **US1, US2, US3, US4** are mutually independent after T001 — any order, much in parallel.
- **Phase 7** (T011–T014) runs last: docs updates (T011/T012) are required for docs-gate, and verification (T013/T014) must follow all edits.
- Within US1: T002→T003 touch the same file (sequential); T004 is `[P]`; T005 verifies after.
- US3: T007 and T008 are `[P]` (different paths).

## Parallel Execution Examples

```text
# After T001, launch the independent story edits together:
T002+T003 (requirements.toml)  ||  T004 (design/README.md)  ||  T006 (Cargo.toml)  ||  T007 (tests/integration)  ||  T008 (benches)  ||  T009 (gh issue)
# Then converge:
T005, T010, T011, T012  →  T013 (ci-local)  →  T014 (quickstart verify)
```

## MVP Scope

US1 alone (T002–T005) is the highest-value slice: it neutralizes the most dangerous
artifact (a manifest that lies about test coverage). US2–US4 complete the "repo matches
reality" goal and satisfy the deferral paper-trail rule.

## Total

14 tasks — US1: 4, US2: 1, US3: 2, US4: 2, Setup: 1, Polish: 4.

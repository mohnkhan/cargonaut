# Feature Specification: Repository Housekeeping — Stale Artifact Reconciliation

**Feature Branch**: `058-repo-housekeeping`

**Created**: 2026-06-21

**Status**: Draft

**Input**: User description: "Repository housekeeping: reconcile/archive stale planning artifacts and remove orphaned scaffolding (requirements.toml, Cargo.toml header, empty tests/integration + benches dirs); document findings as a tracker; double-verify all claims."

## Overview

The repository carries three pieces of decayed metadata left over from an earlier
phase of the project. They are not bugs in shipped code — every crate builds and all
tests pass — but they actively mislead anyone reading the repo to understand it:

1. **`design/contracts/requirements.toml`** — the original 6-phase master manifest
   (FR-001…FR-503, SC-001…SC-010, NFR-001…NFR-008). Of its **59** file-path
   `verification =` links, **only 2 resolve; 57 are dead** (measured). Its header
   asserts *"CI greps this file to verify every requirement has a linked test"* —
   but **no** CI workflow, script, or Makefile target references it (measured). It
   predates the per-feature `specs/NNN/` spec-kit workflow adopted at feature 031 and
   has not tracked reality since.

2. **`Cargo.toml` header comment** — says *"See design/plan.md … `cargo build`
   produces the cargonaut binary (Phase 1 in progress)."* The project shipped
   feature 057; "Phase 1 in progress" and the `design/plan.md` pointer are both stale.

3. **Orphaned empty scaffolding** — `tests/integration/` (untracked, empty on disk)
   and `benches/` (contains only a tracked `.gitkeep`). All real integration tests
   live under `crates/*/tests/` and all real benches under `crates/*/benches/`.
   Nothing in the build references the root locations.

This feature reconciles these artifacts so the repository's self-description matches
its actual structure, and records the findings — plus one deferred follow-up
(`cargonaut-core/src/lib.rs` is a 6.2k-line, ~300-function single module) — in a
durable tracker per the project's deferral paper-trail rule.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Trustworthy contracts manifest (Priority: P1)

A new contributor opens `design/contracts/requirements.toml` to learn which tests
verify which requirements. Today they are misled: the file claims CI enforcement
that does not exist and points at 57 test files that do not exist. After this change
the file clearly announces it is a **historical archive** superseded by the
per-feature `specs/NNN/` workflow, and the false CI-enforcement claim is removed — so
the reader is correctly oriented instead of chasing dead paths.

**Why this priority**: A metadata file that lies about test coverage is the most
dangerous of the three — it looks like authoritative traceability while providing
none. Highest risk of wasted contributor time and false confidence.

**Independent Test**: Open the file; confirm the top banner marks it historical and
points to `specs/NNN/`; confirm no remaining sentence claims active CI greps it.
Confirm the live contract files in the same directory are untouched.

**Acceptance Scenarios**:

1. **Given** the archived `requirements.toml`, **When** a reader opens it, **Then**
   the first lines state it is a historical/superseded artifact and name the current
   source of truth (`specs/NNN/` + each feature's own verification).
2. **Given** the archived file, **When** a reader scans the header, **Then** there is
   no surviving claim that CI greps the file to enforce requirements.
3. **Given** the change, **When** the live contract files (`config.schema.json`,
   `keymap.toml`, `commands.toml`, `*.schema.json`, `plugin-api.md`) are inspected,
   **Then** they are byte-for-byte unchanged.

---

### User Story 2 - Accurate build-manifest header (Priority: P2)

A contributor opening `Cargo.toml` (the first file most people read in a Rust repo)
sees a header that points them at the current planning workflow, not a superseded
plan document and a long-finished "Phase 1".

**Why this priority**: First-read orientation file; cheap to fix; wrong pointer sends
readers to the very artifact being archived in US1.

**Independent Test**: Read the top comment of `Cargo.toml`; confirm it references the
current workflow (`.specify/feature.json` + `specs/NNN/` and/or `CLAUDE.md`) and no
longer says "Phase 1 in progress" or points at `design/plan.md` as the live plan.

**Acceptance Scenarios**:

1. **Given** the edited `Cargo.toml`, **When** a reader reads the header comment,
   **Then** it names the current spec-kit workflow as the source of truth.
2. **Given** the edit, **When** `cargo metadata` / `cargo build` is run, **Then** the
   manifest still parses and builds (comment-only change).

---

### User Story 3 - No orphaned scaffolding (Priority: P3)

A contributor browsing the repo root does not encounter empty `tests/integration/`
and `benches/` directories that imply a layout the project does not use.

**Why this priority**: Lowest risk (empty dirs mislead only mildly) but completes the
"repo matches reality" goal.

**Independent Test**: After the change, root `tests/integration/` and root `benches/`
no longer exist; `cargo test --workspace` and `cargo build --release` still succeed.

**Acceptance Scenarios**:

1. **Given** the change, **When** the repo root is listed, **Then** neither
   `tests/integration/` nor `benches/` is present.
2. **Given** the change, **When** the full CI pipeline runs, **Then** it passes
   (nothing built from the removed locations).

---

### User Story 4 - Durable record of findings + deferral (Priority: P2)

The audit that produced this feature also surfaced a larger structural issue
(`cargonaut-core` is a single 6.2k-line module) that is out of scope here. Per the
project's deferral rule, this is captured as a GitHub issue **and** a `ROADMAP.md`
row so it is not silently forgotten.

**Why this priority**: Mandated by `CLAUDE.md` ("Deferrals — MANDATORY paper trail").
Without it the deferred work decays out of view once this PR merges.

**Independent Test**: A GitHub issue exists with problem/why-deferred/approach/effort
and a `follow-up` label; `ROADMAP.md` has a row referencing it.

**Acceptance Scenarios**:

1. **Given** the merged feature, **When** `ROADMAP.md` is read, **Then** it contains a
   row for the `cargonaut-core` module-split follow-up referencing a GitHub issue.
2. **Given** the GitHub issue, **When** opened, **Then** it states the problem, the
   reason for deferral, a suggested approach, an effort estimate, and a pointer to
   where the deferral was decided.

---

### Edge Cases

- **docs-gate**: This is a `NNN-name` feature branch, so `scripts/ci/docs-gate.sh`
  requires both `README.md` and `Learnings.md` to be modified, OR a `[no-docs]`
  substring in a commit message. Decision: treat this as a real feature and update
  both docs (it changes how the repo is navigated), satisfying the gate honestly.
- **Empty-dir git semantics**: `tests/integration/` is untracked (git stores no empty
  dirs), so its removal is a working-tree-only action with no committed diff; only
  `benches/.gitkeep` produces a tracked deletion. The spec must not assume a git diff
  for the untracked dir.
- **Accidental scope creep**: archiving must not delete or rewrite the *live* contract
  files co-located in `design/contracts/`. Only `requirements.toml` is touched there.
- **Reversibility**: archiving (banner) rather than deleting `requirements.toml`
  preserves the original requirements record, consistent with the project's
  paper-trail values; git history preserves it regardless.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `design/contracts/requirements.toml` MUST be marked as a historical
  artifact via a prominent top-of-file banner that (a) states it is superseded by the
  per-feature `specs/NNN/` workflow and (b) names the current source of truth for
  requirement→test traceability.
- **FR-002**: The false assertion in `requirements.toml` that "CI greps this file to
  verify every requirement" MUST be removed or corrected, because no CI/script/Make
  target references the file (verified).
- **FR-003**: The live machine-readable contract files in `design/contracts/`
  (`config.schema.json`, `keymap.toml`, `commands.toml`, `menu.schema.json`,
  `openers.schema.json`, `plugin-api.md`) MUST remain unchanged.
- **FR-004**: A `design/`-level archive marker (e.g., a `design/README.md` banner or
  equivalent) SHOULD identify the original master-plan documents (`plan.md`,
  `spec.md`, `tasks.md`, `research.md`, `requirements.toml`) as historical, retained
  for context, superseded by `specs/NNN/`.
- **FR-005**: The `Cargo.toml` header comment MUST be corrected to point at the
  current workflow (`.specify/feature.json` + `specs/NNN/`, and/or `CLAUDE.md`) and
  MUST NOT claim "Phase 1 in progress" or present `design/plan.md` as the live plan.
- **FR-006**: The root `tests/integration/` directory MUST be removed.
- **FR-007**: The root `benches/` placeholder (`benches/.gitkeep` and the directory)
  MUST be removed.
- **FR-008**: The change MUST NOT modify any production source under `crates/*/src/`
  or alter build/test behavior; the workspace MUST still pass clippy, `cargo test
  --workspace`, and `cargo build --release`.
- **FR-009**: A GitHub issue MUST be opened recording the audit findings and the
  deferred `cargonaut-core` module-split follow-up, with problem statement, reason for
  deferral, suggested approach, pointer to decision, effort estimate, and a
  `follow-up` label.
- **FR-010**: `ROADMAP.md` MUST gain a row in the appropriate tier referencing the
  issue from FR-009 with a one-line context note.
- **FR-011**: `README.md` and `Learnings.md` MUST be updated per the project docs
  rule (feature history / at-a-glance metrics; learnings entry with ≥3 bullets).

### Key Entities

- **Historical artifact**: a planning document retained for context but no longer
  authoritative (`requirements.toml`, `design/plan.md`, etc.).
- **Live contract**: a machine-readable file still consumed by code/config
  (`config.schema.json`, `keymap.toml`) — must be preserved.
- **Follow-up tracker**: GitHub issue + `ROADMAP.md` row pair that records deferred
  work per the constitution's paper-trail rule.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After the change, a reader of `requirements.toml` can determine within
  the first 15 lines that the file is historical and where the current source of truth
  is — with zero surviving claims of active CI enforcement.
- **SC-002**: 100% of the live contract files in `design/contracts/` are unchanged
  (verified by diff: only `requirements.toml` differs in that directory).
- **SC-003**: `Cargo.toml` header contains no reference to "Phase 1 in progress" and
  no presentation of `design/plan.md` as the current plan; it names the current
  workflow.
- **SC-004**: Root `tests/integration/` and root `benches/` no longer exist.
- **SC-005**: The full CI pipeline (clippy `-D warnings` → `cargo test --workspace` →
  `cargo build --release` → check-pr-body → docs-gate) passes on the branch.
- **SC-006**: Zero files under `crates/*/src/` are modified by this feature.
- **SC-007**: A `follow-up`-labelled GitHub issue exists for the `cargonaut-core`
  split, referenced by exactly one new `ROADMAP.md` row.

## Assumptions

- The reconciliation of `requirements.toml` is an **archive** (banner + corrected
  claim), not a path-by-path reconciliation of all 57 dead links, because (a) nothing
  consumes the file, (b) the per-feature `specs/NNN/` already carry their own
  verification, and (c) reconciling 57 historical requirements to current tests is
  disproportionate effort for a superseded document. Recorded as the chosen default.
- Archiving in place (banner) is preferred over deletion to preserve the original
  requirements record; git history would preserve it either way.
- The `cargonaut-core` module split is **out of scope** for this feature and is
  handled purely as a tracked deferral (issue + ROADMAP), not implemented here.
- "All real benches live per-crate" is treated as established (verified: 11 benches
  under `crates/*/benches/`); spec-doc references to a root `benches/` path are
  conceptual shorthand, not build inputs.
- The docs-gate is satisfied honestly by updating `README.md` + `Learnings.md` rather
  than bypassed with `[no-docs]`, since the repository's navigation does change.

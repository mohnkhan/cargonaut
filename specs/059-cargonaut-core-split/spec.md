# Feature Specification: cargonaut-core God-File Split

**Feature Branch**: `059-cargonaut-core-split`

**Created**: 2026-06-21

**Status**: Draft

**Input**: User description: "Split the 6,246-line crates/cargonaut-core/src/lib.rs god-file into ≥4 cohesive, responsibility-focused submodules while keeping lib.rs a thin module root of re-exports. Behavior-preserving, move-only refactor: the crate's public API must stay byte-for-byte stable so downstream crates need zero changes. Tracks GitHub issue #86; decided in Feature 058 audit."

## Overview

`crates/cargonaut-core/src/lib.rs` has grown to 6,246 lines: a single module holding the `App` state machine, ~14 public types, the full command-dispatch surface, ~70 methods spanning navigation/filesystem/attributes/compare/rename/history/bookmarks/tabs/transfers, ~12 free helper functions, and one ~3,500-line `#[cfg(test)]` block. Every other crate in the workspace subdivides cleanly (`cargonaut-ui-tui` → 6 submodules; `cargonaut-vfs` → `archive/` + `remote/`). This crate does almost no internal subdivision, which hurts navigability, review locality, and incremental-compile granularity.

This feature pays down that structural debt by splitting the file into cohesive submodules **without changing any behavior or any public API**. It is the one genuine internal "god-file" surfaced by the Feature 058 repository-organization audit.

## User Scenarios & Testing *(mandatory)*

The "users" of this feature are the project's **maintainers and contributors** (the people who read, review, and extend `cargonaut-core`) and the **downstream crates** that depend on its public surface.

### User Story 1 - Maintainer navigates to a responsibility area quickly (Priority: P1)

A contributor needs to change how directory comparison works. Today they open a 6,246-line file and scroll/search to find the relevant code interleaved with unrelated concerns. After this feature, they open a focused submodule whose name signals its responsibility (e.g. the compare module) and find the implementation plus its tests in one place.

**Why this priority**: Navigability is the primary pain the issue calls out and the core value of the refactor. It is the minimum viable outcome — a single well-named submodule already delivers locality.

**Independent Test**: Pick one responsibility (e.g. directory compare). Confirm its implementation lives in a single, aptly named submodule rather than scattered through `lib.rs`, and that opening that file shows the related code together.

**Acceptance Scenarios**:

1. **Given** the refactored crate, **When** a maintainer looks for the directory-compare logic, **Then** it is located in one cohesive submodule named for that responsibility, not in `lib.rs`.
2. **Given** the refactored crate, **When** a maintainer opens `lib.rs`, **Then** they see a module root (module declarations, re-exports, and minimal glue) rather than implementation bodies.
3. **Given** the refactored crate, **When** a maintainer counts the implementation submodules, **Then** there are at least 4 focused submodules, each scoped to a coherent responsibility.

---

### User Story 2 - Downstream crates compile unchanged (Priority: P1)

The binary, the TUI layer, and the transfer-facing code all depend on `cargonaut-core`'s public types and functions. After this internal reorganization, every one of those dependents must build and pass tests without a single source edit.

**Why this priority**: A move-only refactor that breaks the public surface defeats its own purpose and forces ripple-edit work across the workspace. Stability of the public API is a hard constraint, equal in priority to the navigability win.

**Independent Test**: Build and test the entire workspace after the split with no edits to any file under `crates/cargonaut-ui-tui/src`, `crates/cargonaut-transfer/src`, or `crates/cargonaut-bin/src`. Everything compiles and passes.

**Acceptance Scenarios**:

1. **Given** the refactored crate, **When** the full workspace is built and tested, **Then** it succeeds with zero changes to any downstream crate's source.
2. **Given** the refactored crate, **When** the set of names exported from `cargonaut-core` is compared before and after, **Then** the public export surface is identical (same names, same paths, same signatures).
3. **Given** existing code that imports a symbol via `cargonaut_core::<Name>`, **When** it is recompiled, **Then** the import resolves exactly as before.

---

### User Story 3 - Behavior and quality gates are provably unchanged (Priority: P1)

The refactor must demonstrably change nothing about how the crate behaves. The existing test suite — which exercises navigation, filesystem ops, attributes, compare, rename/undo, history, bookmarks, tabs, and transfers — must continue to pass, and the project's quality gates (lint-as-errors, documentation-completeness) must stay green.

**Why this priority**: "Behavior-preserving" is only credible if the existing tests still pass and the gates still hold. Without this, the move-only claim is unverified.

**Independent Test**: Run the workspace test suite and the lint/docs gates before and after; both are green after, with the same set of tests executing.

**Acceptance Scenarios**:

1. **Given** the refactored crate, **When** the workspace test suite runs, **Then** every test that passed before still passes, and no test is silently dropped.
2. **Given** the refactored crate, **When** the lint gate runs with warnings treated as errors, **Then** it reports zero warnings.
3. **Given** the refactored crate, **When** the documentation-completeness gate runs, **Then** every public item still carries documentation (no regressions).

---

### Edge Cases

- **A method needs private state owned by the central type.** Methods relocated into submodules must still reach the private fields of the central `App`/pane types. The split must preserve this access without widening any field's visibility beyond what exists today (no new `pub` on internal state).
- **Tests reference private helpers.** Many existing tests call private methods and shared test helpers. The split must keep these tests able to see what they test (they remain in-crate unit tests, not converted to public-only integration tests), and shared helpers must remain reachable by every test that uses them.
- **A responsibility spans the central type's methods AND free functions.** Some concerns (e.g. rename validation, transfer-state projection) exist both as methods and as standalone functions. The split must keep each concern's pieces together rather than separating the method from its helper.
- **A symbol is currently re-exported from another crate.** `cargonaut-core` re-exports some types from sibling crates. Those re-exports must remain at the same public path after the split.
- **Circular module references.** Splitting must not introduce a module-dependency cycle or require a type to be defined in two places.
- **`cargo doc` intra-doc links.** Documentation links between items must still resolve after items move to submodules.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The crate's implementation MUST be reorganized so that `lib.rs` is a module root — containing module declarations, the crate-level documentation, re-exports/glue, and at most minimal coordinating code — rather than the bulk of the implementation.
- **FR-002**: The implementation MUST be distributed across at least 4 focused submodules, each scoped to a single coherent responsibility (candidate responsibilities include: filesystem/navigation, directory compare, bulk rename/undo, directory & command history, file attributes/recursive sizing, jobs/transfers, bookmarks/hotlist, pane/tab state). The exact module boundaries are an implementation decision, provided each module is cohesive.
- **FR-003**: The crate's public API surface MUST remain identical after the split — the same public types, functions, methods, traits, constants, and re-exports, each reachable at the same `cargonaut_core::…` path with the same signature as before.
- **FR-004**: Downstream crates (`cargonaut-ui-tui`, `cargonaut-transfer`, `cargonaut-bin`) MUST require zero source changes to build and pass their tests against the refactored crate.
- **FR-005**: The refactor MUST be behavior-preserving (move-only): no logic changes, no signature changes, no changes to error messages or status strings, no changes to control flow. Edits are limited to relocating code, adding module plumbing, and adjusting `use`/visibility paths needed purely to make the move compile.
- **FR-006**: The existing test suite MUST continue to pass in full, with no test removed, disabled, or silently skipped as a result of the move. Tests MAY be relocated alongside the code they exercise.
- **FR-007**: The relocation MUST NOT widen the visibility of any currently-private item beyond what is strictly required to compile after the move; internal state that is private today MUST remain inaccessible to other crates.
- **FR-008**: The documentation-completeness expectation MUST be upheld — every public item still carries documentation after the move, and intra-documentation links still resolve.
- **FR-009**: The lint gate (warnings-as-errors) MUST pass with zero warnings after the split.
- **FR-010**: After the split, no single resulting source file in the crate SHOULD remain a god-file; in particular the large central test block MUST NOT simply be left whole in `lib.rs` while only production code is moved out.
- **FR-011**: The change MUST be delivered through the mandated branch + PR workflow with the required documentation updates (`README.md` metrics/history, `Learnings.md` section) and the deferral paper-trail closed out (issue #86 referenced/closed; ROADMAP row reconciled).

### Key Entities *(include if feature involves data)*

This feature reorganizes code; it does not introduce or change runtime data. The relevant "entities" are code-organization units:

- **Module root (`lib.rs`)**: After the split, the thin entry point — crate docs, module declarations, and the public re-export surface that defines what `cargonaut_core` exposes.
- **Responsibility submodule**: A cohesive file grouping one concern's types, methods (as additional `impl` blocks on the central type where applicable), free helpers, and co-located tests.
- **Public export surface**: The complete set of names reachable via `cargonaut_core::…` — the invariant that must be byte-for-byte preserved.
- **Central application type (`App`) and pane/state types**: Types whose private fields are accessed by relocated methods; their field visibility must be preserved.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: `lib.rs` shrinks from 6,246 lines to a thin module root (target: under ~400 lines of declarations/re-exports/glue), with implementation living in submodules.
- **SC-002**: The crate contains at least 4 focused implementation submodules, each cohesive (a reviewer can state each module's single responsibility in one sentence).
- **SC-003**: The public export surface of `cargonaut-core` is identical before and after — a diff of the exported-names list shows zero additions, removals, or signature changes.
- **SC-004**: The full workspace builds and the complete test suite passes after the split, with the same number of tests executing as before (no test lost), and zero source edits in any downstream crate.
- **SC-005**: The lint gate (warnings-as-errors) and the documentation-completeness gate both pass with zero warnings.
- **SC-006**: No resulting file in the crate exceeds a reasonable size ceiling for a single concern (target: no production submodule meaningfully larger than the largest pre-existing sibling-crate module; the central test block is partitioned rather than left whole).
- **SC-007**: A contributor can locate the code for any one named responsibility (e.g. "directory compare", "bulk rename") by opening a single file named for it, verified by inspection.

## Assumptions

- **Test placement**: Tests are relocated to sit alongside the code they exercise (idiomatic per-module `#[cfg(test)]` blocks), with shared test helpers placed where every dependent test can reach them. They remain in-crate unit tests so they retain access to private items. This is preferred over leaving the monolithic test block in `lib.rs`, which would leave `lib.rs` a god-file (FR-010).
- **Module count**: "≥4 submodules" is treated as a floor, not a target; the implementation may produce more if that yields better cohesion. Excessive fragmentation (one tiny module per function) is avoided in favor of responsibility-sized modules.
- **Type-definition placement**: Public type definitions may move out of `lib.rs` into submodules and be re-exported, so long as their public path through `cargonaut_core::…` is unchanged via re-exports. Keeping them in `lib.rs` is acceptable only if `lib.rs` still ends up thin.
- **Private-field access across modules**: Relocated methods rely on the language rule that descendant modules can access items kept private to an ancestor module, so internal state need not be made `pub` to support the move. Where a single visibility widening is genuinely unavoidable to compile, it is scoped as narrowly as possible (crate-internal, not public).
- **Move-only discipline**: Aside from mechanical `use`/path/visibility adjustments and module plumbing, no implementation logic is rewritten "while we're in there." Opportunistic cleanups are out of scope and deferred to a separate change.
- **Verification of API stability**: API stability is verified by comparing the crate's exported-symbol surface before and after, and by building the unchanged downstream crates — not merely by the author's inspection.
- **Tracking artifacts**: This feature closes the Feature 058 deferral; on merge, issue #86 is closed and the corresponding ROADMAP row is reconciled, per the project's deferral paper-trail rule.

## Out of Scope

- Any change to runtime behavior, performance characteristics, error/status text, or control flow.
- Any change to the public API (additions, removals, renames, signature changes) — including "improvements" to the surface.
- Refactoring logic for clarity beyond the mechanical move (no algorithm rewrites, no dependency changes, no new abstractions).
- Edits to downstream crates' source.
- Splitting any other crate or file; this feature is scoped to `crates/cargonaut-core`.

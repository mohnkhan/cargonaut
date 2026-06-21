# Feature Specification: Release Process & Version Management

**Feature Branch**: `064-release-versioning`

**Created**: 2026-06-21

**Status**: Draft

**Input**: Plan a release and version management (issue #95) — a written
versioning policy, a documented + automated release process, and a CHANGELOG
convention, so the project can cut `v0.1.0` and subsequent releases consistently.

## Clarifications

### Session 2026-06-21

- Q: SemVer flavor pre-1.0? → A: **SemVer with 0.y.z rules** — while < 1.0, a
  `minor` bump may include breaking changes and `patch` is fixes/additive; this
  is stated explicitly so users know 0.x stability expectations.
- Q: Single version source? → A: **`[workspace.package] version`** — every crate
  already inherits via `version.workspace = true`; bump in exactly one place.
- Q: Actually cut `v0.1.0` in this feature? → A: **No — set up the process and
  prepare the CHANGELOG/version, but pushing the tag + publishing the GitHub
  Release is an outward-facing action done on explicit go-ahead** (out of scope
  for the merge itself).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A maintainer can cut a release by following one document (Priority: P1)

A maintainer wanting to release reads `docs/RELEASING.md` and follows a short,
unambiguous checklist (bump → changelog → verify → tag → push), and the rest
(artifact build + GitHub Release) happens automatically from the pushed tag.

**Why this priority**: Without a written, repeatable process the first release is
ad-hoc and error-prone. This is the core deliverable.

**Independent Test**: A dry-run following `RELEASING.md` up to (not including) the
tag push produces a green `make ci-local` and a correctly-structured CHANGELOG
version section; `make release-check` passes.

**Acceptance Scenarios**:

1. **Given** the repo at a release-ready commit, **When** a maintainer runs the
   documented preflight, **Then** version, CHANGELOG, and CI state are validated.
2. **Given** an annotated `vX.Y.Z` tag is pushed, **When** the release workflow
   runs, **Then** it builds the distributable artifact and publishes a GitHub
   Release with notes derived from the CHANGELOG.

---

### User Story 2 - Anyone can understand the versioning rules (Priority: P2)

A contributor or user reads `docs/VERSIONING.md` and knows what a version number
means (SemVer, 0.x rules), where the single version source is, the MSRV policy,
and what counts as a breaking change.

**Why this priority**: Shared rules prevent accidental breaking releases and make
the version meaningful. Independent of the mechanics.

**Acceptance Scenarios**:

1. **Given** `VERSIONING.md`, **When** a contributor proposes a change, **Then**
   they can classify it as patch/minor/major and know where to bump.

---

### Edge Cases

- Tag pushed without a matching CHANGELOG section → release workflow should fail
  loudly (or refuse) rather than publish empty notes.
- Tag version not matching `[workspace.package] version` → preflight catches it.
- Re-running the release workflow for an existing tag → idempotent / no duplicate.
- A release built from a dirty or non-CI-green commit → preflight refuses.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A `docs/VERSIONING.md` MUST define the SemVer policy (incl. 0.y.z
  pre-1.0 rules), the single version source (`[workspace.package] version`), the
  MSRV policy, and the definition of a breaking change.
- **FR-002**: A `docs/RELEASING.md` MUST give a step-by-step release checklist:
  bump version, update CHANGELOG (version section), verify (`make ci-local`),
  tag (`vX.Y.Z`, annotated), push, and what the automation then does.
- **FR-003**: The CHANGELOG MUST adopt versioned sections with an `Unreleased`
  collector at the top (Keep-a-Changelog style), without losing existing history.
- **FR-004**: A release workflow MUST trigger on `v*` tags, build the
  distributable artifact (`make dist`), and publish a GitHub Release with the
  artifact, a checksum, and notes from the matching CHANGELOG section.
- **FR-005**: A preflight (`make release-check`) MUST verify the tag/version
  consistency, that the CHANGELOG has a section for the version, and a clean tree.
- **FR-006**: The release workflow MUST NOT publish if the version's CHANGELOG
  section is missing (fail loudly).
- **FR-007**: The new `make` target MUST follow the discoverability checklist
  (`.PHONY`, header, `help`).

### Key Entities

- **Release**: a `vX.Y.Z` git tag + a GitHub Release with the dist tarball,
  checksum, and CHANGELOG-derived notes.
- **Version**: the single `[workspace.package] version` value (SemVer).

## Success Criteria *(mandatory)*

- **SC-001**: A maintainer can complete the documented preflight and produce a
  green `make ci-local` + valid CHANGELOG version section with no undocumented
  steps.
- **SC-002**: `make release-check` passes on a correctly-prepared release commit
  and fails on a version/CHANGELOG/tree mismatch.
- **SC-003**: Pushing a `vX.Y.Z` tag produces a GitHub Release with the dist
  artifact + checksum + notes (verified once on the first real release).
- **SC-004**: The release workflow refuses to publish when the CHANGELOG lacks a
  section for the tag's version.

## Assumptions

- `make dist` already builds a stripped static musl tarball; the workflow reuses it.
- Releases are published via GitHub Releases (`gh release` / actions), no crates.io
  publish in this feature (the binary is the artifact; libs are internal).
- The first tag will be `v0.1.0`, cut later on explicit go-ahead.

### Out of Scope

- Actually pushing the tag / publishing `v0.1.0` (done on explicit confirmation).
- Publishing crates to crates.io.
- Signed releases / SBOM / provenance (future hardening).

# Feature Specification: Coverage-Guided Fuzzing for Parsers

**Feature Branch**: `063-fuzzing-parsers`

**Created**: 2026-06-21

**Status**: Draft

**Input**: Implement fuzzing (issue #93) — coverage-guided fuzz targets for the
untrusted-input parsers, plus an always-on randomized invariant gate in CI.

## Clarifications

### Session 2026-06-21

- Q: Real coverage-guided fuzzing vs property testing? → A: **Both** — a
  `cargo-fuzz` (libfuzzer) harness for deep/local + nightly-CI fuzzing, AND
  `proptest` "never-panics / roundtrip" invariants that run in the normal
  (stable) CI as the always-on regression gate.
- Q: Which inputs? → A: The pure `&str` parsers first — `VfsPath::parse`,
  `ModeSpec::parse`, `parse_owner` — each with a "must never panic" invariant.
- Q: SSD safety for fuzz build artifacts? → A: Fuzz builds/corpora use a tmpfs
  `CARGO_TARGET_DIR` via a `make fuzz*` target (Constitution §V).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Parsers never panic on arbitrary input (Priority: P1)

A developer (or CI) feeds random/adversarial byte strings to the user-input
parsers and none of them panics — malformed input always yields a clean error,
never a crash.

**Why this priority**: A panicking parser is a latent crash (now caught by the
Feature 061 net, but wrong). This is the core safety property and the always-on
gate.

**Independent Test**: Run the randomized-invariant suite (thousands of cases per
parser) in normal CI; it passes with no panic.

**Acceptance Scenarios**:

1. **Given** any byte string, **When** `VfsPath::parse` / `ModeSpec::parse` /
   `parse_owner` is called, **Then** it returns `Ok` or `Err` — never panics.
2. **Given** a value that parses, **When** it is rendered and re-parsed (where a
   render exists), **Then** the result is equivalent (roundtrip).

---

### User Story 2 - Deep coverage-guided fuzzing is available (Priority: P2)

A maintainer can run real libfuzzer-driven, coverage-guided fuzzing against each
parser locally and in a periodic CI job to discover deep edge cases beyond what
random property testing reaches.

**Why this priority**: Coverage-guided fuzzing finds inputs property testing
won't; it's the foundation for the future sandbox-escape fuzzer (Constitution
§IV / SC-006). P2 because the P1 invariant gate already prevents regressions.

**Independent Test**: `make fuzz-<target>` builds and runs a libfuzzer target for
a bounded time with no crash; a CI nightly job does the same per PR.

**Acceptance Scenarios**:

1. **Given** the fuzz harness, **When** a maintainer runs a target for a bounded
   time, **Then** it executes coverage-guided iterations and reports no crash.
2. **Given** a fuzz run, **When** it builds/runs, **Then** no build artifact is
   written to the SSD (tmpfs target dir, Constitution §V).

---

### Edge Cases

- Empty input, very long input, invalid UTF-8 bytes, embedded NULs, control
  chars, huge octal numbers, deeply nested/odd scheme+authority combinations.
- A discovered crash must reproduce deterministically from a saved corpus entry.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: `VfsPath::parse`, `ModeSpec::parse`, and `parse_owner` MUST never
  panic on any input; malformed input yields `Err`.
- **FR-002**: A randomized-invariant suite covering FR-001 MUST run in the normal
  (stable) CI and fail the build on any panic.
- **FR-003**: Where a parsed value has a textual rendering, parse∘render MUST
  roundtrip for valid inputs.
- **FR-004**: A `cargo-fuzz` (libfuzzer) target MUST exist for each of the three
  parsers.
- **FR-005**: Fuzz builds and corpora MUST NOT write to the SSD; a documented
  `make` target sets a tmpfs `CARGO_TARGET_DIR` (Constitution §V).
- **FR-006**: A CI job MUST run each fuzz target for a bounded time per PR (smoke
  fuzz), failing on any discovered crash.
- **FR-007**: A discovered crash MUST be reproducible from a saved corpus/artifact.

### Key Entities

- **Fuzz target**: a libfuzzer entry (`fuzz_target!`) wrapping one parser.
- **Invariant test**: a proptest case asserting no-panic / roundtrip.

## Success Criteria *(mandatory)*

- **SC-001**: The randomized-invariant suite runs ≥ 1000 cases per parser in CI
  with zero panics.
- **SC-002**: Each parser has a runnable `cargo-fuzz` target that builds and
  executes coverage-guided iterations.
- **SC-003**: A fuzz run places no build artifacts on the SSD (verified: target
  dir resolves under tmpfs).
- **SC-004**: The stable test suite stays green and the binary ≤ 8 MiB
  (the `fuzz/` crate is excluded from the workspace default build).

## Assumptions

- `proptest` is already a workspace dev-dependency (used for the invariant gate).
- `cargo-fuzz` + nightly are available where deep fuzzing runs (dev host nightly
  is default; CI installs them for the nightly fuzz job).
- The `fuzz/` crate is its own cargo package (not a workspace member of the
  default build), so it doesn't affect the shipped binary or normal `cargo test`.

### Out of Scope

- Fuzzing archive readers (zip/tar) and remote backends — a follow-up.
- The Phase-3 sandbox-escape fuzzer (Constitution SC-006) — separate feature.
- Continuous/OSS-Fuzz integration.

# Implementation Plan: Fuzzing for Parsers (issue #93)

**Branch**: `063-fuzzing-parsers` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

## Summary

Two complementary layers:

1. **Always-on invariant gate (stable CI)** — `proptest` cases in
   `cargonaut-vfs` asserting `VfsPath::parse` / `ModeSpec::parse` / `parse_owner`
   never panic on arbitrary input (random `String` + random bytes via
   `String::from_utf8_lossy`), plus parse∘render roundtrips where a rendering
   exists. Runs in the normal `cargo test` (no new toolchain), so every PR is
   gated (SC-001).
2. **Coverage-guided fuzzing (cargo-fuzz)** — a standalone `fuzz/` crate
   (libfuzzer-sys) with one `fuzz_target!` per parser. Excluded from the
   workspace default build (its own package, `[workspace] exclude`), so it never
   touches the shipped binary or normal `cargo test`. Run via `make fuzz-<t>`
   which sets a tmpfs `CARGO_TARGET_DIR` (Constitution §V). A CI **nightly** job
   installs the toolchain and runs each target for a bounded `-max_total_time`.

## Research decisions

- **R1 — Two layers, not one.** proptest gives a zero-friction, stable-CI
  regression gate (verifiable now); cargo-fuzz gives true coverage-guided depth.
  Shipping both means the everyday gate can't bit-rot on a missing nightly tool.
- **R2 — `fuzz/` excluded from the workspace.** libfuzzer-sys needs nightly +
  sanitizer flags; making it a workspace member would break stable `cargo build`.
  `exclude = ["fuzz"]` keeps the default build/test/binary unaffected (SC-004).
- **R3 — SSD safety.** `cargo fuzz` writes to `fuzz/target` + `fuzz/corpus` +
  `fuzz/artifacts`. The `make fuzz*` targets set `CARGO_TARGET_DIR` to a tmpfs
  path and keep corpora under tmpfs; nothing large lands on the SSD (§V). CI is
  ephemeral (no SSD).
- **R4 — No-panic invariant via `catch_unwind` is unnecessary in proptest**:
  proptest already reports a panic as a test failure with the minimal input, so
  the property body just calls the parser and ignores the `Result`.

## Constitution Check

- **I. Code Quality** — PASS: fuzz crate is `#![no_main]`; proptest tests
  documented; clippy/fmt over the workspace (fuzz excluded).
- **II. Test-First** — PASS: invariant tests are the gate (SC-001); FR-001 is a
  property. SC-002/003 verified by `make fuzz` + a CI nightly job.
- **III/IV** — unaffected (no UI, no benched path).
- **V. SSD Preservation** — PASS: `make fuzz*` uses tmpfs `CARGO_TARGET_DIR`.

**SC → gate**: SC-001 proptest suite in unit-test CI; SC-002 CI nightly fuzz job
+ `make fuzz`; SC-003 the make target's resolved dir is under tmpfs; SC-004
existing build/size gates (fuzz excluded).

## Project Structure

```text
fuzz/                         # NEW standalone cargo-fuzz crate (workspace-excluded)
  Cargo.toml                  # libfuzzer-sys + path dep on cargonaut-vfs
  fuzz_targets/
    vfspath_parse.rs
    modespec_parse.rs
    owner_parse.rs
crates/cargonaut-vfs/tests/parser_fuzz_invariants.rs  # proptest no-panic/roundtrip gate
Cargo.toml                    # [workspace] exclude = ["fuzz"]
Makefile                      # fuzz / fuzz-vfspath / fuzz-modespec / fuzz-owner (+ .PHONY, header, help)
.github/workflows/ci.yml      # nightly fuzz-smoke job (bounded) → ci rollup (or separate non-blocking)
```

**Structure Decision**: keep the experimental, nightly-only fuzz toolchain in an
excluded `fuzz/` crate; put the always-green gate in the existing vfs test
target so PRs are protected without nightly.

## Complexity Tracking
> No violations.

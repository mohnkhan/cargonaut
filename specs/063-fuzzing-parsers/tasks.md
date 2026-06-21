---
description: "Tasks — Feature 063 fuzzing for parsers (issue #93)"
---

# Tasks: Coverage-Guided Fuzzing for Parsers

**Tests**: REQUIRED. **Organization**: by user story.

## Phase 1 — US1: Always-on invariant gate (P1)

- [ ] T001 [US1] (red→green) Add `crates/cargonaut-vfs/tests/parser_fuzz_invariants.rs` with proptest cases: `VfsPath::parse`, `ModeSpec::parse`, `parse_owner` never panic on arbitrary `String` and arbitrary `Vec<u8>` (lossy-decoded); ≥ 1000 cases each (SC-001, FR-001/002).
- [ ] T002 [US1] (green) Add a roundtrip property where a rendering exists (e.g. `VfsPath` display→parse) (FR-003). If no stable rendering exists, document why and skip.

## Phase 2 — US2: cargo-fuzz harness (P2)

- [ ] T003 [US2] Create the standalone `fuzz/` crate: `fuzz/Cargo.toml` (libfuzzer-sys + path dep on `cargonaut-vfs`), and add `exclude = ["fuzz"]` to the root `[workspace]` (FR-004, SC-004/R2).
- [ ] T004 [US2] Add `fuzz/fuzz_targets/{vfspath_parse,modespec_parse,owner_parse}.rs` — each a `fuzz_target!(|data: &[u8]| { if let Ok(s) = std::str::from_utf8(data) { let _ = Parser::parse(s); } })` (FR-004).
- [ ] T005 [US2] Makefile: `fuzz-vfspath` / `fuzz-modespec` / `fuzz-owner` (+ a `fuzz` umbrella) that set `CARGO_TARGET_DIR` to a tmpfs path and run `cargo +nightly fuzz run <t> -- -max_total_time=$(FUZZ_SECS)`; update `.PHONY`, header comment, and `help` (FR-005, SC-003; discoverability checklist).

## Phase 3 — US2 CI

- [ ] T006 [US2] Add a CI `fuzz-smoke` job (nightly toolchain + `cargo install cargo-fuzz`) running each target with a short `-max_total_time`; fail on crash (FR-006). Decide blocking vs nightly-scheduled; if PR-blocking, keep the time small and feed the `ci` rollup.

## Phase 4 — Polish

- [ ] T007 [P] Verify: `make fuzz-vfspath FUZZ_SECS=15` runs coverage-guided with no crash and target dir resolves under tmpfs (SC-002/003); `cargo test -p cargonaut-vfs --test parser_fuzz_invariants` green (SC-001); workspace build/size unaffected (SC-004).
- [ ] T008 [P] Docs: README (metrics + Feature History), Learnings (≥3 bullets), CHANGELOG; ROADMAP row for #93 (resolved or follow-up if CI job deferred). `make help | grep fuzz`.
- [ ] T009 [P] Final gate: `make ci-local`; clippy `-D warnings` + `fmt --check` clean (workspace; fuzz excluded).

## Dependencies
- US1 is independent and the primary gate. US2 builds the harness. CI + polish last.

## MVP
US1 (the always-on no-panic gate) — verifiable on stable CI without nightly.

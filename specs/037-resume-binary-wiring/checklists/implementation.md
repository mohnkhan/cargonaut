# Implementation Quality Checklist: Feature 037 (Resume Binary Wiring + SC-002 Gate)

**Purpose**: Gate the implementation against TDD discipline, the constitution, and the
feature's correctness/safety requirements before the PR merges.
**Created**: 2026-06-15
**Feature**: [spec.md](../spec.md) · [tasks.md](../tasks.md) · [contracts](../contracts/resume-seam.md)

## TDD discipline (Constitution §II)

- [ ] CHK001 Each `(red)` test task is committed in a failing state before its `(green)`
      partner (`T0xx (red): …` → `T0xx (green): …` visible in git log).
- [ ] CHK002 `T003`/`T004` throttle: red test fails without the hook, passes with it.
- [ ] CHK003 `T007`/`T008` core scan+resume tests fail before `T009`/`T010` land.
- [ ] CHK004 `T013`/`T014` start-over/skip tests fail before `T015`/`T016`.
- [ ] CHK005 `T018` PTY test exists and fails (or self-skips) before the wiring is complete.

## Correctness & safety (FR/SC)

- [ ] CHK006 Resume continues from the checkpoint offset, not from zero (SC-002, asserted by
      the PTY test's resumed-bytes bound).
- [ ] CHK007 Completed resume yields `sha256(src) == sha256(dst)` (SC-001/T018).
- [ ] CHK008 Mismatched/changed src or dst never produces a corrupt destination — resume
      fails safe and reports (FR-009/SC-005/C8).
- [ ] CHK009 Malformed/old-version sidecar does not crash or block launch (FR-010/T007 case).
- [ ] CHK010 Skip leaves the sidecar on disk; a fresh scan re-offers it (FR-008/C12).
- [ ] CHK011 Start over deletes the sidecar and truncates the partial destination (FR-007).
- [ ] CHK012 Successful completion removes the sidecar (FR-006).
- [ ] CHK013 No-checkpoints launch shows no prompt and no perceptible delay (FR-003/SC-004).

## Architecture & constitution

- [ ] CHK014 `cargonaut-ui-tui` does not gain a direct dependency on `cargonaut-transfer`
      types (seam preserved via `ResumeOfferView`; mirrors `ProgressView`).
- [ ] CHK015 Resume UI reuses the existing `ResumePromptDialog`; no ad-hoc layout (§III).
- [ ] CHK016 No new keymap.toml bindings introduced (launch-time modal); `r`/`s`/`c` handled
      by the existing widget (§III).
- [ ] CHK017 `#[allow(dead_code)]` removed from the `ActiveDialog::Resume` variant.
- [ ] CHK018 No `unsafe` added; throttle is a safe sleep; all new public items documented
      (`missing_docs`) (§I).
- [ ] CHK019 `CARGONAUT_TRANSFER_THROTTLE_MIBPS` is a no-op when unset (zero production cost).

## Test gate & CI

- [ ] CHK020 PTY test self-skips cleanly when `CARGONAUT_PTY_TESTS` is unset (default
      `cargo test` stays fast).
- [ ] CHK021 Binary located via `env!("CARGO_BIN_EXE_cargonaut")` (no hard-coded target path).
- [ ] CHK022 First run is terminated by SIGKILL (abrupt), not graceful quit.
- [ ] CHK023 PTY test is green and stable across ≥3 consecutive runs (SC-003/T019).
- [ ] CHK024 CI `cargo test` step sets `CARGONAUT_PTY_TESTS=1` so the gate runs per-PR
      (FR-012/T020).

## Process & docs (CLAUDE.md)

- [ ] CHK025 `README.md` At-a-Glance metrics + Feature History row updated (T023).
- [ ] CHK026 `Learnings.md` 037 section added, ≥3 bullets (T024).
- [ ] CHK027 Feature commits do NOT carry `[no-docs]`; docs-gate passes.
- [ ] CHK028 No Claude attribution trailers in commits, PR body, or issue comments.
- [ ] CHK029 Issue #29 closed and its ROADMAP Tier-1 row removed/annotated on merge (T025).
- [ ] CHK030 `make ci-local` green end-to-end (fmt, clippy `-D warnings`, test, build,
      docs-gate) (T021).

## Notes

- Check items off as completed: `[x]`.
- CHK021–CHK022 are the most common PTY-test footguns; verify them early.
- If a `CARGONAUT_ALLOW_SSD_TARGET` waiver is used during dev, record the reason in
  `Learnings.md` (Constitution §V).

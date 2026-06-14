# Implementation Plan: Resume-from-Interrupted-Transfer (Binary Wiring + SC-002 Gate)

**Branch**: `037-resume-binary-wiring` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/037-resume-binary-wiring/spec.md`

## Summary

The resumable-transfer engine (`cargonaut-transfer`) and the resume dialog widget
(`cargonaut-ui-tui::ResumePromptDialog`) are both built and unit-tested, but nothing connects
them to the running binary: `scan_resumable` is never called on launch, `ActiveDialog::Resume`
is never constructed, the `[r]` key handler is a no-op stub, and `resume_transfer` is invoked
only by its own tests. This feature wires the existing pieces together end-to-end and adds the
binary-level SC-002 regression gate that was deferred as T1.08.

The approach mirrors the codebase's existing seam between `cargonaut-core::App` (owns transfer
state, depends on `cargonaut-transfer`) and `cargonaut-ui-tui` (renders, never depends on
transfer types — it consumes UI projections like `App::active_progress() -> ProgressView`).
We add a parallel resume seam: `App` scans for offers on launch and exposes them as a UI
projection (`ResumeOfferView`); the UI builds the existing `ResumePromptDialog` from those
views and dispatches the user's choice back into `App` methods that call `resume_transfer` /
`submit_transfer`. The binary calls the scan once before entering the event loop.

The SC-002 gate is an integration test in `cargonaut-bin/tests/resume_sigkill.rs` that drives
the real binary under a PTY through a start → SIGKILL → relaunch → resume → verify cycle,
gated behind `CARGONAUT_PTY_TESTS=1` and enabled in the existing CI `cargo test` step. To make
the kill land mid-transfer deterministically, the transfer engine gains an opt-in,
env-controlled per-chunk throughput throttle (`CARGONAUT_TRANSFER_THROTTLE_MIBPS`, unset =
off in production).

## Technical Context

**Language/Version**: Rust (edition/rust-version per workspace; stable toolchain in CI).

**Primary Dependencies**: existing — `cargonaut-transfer` (`scan_resumable`, `resume_transfer`,
`submit_transfer`, `TransferCheckpoint`, `ResumableTransfer`), `cargonaut-ui-tui` dialog
widgets, `cargonaut-vfs` (`LocalFs`, `VfsPath`), `ratatui`/`crossterm`. New dev-dependency:
`portable-pty` (PTY harness for the integration test); `sha2` + `tempfile` (already in
workspace) for the test.

**Storage**: checkpoint sidecars on the local filesystem (`.cargonaut-transfer-<id>.json`,
already implemented); no new persisted format.

**Testing**: `cargo test --workspace --lib --tests`; new gated integration test
`crates/cargonaut-bin/tests/resume_sigkill.rs` (opt-in via `CARGONAUT_PTY_TESTS=1`); unit
tests in `cargonaut-core` for the new resume-seam methods; a small `cargonaut-transfer` test
for the throttle env var.

**Target Platform**: Linux (dev host + CI `ubuntu-latest`). PTY test is Unix-only.

**Project Type**: Rust workspace — multi-crate desktop/CLI TUI application.

**Performance Goals**: no regression to SC-004 (≤150 ms cold-cache startup) on the
no-checkpoints hot path; SC-002 (resume within one checkpoint interval) enforced end-to-end.

**Constraints**: Constitution §II TDD (red-before-green per FR), §III UX (reuse shared dialog +
single keymap source), §IV performance gates, §V SSD/tmpfs (test artifacts under tmpfs on dev
host; CI exempt). No `unsafe`. `#![warn(missing_docs)]` on public items.

**Scale/Scope**: small, focused — ~1 new core seam (3–4 methods + 1 projection type), ~1 UI
dispatch wiring change + 1 launch-time scan call, ~1 engine throttle hook, ~1 integration
test, plus README/Learnings.

## Constitution Check

*GATE: must pass before Phase 0 research; re-checked after design.*

| Principle | Gate | Status / Approach |
|-----------|------|-------------------|
| I. Code Quality | clippy `-D warnings`, `missing_docs`, fmt, no undocumented `unsafe` | All new public items documented; no `unsafe`; throttle is safe std sleep. PTY test uses `portable-pty` safe API. |
| II. Test-First | red commit before green per FR; each SC has a CI gate | FRs get failing core/UI tests first; **SC-002 gate is the central deliverable** (FR-011/012). |
| III. UX Consistency | shared dialog widgets; keymap single source | Reuses existing `ResumePromptDialog`; resume keys (`r`/`s`/`c`) already live in the widget; no new keymap.toml bindings needed (launch-time modal, not a pane command). |
| IV. Performance | SC gates as CI tests/benches; no >10% regress | No-offers path adds two `scan_resumable` calls over the launch dirs (a directory `list` + sidecar filter) — measured against SC-004; resume gate added. |
| V. SSD Preservation | artifacts in tmpfs; CI exempt | Test writes large temp files under `tempfile` (TMPDIR → tmpfs on dev host); `make` guard unaffected; CI exempt via `$CI`. |

**No violations requiring Complexity Tracking.** The one addition that touches production code
for test-determinism — the throttle env hook — is justified in research.md (R-002) and is a
zero-cost no-op when the env var is unset.

## Project Structure

### Documentation (this feature)

```text
specs/037-resume-binary-wiring/
├── spec.md              # /speckit-specify output (done)
├── plan.md              # This file
├── research.md          # Phase 0: decisions + rationale
├── data-model.md        # Phase 1: entities, projections, state flow
├── quickstart.md        # Phase 1: how to run the gated test + manual smoke
├── contracts/
│   └── resume-seam.md   # Phase 1: cargonaut-core API additions (the "contract")
├── checklists/
│   └── requirements.md  # spec quality checklist (done)
└── tasks.md             # /speckit-tasks output (next phase)
```

### Source Code (repository root)

```text
crates/
├── cargonaut-transfer/
│   ├── src/job.rs            # +env throttle hook in run_transfer / run_transfer_with_state
│   └── tests/                # +throttle unit/integration coverage
├── cargonaut-core/
│   └── src/lib.rs            # +ResumeOfferView projection; +pending_resumes field;
│                             #  +scan_resume_offers / resume_offer / start_over_offer /
│                             #  skip_offer / pending_resume_views
├── cargonaut-ui-tui/
│   └── src/lib.rs            # launch-time scan → construct ResumePromptDialog;
│                             #  real dispatch at the ActiveDialog::Resume arm (drop the
│                             #  #[allow(dead_code)] on the variant)
└── cargonaut-bin/
    ├── Cargo.toml            # +[dev-dependencies] portable-pty, sha2, tempfile
    └── tests/resume_sigkill.rs  # implement the gated PTY SC-002 test (un-#[ignore])

.github/workflows/ci.yml      # set CARGONAUT_PTY_TESTS=1 on the cargo test step
README.md                     # At-a-Glance metrics + Feature History row
Learnings.md                  # feature retrospective (≥3 bullets)
ROADMAP.md / issue #29        # close the deferral paper-trail
```

**Structure Decision**: Keep the existing crate seam. `cargonaut-core` owns all
`cargonaut-transfer` interaction and exposes UI-agnostic projections; `cargonaut-ui-tui`
renders and routes keys but never names a transfer type. This is exactly how `ProgressView` /
`active_progress()` already work, so resume follows the established pattern rather than
introducing a new dependency edge.

## Phase 0 — Research

See [research.md](./research.md). Key decisions:

- **R-001** Resume seam location: in `cargonaut-core::App`, projected to the UI (mirrors
  `ProgressView`). UI does not depend on transfer types.
- **R-002** Deterministic mid-transfer kill: opt-in env throttle in the engine
  (`CARGONAUT_TRANSFER_THROTTLE_MIBPS`), no-op when unset.
- **R-003** PTY harness: `portable-pty` dev-dependency; binary located via
  `env!("CARGO_BIN_EXE_cargonaut")`; SIGKILL via the child handle (Unix kill).
- **R-004** Scan scope: both launch directories, non-recursive (per clarify).
- **R-005** Offer/dialog index synchronization: rebuild the dialog from remaining offers
  after each choice so indices never drift.
- **R-006** Test sizing: modest file + 1 MiB checkpoint interval + throttle so the run lasts
  several seconds; kill at ~1 s; total test well under the CI step budget.

## Phase 1 — Design

- [data-model.md](./data-model.md) — `ResumeOfferView`, `pending_resumes`, the resume state
  machine, and how it interleaves with the existing transfer registry.
- [contracts/resume-seam.md](./contracts/resume-seam.md) — the new `cargonaut-core` public API
  surface and behavioral contract for each method (the testable seam).
- [quickstart.md](./quickstart.md) — running the gated PTY test locally and in CI, plus the
  manual smoke procedure.

## Complexity Tracking

No constitution violations require justification. The throttle env hook is the only
production-code addition motivated by testability; it is documented (R-002), gated by an unset
default, and carries no runtime cost in normal operation.

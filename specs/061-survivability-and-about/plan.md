# Implementation Plan: Survivability, Crash Safety & About/Version Surface

**Branch**: `061-survivability-and-about` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/061-survivability-and-about/spec.md`

## Summary

Make the TUI file manager survive faults gracefully. Today `[profile.release]`
sets `panic = "abort"`, so any panic instantly `SIGABRT`s the process and skips
the terminal teardown in `cargonaut_ui_tui::run` (`lib.rs:77-81`), stranding the
user in a scrambled raw-mode terminal; there is no panic hook anywhere.

The approach has three coordinated parts:

1. **Capture-in-hook, decide-at-catch.** Flip the release profile to
   `panic = "unwind"` so `catch_unwind` works. A global panic hook does only
   *safe, thread-agnostic* work: it captures the panic (message, location,
   `std::backtrace::Backtrace::force_capture()`, a snapshot of a recent-action
   ring buffer) into a process-global slot and emits a `tracing::error!`. It
   never touches the terminal and never writes files — so it is safe to fire on a
   background worker thread without disturbing the live UI. **Catch sites** then
   decide outcome: *inner* `catch_unwind` boundaries (the synchronous
   `term.draw`, the async key/mouse handler, and each spawned transfer task)
   recover — log at error level, surface a dismissible status message, continue,
   and write **no** crash file; the *outer* `catch_unwind` around the whole UI
   session in `run()` (and around startup in `main`) is reached only by a fault
   that escaped every recoverable boundary → it restores the terminal (existing
   teardown) and writes a single `crash-<ts>.log`. "Fatal vs recovered" is thus
   purely structural — which boundary caught it.

2. **Actionable diagnostics.** The crash report is a self-contained, redacted,
   human-readable file in the existing data dir (`~/.local/share/cargonaut/`)
   carrying version, OS/arch, timestamp, panic message + location, backtrace, and
   the recent-action trail (context the WARN-only `debug.log` cannot provide). On
   exit after a fatal crash the path is printed to the restored terminal; on the
   next launch a one-time "a crash report was saved at …" notice surfaces if
   unseen. Reports are retained bounded (newest N).

3. **Identity surface.** Enrich the existing stubbed F1 Help "About" section AND
   add a dedicated menu-reachable About view, both showing name / version /
   author / copyright / license; extend the clap `--version` long output with the
   copyright + license line.

## Technical Context

**Language/Version**: Rust (workspace `rust-version = 1.76`; dev host nightly-default)

**Primary Dependencies**: ratatui + crossterm (TUI), tokio (multi-thread async),
clap (CLI), tracing/tracing-subscriber (logging), thiserror. **No new runtime
dependencies planned** — diagnostics use `std::backtrace` (stable since 1.65) and
`std::panic`. `portable-pty` (already present for the PTY harness) backs the
integration test.

**Storage**: Plain files under the XDG data dir `~/.local/share/cargonaut/`
(`debug.log` already lives here; add `crash-<ts>.log` + a `crash-seen` marker).

**Testing**: `cargo test` (unit + integration); gated PTY integration tests via
`CARGONAUT_PTY_TESTS=1` (Feature 037 pattern).

**Target Platform**: Linux terminal.

**Project Type**: Single Rust workspace, 6 crates (desktop/CLI TUI app).

**Performance Goals**: No perf regression. Recovery adds at most one
`catch_unwind` per frame/event (negligible). Ring-buffer record is an O(1) push
on a bounded `VecDeque` behind a `Mutex`.

**Constraints**: Stripped release binary MUST stay ≤ 8 MiB (NFR-001); currently
≈ 2.97 MiB. `panic = "unwind"` adds unwinding tables — measured in research,
expected well within headroom. Constitution §V tmpfs build rule applies.

**Scale/Scope**: 4 crates touched (bin, ui-tui, core, transfer) + root
`Cargo.toml` (profile). New `cargonaut-core::diag` module; new
`ActiveDialog::About`; one new `Command` variant; hidden test-only panic
injection point.

## Constitution Check

*GATE: must pass before Phase 0 and re-checked after Phase 1.*

- **I. Code Quality (NON-NEGOTIABLE)** — PASS (plan). New public items in
  `cargonaut-core` get `///` docs (`#![warn(missing_docs)]`); `clippy -D
  warnings` and `cargo fmt --check` enforced. No `broken-intra-doc-links`.
- **II. Test-First (NON-NEGOTIABLE)** — PASS (plan). Every FR gets a red→green
  test pair; every SC gets a CI gate (mapping below). Core-crate coverage stays
  ≥ 80 % (NFR-007) — the diag module is pure and heavily unit-tested.
- **III. UX Consistency** — PASS (plan). The About menu entry maps to a
  `Command`; any new keybinding lands in `design/contracts/keymap.toml` first;
  `HELP_SECTIONS` keeps covering all bindings (`help_covers_all_keymap_bindings`).
- **IV. Performance (NON-NEGOTIABLE)** — PASS (plan). No tracked bench affected;
  recovery overhead negligible and off the benched paths.
- **V. SSD Preservation (NON-NEGOTIABLE)** — PASS. All builds via `make`
  (`check-tmpfs` guard); no `cargo clean` / `rm -rf target`.

**SC → gate mapping** (Constitution §II):

| SC | Gate |
|----|------|
| SC-001 terminal restored after crash | gated PTY test asserts cooked mode post-exit |
| SC-002 crash report exists w/ version+platform+location+backtrace | gated PTY test asserts file + content; unit test on formatter |
| SC-003 recoverable fault keeps session alive | run-loop test injects a render panic, asserts loop continues |
| SC-004 background-task fault isolated | transfer registry test: panicking task → job Failed, app usable |
| SC-005 version/author/copyright/license in-app + CLI | unit test on About lines; CLI `--version` test |
| SC-006 report locates failing source | inspection + formatter unit test (location present) |
| SC-007 binary ≤ 8 MiB | `scripts/check-binary-size.sh` (existing CI gate) |
| SC-008 no credentials in report | unit test: formatter omits/redacts secret-bearing fields |
| SC-009 one-time next-launch notice | unit test on the seen-marker logic |

No violations → Complexity Tracking left empty.

## Project Structure

### Documentation (this feature)

```text
specs/061-survivability-and-about/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions (abort→unwind, backtrace, redaction, injection)
├── data-model.md        # Phase 1 — CapturedPanic, RecentActionBuffer, CrashReport, AboutInfo
├── quickstart.md        # Phase 1 — validation (PTY crash test, About, --version)
├── contracts/           # Phase 1 — module/CLI/file-format contracts
│   ├── diag-api.md
│   ├── crash-report-format.md
│   └── cli-and-about.md
└── checklists/requirements.md
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   └── src/
│       ├── diag.rs            # NEW: recent-action ring buffer, captured-panic slot,
│       │                      #      crash-report formatter (pure), crash-file IO +
│       │                      #      retention + next-launch seen-marker (dir injectable),
│       │                      #      AboutInfo. Heavily unit-tested.
│       ├── command.rs         # +Command::ShowAbout
│       ├── app.rs             # dispatch() records each command into the ring buffer
│       └── lib.rs             # `pub mod diag;`
├── cargonaut-ui-tui/
│   └── src/
│       ├── lib.rs             # run(): outer catch_unwind + write fatal crash file;
│       │                      # run_loop(): inner catch_unwind around term.draw and the
│       │                      # async key/mouse handler → recover+status; ActiveDialog::About
│       ├── dialog.rs          # enrich HELP_SECTIONS "About"; AboutDialog widget + render
│       └── chrome.rs          # add About entry to the menu bar
├── cargonaut-transfer/
│   └── src/job.rs             # wrap the spawned transfer body so a panic → job Failed (FR-008)
└── cargonaut-bin/
    └── src/main.rs            # install panic hook early; clap long_version w/ copyright;
                               # outer startup catch; next-launch crash notice; honor
                               # CARGONAUT_PANIC_INJECT test hook

Cargo.toml                     # [profile.release] panic = "abort" → "unwind"
design/contracts/keymap.toml   # only if an About keybinding is added
```

**Structure Decision**: Single workspace. Pure, testable diagnostics logic
concentrates in `cargonaut-core::diag` (where ≥ 80 % coverage is required and is
easy to hit with unit tests); terminal/`catch_unwind` wiring stays in
`cargonaut-ui-tui`; process-global concerns (panic-hook install, CLI, startup)
stay in `cargonaut-bin`; task-panic isolation is a localized change in
`cargonaut-transfer`. This keeps the `catch_unwind` glue at the edges and the
logic in the well-tested core.

## Complexity Tracking

> No constitution violations — section intentionally empty.

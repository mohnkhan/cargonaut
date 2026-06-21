# Implementation Plan: Survivability Follow-ups (issue #90)

**Branch**: `062-survivability-followups` | **Date**: 2026-06-21 | **Spec**: [spec.md](./spec.md)

## Summary

Extend Feature 061's "capture-in-hook, decide-at-catch" architecture to the two
remaining interactive surfaces and add the dedicated About view:

1. **Input recovery (US1)** — wrap the async `handle_key`/`handle_mouse` calls in
   `run_loop`'s `tokio::select!` with `futures::FutureExt::catch_unwind` over
   `AssertUnwindSafe`, mirroring the render boundary: on catch, drain the
   captured panic, set a status message, continue; bounded by a consecutive-input
   -panic counter that escalates to `Error::FatalPanic` after 3. Add a
   `diag::maybe_inject_panic("input")` seam at the handler entry.
2. **Transfer task → Failed (US2)** — at the spawn sites in
   `cargonaut-transfer/src/job.rs`, wrap the task body in `catch_unwind`; on
   panic, send `TransferState::Failed { error, resumable: false }` through the
   existing `watch` channel. The sender is shared with `run_transfer` via
   `Arc<watch::Sender<_>>` (its `send` takes `&self`, so the many existing
   `state_tx.send(..)` calls compile unchanged). Guard against downgrading a job
   that already reached a terminal state. Add `maybe_inject_panic("task")`.
3. **About view (US3)** — add a UI-only keymap `Command::ShowAbout` (like
   `ShowHelp`), an `ActiveDialog::About` rendering `diag::about_lines()` with
   Esc/Enter dismissal, a render-match arm, and a menu-bar entry in `chrome.rs`.
   No `keymap.toml` binding (so help-coverage is untouched).
4. **Unwrap audit (US4)** — survey a reviewed shortlist of normal-operation hot
   paths and convert risky `unwrap`/`expect` to handled errors/log.

## Technical Context

Rust workspace; ratatui/crossterm/tokio TUI. **No new dependencies.** Reuses the
`cargonaut-core::diag` module and `cargonaut-transfer` state machine from Feature
061. Binary must stay ≤ 8 MiB (currently 4.15). Constitution §V tmpfs applies.

## Constitution Check

- **I. Code Quality** — PASS: new public items documented; clippy/fmt enforced.
- **II. Test-First** — PASS: each FR red→green; SCs gated (input-recovery test,
  transfer-Failed test, About open/close test, binary-size gate).
- **III. UX Consistency** — PASS: About is a `Command`-mapped menu entry; no new
  binding → `keymap.toml` + `help_covers_all_keymap_bindings` untouched.
- **IV. Performance** — PASS: one extra `catch_unwind` per input event
  (negligible); no benched path affected.
- **V. SSD Preservation** — PASS: builds via `make`.

**SC → gate**: SC-001 input-recovery run-loop test; SC-002 transfer registry/job
test (panicking task → `Failed`); SC-003 About dialog open/close + content test;
SC-004 `check-binary-size.sh` + full suite.

## Project Structure

```text
crates/cargonaut-ui-tui/src/
  lib.rs       # input catch_unwind boundary + counter; ActiveDialog::About + render + dispatch_ui_command arm
  dialog.rs    # AboutDialog widget (renders diag::about_lines())
  chrome.rs    # menu "About" entry → Command::ShowAbout
  keymap.rs    # +Command::ShowAbout (UI-only)
crates/cargonaut-transfer/src/job.rs   # Arc<watch::Sender>; spawn-body catch_unwind → Failed; maybe_inject_panic("task")
crates/cargonaut-core/src/{app,attrs,fsops}.rs (audit, US4)
```

**Structure Decision**: pure extension of Feature 061; no new modules. Recovery
glue stays at the UI/transfer edges; identity reuses `diag`.

## Complexity Tracking

> No constitution violations.

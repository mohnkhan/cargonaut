---
description: "Tasks — Feature 062 survivability follow-ups (issue #90)"
---

# Tasks: Survivability Follow-ups

**Tests**: REQUIRED (Constitution §II TDD). **Organization**: by user story.

## Phase 1 — US1: Input-handler recovery (P1)

- [X] T001 [US1] (red) Run-loop test in `crates/cargonaut-ui-tui/src/lib.rs` (TestBackend): with a one-shot injected input panic, the loop continues and the status shows "recovered from internal input error"; no crash file.
- [X] T002 [US1] (green) In `run_loop`, wrap the `handle_key`/`handle_mouse` calls (the `tokio::select!` arms) with `futures::FutureExt::catch_unwind(AssertUnwindSafe(..))`; on `Err` drain `diag::take_captured_panic()`, set status, increment a consecutive-input-panic counter, `continue`; reset on a clean event; escalate to `Error::FatalPanic` after 3. Add `diag::maybe_inject_panic("input")` at handler entry.

## Phase 2 — US2: Transfer task → Failed (P1)

- [X] T003 [US2] (red) Test in `crates/cargonaut-transfer` (job module/tests): a panicking transfer body drives the job's `watch` state to `TransferState::Failed`; the registry stays usable; a job already terminal is not downgraded.
- [X] T004 [US2] (green) In `crates/cargonaut-transfer/src/job.rs`, change `run_transfer`/`run_transfer_with_state` to take `Arc<watch::Sender<TransferState>>`; at both spawn sites wrap the body in `catch_unwind` and on panic send `Failed` (unless already terminal). Add `diag::maybe_inject_panic("task")` in the task body (transfer dev-dep on cargonaut-core's diag, or a local inject check).

## Phase 3 — US3: Dedicated About view (P2)

- [X] T005 [US3] (green) Add UI-only `Command::ShowAbout` to `crates/cargonaut-ui-tui/src/keymap.rs` (serde `show-about`).
- [X] T006 [US3] (green) Add `ActiveDialog::About` + an `AboutDialog` in `dialog.rs` rendering `diag::about_lines()` (centered), with a render-match arm in `lib.rs` and Esc/Enter dismissal.
- [X] T007 [US3] (green) Handle `Command::ShowAbout` in `dispatch_ui_command` (open the dialog); add an "About" entry to the menu bar in `chrome.rs`.
- [X] T008 [P] [US3] (red→green) Test: opening via `Command::ShowAbout` yields an About dialog whose content includes version/author/copyright/license; Esc closes it (SC-003).

## Phase 4 — US4: Unwrap audit (P3)

- [X] T009 [P] [US4] Survey `crates/cargonaut-core/src/{app,attrs,fsops}.rs` (+ `cargonaut-ui-tui/src/lib.rs`) for production `unwrap`/`expect` on normal-operation paths; convert a reviewed shortlist to handled errors/log without changing success behavior. Record the shortlist + rationale in Learnings.

## Phase 5 — Polish

- [X] T010 [P] Verify SC-004: `make build-release` + `check-binary-size.sh` (≤ 8 MiB); `CARGONAUT_PTY_TESTS=1 cargo test --workspace --lib --tests` green (incl. Feature 061 crash test).
- [X] T011 [P] Docs: README (metrics + Feature History), Learnings (≥3 bullets), CHANGELOG; close #90 + remove its ROADMAP row.
- [X] T012 [P] Final gate: `make ci-local`; clippy `-D warnings` + `fmt --check` clean.

## Dependencies
- US1, US2, US3, US4 are mutually independent (different files). Polish last.
- US1/US2 reuse Feature 061's `diag` seams (already present).

## MVP
US1 + US2 (the recovery completion) are the highest-value; US3/US4 independent.

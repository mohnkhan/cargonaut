---
description: "Task list — Feature 061 Survivability, Crash Safety & About/Version"
---

# Tasks: Survivability, Crash Safety & About/Version Surface

**Input**: Design documents from `specs/061-survivability-and-about/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: REQUIRED — Constitution §II mandates TDD (red commit before green) for
every FR and a CI gate per SC.

**Organization**: by user story (US1 P1 → US4 P2). MVP = US1.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: parallelizable (different file, no incomplete dependency)
- **[US#]**: owning user story (setup/foundational/polish have no story label)

## Path Conventions

Rust workspace. Core diag logic: `crates/cargonaut-core/src/diag.rs`. UI:
`crates/cargonaut-ui-tui/src/`. Binary: `crates/cargonaut-bin/src/main.rs`.
Transfer: `crates/cargonaut-transfer/src/job.rs`.

---

## Phase 1 — Setup

- [ ] T001 Flip `[profile.release]` `panic = "abort"` → `panic = "unwind"` in `Cargo.toml` (root); leave `opt-level="z"`, `lto="fat"`, `codegen-units=1`, `strip="symbols"` unchanged (research R1).
- [ ] T002 Create `crates/cargonaut-core/src/diag.rs` with module-level `//!` docs and add `pub mod diag;` to `crates/cargonaut-core/src/lib.rs` (keeps `#![warn(missing_docs)]` clean).

---

## Phase 2 — Foundational (blocking prerequisites for US1/US2/US3)

**Goal**: the always-safe capture core that the panic hook and every catch site share.

- [ ] T003 [P] (red) Unit tests in `crates/cargonaut-core/src/diag.rs` for the capture slot: `take_captured_panic()` is `None` initially; a set→take roundtrip returns the stored `CapturedPanic` and re-take is `None`.
- [ ] T004 (green) Implement `CapturedPanic` struct + process-global `Mutex<Option<CapturedPanic>>` + `take_captured_panic()` in `crates/cargonaut-core/src/diag.rs` (per contracts/diag-api.md).
- [ ] T005 (red) Test in `diag.rs`: after `install_panic_hook()`, a `catch_unwind`'d `panic!` populates the slot with non-empty `message`, a `location`, thread name, and non-empty `backtrace`.
- [ ] T006 (green) Implement `install_panic_hook()` in `diag.rs` — capture message/location/thread + `Backtrace::force_capture()` + recent-action snapshot, `tracing::error!`, NO terminal/file work; idempotent; suppresses default stderr dump.
- [ ] T007 [P] (green) Implement `maybe_inject_panic(site)` + `data_dir()` in `diag.rs` with unit tests (`maybe_inject_panic` is inert when `CARGONAUT_PANIC_INJECT` unset; `data_dir` honors `$XDG_DATA_HOME`).

**Checkpoint**: capture + hook compile and are unit-green; no behavior wired into the app yet.

---

## Phase 3 — User Story 1: A crash never wrecks the terminal, and leaves a clue (P1) 🎯 MVP

**Goal**: any fatal fault → clean terminal + a crash report + a pointer line.

**Independent test**: run the binary under a PTY with an injected fatal fault;
after exit the PTY is cooked and a `crash-*.log` exists (SC-001, SC-002).

- [ ] T008 [P] [US1] (red) Unit test in `diag.rs`: `format_crash_report` is deterministic for fixed inputs and contains `version`, `platform`, the `## Panic` heading with `location`, and the `## Backtrace` heading (contracts/crash-report-format.md).
- [ ] T009 [US1] (green) Implement `ReportMeta` + `format_crash_report(meta, panic)` (pure) in `diag.rs`.
- [ ] T010 [P] [US1] (red) Unit tests in `diag.rs` (tempdir) for `write_report` (creates `crash-<ts>.log` with body) and `prune_reports(dir, 10)` (keeps newest 10 by lexical name, deletes older).
- [ ] T011 [US1] (green) Implement `write_report` + `prune_reports` (default keep=10) in `diag.rs`; all IO returns `io::Result`, never panics (FR-013).
- [ ] T012 [US1] (green) In `crates/cargonaut-ui-tui/src/lib.rs` `run()`, wrap the `run_loop` await in `FutureExt::catch_unwind(AssertUnwindSafe(..))`; on panic still run the existing teardown (`restore_terminal_modes` + `disable_raw_mode` + `show_cursor`) then return a typed `Error::FatalPanic` (panic already captured by the hook).
- [ ] T013 [US1] (green) In `crates/cargonaut-bin/src/main.rs`: call `diag::install_panic_hook()` before anything else; on `Error::FatalPanic` (or a startup panic) format + `write_report` + `prune_reports`, print "crash report saved at <path>" to the restored terminal (FR-006), exit non-zero; on write failure print "could not save crash report (<reason>)" and still exit cleanly (FR-013).
- [ ] T014 [US1] (green) Add `diag::maybe_inject_panic("startup")` in `main` before UI launch and `maybe_inject_panic("render")` just before `term.draw` in `run_loop` (test seam for US1; R8).
- [ ] T015 [US1] (red) Gated PTY test `crates/cargonaut-bin/tests/crash_safety.rs` (`#![cfg(...)]` self-skip unless `CARGONAUT_PTY_TESTS=1`): spawn the real binary with `CARGONAUT_PANIC_INJECT=render` + a temp `XDG_DATA_HOME`, send one key, then assert (a) the PTY is back in cooked mode = SC-001, (b) a `crash-*.log` exists containing version/platform/location/backtrace = SC-002, (c) the printed line names the path.
- [ ] T016 [US1] (green) Make T015 pass; adjust `run()`/`main` wiring as needed.

**Checkpoint**: US1 shippable — fatal crashes are safe and diagnosable. This is the MVP.

---

## Phase 4 — User Story 2: One failure doesn't sink the whole session (P2)

**Goal**: faults while drawing, handling one input, or in a background task are contained; the session stays interactive.

**Independent test**: inject a one-shot render/input panic → loop continues with a status message; a panicking transfer task → job Failed, app usable (SC-003, SC-004).

- [ ] T017 [P] [US2] (red) Run-loop recovery test in `crates/cargonaut-ui-tui/src/lib.rs` (TestBackend): with a one-shot injected render panic, the loop continues and the status line shows a "recovered from internal error" message; no crash file written.
- [ ] T018 [US2] (green) Wrap the synchronous `term.draw(|f| ..)` in `run_loop` with `std::panic::catch_unwind(AssertUnwindSafe(..))`; on `Err` take the captured panic, `tracing::error!`, set `status`, continue; rate-limit — after N consecutive recovered render panics, escalate to fatal (write report + exit) to avoid a hot loop (R7).
- [ ] T019 [US2] (green) Wrap the async key/mouse handling in `run_loop` with `futures::FutureExt::catch_unwind(AssertUnwindSafe(..))`; recover + status as in T018; add `maybe_inject_panic("input")` at the handler entry.
- [ ] T020 [P] [US2] (red) Transfer isolation test in `crates/cargonaut-transfer/src/job.rs` (or `tests/`): a panicking transfer body resolves its job to `Failed` (not a hung/missing job); the registry remains usable.
- [ ] T021 [US2] (green) In `crates/cargonaut-transfer/src/job.rs`, wrap the spawned transfer body so a panic is caught and the job transitions to `Failed` (FR-008); add `maybe_inject_panic("task")` in the task body.

**Checkpoint**: US2 shippable — the named recovery surfaces no longer kill the app.

---

## Phase 5 — User Story 3: Crash reports a developer can act on (P2)

**Goal**: the recent-action trail enriches reports; next launch surfaces an unseen report once; reports stay secret-free and bounded.

**Independent test**: crash after a known action sequence → report lists those actions; relaunch shows a one-time notice; a configured secret never appears (SC-005-context, SC-008, SC-009).

- [ ] T022 [P] [US3] (red) Ring-buffer tests in `diag.rs`: capacity 64 drops oldest; `recent_actions()` returns oldest-first; `seq` increments.
- [ ] T023 [US3] (green) Implement `ActionRecord` + `RecentActionBuffer` (`record_action`, `recent_actions`) in `diag.rs`.
- [ ] T024 [US3] (green) Call `diag::record_action(<variant>, <coarse detail>)` from `App::dispatch` in `crates/cargonaut-core/src/app.rs` for each `Command`; detail is secret-free (pane id / index only) per FR-015.
- [ ] T025 [US3] (green) Snapshot `recent_actions()` into `CapturedPanic` in the hook (T006) and render a `## Recent actions` section in `format_crash_report` (T009); update its unit test (FR-005).
- [ ] T026 [P] [US3] (red) SC-008 redaction test in `diag.rs`/bin: with a sentinel "password" recorded only through legitimate paths, a formatted report contains no occurrence of the sentinel.
- [ ] T027 [P] [US3] (red) Next-launch notice tests in `diag.rs` (tempdir): `unseen_report` returns the newest report when no/older marker, `None` after `mark_seen`; fires exactly once (SC-009).
- [ ] T028 [US3] (green) Implement `unseen_report` + `mark_seen` (`crash-seen` marker) in `diag.rs`.
- [ ] T029 [US3] (green) In `main.rs` startup, if `diag::unseen_report(data_dir)` is `Some`, surface a one-time status notice naming the path and call `mark_seen` (FR-006a).

**Checkpoint**: US3 shippable — reports are actionable and the notice is reliable.

---

## Phase 6 — User Story 4: Knowing what you're running and who made it (P2)

**Goal**: version/author/copyright/license visible in F1 Help, a dedicated About dialog, and CLI `--version` long output.

**Independent test**: open help About + About dialog + run `--version`; all show the same identity details (SC-005).

- [ ] T030 [P] [US4] (red) `about_lines()` test in `diag.rs`: output contains the version, author, copyright (`© 2024–2026 Mohiuddin Khan Inamdar`), and `MIT OR Apache-2.0`.
- [ ] T031 [US4] (green) Implement `AboutInfo` + `about()` + `about_lines()` in `diag.rs`.
- [ ] T032 [US4] (green) Enrich the `HELP_SECTIONS` "About" entry in `crates/cargonaut-ui-tui/src/dialog.rs` to render `about_lines()`; keep `help_covers_all_keymap_bindings` green.
- [ ] T033 [US4] (green) Add `Command::ShowAbout` in `crates/cargonaut-core/src/command.rs`; add `ActiveDialog::About` + an `AboutDialog` render + Esc/Enter lifecycle in `crates/cargonaut-ui-tui/src/lib.rs`/`dialog.rs`.
- [ ] T034 [US4] (green) Add an "About" entry to the menu bar in `crates/cargonaut-ui-tui/src/chrome.rs` mapping to `Command::ShowAbout` (keybinding, if any, added to `design/contracts/keymap.toml` first).
- [ ] T035 [US4] (green) In `crates/cargonaut-bin/src/main.rs`, set `#[command(version, long_version = LONG_VERSION)]` with a `concat!` `LONG_VERSION` const (version + copyright + license + repo) built to match `about_lines()`.
- [ ] T036 [P] [US4] (red→green) Tests: help-About content (ui-tui), About dialog open/close (ui-tui), and `--version` long output contains the copyright (`crates/cargonaut-bin/tests/`), satisfying SC-005.

**Checkpoint**: US4 shippable independently.

---

## Phase 7 — Polish & Cross-Cutting

- [ ] T037 [P] FR-009 audit: survey production (non-test) `unwrap()`/`expect()` in hot paths (`crates/cargonaut-core/src/{app,attrs,fsops}.rs`, `cargonaut-ui-tui/src/lib.rs`) and convert risky ones to handled errors/log; record scope in Learnings.
- [ ] T038 [P] Verify SC-007: `make build-release && bash scripts/check-binary-size.sh`; record the `panic=unwind` size delta in `Learnings.md`.
- [ ] T039 [P] Docs: update `README.md` (At-a-Glance metrics + Feature History) and `Learnings.md` (≥3 bullets: capture-in-hook/decide-at-catch, abort→unwind tradeoff, AssertUnwindSafe caveat); update `CHANGELOG.md`; update `ROADMAP.md` if anything is deferred.
- [ ] T040 [P] Final gate: `make ci-local` + `CARGONAUT_PTY_TESTS=1 cargo test --workspace --lib --tests`; clippy `-D warnings` and `cargo fmt --check` clean.

---

## Dependencies & Execution Order

- **Setup (P1-T002)** → **Foundational (T003–T007)** → user stories.
- **US1 (T008–T016)** depends on Foundational. **MVP stops here.**
- **US2 (T017–T021)** depends on Setup T001 (unwind) + Foundational; independent of US1's report code but shares the capture slot.
- **US3 (T022–T029)** depends on Foundational (hook) + US1 (report formatter to extend); ring buffer is independent.
- **US4 (T030–T036)** depends only on Foundational T002 (diag module exists); otherwise fully independent — could ship first.
- **Polish (T037–T040)** last.

## Parallel Opportunities

- Foundational: T003 ∥ T007.
- US1: T008 ∥ T010 (different test fns, same file — serialize commits but parallel authoring).
- US4 is parallelizable against US1–US3 (disjoint files except `diag.rs` additions).
- Polish: T037 ∥ T038 ∥ T039 ∥ T040 (distinct concerns).

## Implementation Strategy

1. **MVP**: Setup + Foundational + US1 → ship crash-safety (the highest-value, most-visible fix).
2. Add US2 (recovery) once US1's clean-exit backstop exists.
3. Add US3 (rich diagnostics) and US4 (About) — independent, either order.
4. Polish: unwrap audit, size verify, docs, full CI.

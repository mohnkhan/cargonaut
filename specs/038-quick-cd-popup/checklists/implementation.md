# Implementation Checklist: Quick-CD Popup with Tab-Completion

**Feature**: 038-quick-cd-popup | **Created**: 2026-06-15
**Drives**: `/speckit-implement`. Check items as they land. Each behavioral item
maps to a task (T###) and an FR/SC.

## Pre-flight

- [ ] CH01 `make tmpfs-status` shows `target/` linked to tmpfs (Constitution §V) [T001]
- [ ] CH02 Clean baseline: `make build && make test` green before changes [T001]
- [ ] CH03 No new crate dependencies added (Cargo.toml diff empty) [T001, NFR-001]

## Foundational — shared widget & open/render seam

- [ ] CH04 `PathInputDialog` + `PathInputAction` added to `dialog.rs` with doc comments [T002/T003, §I]
- [ ] CH05 `new(title, prompt, initial)` prefills buffer, cursor at end [T003, FR-014/W1]
- [ ] CH06 Char/Backspace edit buffer; render uses `theme.dialog_style()`, `Clear` first [T003, FR-002]
- [ ] CH07 `ActiveDialog::QuickCd { widget }` variant added; render arm wired [T004, FR-001]
- [ ] CH08 `Command::QuickCdPopup` opens the dialog prefilled with active cwd display; `mode=Dialog` [T004, FR-014]
- [ ] CH09 Old core status-stub for `QuickCdPopup` removed; variant retained; stub-asserting test updated [T005]

## US1 (P1, MVP) — type a path → navigate

- [ ] CH10 (red committed before green) core tests for resolver + `quick_cd` [T006, §II]
- [ ] CH11 Path resolver handles absolute / URI / relative / `..` / trailing `/` [T007, FR-012/R-003]
- [ ] CH12 `App::quick_cd` routes through `navigate_to`; returns events [T007, FR-004]
- [ ] CH13 Successful accept records previous cwd in `dir_history_back` [T006/T007, FR-005]
- [ ] CH14 Inactive pane unchanged by any accept [T006/T007, FR-013]
- [ ] CH15 Empty/whitespace accept = no-op, prompt stays open [T006, US3#3]
- [ ] CH16 Enter→`Submit`, Esc→`Cancel` widget behavior tested [T008/T009, FR-010]
- [ ] CH17 Event-loop QuickCd branch: Submit(Ok)→apply+close; Cancel→close [T010, FR-001/FR-010]
- [ ] CH18 Manual smoke: Alt-c → type real path → Enter moves active pane [T011, SC-001]

## US2 (P2) — Tab-completion

- [ ] CH19 (red before green) core tests for `complete_cd` ordering/filtering [T012, §II]
- [ ] CH20 `complete_cd` returns dirs only (files + missing excluded) [T013, FR-008/SC-003]
- [ ] CH21 Recent-dir matches ordered first; filesystem children after; de-duplicated [T013, FR-008]
- [ ] CH22 Unique prefix ⇒ single candidate (one-keystroke complete) [T012, SC-002]
- [ ] CH23 No match ⇒ empty Vec (drives "(no matches)") [T012/T013, FR-009]
- [ ] CH24 (red before green) widget completion-cycle tests [T014, §II]
- [ ] CH25 Tab stale-cache ⇒ `RequestCompletions`; fresh ⇒ cycle+wrap [T015, FR-007/W3]
- [ ] CH26 `apply_completions`: non-empty sets buffer+fresh cache; empty sets note [T015, W4]
- [ ] CH27 Edit invalidates cache (next Tab re-requests) [T015, R-005]
- [ ] CH28 Event-loop handles `RequestCompletions` via `complete_cd` + `apply_completions` [T016, FR-007]
- [ ] CH29 Manual smoke: Tab completes unique; cycles multiple [T017]

## US3 (P3) — cancel & bad-path recovery

- [ ] CH30 (red before green) core tests: bad path ⇒ `Err`, state unchanged [T018, FR-006/SC-005]
- [ ] CH31 `quick_cd` error path leaves cwd + history byte-for-byte unchanged [T019, SC-004/Q3]
- [ ] CH32 (red before green) widget `set_error` render + clear-on-edit tests [T020, §II]
- [ ] CH33 `set_error` shows inline error; keeps dialog open; cleared on edit [T021, FR-006/W7]
- [ ] CH34 Event-loop Submit(Err)→`set_error`, dialog stays open [T022, FR-006]
- [ ] CH35 One modal at a time confirmed (no stacking on Alt-c while open) [T022, FR-011]
- [ ] CH36 Manual smoke: bad path stays open w/ error; Esc closes clean [T023]

## Gates, docs & close-out

- [ ] CH37 SC-006 injected-input E2E test (accept + cancel + error-recovery) green [T024, SC-006]
- [ ] CH38 `make ci-local` green: clippy `-D warnings`, fmt, test, release build, docs [T025, §I]
- [ ] CH39 README "At a Glance" + Feature History updated [T026, docs gate]
- [ ] CH40 Learnings.md ≥3 bullets (async-completion seam, navigate_to reuse, shared widget) [T027, docs gate]
- [ ] CH41 ROADMAP.md #31 marked resolved; #32/#33 noted unblocked [T028]
- [ ] CH42 PR opened to `main`; CI green; issue #31 closed via PR [T029]

## Constitution sign-off

- [ ] CH43 §I no `unsafe`, missing_docs clean, fmt/clippy pass
- [ ] CH44 §II every FR has a `(red)`→`(green)` commit pair in history
- [ ] CH45 §III dialog reuses shared `dialog.rs` widget; keymap unchanged (single source); themed render
- [ ] CH46 §IV no perf-gate regression (no heavy deps; completion off the render path)
- [ ] CH47 §V no forbidden `cargo clean`/`rm -rf target`; tmpfs intact

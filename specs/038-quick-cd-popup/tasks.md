# Tasks: Quick-CD Popup with Tab-Completion

**Input**: Design documents from `specs/038-quick-cd-popup/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/quick-cd-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR gets
a red→green pair; git history MUST show `(red)` before `(green)`. SC-006 is the
gating injected-input test.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files, no ordering dependency)
- **[Story]**: US1 / US2 / US3, or FOUND/SETUP/POLISH
- File paths are exact.

## Conventions

- Core crate: `crates/cargonaut-core/src/lib.rs`
- Dialog widget: `crates/cargonaut-ui-tui/src/dialog.rs`
- Event loop: `crates/cargonaut-ui-tui/src/lib.rs`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands
  the implementation. `[P]` tests within a phase may be authored together.

---

## Phase 1: Setup

- [ ] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). No new dependencies are
  added by this feature — verify `Cargo.toml` files are untouched.

---

## Phase 2: Foundational (Blocking — shared widget + open/render wiring)

**Purpose**: The shared `PathInputDialog` skeleton and the open/render seam that
every user story builds on. The widget is authored here to be reusable by #32/#33
(research R-006).

**⚠️ No user-story phase can start until this is complete.**

- [ ] T002 [FOUND] (red) In `crates/cargonaut-ui-tui/src/dialog.rs`, add failing
  unit tests for the new `PathInputDialog`: `new(title, prompt, initial)` ⇒
  `value() == initial`; `Char`/`Backspace` edit the buffer (contract W1, W2); a
  `TestBackend` render shows the title and the prefilled text. Add
  `PathInputAction` enum (`Consumed | Edited | RequestCompletions{text} |
  Submit(String) | Cancel`).
- [ ] T003 [FOUND] (green) Implement `PathInputDialog` struct + `new`/`value`/
  `handle_key` (Char/Backspace/Enter→Submit/Esc→Cancel; Tab stubbed to
  `RequestCompletions` for now) + `render` (modal, `theme.dialog_style()`,
  `Clear` first — mirror `TextInputDialog`). Make T002 pass. `#![warn(missing_docs)]`
  clean.
- [ ] T004 [FOUND] In `crates/cargonaut-ui-tui/src/lib.rs`: add
  `ActiveDialog::QuickCd { widget: PathInputDialog }`; in `dispatch_ui_command`
  handle `Command::QuickCdPopup` by opening the dialog prefilled with
  `app.active_pane_state().cwd.display()` and setting `mode = Mode::Dialog`
  (remove the old "not yet available"/status path for this command); add the
  render arm `ActiveDialog::QuickCd { widget } => widget.render(...)`. (Key
  handling for the new variant lands per-story below; until then the branch may
  just close on Esc.)
- [ ] T005 [FOUND] In `crates/cargonaut-core/src/lib.rs`, stop emitting the
  `QuickCdPopup` status stub — the command is now opened UI-side. Remove the
  `"QuickCd popup not yet implemented (T1.25 stub)"` placeholder and its enum doc
  note; keep the `QuickCdPopup` variant. Adjust/remove any test asserting the
  stub status.

**Checkpoint**: Alt-c opens an empty-but-prefilled modal that renders and closes
on Esc. No navigation/completion yet.

---

## Phase 3: User Story 1 — Jump to a directory by typing its path (P1) 🎯 MVP

**Goal**: Alt-c → prefilled prompt → type a path → Enter navigates the active
pane (FR-001, FR-002, FR-004, FR-005, FR-012, FR-013, FR-014).

**Independent Test**: `quick_cd` with a valid absolute/relative path changes the
active pane cwd and records history; inactive pane unchanged.

- [ ] T006 [US1] (red) In `crates/cargonaut-core/src/lib.rs` tests, add failing
  `#[tokio::test]`s with `TempDir` fixtures for the path resolver + `quick_cd`:
  (a) absolute path navigates active pane; (b) relative path resolves against
  active cwd; (c) `..` segment ascends; (d) trailing `/` ignored; (e) successful
  nav pushes previous cwd to `dir_history_back` (FR-005); (f) inactive pane
  unchanged (FR-013); (g) empty/whitespace input ⇒ `Ok(no events)`, no nav
  (US3 #3 — colocated here as it's part of accept). Asserts on `app.pane(active).cwd`.
- [ ] T007 [US1] (green) Implement the path-resolution helper (research R-003)
  and `App::quick_cd(&mut self, path_text) -> Result<Vec<Event>, AppError>`
  routing through the existing `navigate_to` (contract Q1–Q4). Make T006 pass.
- [ ] T008 [US1] (red) In `dialog.rs`, add a failing test that `Enter` returns
  `PathInputAction::Submit(value)` and `Esc` returns `Cancel` (W5, W6).
- [ ] T009 [US1] (green) Ensure `handle_key` satisfies T008 (mostly from T003);
  fill any gap. 
- [ ] T010 [US1] In `crates/cargonaut-ui-tui/src/lib.rs`, wire the
  `ActiveDialog::QuickCd` key branch in `handle_key`: on `Submit(text)` →
  if `text.trim().is_empty()` no-op (stay open); else `app.quick_cd(&text).await`
  → `Ok` apply events + close + `Mode::Pane`; (error handling completed in US3).
  On `Cancel` → close + `Mode::Pane`.
- [ ] T011 [US1] Manual smoke per quickstart steps 1–4 (build, Alt-c, type a real
  path, Enter); confirm active pane moved and history back-entry exists.

**Checkpoint**: MVP — keyboard-only directory jump by typed path works end to end.

---

## Phase 4: User Story 2 — Tab-completion (P2)

**Goal**: Tab completes/cycles the final path segment against the active pane's
VFS directories + recent-dir history, recent-first, dirs-only (FR-007, FR-008,
FR-009).

**Independent Test**: `complete_cd` returns correctly-ordered, dir-only,
de-duplicated candidates; widget cycles a fresh cache and re-requests a stale one.

- [ ] T012 [US2] (red) In `crates/cargonaut-core/src/lib.rs` tests, add failing
  `#[tokio::test]`s for `complete_cd`: (a) unique prefix ⇒ single candidate
  (SC-002); (b) multiple matches ⇒ all returned, filesystem children in sort
  order; (c) a matching `dir_history_back` entry appears **first** (FR-008/C3);
  (d) files and a non-existent dir are excluded (SC-003/C1, C5); (e)
  de-duplicated (C2); (f) no match ⇒ empty Vec (FR-009).
- [ ] T013 [US2] (green) Implement `App::complete_cd(&self, partial) ->
  Vec<String>` (research R-004; split into dir-prefix + last segment, list via
  `local_fs.list`, filter `VfsKind::Dir` + prefix, merge recent dirs recent-first,
  dedup). Make T012 pass.
- [ ] T014 [US2] (red) In `dialog.rs`, add failing tests for completion cycling
  (contract W3, W4): Tab on a stale cache ⇒ `RequestCompletions{text}`;
  `apply_completions(vec![a,b,c])` sets buffer to `a` and marks fresh; subsequent
  Tab ⇒ `Consumed` and buffer advances `a→b→c→a` (wrap); editing invalidates the
  cache (next Tab re-requests); `apply_completions(vec![])` sets a "(no matches)"
  note and leaves the buffer unchanged.
- [ ] T015 [US2] (green) Implement completion-cache fields + `apply_completions`
  + Tab logic in `PathInputDialog`. Make T014 pass.
- [ ] T016 [US2] In `crates/cargonaut-ui-tui/src/lib.rs`, handle
  `RequestCompletions{text}` in the QuickCd key branch: `let c =
  app.complete_cd(&text).await; widget.apply_completions(c);`.
- [ ] T017 [US2] Manual smoke per quickstart step 3 (Tab completes unique;
  Tab cycles multiple).

**Checkpoint**: Tab-completion works against filesystem + recent dirs.

---

## Phase 5: User Story 3 — Cancel & bad-path recovery (P3)

**Goal**: Esc cancels with zero side effects; invalid Enter keeps the prompt open
with an inline error; one modal at a time (FR-006, FR-010, FR-011; SC-004, SC-005).

**Independent Test**: cancel leaves both panes identical to pre-open; accept of a
bad path returns `Err`, no nav, error shown, prompt stays open.

- [ ] T018 [US3] (red) In `crates/cargonaut-core/src/lib.rs` tests, add failing
  `#[tokio::test]`s: (a) `quick_cd` on a non-existent path ⇒ `Err`, active cwd
  unchanged, history unchanged (FR-006/C-Q3, SC-005); (b) `quick_cd` on a path
  that is a file ⇒ `Err`, no nav; (c) cancel-equivalent: opening + not calling
  `quick_cd` leaves state untouched (SC-004) — assert via no state mutation from
  `complete_cd` (read-only).
- [ ] T019 [US3] (green) Confirm `quick_cd`'s error path leaves state unchanged
  (inherited from `navigate_to` listing-before-mutate). Add any guard needed to
  ensure no partial mutation. Make T018 pass.
- [ ] T020 [US3] (red) In `dialog.rs`, add failing tests: `set_error("…")` makes
  the next render include the error text; a subsequent `Char`/`Backspace` clears
  the error (W7).
- [ ] T021 [US3] (green) Implement `set_error` + error/`note` rendering + clear-on-
  edit in `PathInputDialog`. Make T020 pass.
- [ ] T022 [US3] In `crates/cargonaut-ui-tui/src/lib.rs`, complete the QuickCd
  `Submit` branch: on `Err(e)` from `quick_cd`, call `widget.set_error(e.to_string())`
  and **leave the dialog open** (do not reset `active_dialog`/`mode`). Confirm
  `dispatch_ui_command` cannot stack a second modal while one is open (FR-011 —
  it's already guarded by the `active_dialog.is_some()` dialog branch returning
  early in `handle_key`; add a focused assertion/comment).
- [ ] T023 [US3] Manual smoke per quickstart steps 5–6 (bad path stays open with
  error; Esc closes clean).

**Checkpoint**: Full feature behavior per spec, all FRs covered.

---

## Phase 6: Polish, gates & docs

- [ ] T024 [POLISH] SC-006 end-to-end core test: a single `#[tokio::test]` that
  drives open→type→`complete_cd`→`quick_cd` (accept) AND a cancel path AND an
  error-recovery path (accept a bad path ⇒ `Err`, then accept a valid path ⇒
  `Ok`), asserting the success outcome, the zero-side-effect cancel, and that the
  bad-path accept did not mutate state. This is the named injected-input gate
  (T1.25 origin). Symlink-to-dir targets are inherited from `navigate_to` and not
  separately exercised here (noted, not tested).
- [ ] T025 [POLISH] `make ci-local` green: clippy `-D warnings`, `cargo fmt
  --check`, `cargo test --workspace`, release build, doc build. Fix any lint.
- [ ] T026 [POLISH] [P] Update `README.md`: "At a Glance" metrics (test count,
  feature count, binary size) + a "Feature History" one-line entry for Feature
  038. (MANDATORY docs gate.)
- [ ] T027 [POLISH] [P] Update `Learnings.md`: ≥3 bullets — the async-completion-
  in-sync-widget seam (R-005), reuse of `navigate_to` for free history+validation
  (R-002), and the shared-widget-for-#32/#33 decision (R-006).
- [ ] T028 [POLISH] [P] Update `ROADMAP.md`: move #31 to resolved/done with a note
  that #32 and #33 are now unblocked (shared `PathInputDialog` shipped).
- [ ] T029 [POLISH] Close issue #31 referencing the PR; confirm #32/#33 issue
  notes mention the now-available shared widget.

---

## Dependencies & ordering

- **Setup (T001)** → **Foundational (T002–T005)** → user stories.
- **US1 (T006–T011)** is the MVP and depends only on Foundational.
- **US2 (T012–T017)** depends on Foundational; independent of US1 logic but
  shares the QuickCd key branch (T010 before T016 to avoid churn).
- **US3 (T018–T023)** depends on US1 (extends the `Submit` branch from T010/T022).
- **Polish (T024–T029)** last; T024 depends on US1+US2 logic existing.

## Parallelization notes

- Within a `(red)` step, the core-test file and the dialog-test file are different
  files → their red authoring is `[P]`-friendly, but keep each red→green pair
  sequential.
- T026/T027/T028 touch different files (`README.md`/`Learnings.md`/`ROADMAP.md`)
  → `[P]`.

## Coverage map (FR → tasks)

| FR | Tasks |
|----|-------|
| FR-001 open prompt | T004, T010 |
| FR-002 edit text | T002/T003, T008/T009 |
| FR-003 modal captures keys | T004, T010, T022 |
| FR-004 accept navigates | T006/T007, T010 |
| FR-005 history updated | T006/T007 |
| FR-006 invalid → open+error | T018/T019, T020/T021, T022 |
| FR-007 Tab cycle+wrap | T014/T015 |
| FR-008 candidate sources/order | T012/T013 |
| FR-009 no-match feedback | T012/T013, T014/T015 |
| FR-010 Esc cancel no side effects | T008/T009, T010, T018 |
| FR-011 one modal at a time | T022 |
| FR-012 relative vs absolute | T006/T007 |
| FR-013 active pane only | T006/T007 |
| FR-014 prefill cwd | T002/T003, T004 |
| SC-001 keyboard-only nav (MVP) | T006/T007, T010, T011, T024 |
| SC-002 one-keystroke unique | T012 |
| SC-003 dirs-only candidates | T012/T013 |
| SC-004 cancel zero side effects | T018, T024 |
| SC-005 invalid never navigates | T018/T019 |
| SC-006 injected-input E2E gate | T024 |

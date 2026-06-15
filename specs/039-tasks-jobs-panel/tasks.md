---
description: "Task list for Feature 039 — Tasks/Jobs Panel Popup"
---

# Tasks: Tasks/Jobs Panel Popup

**Input**: Design documents from `specs/039-tasks-jobs-panel/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: REQUIRED. Constitution §II (NON-NEGOTIABLE) mandates TDD — every
functional task is authored as a failing test first, then the implementation that
makes it pass. Git history MUST show the red commit before the green commit
(e.g. `T004 (red): …` → `T004 (green): …`). Test and impl are listed as separate
tasks so the red→green boundary is explicit.

**Organization**: Grouped by user story (US1 view, US2 cancel, US3 pause/resume)
so each ships as an independently testable increment.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files / no dependency on incomplete tasks)
- **[Story]**: US1 / US2 / US3 (setup, foundational, polish carry no story label)

## Path Conventions

- Core: `crates/cargonaut-core/src/lib.rs`, `crates/cargonaut-core/tests/`
- TUI: `crates/cargonaut-ui-tui/src/dialog.rs`, `crates/cargonaut-ui-tui/src/lib.rs`
- Keymap: `design/contracts/keymap.toml`

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm the working environment and existing seams before changes.

- [ ] T001 Confirm tmpfs target is active (`make tmpfs-status`) and the baseline is green (`make test`) before touching code — Constitution §V.
- [ ] T002 Verify the F12 binding maps to `ShowTasksPanel` in `design/contracts/keymap.toml` and check whether a `:jobs` command entry exists; record the finding in the PR (R-006). If `:jobs` resolves to the same command, no keymap change is needed.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: The UI-agnostic read seam every user story renders from. No story can
begin until `job_views()` and its projection types exist.

**⚠️ CRITICAL**: Blocks US1, US2, and US3.

- [ ] T003 Write failing unit tests for the registry projection in `crates/cargonaut-core/src/lib.rs` (`#[cfg(test)]`): empty registry → `job_views()` returns `[]`; after two copies → two `JobView`s in submit order with `Running`/`Queued` status; a job whose token was cancelled (and not paused) classifies as `JobStatus::Cancelled`. (RED)
- [ ] T004 Implement `JobStatus`, `JobView` (with doc comments), add `paused: HashSet<TransferId>` to `App` (initialize in `App::new`), and `pub fn job_views(&self) -> Vec<JobView>` per `contracts/core-api.md` / `data-model.md`; change the core `ShowTasksPanel` dispatch arm to a no-op (`Ok(vec![])`) like `QuickCdPopup`; delete the old `show_tasks_panel_emits_status_with_transfer_count` stub test. Make T003 pass. (GREEN) — `crates/cargonaut-core/src/lib.rs`

**Checkpoint**: Core exposes a stable, tested read projection of all jobs.

---

## Phase 3: User Story 1 — See what transfers are running (Priority: P1) 🎯 MVP

**Goal**: F12 opens a modal panel listing every transfer with state/progress;
navigate and close; live refresh while open; empty state handled.

**Independent Test**: Submit two transfers, dispatch `ShowTasksPanel`, assert the
modal opens listing both rows; move the selection; press Esc and assert it closes
with panes untouched.

- [ ] T005 [P] [US1] Write failing widget tests in `crates/cargonaut-ui-tui/src/dialog.rs` (TestBackend): `TasksPanelDialog::new` selects row 0; Up/Down and `j`/`k` move and clamp selection; `Esc` → `TasksAction::Close`; `render` draws one line per row with status labels and highlights the selection; empty list renders a "No transfers" line; `set_rows` with a shorter list clamps the selection in-bounds. (RED)
- [ ] T006 [US1] Implement `JobRow`, `TasksAction`, and `TasksPanelDialog` (`new`/`set_rows`/`len`/`is_empty`/`focused_index`/`focused`/`handle_key` for nav + `c`/`p`/`r`/`Esc` + `render`) per `contracts/tasks-panel-widget.md`, with doc comments. Make T005 pass. (GREEN) — `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T007 [US1] Add a `JobRow::from(&JobView)` mapping (label `"<src> → <dst>"` display-shortened, `status_label` e.g. `"Running 62%"`/`"Paused"`/`"Completed ✓"`, and `can_cancel`/`can_pause`/`can_resume` from the eligibility table in `data-model.md`). Cover the status-label + truncation formatting with a unit test (RED first, then GREEN). — `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T008 [P] [US1] Write a failing dispatch test in `crates/cargonaut-ui-tui/src/lib.rs`: dispatching `Command::ShowTasksPanel` sets `Some(ActiveDialog::TasksPanel { .. })` and `Mode::Dialog`; a second `ShowTasksPanel` (or `Esc`) closes it; no second modal stacks (FR-013). (RED)
- [ ] T009 [US1] Add `ActiveDialog::TasksPanel { widget: TasksPanelDialog }`; open it in `dispatch_ui_command` from `app.job_views()` (alongside `QuickCdPopup`/`TogglePanelFilter`, returning early); route `handle_key` for navigation + `TasksAction::Close`; add the `draw_frame` render arm; refresh the widget rows from `app.job_views()` each frame (FR-008) preserving/clamping selection. Make T008 pass. (GREEN) — `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: MVP — the panel opens, lists jobs with live progress, navigates,
and closes. US2/US3 add per-row actions.

---

## Phase 4: User Story 2 — Cancel a transfer from the panel (Priority: P2)

**Goal**: Cancel the selected transfer; only that job stops; its row shows
Cancelled.

**Independent Test**: Submit two transfers, cancel one by id, assert it reaches
Cancelled while the other keeps running.

- [ ] T010 [P] [US2] Write failing core tests in `crates/cargonaut-core/src/lib.rs`: `cancel_transfer(id)` cancels the named job (its token is cancelled), removes the id from `paused`, and leaves sibling transfers running; an unknown id is a safe no-op. (RED)
- [ ] T011 [US2] Implement `pub fn cancel_transfer(&mut self, id: TransferId) -> Vec<Event>` per `contracts/core-api.md`; reimplement `CancelCurrentTransfer` to delegate to `cancel_transfer(transfer_order.last())`. Make T010 pass. (GREEN) — `crates/cargonaut-core/src/lib.rs`
- [ ] T012 [P] [US2] Write a failing TUI test in `crates/cargonaut-ui-tui/src/lib.rs`: with the panel open over a running job, a `c` keypress routes to `app.cancel_transfer(focused id)` and after the per-frame refresh the row's status becomes `Cancelled`; the panel stays open. (RED)
- [ ] T013 [US2] Wire `TasksAction::Cancel(i)` in the `handle_key` `TasksPanel` arm to `app.cancel_transfer(rows[i].id)`, then refresh rows. Make T012 pass. (GREEN) — `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: US1 + US2 — view and cancel work independently.

---

## Phase 5: User Story 3 — Pause and resume a transfer from the panel (Priority: P3)

**Goal**: Pause the selected transfer (others continue); resume it from its
checkpoint to completion. Realizes the issue's named acceptance test (SC-003).

**Independent Test**: Submit three throttled transfers, pause one, assert it stops
while the other two complete; resume it and assert it completes.

- [ ] T014 [P] [US3] Write failing core tests in `crates/cargonaut-core/src/lib.rs`: `pause_transfer(id)` on a running job inserts the id into `paused` and `job_views()` classifies it as `JobStatus::Paused` (even though the raw snapshot is `Canceled`); pause on a terminal or unknown id is a no-op. (RED)
- [ ] T015 [US3] Implement `pub fn pause_transfer(&mut self, id: TransferId) -> Vec<Event>` per `contracts/core-api.md` (cancel token + insert into `paused`, eligibility-gated). Make T014 pass. (GREEN) — `crates/cargonaut-core/src/lib.rs`
- [ ] T016 [P] [US3] Write failing core tests for resume in `crates/cargonaut-core/src/lib.rs`: a paused job that has a checkpoint sidecar resumes (same `TransferId`), clears the `paused` marker, and reaches `Completed`; resume on a non-paused id is a no-op; a job paused before its first checkpoint falls back to a fresh restart and still completes. (RED)
- [ ] T017 [US3] Implement `pub async fn resume_paused(&mut self, id: TransferId) -> Result<Vec<Event>, AppError>` per `contracts/core-api.md` (`scan_resumable` on the dst parent → match `job_id` → `resume_transfer`; fallback to `submit_transfer` updating `transfer_order` in place; clear `paused`). Make T016 pass. (GREEN) — `crates/cargonaut-core/src/lib.rs`
- [ ] T018 [US3] Add the SC-003 integration test `three_jobs_pause_one_others_continue` in `crates/cargonaut-core/tests/jobs_panel.rs`: with `CARGONAUT_TRANSFER_THROTTLE_MIBPS` set, drive an `App` over two temp dirs to submit 3 copies, pause one by id, assert it stays `Paused` (no further progress) while the other two reach `Completed`, then `resume_paused` it and assert it reaches `Completed`. (RED before T015/T017 land; GREEN after) — `crates/cargonaut-core/tests/jobs_panel.rs`
- [ ] T019 [P] [US3] Write a failing TUI test in `crates/cargonaut-ui-tui/src/lib.rs`: `p` on a running row routes to `app.pause_transfer` and the row renders `Paused` after refresh; `r` on a paused row routes to `app.resume_paused`. (RED)
- [ ] T020 [US3] Wire `TasksAction::Pause(i)`/`Resume(i)` in the `handle_key` `TasksPanel` arm (`pause_transfer`; `resume_paused(...).await`), then refresh rows. Make T019 pass. (GREEN) — `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: All three user stories independently functional; SC-003 green.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: Gates, docs, and the deferral/issue paper trail.

- [ ] T021 Run `make ci-local` (clippy `-D warnings` → `cargo test --workspace` → release build → docs-gate); fix any clippy/fmt/`missing_docs` findings across the touched files.
- [ ] T022 [P] Run the quickstart manual smoke test (`specs/039-tasks-jobs-panel/quickstart.md`) against `make build` and confirm all SC scenarios; capture the result in the PR body.
- [ ] T023 [P] Update `README.md`: bump the "At a Glance" metrics table (test count, feature count, binary size) and add a one-line "Feature History" entry for Feature 039. (MANDATORY docs-gate.)
- [ ] T024 [P] Append a Feature 039 section to `Learnings.md` (≥3 bullets): pause-as-cancel+checkpoint vs in-place suspend (and why the engine seam forced it); reusing `scan_resumable` to locate the in-session checkpoint instead of a new transfer-crate API; the `paused` marker as the paused-vs-cancelled source of truth (because `TransferJob` exposes no `watch::Sender`). (MANDATORY docs-gate.)
- [ ] T025 [P] Update `CHANGELOG.md`; add/confirm a `ROADMAP.md` note if anything was descoped (none expected — full FR set ships); close issue #32 referencing the PR once merged.

---

## Dependencies & Execution Order

### Phase dependencies

- **Setup (Phase 1)**: no dependencies.
- **Foundational (Phase 2, T003–T004)**: depends on Setup; **blocks all stories**.
- **US1 (Phase 3)**: depends on Foundational. The MVP.
- **US2 (Phase 4)** and **US3 (Phase 5)**: depend on Foundational. Their **core**
  tasks (cancel/pause/resume methods) depend only on Foundational and can be built
  in parallel with the US1 widget. Their **TUI wiring** tasks (T013, T020) depend
  on US1's `handle_key`/`ActiveDialog` wiring (T009).
- **Polish (Phase 6)**: after the desired stories are complete.

### Within each story

- The `(red)` test task precedes its `(green)` implementation task — do not invert.
- Core method before its TUI wiring.

### Parallel opportunities

- T005/T008 (US1 tests, different files) can be written in parallel.
- US2 core (T010→T011) and US3 core (T014→T015, T016→T017) can proceed in parallel
  with each other and with the US1 widget, since they touch disjoint code paths
  (core methods vs `dialog.rs`).
- Polish T022–T025 are independent docs/validation tasks ([P]).

---

## Parallel Example: after Foundational

```text
# Core action methods (cargonaut-core/src/lib.rs region) — sequential red→green,
# but independent of the US1 widget work in dialog.rs:
Task: T010→T011  cancel_transfer(id)
Task: T014→T015  pause_transfer(id)
Task: T016→T017  resume_paused(id)

# Simultaneously, the US1 widget (cargonaut-ui-tui/src/dialog.rs):
Task: T005→T006  TasksPanelDialog
```

---

## Implementation Strategy

### MVP first (US1)

1. Phase 1 Setup → Phase 2 Foundational → Phase 3 US1.
2. **STOP and VALIDATE**: F12 opens a live, navigable, dismissible jobs panel.
3. Demo the MVP.

### Incremental delivery

1. Foundation + US1 → panel visible (MVP).
2. + US2 → cancel from the panel.
3. + US3 → pause/resume; SC-003 green — feature-complete.
4. Polish: gates + mandatory docs (README, Learnings) + close #32.

## Notes

- `[P]` = different files, no dependency on an incomplete task.
- Build/test only via `make` targets (Constitution §V); never `cargo clean` /
  `rm -rf target`.
- Commit after each red and each green task; no `Co-Authored-By: Claude` trailer
  (CLAUDE.md). Feature-branch PR MUST modify both `README.md` and `Learnings.md`
  (docs-gate).

# Research: Tasks/Jobs Panel Popup

**Feature**: 039-tasks-jobs-panel | **Date**: 2026-06-15

This feature has no unknown external technologies; the research below resolves
the *design* decisions implied by the spec's clarifications against the existing
codebase. All findings are derived from reading the current sources, not from
external sources.

## R-001 — Pause/resume strategy

**Decision**: Pause = signal the running transfer's existing `CancellationToken`
(which makes the copy loop emit `Canceled` and **leave** the checkpoint sidecar
on disk) and record the job id in a new `paused` marker set on `App`. Resume =
locate that job's checkpoint sidecar, rebuild a `TransferCheckpoint`, and call the
existing `resume_transfer(...)`, which spawns a fresh copy task with a **new**
`CancellationToken` and the **same** `TransferId`. Replace the registry entry
under that id and clear the marker.

**Rationale**:
- The copy loop (`crates/cargonaut-transfer/src/job.rs:246-255`) already checks
  `cancel.is_cancelled()` at the top of every chunk iteration and, on cancel,
  emits `TransferState::Canceled` while *leaving the checkpoint in place* —
  exactly the "stop now, stay resumable" semantics pause needs.
- `resume_transfer` (`job.rs:506`) is already implemented and covered by tests
  (`resume_completes_partial_transfer`, `resume_preserves_job_id_from_checkpoint`,
  CRC-mismatch rejection). It reuses `checkpoint.job_id` so the resumed job keeps
  its identity, which keeps `transfer_order` stable (no duplicate row).
- `TransferJob` exposes only a `watch::Receiver` (read-only) and a
  `CancellationToken` — the App **cannot** push a `Paused` state into the watch
  channel from outside. So a true in-process "park the task" pause would require
  threading a new pause primitive through `run_transfer` *and* handing the App the
  `watch::Sender`. That is an invasive change to a NON-NEGOTIABLE-perf,
  heavily-tested engine for marginal benefit.
- "Lose at most one checkpoint interval" (FR-011) is satisfied: resume picks up
  from the last fsync'd checkpoint, identical to SIGKILL-resume (SC-002).

**Alternatives considered**:
- *In-place suspend* (cooperative pause gate inside the copy loop; task parks with
  file handles open). Rejected: invasive engine change, holds file descriptors
  while paused, and duplicates resume semantics the checkpoint path already
  provides. Recorded as the non-chosen clarification option; can be revisited if
  instant resume (no stream re-open) becomes a requirement.
- *Remove the job and re-add on resume*. Rejected: would reorder the list and lose
  the row's place; reusing the same id keeps `transfer_order` stable.

## R-002 — Locating a paused job's checkpoint for resume

**Decision**: On resume, run `scan_resumable(local_fs, <dst-parent-dir>)` and
select the `ResumableTransfer` whose `checkpoint.job_id == id`. If none is found
(the job was paused before its first checkpoint interval was written), fall back
to a fresh `submit_transfer(src, dst)` for that job (it restarts from zero) — the
spec's "≤ one checkpoint interval lost" bound degrades gracefully to "lost the
sub-interval head start", which is correct because no durable progress existed.

**Rationale**:
- `scan_resumable` (`crates/cargonaut-transfer/src/checkpoint.rs:88`) is the
  existing, tested way to turn on-disk sidecars into `TransferCheckpoint`s plus a
  validated CRC chain. The sidecar path is deterministic
  (`<dst-parent>/.cargonaut-transfer-<id>.json`, `job.rs:167`), and `App` still
  holds the job's `src`/`dst` paths in the registry, so the parent dir is known.
- Reusing `scan_resumable` avoids adding a new "load one checkpoint by path" API
  to the transfer crate; it also inherits the defensive CRC re-validation that
  `resume_transfer` expects.

**Alternatives considered**:
- Add `load_checkpoint(path) -> ResumableTransfer` to the transfer crate.
  Reasonable, but unnecessary now; `scan_resumable` on the (small) destination
  directory is adequate and already tested. Noted as a possible later
  simplification if scan cost ever matters.

## R-003 — Distinguishing user-paused from cancelled

**Decision**: Hold a `paused: HashSet<TransferId>` on `App`. The job's observable
`TransferState` after a pause is `Canceled` (the loop can't express "paused"),
so the `paused` set is the source of truth for "this was a deliberate pause". The
`JobView` projection maps a job that is in the `paused` set to `JobStatus::Paused`
regardless of its raw `Canceled` snapshot; `cancel_transfer` removes the id from
the set first so a real cancel renders as `Cancelled`.

**Rationale**: Satisfies FR-017 (paused must be distinguishable from cancelled)
and FR-004 without changing the engine's `TransferState` enum or its tests. The
set is also the gate for FR-012 (resume offered only for paused jobs).

**Alternatives considered**: Add a `Paused` arm the App emits — impossible
without the `watch::Sender`. Add a parallel per-job status map — heavier than a
set; the set plus the raw snapshot fully determines the displayed status.

## R-004 — Live refresh while the panel is open (FR-008)

**Decision**: The event loop already redraws on a ~100 ms tick
(`crates/cargonaut-ui-tui/src/lib.rs`, tick branch) and on every key. Before
rendering the `TasksPanel`, rebuild the widget's rows from a fresh
`app.job_views()` snapshot, preserving (and clamping) the current selection
index. The widget exposes `set_rows(Vec<JobRow>)` for this; `ListState` selection
is retained across refreshes.

**Rationale**: Mirrors how the progress dialog already reads `active_progress()`
each frame. Keeps the dialog a pure view over core state — no duplicated job
bookkeeping in the UI (FR-002). Clamping on refresh satisfies the "list changes
while open" edge case (selection never points past the end).

**Alternatives considered**: Snapshot once at open and never refresh — violates
FR-008. Push events into the dialog — more plumbing than re-reading the snapshot.

## R-005 — Per-row action keys and dialog ownership

**Decision**: The `TasksPanelDialog` owns its keys, returning a `TasksAction`
from `handle_key`: `Up/Down`/`j`/`k` move selection (no action); `c`/`p`/`r` →
`Cancel`/`Pause`/`Resume` of the focused index; `Esc` (and F12 at the loop level)
→ `Close`. The event loop maps the returned action to `app.cancel_transfer` /
`app.pause_transfer` / `app.resume_paused` by the focused job's id, then refreshes
rows and keeps the panel open (close only on `Close`).

**Rationale**: Identical ownership model to `ResumePromptDialog::handle_key`
(returns `(index, ResumeChoice)`) and `ConfirmDialog` (owns `y`/`n`) — Constitution
§III consistency. F12 is already bound in `keymap.toml`; per-row keys live in the
widget, so no new global bindings are needed.

**Alternatives considered**: Route per-row keys through `keymap.toml` + new
`Command` variants. Rejected: dialog-internal keys are the established pattern;
adding global commands for modal-only keys pollutes the keymap.

## R-006 — `:jobs` command parity

**Decision**: F12 is the bound trigger (`design/contracts/keymap.toml`,
`ShowTasksPanel`). During implementation, verify whether a `:`-command entry
exists for jobs; if the command surface maps `:jobs` to the same `ShowTasksPanel`
command, no extra work is needed. If no command palette exists yet, F12 is the
delivered trigger and `:jobs` is satisfied by the same command whenever the
palette lands (not in scope to build the palette here).

**Rationale**: The spec lists "F12 / `:jobs`" as two triggers for one action.
Both must resolve to `Command::ShowTasksPanel`; building a command palette is out
of scope.

## Summary of decisions

| ID | Decision | Touches |
|----|----------|---------|
| R-001 | Pause = cancel+checkpoint; resume = `resume_transfer` re-arm | core only |
| R-002 | Find checkpoint via `scan_resumable` on dst parent; fall back to fresh submit | core only |
| R-003 | `paused: HashSet<TransferId>` is the paused-vs-cancelled source of truth | core only |
| R-004 | Rebuild panel rows from `job_views()` each frame, clamp selection | ui-tui |
| R-005 | Dialog owns `c`/`p`/`r`/arrows/Esc; loop maps action→App method | ui-tui |
| R-006 | F12 trigger now; `:jobs` = same command, palette out of scope | keymap (verify) |

All spec clarifications are resolved; no `NEEDS CLARIFICATION` remains.

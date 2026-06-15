# Data Model: Tasks/Jobs Panel Popup

**Feature**: 039-tasks-jobs-panel | **Date**: 2026-06-15

No persisted data and no new on-disk formats. This feature adds in-memory
projections and one marker set, plus a UI-side view row. All entities below are
derived from the existing transfer registry; the engine's own types
(`TransferJob`, `TransferState`, `TransferCheckpoint`) are unchanged.

## Entities

### JobStatus (core, new) — UI-agnostic status classification

A projection of the engine's lifecycle into the states the panel renders. Keeps
`cargonaut-transfer`'s `TransferState` out of the UI (same boundary as
`ProgressView`).

| Variant | Meaning | Source |
|---------|---------|--------|
| `Queued` | Submitted, not yet progressing | `TransferState::Queued` |
| `Running { bytes_done, bytes_total, eta_secs, throughput_mibs }` | In flight | `TransferState::Running{..}` |
| `Paused` | User-paused (resumable) | id ∈ `App.paused` (overrides raw `Canceled`) |
| `Completed { verified }` | Finished | `TransferState::Completed{ sha256_match }` |
| `Failed { resumable }` | Errored | `TransferState::Failed{ resumable, .. }` |
| `Cancelled` | User-cancelled (not paused) | `TransferState::Canceled` and id ∉ `paused` |

**Derivation rule** (single source of truth): take the job's `watch::Receiver`
snapshot; **if** the id is in `App.paused`, classify as `Paused`; otherwise map
the raw `TransferState` arm directly. This is the only place the paused marker is
consulted for display.

**Action eligibility** (drives FR-012):
| Status | Cancel | Pause | Resume |
|--------|:------:|:-----:|:------:|
| Queued | ✓ | ✓ | — |
| Running | ✓ | ✓ | — |
| Paused | ✓ | — | ✓ |
| Completed / Failed / Cancelled | — | — | — |

Ineligible actions are no-ops (FR-012, SC-006).

### JobView (core, new) — one row's worth of UI-facing data

`#[derive(Debug, Clone, PartialEq)]`. Built by `App::job_views()`; mirrors the
shape/role of `ProgressView` and `ResumeOfferView`.

| Field | Type | Notes |
|-------|------|-------|
| `id` | `TransferId` | identity; the action target |
| `src` | `String` | source path/URI for display (caller may shorten) |
| `dst` | `String` | destination path/URI for display |
| `mode` | `TransferMode` | Copy/Move (for the row label) |
| `status` | `JobStatus` | classified status incl. progress |

Rows are returned in `transfer_order` (submit order) so the list is stable across
refreshes.

### App.paused (core, new field) — user-paused marker

`paused: std::collections::HashSet<TransferId>`. Records jobs the user paused
(vs. cancelled). Mutated by `pause_transfer` (insert), `resume_paused` (remove on
success), and `cancel_transfer` (remove, so the job renders `Cancelled`). It is
the source of truth for the `Paused` classification and for resume eligibility.

### JobRow (ui-tui, new) — dialog list item

The widget's per-row display model, built from a `JobView`. Holds the
already-formatted strings the row renders (so the widget does no core-type
formatting):

| Field | Type | Notes |
|-------|------|-------|
| `id` | `TransferId` | echoed back so the loop can target the App method |
| `label` | `String` | `"<src> → <dst>"`, display-shortened |
| `status_label` | `String` | e.g. `"Running 62%"`, `"Paused"`, `"Completed ✓"` |
| `can_cancel` / `can_pause` / `can_resume` | `bool` | from the eligibility table |

### TasksAction (ui-tui, new) — widget → loop result

Returned by `TasksPanelDialog::handle_key`:
`Cancel(usize)` | `Pause(usize)` | `Resume(usize)` | `Close`, where `usize` is the
focused row index (the loop reads `rows[index].id` to call the App). Navigation
keys return `None`.

## State transitions (per transfer, as seen by the panel)

```text
        submit                pause (c-token cancel,           resume (resume_transfer,
   ──────────────▶ Running ───── keep checkpoint) ────▶ Paused ──── new c-token) ─────▶ Running ──▶ Completed
                      │                                   │                                  │
            cancel    │                            cancel │                          (normal)│
        (c-token)     ▼                         (c-token) ▼                                  ▼
                  Cancelled  ◀──────────────────────────  Cancelled                       Failed
```

- `Running → Paused`: `pause_transfer` cancels the token (loop emits `Canceled`,
  leaves sidecar) and inserts the id into `paused`; the projection shows `Paused`.
- `Paused → Running`: `resume_paused` finds the sidecar, calls `resume_transfer`
  (same id, fresh token), replaces the registry entry, removes the id from
  `paused`.
- `Paused → Cancelled`: `cancel_transfer` removes the id from `paused`; the job's
  task is already stopped, so it surfaces as `Cancelled`.
- Terminal states (`Completed`/`Failed`/`Cancelled`) accept no actions.

## Validation / invariants

- A job id appears **at most once** in `transfer_order` and the panel list;
  resume reuses the same id (no duplicate rows).
- `paused` only ever contains ids present in `transfers`; entries are removed on
  resume, cancel, or whenever the job reaches a terminal non-paused state during
  display classification (defensive: a `Completed` job is never shown `Paused`).
- The selection index is always clamped to `[0, rows.len())` after every refresh
  (or `None` when the list is empty).
- The projection performs **no I/O**; only `resume_paused` (and the fallback
  `submit_transfer`) touch the filesystem.

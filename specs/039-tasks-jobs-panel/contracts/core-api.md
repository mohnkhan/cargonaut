# Contract: cargonaut-core public API additions

**Feature**: 039-tasks-jobs-panel

All additions live in `crates/cargonaut-core/src/lib.rs`. Public items carry doc
comments (`#![warn(missing_docs)]`). No `unsafe`. These mirror the existing
`ProgressView` / `active_progress()` / Feature-037 resume seam.

## Types

```rust
/// UI-agnostic status of one transfer, as rendered by the tasks panel.
#[derive(Debug, Clone, PartialEq)]
pub enum JobStatus {
    Queued,
    Running { bytes_done: u64, bytes_total: u64, eta_secs: u32, throughput_mibs: f32 },
    Paused,
    Completed { verified: bool },
    Failed { resumable: bool },
    Cancelled,
}

/// UI-facing projection of one registry transfer (one panel row).
#[derive(Debug, Clone, PartialEq)]
pub struct JobView {
    pub id: TransferId,
    pub src: String,
    pub dst: String,
    pub mode: TransferMode,
    pub status: JobStatus,
}
```

(`TransferId` and `TransferMode` are already re-exported from `cargonaut-core`.)

## Methods on `App`

### `pub fn job_views(&self) -> Vec<JobView>`
- **Pure** (no I/O). Returns one `JobView` per id in `transfer_order` (submit
  order), classifying status per the data-model derivation rule (the `paused`
  marker overrides a raw `Canceled` snapshot → `Paused`).
- Guarantees: order stable across calls; ids unique; never panics on an empty
  registry (returns `[]`).
- Satisfies: FR-002, FR-003, FR-004, FR-014 (empty → `[]`, UI shows empty state).

### `pub fn pause_transfer(&mut self, id: TransferId) -> Vec<Event>`
- If `id` is unknown → `[]` (no-op).
- If the job's current status is `Queued`/`Running` and `id ∉ paused`: call
  `job.cancel.cancel()`, insert `id` into `paused`, return
  `[Event::Status(format!("Paused transfer {id:?}"))]`.
- If the job is terminal or already paused → `[]` (safe no-op).
- Side effects: cancels the token only; the engine leaves the checkpoint sidecar
  in place. Does **not** remove the job from `transfers`/`transfer_order`.
- Satisfies: FR-010, FR-012, FR-016, FR-017; the "others continue" guarantee is
  structural (each transfer is an independent task; only this token is cancelled).

### `pub async fn resume_paused(&mut self, id: TransferId) -> Result<Vec<Event>, AppError>`
- If `id ∉ paused` → `Ok(vec![])` (resume only applies to paused jobs; FR-012).
- Resolve the job's `dst` parent directory from the registry entry; run
  `scan_resumable(local_fs, dst_parent)` and select the `ResumableTransfer` whose
  `checkpoint.job_id == id`.
  - **Found** → `resume_transfer(local_fs, local_fs, checkpoint, transfer_opts())`.
    On `Ok(job)`: replace `transfers[id]` with the new job (same id), remove `id`
    from `paused`, return `[Event::TransferProgressed(id)]`. On `Err(e)`: leave
    `paused` as-is, return `[Event::Status(format!("Cannot resume: {e}"))]`.
  - **Not found** (paused before first checkpoint) → fall back to
    `submit_transfer(local_fs, src, local_fs, dst, transfer_opts())` for the job's
    `src`/`dst`; on success replace `transfers[id_new]`… — note `submit_transfer`
    mints a **new** id, so update `transfer_order` in place (replace the old id
    with the new one at the same position) and remove the old id from `paused`.
    Return `[Event::TransferProgressed(new_id)]`.
- Re-arm guarantee: the resumed/restarted job carries a fresh `CancellationToken`,
  so it can be paused or cancelled again (FR-011).
- Satisfies: FR-011, FR-012; SC-004 (resumed job completes, ≤1 interval lost).

### `pub fn cancel_transfer(&mut self, id: TransferId) -> Vec<Event>`
- Generalizes the existing `CancelCurrentTransfer` (which cancels
  `transfer_order.last()`).
- If `id` unknown → `[Event::Status("No such transfer".into())]` (or `[]`).
- Otherwise: remove `id` from `paused` (so it renders `Cancelled`, not `Paused`),
  call `job.cancel.cancel()`, return `[Event::Status(format!("Canceled transfer {id:?}"))]`.
- On a terminal job: cancelling an already-finished token is harmless (no-op
  effect); never panics. Satisfies FR-009, FR-012, SC-002, SC-006.

## Backward-compatibility

- `CancelCurrentTransfer` keeps working; it MAY be reimplemented to delegate to
  `cancel_transfer(transfer_order.last())`.
- `ShowTasksPanel`'s core dispatch arm becomes a no-op (`Ok(vec![])`) like
  `QuickCdPopup` / `TogglePanelFilter`: the TUI intercepts it to open the modal.
  The old status-string stub and its test (`show_tasks_panel_emits_status_with_transfer_count`)
  are replaced by the new behavior + tests.

## Test contract (core)

- `job_views_*`: empty registry → `[]`; after two copies → two rows in submit
  order with `Running`/`Queued` status; a paused id classifies as `Paused` even
  though its raw snapshot is `Canceled`; a cancelled id classifies as `Cancelled`.
- `pause_transfer_*`: pausing a running job inserts into `paused` and its
  `job_views` row becomes `Paused`; pausing an unknown/terminal id is a no-op.
- `resume_paused_*`: a paused job with a checkpoint resumes (same id), clears the
  marker, and reaches `Completed`; resume on a non-paused id is a no-op.
- `cancel_transfer_*`: cancels the named job, removes it from `paused`, leaves
  siblings running.
- **SC-003 integration** (`three_jobs_pause_one_others_continue`): with
  `CARGONAUT_TRANSFER_THROTTLE_MIBPS` set, submit 3 copies via `App`, pause one by
  id, assert the paused one stops (status `Paused`, no further `Running`) while the
  other two reach `Completed`; then `resume_paused` the paused one and assert it
  reaches `Completed`.

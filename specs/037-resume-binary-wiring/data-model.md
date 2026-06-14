# Data Model: Resume-from-Interrupted-Transfer (Binary Wiring + SC-002 Gate)

This feature adds **no new persisted data**. The on-disk `TransferCheckpoint` sidecar format
is unchanged. What changes is in-memory App state and one UI-facing projection.

## Existing types (consumed, not modified)

- `cargonaut_transfer::TransferCheckpoint` — persisted sidecar (`version`, `job_id`,
  `src_uri`, `src_size`, `src_sha256_prefix`, `dst_uri`, `bytes_written`, `chunk_crcs`,
  `chunk_size_bytes`, timestamps). On disk as `.cargonaut-transfer-<job-id>.json` beside the
  destination.
- `cargonaut_transfer::ResumableTransfer` — scan result: `{ checkpoint, checkpoint_path,
  source_unchanged, dest_intact }`. Returned by `scan_resumable`.
- `cargonaut_transfer::TransferJob` / `TransferState` — in-flight handle + progress, already
  registered in `App.transfers` / `App.transfer_order`.
- `cargonaut_ui_tui::dialog::ResumableSummary` — the widget's per-row view
  `{ src, dst, bytes_written_mib, src_size_mib, source_unchanged, dest_intact }`.
- `cargonaut_ui_tui::dialog::ResumeChoice` — `Resume | StartOver | Skip`.

## New type — `cargonaut_core::ResumeOfferView`

A UI-agnostic projection of one `ResumableTransfer`, mirroring `ProgressView`'s role. Carries
exactly what the UI needs to render a row; no `cargonaut-transfer` type leaks across the seam.

```text
pub struct ResumeOfferView {
    pub src: String,            // display-shortened source URI/path
    pub dst: String,            // display-shortened destination URI/path
    pub bytes_written_mib: f32, // checkpoint.bytes_written as MiB
    pub src_size_mib: f32,      // checkpoint.src_size as MiB
    pub source_unchanged: bool, // ResumableTransfer.source_unchanged
    pub dest_intact: bool,      // ResumableTransfer.dest_intact
}
```

The UI maps `ResumeOfferView` → `ResumableSummary` field-for-field when building the dialog.

## New App state

```text
struct App {
    // ...existing fields...
    pending_resumes: Vec<ResumableTransfer>, // scan order; the source of truth for offers
}
```

- Populated by `scan_resume_offers` at launch; drained one element at a time as the user acts.
- `resume_offer(i)` / `start_over_offer(i)` consume `pending_resumes[i]` (act, then remove).
- `skip_offer(i)` removes `pending_resumes[i]` without acting (sidecar left on disk).
- `pending_resume_views()` projects the current vec to `Vec<ResumeOfferView>`.

## State flow (launch → resume)

```text
binary start
  └─ App::new(left, right)                         # existing: lists both panes
  └─ App::scan_resume_offers()                     # NEW: scan_resumable(left.cwd), scan_resumable(right.cwd)
        ├─ none found  ───────────────────────────▶ normal event loop (hot path, unchanged)
        └─ ≥1 found: store in pending_resumes, return Vec<ResumeOfferView>
              └─ UI: active_dialog = Resume(ResumePromptDialog::new(views→summaries))
                    │
                    ▼ (modal event loop)
              key 'r' on offer i ─▶ App::resume_offer(i)
                                      ├─ resume_transfer(local_fs, local_fs, cp.clone(), opts)
                                      ├─ transfers.insert(id, job); transfer_order.push(id)
                                      └─ pending_resumes.remove(i)
              key 's' on offer i ─▶ App::start_over_offer(i)
                                      ├─ delete sidecar (checkpoint_path)
                                      ├─ submit_transfer(src, dst, opts)   # truncates dst
                                      ├─ register job
                                      └─ pending_resumes.remove(i)
              key 'c'/Esc on i  ─▶ App::skip_offer(i)
                                      └─ pending_resumes.remove(i)   # sidecar stays
                    │
                    ▼ after each choice
              UI rebuilds dialog from pending_resume_views()
                    ├─ empty  ─▶ dismiss dialog, mode = Pane (normal loop; transfers run)
                    └─ nonempty ─▶ show remaining offers
```

Once dismissed, resumed/started transfers live in the normal transfer registry and surface
through the existing progress UI (`active_progress()` / `progress_summary`) and completion
handling — no separate resume progress path.

## Transfer engine: throttle hook (no type change)

`run_transfer` and `run_transfer_with_state` read `CARGONAUT_TRANSFER_THROTTLE_MIBPS` once at
start. If set to a positive number `m`, after each written chunk the loop sleeps enough to cap
throughput at `m` MiB/s. Unset/invalid ⇒ no sleep (production default). No struct or public
signature changes; purely internal loop behavior controlled by the environment.

## Integrity & failure states (unchanged engine guarantees, now reachable)

- `resume_transfer` re-verifies the destination CRC chain and **refuses** on mismatch
  (`TransferError::Checkpoint`) — surfaces as a status/error in the UI (FR-009/SC-005).
- A malformed/old-version sidecar is silently skipped by `scan_resumable` (FR-010) — it never
  enters `pending_resumes`.
- On successful completion the engine unlinks the sidecar (FR-006); the offer is already gone
  from `pending_resumes`.

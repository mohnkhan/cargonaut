# Contract: `cargonaut-core` Resume Seam

The testable public API added to `cargonaut_core::App`. Each method below has a behavioral
contract and is covered by a `cargonaut-core` unit test (TDD: failing test first).

## `ResumeOfferView` (new public struct)

UI-agnostic projection of one resumable transfer. Fields per [data-model.md](../data-model.md).
Derives `Debug, Clone`. All fields public, all documented (`missing_docs`).

---

## `App::scan_resume_offers`

```text
pub async fn scan_resume_offers(&mut self) -> Result<Vec<ResumeOfferView>, AppError>
```

**Behavior**
- Calls `scan_resumable(self.local_fs.clone(), dir)` for each distinct pane cwd (left, right).
- Stores all found `ResumableTransfer`s in `self.pending_resumes` (scan order; left dir
  before right dir; duplicates by directory removed).
- Returns the projected `Vec<ResumeOfferView>` (same order).

**Contract / acceptance**
- C1: With a valid sidecar + partial destination present in a pane dir, returns ≥1 offer and
  populates `pending_resumes` with the same count. *(FR-001, FR-002)*
- C2: With no sidecars in either dir, returns an empty vec and leaves `pending_resumes` empty;
  no error. *(FR-003)*
- C3: A malformed/old-version sidecar is not returned (delegated to `scan_resumable`). *(FR-010)*
- C4: Both panes in the same directory ⇒ that directory scanned once (no duplicate offers).

---

## `App::pending_resume_views`

```text
pub fn pending_resume_views(&self) -> Vec<ResumeOfferView>
```

**Behavior**: Projects the current `self.pending_resumes` to views, in order. Pure, no I/O.

**Contract**
- C5: Count equals `pending_resumes.len()`; order preserved. Used by the UI to rebuild the
  dialog after each choice (R-005).

---

## `App::resume_offer`

```text
pub async fn resume_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError>
```

**Behavior**
- Validates `index < pending_resumes.len()` (else returns a `Status` event, no panic).
- Calls `resume_transfer(local_fs, local_fs, checkpoint.clone(), opts)` with `opts` built from
  config (`checkpoint_interval_mib`, `verify_after_copy`) as in `confirm_copy`.
- On success: inserts the job into `transfers`/`transfer_order`, removes the offer from
  `pending_resumes`, returns `vec![Event::TransferProgressed(id)]`.
- On `resume_transfer` error (e.g. CRC mismatch): removes the offer, returns a `Status`/error
  surfacing the reason — never produces a corrupt destination. *(FR-009, SC-005)*

**Contract / acceptance**
- C6: Resuming a valid offer registers exactly one new transfer and emits
  `TransferProgressed`. *(FR-005)*
- C7: The resumed transfer continues from the checkpoint offset (engine-guaranteed; asserted
  end-to-end by the SC-002 PTY test). *(SC-002)*
- C8: A mismatched/changed source or destination fails safe with a reported error and no new
  successful transfer. *(FR-009, SC-005)*
- C9: After resume, the offer is no longer in `pending_resumes`.

---

## `App::start_over_offer`

```text
pub async fn start_over_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError>
```

**Behavior**
- Validates index.
- Deletes the checkpoint sidecar at `ResumableTransfer.checkpoint_path` (best-effort; absence
  is not an error).
- Calls `submit_transfer(src, dst, opts)` with `src`/`dst` parsed from the checkpoint URIs —
  `submit_transfer` truncates the destination, discarding the partial. *(R-007)*
- Registers the job, removes the offer, returns `vec![Event::TransferProgressed(id)]`.

**Contract / acceptance**
- C10: Start-over removes the old sidecar and starts a fresh transfer from offset 0. *(FR-007)*
- C11: After start-over the offer is gone from `pending_resumes`.

---

## `App::skip_offer`

```text
pub fn skip_offer(&mut self, index: usize)
```

**Behavior**: Removes `pending_resumes[index]` (bounds-checked, no-op if out of range). Does
**not** touch the sidecar on disk and starts no transfer.

**Contract / acceptance**
- C12: Skip starts no transfer and leaves the sidecar in place (so a fresh `scan_resume_offers`
  would find it again). *(FR-008)*
- C13: After skip the offer is gone from the in-memory `pending_resumes` for this session.

---

## UI wiring contract (`cargonaut-ui-tui`)

- The `ActiveDialog::Resume` variant loses `#[allow(dead_code)]` (it is now constructed).
- At launch, after `App::new`, the loop calls `scan_resume_offers`; if non-empty it sets
  `active_dialog = Some(ActiveDialog::Resume(ResumePromptDialog::new(summaries)))`.
- The `ActiveDialog::Resume` key arm dispatches `ResumeChoice` to the matching `App` method by
  index, then rebuilds the dialog from `pending_resume_views()` (dismiss if empty). *(R-005)*
- All rendering reuses the existing `ResumePromptDialog`; no new keymap bindings. *(§III)*

## Engine throttle contract (`cargonaut-transfer`)

- `CARGONAUT_TRANSFER_THROTTLE_MIBPS` unset/invalid ⇒ identical behavior to today (no sleep).
- Set to positive `m` ⇒ copy throughput capped at ~`m` MiB/s in both fresh and resumed loops.
- Covered by a focused test asserting a throttled transfer takes measurably longer than an
  unthrottled one for the same payload (loose lower-bound to avoid flakiness).

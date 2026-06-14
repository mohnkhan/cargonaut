---
description: "Task list for Feature 037 — Resume-from-Interrupted-Transfer binary wiring + SC-002 gate"
---

# Tasks: Resume-from-Interrupted-Transfer (Binary Wiring + SC-002 Gate)

**Input**: Design documents in `specs/037-resume-binary-wiring/`
**Prerequisites**: spec.md, plan.md, research.md, data-model.md, contracts/resume-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR gets a failing
test committed before the implementation that satisfies it; per-task git history shows the red
commit preceding the green commit (`T0xx (red): …` → `T0xx (green): …`).

**Organization**: grouped by user story (US1 P1 → US2 P2 → US3 P3) so each is independently
testable. Crate paths:
`crates/cargonaut-{transfer,core,ui-tui,bin}`.

## Format: `[ID] [P?] [Story] Description`

- **[P]** = different file, no dependency on another uncommitted task — may run in parallel.

---

## Phase 1: Setup (Shared Infrastructure)

- [ ] **T001** Add `portable-pty` to `[workspace.dependencies]` in root `Cargo.toml`
  (e.g. `portable-pty = "0.8"`); add `[dev-dependencies]` to `crates/cargonaut-bin/Cargo.toml`
  pulling `portable-pty`, `sha2 = { workspace = true }`, `tempfile = { workspace = true }`.
  Verify `cargo metadata` resolves and binary size is unaffected (dev-dep only).
- [ ] **T002** [P] Confirm `make tmpfs-status` shows `target/` linked to tmpfs before any
  build/test work (Constitution §V). Record nothing if green.

**Checkpoint**: deps available; build artifacts in tmpfs.

---

## Phase 2: Foundational (Blocking Prerequisites)

**⚠️ Blocks all user stories.**

- [ ] **T003** [P] (red) Add `cargonaut-transfer` test asserting a transfer with
  `CARGONAUT_TRANSFER_THROTTLE_MIBPS` set takes measurably longer than one without, for an
  identical payload (loose lower bound). Test in `crates/cargonaut-transfer/src/job.rs` tests
  module or `crates/cargonaut-transfer/tests/throttle.rs`. MUST fail (no throttle yet).
- [ ] **T004** (green) Implement the throttle hook in `crates/cargonaut-transfer/src/job.rs`:
  read `CARGONAUT_TRANSFER_THROTTLE_MIBPS` once at the start of `run_transfer` AND
  `run_transfer_with_state`; if set to a positive value, sleep between written chunks to cap
  throughput. Unset/invalid ⇒ no sleep. No public signature change. Make T003 pass. (R-002)
- [ ] **T005** [P] (red) Add `cargonaut-core` unit test for `ResumeOfferView` + the
  `pending_resume_views()` projection shape (construct an `App`, assert empty initially).
  In `crates/cargonaut-core/src/lib.rs` tests. MUST fail (types/methods don't exist).
- [ ] **T006** (green) Add the `ResumeOfferView` public struct (documented, `Debug, Clone`),
  the `pending_resumes: Vec<ResumableTransfer>` field on `App` (init empty in `App::new`), and
  `pub fn pending_resume_views(&self) -> Vec<ResumeOfferView>`. Make T005 pass. (data-model.md)

**Checkpoint**: engine throttle + core projection scaffolding in place.

---

## Phase 3: User Story 1 — Resume an interrupted copy after relaunch (P1) 🎯 MVP

**Goal**: A user can relaunch, be offered the unfinished transfer, choose resume, and get a
byte-identical destination.

**Independent test**: drive `scan_resume_offers` + `resume_offer` against a staged checkpoint
in a unit test; full binary path covered by US3's PTY test.

### Tests (write first, MUST fail)

- [ ] **T007** [P] [US1] (red) `cargonaut-core` test for `scan_resume_offers`: stage a valid
  sidecar + partial destination in a temp pane dir (reuse the transfer crate's checkpoint
  staging helpers / `submit_transfer`+cancel), construct `App` pointed at it, assert ≥1 offer
  returned and `pending_resumes` populated (contract C1); a second case with no sidecars
  returning empty, no error (C2); and a third case where a malformed/garbage
  `.cargonaut-transfer-*.json` sits in the dir — assert launch-scan returns no offer for it
  and does not error (FR-010, C3). In `crates/cargonaut-core/src/lib.rs` tests.
- [ ] **T008** [P] [US1] (red) `cargonaut-core` test for `resume_offer`: after a staged
  checkpoint, `resume_offer(0)` registers exactly one transfer, emits `TransferProgressed`,
  removes the offer (C6, C9); and the mismatch case fails safe with no successful transfer
  (C8). MUST fail.

### Implementation

- [ ] **T009** [US1] (green) Implement `App::scan_resume_offers` — scan each distinct pane cwd
  via `scan_resumable(self.local_fs.clone(), dir)`, store results in `pending_resumes`, return
  projected views. De-dup identical pane dirs (C4). Make T007 pass. (FR-001/002/003, R-004)
- [ ] **T010** [US1] (green) Implement `App::resume_offer(index)` — bounds-check; build `opts`
  from config (as `confirm_copy`); call `resume_transfer`; register job in
  `transfers`/`transfer_order`; remove offer; map engine error to a safe `Status`/error event
  (no corrupt dst). Make T008 pass. (FR-005/006/009, SC-005, R-001)
- [ ] **T011** [US1] Wire launch-time scan in `crates/cargonaut-ui-tui/src/lib.rs`: after the
  panes are built in `run_loop`, call `app.scan_resume_offers().await`; if non-empty, set
  `active_dialog = Some(ActiveDialog::Resume(ResumePromptDialog::new(views→summaries)))`. Add a
  private `ResumeOfferView → ResumableSummary` mapper. (FR-001/002, §III)
- [ ] **T012** [US1] Implement the real `[r]` dispatch in the `ActiveDialog::Resume` arm of
  `handle_key` (`crates/cargonaut-ui-tui/src/lib.rs` ~line 357): on `ResumeChoice::Resume`,
  call `app.resume_offer(idx).await`, then rebuild the dialog from `pending_resume_views()`
  (dismiss + `mode = Pane` if empty). Drop `#[allow(dead_code)]` on the `Resume` variant. (R-005)

**Checkpoint**: launch → prompt → `[r]` → resume works end-to-end at the library/UI level.

---

## Phase 4: User Story 2 — Choose start over / skip (P2)

**Goal**: the prompt offers all three actions and they behave correctly.

### Tests (write first, MUST fail)

- [ ] **T013** [P] [US2] (red) `cargonaut-core` test for `start_over_offer`: staged checkpoint
  → `start_over_offer(0)` removes the sidecar file, starts a fresh transfer, removes the offer
  (C10, C11). MUST fail.
- [ ] **T014** [P] [US2] (red) `cargonaut-core` test for `skip_offer`: `skip_offer(0)` starts
  no transfer, leaves the sidecar on disk, removes the in-memory offer; a re-scan finds it
  again (C12, C13). MUST fail.

### Implementation

- [ ] **T015** [US2] (green) Implement `App::start_over_offer(index)` — delete
  `checkpoint_path` (best-effort), parse src/dst URIs, `submit_transfer` (truncates dst),
  register job, remove offer. Make T013 pass. (FR-007, R-007)
- [ ] **T016** [US2] (green) Implement `App::skip_offer(index)` — bounds-checked removal from
  `pending_resumes`, no disk/transfer side effects. Make T014 pass. (FR-008)
- [ ] **T017** [US2] Extend the `ActiveDialog::Resume` dispatch in
  `crates/cargonaut-ui-tui/src/lib.rs` to route `ResumeChoice::StartOver → start_over_offer`
  and `ResumeChoice::Skip → skip_offer`, each followed by the rebuild-or-dismiss logic; verify
  multi-offer advance (US2 scenario 3).

**Checkpoint**: all three prompt choices functional.

---

## Phase 5: User Story 3 — SC-002 enforced end-to-end in CI (P3)

**Goal**: an automated, gated test drives the real binary through SIGKILL→resume→verify; CI
runs it.

### Tests (this IS the deliverable)

- [ ] **T018** [US3] (red) Implement `crates/cargonaut-bin/tests/resume_sigkill.rs`: replace
  the `#[ignore]`d stub with a real test that **self-skips when `CARGONAUT_PTY_TESTS` is
  unset**. When set: locate the binary via `env!("CARGO_BIN_EXE_cargonaut")`; create a ~128 MiB
  deterministic source temp file + a destination temp dir; spawn under `portable-pty` with
  `CARGONAUT_TRANSFER_THROTTLE_MIBPS` and a 1 MiB checkpoint interval (throwaway `--config` or
  env); send `F5` + confirm; poll until sidecar exists and partial dst `< src` then SIGKILL;
  relaunch; detect the resume prompt; send `r`; wait for completion; assert
  `sha256(src) == sha256(dst)` and resumed-bytes ≤ pre-kill offset + one checkpoint interval
  (SC-002). Determine the copy-confirm key by reading `ConfirmDialog::handle_key`. (FR-011, R-003/006)
- [ ] **T019** [US3] (green) Run `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test
  resume_sigkill` until green and stable across ≥3 runs (tune file size / throttle / poll
  thresholds as needed; record final constants). (SC-003)
- [ ] **T020** [US3] Enable the gate in CI: set `CARGONAUT_PTY_TESTS=1` (and any required
  throttle/size env) on the `cargo test --workspace --lib --tests` step in
  `.github/workflows/ci.yml`. (FR-012, R-008)

**Checkpoint**: SC-002 enforced end-to-end on every PR.

---

## Phase 6: Polish & Cross-Cutting

- [ ] **T021** [P] Run `make ci-local` (fmt + clippy `-D warnings` + test + release build +
  docs-gate) and fix anything red. Confirm no new clippy warnings, `missing_docs` satisfied.
- [ ] **T022** [P] Verify SC-004 not regressed: launching with no checkpoints shows no prompt
  and adds no perceptible startup delay (spot-check; the scan is one `list` per pane dir).
- [ ] **T023** Update `README.md` — At-a-Glance metrics (test count, feature count, binary
  size) + a Feature History row for 037.
- [ ] **T024** Append a `Learnings.md` section for 037 (≥3 bullets): the unwired-engine
  discovery, the throttle-env decision for deterministic SIGKILL timing, PTY harness gotchas,
  any `CARGONAUT_ALLOW_SSD_TARGET` waiver if used.
- [ ] **T025** Close the deferral paper-trail for issue #29: comment the resolution and close
  it on merge; remove/annotate the corresponding `ROADMAP.md` Tier-1 row. Note in the PR that
  this also lays the PTY harness groundwork referenced by #30.

---

## Dependencies & Execution Order

- **Setup (P1)** → **Foundational (P2)** → user stories.
- **US1 (P3 phase)** depends on Foundational (T006 projection, T004 throttle not strictly
  needed until US3 but lands early). US1 is the MVP.
- **US2** depends on Foundational + the UI dispatch scaffold from T012 (shares the
  `ActiveDialog::Resume` arm — T017 extends what T012 introduces).
- **US3** depends on US1 (the binary must actually resume) and the throttle (T004).
- **Polish** depends on all stories.

### TDD ordering (per task pair)

Each `(red)` task is committed failing before its `(green)` partner:
T003→T004, T005→T006, T007/T008→T009/T010, T013→T015, T014→T016, T018→T019.

### Parallel opportunities

- T003 ∥ T005 (different crates).
- T007 ∥ T008, T013 ∥ T014 (independent test cases, same file → coordinate or sequence the
  commits; mark [P] for authoring, serialize the file writes).
- T021 ∥ T022 (read-only checks); T023 ∥ T024 (different files).

## Implementation Strategy

1. Setup + Foundational (T001–T006).
2. **US1 (T007–T012) → STOP & VALIDATE**: resume works (MVP). 
3. US2 (T013–T017): start-over + skip.
4. US3 (T018–T020): the SC-002 gate — the originating issue #29 ask.
5. Polish (T021–T025): CI green, docs, deferral paper-trail.

## Notes

- Determine the copy-confirm key empirically from `ConfirmDialog::handle_key` before writing
  the PTY key sequence (T018).
- The `[no-docs]` commit marker is for infra/spec commits only; the implementation PR MUST
  modify both `README.md` and `Learnings.md` (docs-gate) — do NOT use `[no-docs]` on the
  feature commits.
- Commit messages: no Claude attribution trailers (CLAUDE.md).

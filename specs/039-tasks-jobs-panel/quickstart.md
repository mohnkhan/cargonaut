# Quickstart / Validation: Tasks/Jobs Panel Popup

**Feature**: 039-tasks-jobs-panel

How to build, test, and manually validate the tasks panel. Implementation detail
lives in the contracts and `tasks.md`; this is the run/validate guide.

## Prerequisites

- Repo checked out on branch `039-tasks-jobs-panel`.
- tmpfs target active (Constitution §V): `make tmpfs-status` shows the `target/`
  symlink. If not: `make tmpfs-setup`.

## Build & test (the gates)

```bash
make ci-local            # full pipeline: clippy -D warnings → test → release → docs-gate
# or, iterating on one crate:
make test                # cargo test --workspace (runs check-tmpfs first)
cargo test -p cargonaut-core jobs        # core projection + pause/resume tests
cargo test -p cargonaut-ui-tui tasks_panel   # widget + dispatch tests
```

Expected: all green, including the SC-003 integration test
`three_jobs_pause_one_others_continue` and the SC-007 end-to-end dispatch test.

## Automated validation scenarios

The following are the authoritative checks (see contracts for exact names):

1. **SC-001 — visibility**: `job_views()` after two copies returns two rows in
   submit order, each with `src`/`dst` and a `Running`/`Queued` status.
2. **SC-002 — isolated cancel**: `cancel_transfer(id)` stops that job; siblings
   keep running.
3. **SC-003 — three-job pause** (headline): submit 3 throttled copies, pause one,
   assert it stops while the other two complete; resume it and assert it
   completes. (`CARGONAUT_TRANSFER_THROTTLE_MIBPS` keeps copies in flight.)
4. **SC-004 — resume completes**: a paused job, resumed, reaches `Completed`.
5. **SC-005 — close is inert**: opening then closing the panel (Esc) leaves both
   panes and all transfers unchanged.
6. **SC-006 — terminal no-ops**: cancel/pause/resume on a completed/failed/
   cancelled job changes nothing and never panics.
7. **SC-007 — end-to-end**: `ShowTasksPanel` opens the modal; navigate; act;
   close. Covered by TUI dispatch + widget tests.

## Manual smoke test (optional)

```bash
make build && ./target/debug/cargonaut <dirA> <dirB>
```

1. In `dirA`, select a large file (or several) and press **F5** to copy to
   `dirB`. Start a few so transfers are in flight (use a large file or set
   `CARGONAUT_TRANSFER_THROTTLE_MIBPS=4` to slow them).
2. Press **F12** → the tasks panel opens listing the transfers with live
   progress.
3. Move the selection with **↑/↓** (or **j/k**).
4. Press **p** on one row → it shows **Paused**; the others keep advancing.
5. Press **r** on the paused row → it resumes and continues to completion.
6. Press **c** on a running row → it shows **Cancelled**; others unaffected.
7. Press **Esc** (or **F12**) → the panel closes; panes are exactly as before.
8. With no transfers, press **F12** → the panel shows an explicit empty state.

## References

- Behavior: [spec.md](./spec.md) (FR-001…FR-018, SC-001…SC-007)
- Decisions: [research.md](./research.md)
- Shapes: [data-model.md](./data-model.md)
- API: [contracts/core-api.md](./contracts/core-api.md),
  [contracts/tasks-panel-widget.md](./contracts/tasks-panel-widget.md)

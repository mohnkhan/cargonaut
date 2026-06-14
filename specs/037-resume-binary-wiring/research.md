# Research: Resume-from-Interrupted-Transfer (Binary Wiring + SC-002 Gate)

Phase 0 decisions. Each entry: **Decision**, **Rationale**, **Alternatives rejected**.

## R-001 — Where the resume seam lives

**Decision**: Put all resume orchestration in `cargonaut-core::App`. Add a UI-agnostic
projection type `ResumeOfferView` and methods `scan_resume_offers`, `resume_offer`,
`start_over_offer`, `skip_offer`, `pending_resume_views`. `cargonaut-ui-tui` builds the
existing `ResumePromptDialog` from `Vec<ResumeOfferView>` (mapping each into the widget's own
`ResumableSummary`) and routes the user's `ResumeChoice` back into the `App` methods.

**Rationale**: The codebase already enforces this seam — `App::active_progress()` returns a
`ProgressView` precisely so the UI never depends on `cargonaut-transfer` types (see
`crates/cargonaut-core/src/lib.rs` doc: "Lets the UI render a progress dialog without
depending on the transfer crate's types"). `dialog.rs` even documents the intent: "The App
builds these before constructing the dialog." Following the established pattern keeps the
dependency graph clean (`ui-tui → core → transfer`, never `ui-tui → transfer`) and keeps the
transfer-registry mutation (insert into `transfers`/`transfer_order`) in one place.

**Alternatives rejected**:
- *UI calls `scan_resumable`/`resume_transfer` directly*: would add a `ui-tui → transfer`
  dependency edge that the project deliberately avoids, and would split transfer-registry
  ownership across two crates.
- *A new `cargonaut-resume` crate*: over-engineered for ~4 methods; the logic is App state.

## R-002 — Deterministic mid-transfer SIGKILL

**Decision**: Add an opt-in per-chunk throughput throttle to the engine, controlled by the
environment variable `CARGONAUT_TRANSFER_THROTTLE_MIBPS`. When unset or unparsable, there is
no throttle (production default). When set, the copy loop sleeps between chunks to cap
throughput at the given MiB/s. Applied in both `run_transfer` and `run_transfer_with_state`.

**Rationale**: SC-003 requires the end-to-end test to be **deterministic** (no timing flakes).
On the dev host and CI, the destination lives in tmpfs (RAM-backed), so a copy of any
CI-reasonable size completes in a fraction of a second — far too fast to reliably issue a
mid-transfer SIGKILL by polling. A bounded throttle widens the in-flight window to several
seconds so the kill lands deterministically, and lets resume demonstrably copy *less* than the
full file (proving SC-002). The hook is read once at transfer start, costs nothing when unset,
and contains no `unsafe`. The clarify session explicitly sanctioned "a throughput throttle" as
the mechanism.

**Alternatives rejected**:
- *Large file, no throttle*: a multi-GiB file is slow and disk/space-heavy on CI runners and
  still races against tmpfs speed; rejected per clarify (modest-file decision) and SC-003.
- *Kill on first checkpoint sidecar appearance, no throttle*: the window between first
  checkpoint and completion on tmpfs can be tens of milliseconds — flaky.
- *Test-only feature flag (`#[cfg(test)]`)*: the test drives the **separate binary process**,
  so a `cfg(test)` path in the library is not compiled into that binary. An env var is the
  only mechanism that crosses the process boundary.

## R-003 — PTY harness, binary location, and kill semantics

**Decision**: Add `portable-pty` as a `cargonaut-bin` dev-dependency. Locate the binary under
test via Cargo's `env!("CARGO_BIN_EXE_cargonaut")` (provided automatically to integration
tests of the bin crate). Spawn it on a PTY slave, write key bytes to the PTY master to drive
F5/confirm/`r`, and SIGKILL the first run via the child handle (`Child::kill`, which is
SIGKILL on Unix).

**Rationale**: The binary opens an alternate-screen raw-mode TUI via `crossterm`, which
requires a real terminal; `portable-pty` provides one without platform-specific `openpty`
plumbing. `CARGO_BIN_EXE_<name>` is the canonical, build-graph-correct way to find the freshly
built binary (no hard-coded `target/` path — also respects the tmpfs symlink). The first run
must die abruptly (SIGKILL, not graceful) to mirror SC-002's "resume from SIGKILL".

**Alternatives rejected**:
- *`rexpect`/`expectrl`*: heavier expect-DSL dependencies; we only need raw read/write +
  spawn, which `portable-pty` covers directly. (Either remains a fallback if needed.)
- *`std::process::Command` without a PTY*: the TUI bails or misbehaves without a tty; key
  injection via stdin pipe is unreliable for a raw-mode crossterm app.
- *Hard-coded `target/release/cargonaut` path*: breaks under the tmpfs `target/` symlink and
  debug/release split; `CARGO_BIN_EXE_*` is correct.

## R-004 — Launch-time scan scope

**Decision**: On launch, scan both panel directories (`left` and `right` cwd) non-recursively
for orphan sidecars, de-duplicating if both panes start in the same directory. (Clarified
2026-06-15.)

**Rationale**: Either pane can be a copy destination, and a user may relaunch with paths in
either order. Non-recursive keeps the scan to one `list` per directory (the same `list` the
panes already perform), protecting the SC-004 startup budget. `scan_resumable` already takes a
single destination directory, so we call it once per pane dir.

**Alternatives rejected**: destination-pane-only (misses swapped relaunch); recursive walk
(startup-cost and unbounded-tree risk, conflicts with SC-004).

## R-005 — Offer ↔ dialog index synchronization

**Decision**: `App` holds `pending_resumes: Vec<ResumableTransfer>` in scan order. The dialog
is built from `pending_resume_views()` in the same order. `ResumePromptDialog::handle_key`
returns `(index, choice)`; the UI calls the matching `App` method with that index, the method
acts on `pending_resumes[index]` and removes it, then the UI **rebuilds** the dialog from the
now-shorter `pending_resume_views()` (or dismisses it if empty).

**Rationale**: Rebuilding after each action guarantees the index always refers to the current
pending list — no drift, no stale-handle bugs, and it naturally advances through multiple
offers (FR + US2 scenario 3). Simpler and less error-prone than threading stable IDs through
the widget, which doesn't carry an ID field.

**Alternatives rejected**: key offers by `checkpoint.job_id` string (the widget has no ID
field; would require widening `ResumableSummary`); mutate the dialog's internal vec in place
(would desync from `App` state).

## R-006 — Test sizing and timing

**Decision**: Source file ≈ 128 MiB of deterministic bytes; pass a test config with
`checkpoint_interval_mib = 1`; set `CARGONAUT_TRANSFER_THROTTLE_MIBPS ≈ 32`. The first run is
SIGKILLed ~1 s in (after polling shows the sidecar exists and the partial destination is
non-trivial but `< src_size`). The relaunch resumes to completion (throttle still applied);
total test wall-clock target a few seconds, well under the 15-minute CI job timeout.

**Rationale**: 128 MiB / 32 MiB·s⁻¹ ≈ 4 s of transfer — a comfortable window to catch
mid-flight, while 1 MiB checkpoints mean the resume re-copies ≤1 MiB beyond the last
checkpoint (the SC-002 assertion). Deterministic and fast. Sizes are tunable constants in the
test if a runner proves slower/faster.

**Alternatives rejected**: literal 1–4 GiB (slow, disk-heavy — rejected per clarify); tiny
file with API-forced checkpoint (copy finishes before the kill).

## R-007 — Start-over semantics

**Decision**: "Start over" deletes the checkpoint sidecar, then calls `submit_transfer`
(fresh). The engine's `submit_transfer` already opens the destination with `Truncate`, so the
partial destination is overwritten from offset 0 — no separate partial-file delete needed.

**Rationale**: Minimal and correct; relies on documented engine behavior
(`dst.write_stream(0, Truncate)`). Removing the sidecar first prevents the stale checkpoint
from being re-offered on the next launch.

**Alternatives rejected**: manually unlinking the partial destination (redundant given
truncate-on-open).

## R-008 — CI enablement

**Decision**: Set `CARGONAUT_PTY_TESTS=1` (and the throttle/size env as needed) on the
existing `cargo test --workspace --lib --tests` step in `.github/workflows/ci.yml`. No new
job. (Clarified 2026-06-15.)

**Rationale**: One pipeline, minimal plumbing; the modest test sizing keeps the step within
budget. Keeping the test opt-in means local `cargo test` stays fast for everyday development
while CI still enforces the SC-002 gate on every PR (Constitution §IV: every SC needs a CI
gate).

**Alternatives rejected**: separate CI job (more workflow plumbing, longer pipeline);
nightly-only (weakens the gate — a regression could merge between nightlies).

# Feature Specification: Resume-from-Interrupted-Transfer (Binary Wiring + SC-002 Gate)

**Feature Branch**: `037-resume-binary-wiring`

**Created**: 2026-06-15

**Status**: Draft

**Input**: User description: "Wire end-to-end resume-from-interrupted-transfer at the binary level and close the SC-002 binary-level gate (issue #29 / T1.08)."

## Overview

The transfer engine already knows how to resume an interrupted copy: `cargonaut-transfer`
ships `scan_resumable` (discovers orphan checkpoint sidecars), `resume_transfer` (continues
from a checkpoint with CRC validation), and the `ResumePromptDialog` widget exists and is
unit-tested in `cargonaut-ui-tui`. **None of it is reachable by a user.** On launch the
binary never calls `scan_resumable`, never constructs the resume dialog, and the `[r]` key
handler is an explicit no-op stub that simply dismisses the dialog (`// T1.14/T1.15: actually
dispatch the resume here. For Phase 1 MVP we just dismiss`). `resume_transfer` is never
invoked outside its own unit tests.

The consequence is a hole in a NON-NEGOTIABLE guarantee: **SC-002 (resume from a SIGKILL
within one checkpoint interval)** is proven only at the transfer-crate level, not end-to-end
through the running binary. This feature connects the already-built pieces so a real user can
resume an interrupted transfer, and closes the binary-level SC-002 regression gate with an
automated test.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Resume an interrupted copy after relaunch (Priority: P1)

A user starts copying a large file (e.g. a multi-gigabyte archive) from one panel to the
other. Partway through, the process dies — the machine loses power, the terminal is closed,
or the process is killed. When the user launches cargonaut again pointed at the same
destination, they are told an unfinished transfer was found and offered to resume it.
Choosing resume continues the copy from where it stopped (not from zero) and finishes with a
destination that is byte-for-byte identical to the source.

**Why this priority**: This is the entire user-visible value of the feature and the reason
SC-002 exists. Without it, the checkpoint sidecars the engine writes are dead weight — a user
who suffers an interruption has no way to benefit from them and must recopy from scratch.
This is the MVP: it alone delivers a complete, demonstrable capability.

**Independent Test**: Start a copy of a large file, kill the process mid-transfer, relaunch
against the same destination, choose resume, and confirm the destination's checksum matches
the source and that the resumed run transferred materially less than the full file size.

**Acceptance Scenarios**:

1. **Given** an interrupted transfer left a valid checkpoint sidecar and a partial
   destination file, **When** the user launches cargonaut pointed at that destination
   directory, **Then** a resume prompt appears listing the unfinished transfer with its
   source, destination, and progress so far.
2. **Given** the resume prompt is showing an unfinished transfer, **When** the user chooses
   resume, **Then** the copy continues from the checkpointed offset and, on completion, the
   destination is byte-for-byte identical to the source (verified by checksum).
3. **Given** a resume is in progress, **When** it completes, **Then** the checkpoint sidecar
   is removed and the normal completion feedback (the same shown for a fresh copy) is
   presented.
4. **Given** the destination or source changed since the checkpoint was written (size or
   content no longer matches the recorded fingerprint), **When** the user attempts resume,
   **Then** the system refuses to silently resume and surfaces the mismatch rather than
   producing a corrupt file.

---

### User Story 2 - Choose what to do with a found transfer (Priority: P2)

When offered an unfinished transfer on launch, the user can decide between three actions:
resume it, start it over from scratch (discarding the old progress), or skip the offer for
now and deal with it later.

**Why this priority**: Resume alone (P1) is a complete capability, but real users need an
escape hatch. A checkpoint can become stale or unwanted; forcing resume-or-nothing is a
worse experience than offering start-over and skip. This rounds out the prompt into the
full three-way choice the dialog was designed for.

**Independent Test**: With a checkpoint present, exercise each of the three choices and
confirm: resume continues, start-over discards the checkpoint and copies fresh, and skip
leaves the checkpoint untouched so it is offered again on the next launch.

**Acceptance Scenarios**:

1. **Given** the resume prompt lists an unfinished transfer, **When** the user chooses start
   over, **Then** the old checkpoint and partial destination are discarded and a fresh copy
   begins from the beginning.
2. **Given** the resume prompt lists an unfinished transfer, **When** the user chooses skip
   (or dismisses the prompt), **Then** no transfer is started, the checkpoint sidecar is left
   in place, and the same offer reappears on the next launch.
3. **Given** multiple unfinished transfers are found, **When** the prompt is shown, **Then**
   each is listed and the user can act on the focused one, with the prompt advancing through
   the remaining offers.

---

### User Story 3 - SC-002 enforced end-to-end in CI (Priority: P3)

A maintainer needs confidence that the whole resume pipeline — launch detection, prompt,
resume dispatch, engine continuation, integrity verification — keeps working as the code
evolves. An automated test drives the real binary through a kill-and-resume cycle and fails
the build if the resumed copy is wrong or incomplete.

**Why this priority**: The capability (P1/P2) is what users get; this gate is what keeps it
from silently breaking. It is lower priority only because it protects the feature rather than
delivering it, and it depends on P1 existing first. Per Constitution §IV, SC-002 is
NON-NEGOTIABLE and every Success Criterion must have a CI gate, so this is required for the
feature to be considered complete — not optional polish.

**Independent Test**: Run the gated end-to-end test; it spawns the binary, starts a copy of a
large file, kills it mid-flight, relaunches, drives the resume choice, waits for completion,
and asserts a source/destination checksum match — all without human interaction.

**Acceptance Scenarios**:

1. **Given** the end-to-end resume test, **When** it runs, **Then** it spawns the real binary,
   begins a copy, kills the process mid-transfer, relaunches against the same destination,
   selects resume, and asserts the destination checksum matches the source.
2. **Given** the default test suite (`cargo test`), **When** it runs without the opt-in
   enabled, **Then** the expensive end-to-end test is skipped so routine test runs stay fast.
3. **Given** the continuous-integration pipeline, **When** it runs, **Then** the end-to-end
   resume test is exercised (opt-in enabled) so a regression in any link of the resume chain
   blocks merge.

---

### Edge Cases

- **No checkpoints present**: launching with no resumable transfers shows no prompt and goes
  straight to the normal two-panel view — the common case must not regress or add latency.
- **Stale/mismatched checkpoint**: source or destination changed since checkpoint creation —
  resume must fail safe (refuse and report) rather than write a corrupt destination.
- **Corrupt or unreadable sidecar**: a malformed checkpoint file must not crash launch; it is
  ignored (and ideally reported) so the user still reaches the panels.
- **User skips, then relaunches**: a skipped offer must persist and reappear; skipping must
  not delete the checkpoint.
- **Completion during resume**: on successful resume the sidecar is cleaned up so the same
  transfer is not offered again on the next launch.
- **Kill before first checkpoint**: if the process dies before any checkpoint interval
  elapsed, there may be nothing to resume — behavior is to offer nothing (or offer a
  zero-progress restart), never to claim false progress.
- **Multiple resumable transfers**: more than one orphan checkpoint in the destination is
  listed and handled one at a time.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: On startup, the binary MUST scan the relevant destination/working location(s)
  for orphan checkpoint sidecars before presenting the normal interactive view.
- **FR-002**: When one or more resumable transfers are found, the system MUST present the
  resume prompt listing each found transfer with its source, destination, and progress so far.
- **FR-003**: When no resumable transfers are found, the system MUST proceed directly to the
  normal view with no prompt and no user-perceptible added startup delay.
- **FR-004**: The resume prompt MUST offer three actions per listed transfer: resume, start
  over, and skip/cancel.
- **FR-005**: Choosing resume MUST continue the transfer from the checkpointed offset (not
  from the beginning) and report progress through the same transfer-progress feedback used
  for a fresh copy.
- **FR-006**: On successful completion of a resumed transfer, the system MUST verify
  destination integrity against the source and MUST remove the checkpoint sidecar.
- **FR-007**: Choosing start over MUST discard the existing checkpoint (and partial
  destination) and begin a fresh copy from the beginning.
- **FR-008**: Choosing skip/cancel MUST start no transfer and MUST leave the checkpoint
  sidecar in place so the offer reappears on the next launch.
- **FR-009**: If a checkpoint's recorded source/destination fingerprint no longer matches the
  current files, resume MUST fail safe — refuse to produce a corrupt destination and surface
  the reason — rather than proceeding silently.
- **FR-010**: A malformed or unreadable checkpoint sidecar MUST NOT crash or block launch; it
  is skipped so the user still reaches the normal view.
- **FR-011**: An automated end-to-end test MUST drive the real binary through a
  start → kill-mid-transfer → relaunch → resume → completion cycle and assert a source/
  destination checksum match, validating SC-002 at the binary level.
- **FR-012**: The end-to-end resume test MUST be opt-in (gated) so it does not run during the
  default test suite, and the continuous-integration pipeline MUST enable that opt-in so the
  gate is enforced on every change.
- **FR-013**: All resume UI MUST reuse the existing shared dialog widgets and the single
  keymap source of truth (no ad-hoc layouts or hardcoded keybindings), per Constitution §III.

### Key Entities *(include if feature involves data)*

- **Checkpoint sidecar**: a small metadata record written alongside an in-progress
  destination that captures enough state (source/destination identity, size, integrity
  fingerprint, bytes written, chunk checksums, checkpoint interval) to resume the copy. Lives
  next to the destination and is removed on successful completion. *(Already implemented;
  consumed, not redesigned, by this feature.)*
- **Resumable-transfer offer**: the user-facing summary of one found checkpoint shown in the
  prompt — source, destination, progress so far, and whether source/destination still appear
  intact.
- **Resume choice**: the user's decision for a given offer — resume, start over, or
  skip/cancel.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user whose large-file copy was interrupted can, by relaunching and choosing
  resume, obtain a destination that is byte-for-byte identical to the source in 100% of runs
  where the source and destination are unchanged since the interruption.
- **SC-002**: A resumed transfer re-copies no more than one checkpoint interval's worth of
  data beyond what had already been written before the interruption (i.e. resume continues
  from the last checkpoint, not from zero) — the binary-level enforcement of Constitution §IV
  SC-002.
- **SC-003**: The end-to-end kill-and-resume test passes deterministically (no ENOSPC or
  timeout flakes within its configured bounds) when run in the opt-in mode, and is exercised
  by CI on every change.
- **SC-004**: Launching with no resumable transfers present shows no prompt and adds no
  user-perceptible startup delay versus the current behavior (cold-cache startup stays within
  the existing ≤150 ms SC-004 budget).
- **SC-005**: A resume attempt against a mismatched/changed source or destination never
  produces a corrupt destination — it fails safe and reports the mismatch in 100% of such
  cases.

## Assumptions

- The transfer engine (`scan_resumable`, `resume_transfer`, checkpoint sidecar format,
  CRC-chain validation) is correct and complete as already unit-tested; this feature wires it
  into the binary and does not redesign it.
- The `ResumePromptDialog` widget's rendering and key handling are correct as already
  unit-tested; this feature constructs it, shows it on launch, and dispatches its outcomes.
- The location scanned for checkpoints on launch is the destination/working directory(ies)
  derived from the panels the binary is started with — the same directories a copy would
  target. (To be confirmed in clarify/plan: exact scan scope.)
- "Multi-gigabyte" test file size is large enough to guarantee the copy is still in flight
  when the kill signal is sent on the CI runner; the exact size and timing are tuned during
  implementation to stay within runner disk/time limits (the destination tree lives in tmpfs
  per Constitution §V on the dev host; CI is exempt).
- The opt-in mechanism for the expensive test is an environment flag; CI sets it. Default
  developer `cargo test` runs leave it unset.
- TDD per Constitution §II applies: each FR gets a failing test committed before the
  implementation that satisfies it, with red-before-green commit history.
- Resume-on-launch is additive to the existing startup flow; the no-checkpoints path is the
  hot path and must remain unchanged in observable behavior.

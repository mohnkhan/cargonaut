# Feature Specification: Tasks/Jobs Panel Popup

**Feature Branch**: `039-tasks-jobs-panel`

**Created**: 2026-06-15

**Status**: Clarified

**Input**: User description: "Tasks/jobs panel popup (issue #32, Feature 028 follow-up, FR-016 / NFR-004). F12 / :jobs opens a modal list panel over the App transfer registry snapshot, listing live transfer jobs with per-row actions: cancel (existing CancelCurrentTransfer path), pause, and resume (cooperative pause flag + cancellation-token re-arm). The panel is built as a shared modal list dialog (Constitution §III shared dialog widgets) over the existing App transfer registry. Acceptance: F12/:jobs opens a panel listing live jobs with working pause/resume/cancel; integration test submits 3 jobs, pauses one, asserts the other two continue."

## Clarifications

### Session 2026-06-15

The following decisions were resolved as defaults (informed by the existing
transfer-engine architecture); they were proposed for confirmation but not
explicitly user-selected, so the lowest-risk, architecture-aligned option was
recorded in each case. Any can be revisited before `/speckit-plan` is acted on.

- Q: How should pause/resume work, given the engine already has a tested
  resume-from-checkpoint path but no in-loop pause primitive? → A (default):
  **Checkpoint + re-arm** — pausing signals the running transfer's existing
  cancellation token (which leaves the checkpoint sidecar in place) and marks the
  registry entry as user-paused so it renders as Paused (not Cancelled); resuming
  restarts the transfer through the existing `resume_transfer` checkpoint path
  with a fresh cancellation token. This reuses the tested resume machinery and
  avoids an invasive change to the copy loop; a resumed transfer loses at most one
  checkpoint interval. (FR-010, FR-011, FR-016, FR-017)
- Q: Which transfers should the panel list? → A (default): **All session jobs** —
  every entry in the registry is listed, including completed, failed, and
  cancelled transfers, distinguished by state; terminal rows are visible but their
  per-row actions are no-ops. (FR-002, FR-004, FR-012)
- Q: What in-panel keys drive the per-row actions? → A (default): **c / p / r
  mnemonics** — `c` cancel, `p` pause, `r` resume, Up/Down or `j`/`k` to move the
  selection, Escape or the tasks shortcut to close. Per-row keys are handled
  inside the dialog widget (as the existing confirm dialog handles its own keys).
  (FR-006, FR-018)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - See what transfers are running (Priority: P1)

A user who has started one or more file transfers wants to know what is
currently happening: which transfers exist, what each is doing (source,
destination, how far along), and whether any have finished or failed. They press
the tasks shortcut (F12) or type the `:jobs` command, and a panel appears listing
every transfer the session knows about, each row showing enough to identify it
and its current state and progress. They press Escape (or the same shortcut) to
dismiss the panel and return to browsing.

**Why this priority**: This is the core of the feature and the minimum that
satisfies NFR-004 (operational visibility into running transfers). Today F12 /
`:jobs` only shows a status-bar count placeholder; the panel turning that into a
real, readable list of jobs is the headline value and is independently useful
even with no per-row actions. It is the MVP.

**Independent Test**: Submit two or more transfers, open the panel via the
tasks action, and assert the panel lists exactly those transfers with their
identifying details and current state/progress; dismiss it and assert the panel
closes and the underlying panes are unchanged. Fully testable with injected key
input against application state, with no dependency on any per-row action.

**Acceptance Scenarios**:

1. **Given** the session has two active transfers, **When** the user invokes the
   tasks action, **Then** a panel opens listing both transfers, each row
   identifying its source and destination and showing its current state and
   progress.
2. **Given** the tasks panel is open, **When** the user moves the selection up
   and down, **Then** the highlighted row changes accordingly and stays within
   the list bounds.
3. **Given** the tasks panel is open, **When** the user presses Escape (or the
   tasks shortcut again), **Then** the panel closes, no transfer is affected, and
   both panes remain exactly as they were.
4. **Given** the session has no transfers at all, **When** the user invokes the
   tasks action, **Then** the panel opens and clearly indicates there are no
   transfers, rather than appearing broken or empty without explanation.
5. **Given** the tasks panel is open while a transfer is making progress,
   **When** that transfer advances, **Then** the panel's displayed progress for
   that row reflects the advance without the user reopening the panel.

---

### User Story 2 - Cancel a transfer from the panel (Priority: P2)

A user looking at the list of transfers decides one of them should stop — it was
started by mistake, or is no longer needed. They highlight that transfer's row in
the panel and trigger cancel. The selected transfer stops, its row reflects that
it was cancelled, and the other transfers are unaffected.

**Why this priority**: Cancelling a specific job is the most-requested control
and reuses the existing cancellation path, so it is high value at low added risk.
It builds directly on US1 (you must be able to see and select a job before you
can cancel it) but is independently demonstrable once the list exists.

**Independent Test**: Submit two transfers, open the panel, select one, trigger
cancel, and assert that the selected transfer reaches a cancelled state while the
other transfer continues running. Testable with injected input against
application state.

**Acceptance Scenarios**:

1. **Given** the tasks panel lists at least two running transfers, **When** the
   user selects one and triggers cancel, **Then** the selected transfer stops and
   the other transfer continues running unaffected.
2. **Given** a transfer has been cancelled from the panel, **When** the panel
   updates, **Then** that transfer's row reflects the cancelled state and it is
   not presented as still running.
3. **Given** the selected row is a transfer that has already finished or failed,
   **When** the user triggers cancel, **Then** nothing harmful happens (no crash,
   no effect on other transfers) and the already-terminal row is unchanged.

---

### User Story 3 - Pause and resume a transfer from the panel (Priority: P3)

A user wants to temporarily hold one transfer — to free bandwidth for another, or
to wait for something — without losing the progress already made. They select the
transfer's row and trigger pause; that transfer stops making progress and its row
shows it is paused, while every other transfer keeps running. Later they select
the paused transfer and trigger resume; it continues from where it left off.

**Why this priority**: Pause/resume is the most valuable for managing concurrent
work but also the most complex (it requires cooperative pausing and re-arming the
transfer so it can continue). The happy paths in US1 and US2 deliver value on
their own, so this is sequenced last while still being a required part of the
feature's acceptance.

**Independent Test**: Submit three transfers, open the panel, pause exactly one,
and assert that the paused transfer makes no further progress while the other two
continue to completion; then resume the paused one and assert it continues and
finishes. This is the issue's named acceptance test.

**Acceptance Scenarios**:

1. **Given** three transfers are running, **When** the user pauses exactly one of
   them, **Then** the paused transfer stops advancing and the other two continue
   running toward completion.
2. **Given** a transfer has been paused, **When** the panel updates, **Then** its
   row reflects the paused state, distinct from running, cancelled, and finished.
3. **Given** a transfer is paused, **When** the user selects it and triggers
   resume, **Then** it continues making progress from where it paused (no lost or
   re-done work beyond at most one checkpoint interval) and eventually completes.
4. **Given** the selected row is a transfer that is not in a pausable state (for
   example already finished, failed, or cancelled), **When** the user triggers
   pause or resume, **Then** the action is a no-op with no harmful effect.

---

### Edge Cases

- **Empty list**: Invoking the tasks action with no transfers opens the panel
  with an explicit "no transfers" indication (US1 scenario 4), not a blank or
  broken-looking modal.
- **Job finishes while selected**: A transfer that completes, fails, or is
  cancelled while its row is selected updates in place; the selection remains
  valid and stays within bounds.
- **List changes while open**: If the set of transfers changes while the panel is
  open (a new one starts, or one ends), the selection index stays within the
  current bounds and never points past the end of the list.
- **Action on a terminal job**: Cancel/pause/resume on a job that is already in a
  terminal state (completed, failed, cancelled) is a safe no-op (US2 sc. 3, US3
  sc. 4).
- **Resume a non-paused job / pause a paused job**: Triggering resume on a running
  job, or pause on an already-paused job, has no harmful effect.
- **Another modal open**: The tasks panel cannot be opened on top of a different
  modal; only one modal is active at a time, and invoking the tasks action while
  the panel is already open does not stack a second panel.
- **Long source/destination paths**: Rows with paths too long for the panel width
  are truncated for display without breaking the layout or hiding the job's state.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST open a modal tasks panel when the user invokes the
  tasks action (F12 or the `:jobs` command), replacing the current status-bar
  placeholder behavior.
- **FR-002**: The tasks panel MUST list the transfers known to the current
  session, with one row per transfer, built from the application's existing
  transfer registry (no parallel bookkeeping introduced for display).
- **FR-003**: Each row MUST identify its transfer (source and destination) and
  display the transfer's current state and progress in a human-readable form.
- **FR-004**: The displayed state MUST distinguish, at minimum, the states:
  queued/running, paused, completed, failed, and cancelled.
- **FR-005**: While the panel is open, it MUST capture keyboard input so that
  navigation and per-row action keys act on the panel rather than triggering
  other application shortcuts.
- **FR-006**: Users MUST be able to move the selection between rows; the selection
  MUST always remain within the bounds of the current list, including when the
  underlying list changes while the panel is open.
- **FR-007**: Pressing Escape (or invoking the tasks action again) MUST close the
  panel without affecting any transfer and without modifying either pane.
- **FR-008**: While the panel is open, the displayed progress and state for each
  row MUST reflect ongoing changes to the underlying transfers without requiring
  the user to reopen the panel.
- **FR-009**: Users MUST be able to cancel the selected transfer from the panel;
  cancellation MUST use the application's existing transfer-cancellation path and
  MUST affect only the selected transfer.
- **FR-010**: Users MUST be able to pause the selected transfer from the panel; a
  paused transfer MUST stop making progress while every other transfer continues
  unaffected. Pause is realized by signalling the transfer's existing cancellation
  mechanism (which preserves its resume checkpoint) and marking the registry entry
  as user-paused; the row MUST render as paused, distinct from cancelled.
- **FR-011**: Users MUST be able to resume a paused transfer from the panel; on
  resume the transfer MUST continue from its preserved checkpoint, losing at most
  one checkpoint interval of progress, and MUST be able to run to completion.
  Resume re-arms the transfer with a fresh cancellation handle so it can later be
  paused or cancelled again.
- **FR-012**: Per-row actions (cancel, pause, resume) MUST be safe no-ops when the
  selected transfer is not in a state that supports the action (cancel/pause on a
  transfer that has already completed, failed, or been cancelled; resume on a
  transfer that is not paused).
- **FR-013**: The system MUST allow only one modal at a time; invoking the tasks
  action while the panel (or another modal) is open MUST NOT stack a second
  panel.
- **FR-014**: When the session has no transfers, the panel MUST open and clearly
  indicate the empty state rather than appearing broken.
- **FR-015**: The tasks panel MUST be implemented using the shared dialog/modal
  widgets (per Constitution §III), not an ad-hoc layout in feature code.
- **FR-016**: Pausing a transfer MUST preserve its on-disk resume checkpoint so
  that a subsequent resume does not restart the transfer from the beginning.
- **FR-017**: A user-paused transfer MUST be distinguishable in the registry from
  one cancelled by the user, so the panel can render the correct state and so that
  resume is offered only for paused transfers.
- **FR-018**: Within the panel, the keys MUST be: Up/Down (or `j`/`k`) to move the
  selection; `c` to cancel, `p` to pause, `r` to resume the selected row; Escape
  (or the tasks shortcut) to close. These per-row keys are consumed by the panel
  while it is open (per FR-005).

### Key Entities *(include if feature involves data)*

- **Tasks Panel**: The transient modal state representing an open jobs view.
  Holds the current selection position and is rendered from a read-only snapshot
  of the transfer registry. Exists only while the panel is open.
- **Transfer Job (registry entry)**: An existing record of one transfer the
  session is managing — its identity, source, destination, mode, observable
  state/progress, and the means to cancel it. Consumed by the panel for display
  and as the target of per-row actions; not redefined by this feature.
- **Transfer State**: The existing observable lifecycle of a transfer
  (queued/running with progress, paused, completed, failed, cancelled) that the
  panel renders per row and that per-row actions transition.
- **User-Paused Marker**: A per-transfer indication, held in the registry, that a
  transfer was paused by the user (as opposed to cancelled). It distinguishes a
  pause from a cancellation so the panel renders the correct state and offers
  resume only for paused transfers; consumed alongside the transfer's observable
  state when rendering a row.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can see every transfer the session is managing, with its
  current state and progress, from a single panel opened by one action — without
  consulting logs or the status bar.
- **SC-002**: Cancelling a transfer from the panel stops exactly that transfer
  and leaves all other transfers running (zero collateral effect on siblings).
- **SC-003**: With three transfers running, pausing exactly one stops that one's
  progress while the other two continue to completion; this is verified by an
  automated test that passes in CI (the issue's named acceptance test).
- **SC-004**: A paused transfer, once resumed, completes successfully, having lost
  at most one checkpoint interval of progress.
- **SC-005**: Closing the panel (Escape or re-invoking the action) leaves both
  panes and every transfer byte-for-byte unaffected (zero side effects on close).
- **SC-006**: Every per-row action invoked on a terminal (completed/failed/
  cancelled) transfer is a no-op — 100% of such invocations leave all transfers
  unchanged and never crash.
- **SC-007**: The end-to-end behavior (open → navigate → act → close, including
  the three-job pause scenario) is covered by automated tests that pass in CI.

## Assumptions

- The application already maintains a canonical registry of the session's
  transfers, with per-transfer identity, source/destination, an observable
  state/progress channel, and an existing cancellation mechanism. This feature
  reads that registry for display and drives the existing mechanisms; it does not
  introduce a parallel source of truth.
- A single canonical cancellation path already exists (the same path the current
  "cancel current transfer" action uses); the panel's cancel routes through it
  rather than adding a new one.
- The transfer engine already checkpoints progress periodically (used by resume);
  pause/resume builds on that so resuming loses at most one checkpoint interval.
- The panel is built on the shared modal/list dialog building blocks (Constitution
  §III), reusing the established modal lifecycle (open, capture input, render,
  dismiss) used by existing dialogs.
- Exactly one modal is active at a time; the application already enforces this for
  existing dialogs and the tasks panel participates in the same discipline.
- The tasks action is already bound (F12 / `:jobs`) and dispatched today to a
  placeholder; this feature replaces the placeholder's behavior, not the binding.
- The current phase targets the local filesystem backend; the panel and its
  actions are not specific to a backend and should not preclude remote/archive
  transfers, but those are not exercised here.

## Out of Scope

- Persisting transfer history across application restarts; the panel reflects only
  the current session's registry.
- Bulk/multi-select actions (pause/cancel several jobs at once); actions operate on
  the single selected row.
- Reordering or reprioritising the transfer queue from the panel.
- Adding new transfers from within the panel (it manages existing ones only).
- Detailed per-transfer drill-down views (per-file logs, error stack traces)
  beyond the summarized state/progress shown per row.
- Throttling/bandwidth-limit controls from the panel.
- Remote/archive backend-specific behaviors.

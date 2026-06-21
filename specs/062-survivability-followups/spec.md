# Feature Specification: Survivability Follow-ups (Feature 061 polish)

**Feature Branch**: `062-survivability-followups`

**Created**: 2026-06-21

**Status**: Draft

**Input**: Implement issue #90 — the deferred polish from Feature 061: in-session
recovery for input-handler and background-task faults, a dedicated About view,
and a production `unwrap`/`expect` audit.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A fault handling one keystroke doesn't end the session (Priority: P1)

A user presses a key (or clicks) and the handler hits an unexpected fault. Today
that fault escalates to a clean *fatal* exit (terminal restored + crash report).
Instead, like the render path already does, the fault is contained: the session
shows a dismissible error and stays interactive so the user can retry.

**Why this priority**: Completes the "recover & continue" guarantee from Feature
061 for the second of the two named interactive surfaces (input, alongside
render). Highest remaining survivability value.

**Independent Test**: Inject a one-shot input-handler fault; confirm the session
remains interactive afterward and a status message is shown; no crash file.

**Acceptance Scenarios**:

1. **Given** a running session, **When** a fault occurs while handling a single
   key or mouse event, **Then** it is contained, an error is surfaced, and the
   session stays interactive (no process exit, no crash file).
2. **Given** repeated input faults, **When** they recur, **Then** the app does
   not spin or wedge (bounded, like the render path).

---

### User Story 2 - A crashed transfer shows as Failed, app stays usable (Priority: P1)

A background file transfer hits a fault mid-run. The application keeps running
(already true under unwinding), and the affected job is shown as **Failed** in
the tasks panel rather than silently hanging in its last state.

**Why this priority**: Closes FR-008's remaining half — the process already
survives a task panic (tokio isolates it), but the job's state must reflect the
failure so the user isn't misled.

**Independent Test**: Inject a panic into a transfer task; confirm the job
transitions to `Failed` and the rest of the app is fully usable.

**Acceptance Scenarios**:

1. **Given** an in-flight transfer, **When** its task faults, **Then** the job's
   observable state becomes `Failed` and the application remains usable.
2. **Given** other concurrent transfers, **When** one task faults, **Then** the
   others are unaffected.

---

### User Story 3 - A dedicated About screen from the menu (Priority: P2)

A user opens the application menu and picks "About" to see a focused screen with
the application name, version, author, copyright, and license — complementing the
F1 Help section and `--version` that already exist.

**Why this priority**: Completes FR-012 of Feature 061 (the dedicated view half).
Independent and self-contained.

**Independent Test**: Open the menu → About; confirm the modal shows all identity
fields and dismisses with Esc/Enter.

**Acceptance Scenarios**:

1. **Given** the main view, **When** the user opens the menu and selects About,
   **Then** a modal shows name/version/author/copyright/license.
2. **Given** the About modal is open, **When** the user presses Esc or Enter,
   **Then** it closes and normal navigation resumes.

---

### User Story 4 - Fewer avoidable panics in normal operation (Priority: P3)

Risky `unwrap`/`expect` calls on the normal-operation hot paths are converted to
handled errors that flow through the existing status/error mechanism, so ordinary
conditions (a vanished file, a races directory read) degrade gracefully instead
of risking a panic.

**Why this priority**: Reduces the panic surface so the (now safe) crash path
fires less often. Lower priority because it is a hardening audit, not a visible
capability.

**Independent Test**: Review the audited sites; confirm each converted site
returns/handles an error and is covered by a test where practical.

**Acceptance Scenarios**:

1. **Given** an audited hot path, **When** an expected error condition occurs,
   **Then** it is handled (status/log) rather than panicking.

---

### Edge Cases

- An input fault that recurs every event must not wedge the UI (bounded recovery,
  mirroring the render escalation).
- A transfer that faults *after* already reaching a terminal state must not be
  downgraded/overwritten incorrectly.
- The About modal must not open over another modal in a conflicting way (respects
  existing single-active-dialog model).
- The unwrap audit must not change behavior on success paths — only failure
  handling.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A fault while handling a single input (key or mouse) event MUST be
  contained — logged at error level, surfaced as a dismissible status message,
  and leave the session interactive; it MUST NOT write a crash-report file.
- **FR-002**: Input-fault recovery MUST be bounded so a persistently failing
  handler escalates to a clean fatal exit rather than spinning.
- **FR-003**: A fault in a background transfer task MUST transition that job's
  observable state to `Failed` without terminating the application or affecting
  other jobs.
- **FR-004**: Users MUST be able to open a dedicated About view from the
  application menu showing name, version, author, copyright, and license.
- **FR-005**: The About view MUST dismiss with Esc and Enter and restore normal
  pane navigation.
- **FR-006**: Identified avoidable panic sites on normal-operation hot paths MUST
  be converted to handled errors/log without changing success-path behavior.
- **FR-007**: All Feature 061 crash-safety guarantees MUST continue to hold
  (fatal faults restore the terminal and write a report; recovered render faults
  keep the session alive).

### Key Entities

- **About view**: a modal rendering the existing `diag::about_lines()` identity.
- (No new persistent data; reuses Feature 061's `diag` module + transfer state.)

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A forced input-handler fault leaves the session interactive in 100%
  of test runs; no crash file is written for it.
- **SC-002**: A forced transfer-task fault results in a `Failed` job and a usable
  app in 100% of test runs.
- **SC-003**: The user can reach the About view from the menu within two
  keystrokes and read name/version/author/copyright/license.
- **SC-004**: The stripped release binary stays ≤ 8 MiB; the full test suite
  (incl. the Feature 061 gated PTY crash test) stays green.

## Assumptions

- Reuses Feature 061's `cargonaut-core::diag` seams (`take_captured_panic`,
  `maybe_inject_panic("input"|"task")`, `about_lines`) — already present.
- Input recovery mirrors the render boundary (catch + status + bounded
  escalation) using `futures::FutureExt::catch_unwind` over the async handler.
- Transfer-task marking wraps the spawned body so a panic sends
  `TransferState::Failed` via the existing watch channel; the job is not
  downgraded if already terminal.
- The About view is a new keymap `Command::ShowAbout` → `ActiveDialog::About`
  (UI-only, mirroring `ShowHelp`), reachable via a new menu entry; no new
  key binding required (so `keymap.toml`/help-coverage are untouched).
- The unwrap audit is bounded to a reviewed shortlist of normal-operation hot
  paths; it is hardening, not exhaustive removal.

### Out of Scope

- Re-running transfers automatically after a Failed-by-panic (user re-initiates).
- Localization of About / error text.
- Changing the Feature 061 architecture (only extending it).

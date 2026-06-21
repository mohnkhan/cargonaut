# Feature Specification: Survivability, Crash Safety & About/Version Surface

**Feature Branch**: `061-survivability-and-about`

**Created**: 2026-06-21

**Status**: Draft

**Input**: User description: "improve the survivability of this app; add help about copyrights to the author and show the version of the app; make it easy to debug crashes and prevent total crash gracefully — via full spec-kit development."

## Clarifications

### Session 2026-06-21

- Q: How aggressively should the app recover from faults (restore-then-exit vs recover-and-continue)? → A: **Recover & continue** — the runtime is allowed to unwind on fault so failures while drawing, while handling a single input, or inside background tasks are caught and the session survives (small binary-size cost accepted given headroom).
- Q: When a non-fatal fault is caught and the session continues, what is recorded? → A: **Log + in-app error** — recovered faults are written to `debug.log` at error level and shown as a dismissible on-screen message; a separate crash-report file is reserved for fatal/unrecovered crashes only (avoids flooding the data dir during a flurry of recoverable faults).
- Q: Where should the in-app About information live? → A: **Both** — enrich the existing F1 Help "About" section and add a dedicated About view reachable from the menu.
- Q: When is the user told where the crash report is? → A: **On exit and next launch** — the path is printed to the restored terminal as the process exits, and on the next launch a one-time notice surfaces if a not-yet-seen crash report exists.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - A crash never wrecks the terminal, and leaves a clue (Priority: P1)

A user is working in cargonaut when the application hits an unexpected, fatal
fault. Instead of being dumped into a scrambled, unresponsive terminal (raw mode
still on, alternate screen still active, cursor hidden, mouse capture still
grabbing clicks) that forces them to blindly type `reset`, the user is returned
to a clean, fully usable shell prompt with a short, legible message telling them
what happened and where the crash details were saved.

**Why this priority**: This is the foundational safety net and the single most
visible failure today — a panic currently `SIGABRT`s the process and skips all
terminal teardown, leaving the user stranded. It delivers value on its own even
if nothing else in the feature ships, and every other story builds on the same
crash-interception seam.

**Independent Test**: Force a fault on demand (a hidden test trigger), run the
binary under a pseudo-terminal, and confirm after exit that the terminal is back
in cooked mode with the primary screen restored and the cursor visible, and that
a crash-report file was created — no other part of the feature required.

**Acceptance Scenarios**:

1. **Given** cargonaut is running in a normal terminal, **When** a fatal fault
   occurs anywhere in the interactive session, **Then** the terminal is restored
   to a usable state (normal/cooked input, primary screen, visible cursor, mouse
   capture released) before the process exits.
2. **Given** a fatal fault has occurred, **When** the process exits, **Then** a
   single human-readable line is printed to the restored terminal naming what
   happened and the full path to the saved crash report.
3. **Given** a fatal fault occurs while the alternate screen is active, **When**
   the crash handling runs, **Then** the user does not have to run `reset` or
   close the terminal to continue working.

---

### User Story 2 - One failure doesn't sink the whole session (Priority: P2)

A user triggers an operation that fails unexpectedly in a way that would
previously have brought down the entire application (for example a fault while
drawing a single panel, while handling one keypress, or inside a background
file-transfer job). Instead of losing their whole session — both panes, tab
layout, in-flight jobs — the user sees a dismissible error, and the application
keeps running so they can carry on or retry.

**Why this priority**: This is the heart of "prevent total crash gracefully."
It converts a class of whole-app deaths into contained, recoverable incidents.
It is P2 rather than P1 because it depends on the crash-interception seam from
US1 and involves a broader, riskier change (containing faults mid-session and in
background work) that is only safe to ship once US1's guaranteed-clean-exit
backstop exists.

**Independent Test**: Inject a non-fatal fault into a single screen/operation
and into a background job via a test trigger; confirm the session stays
interactive afterward (input still handled, both panes still navigable) and the
error was surfaced to the user and recorded.

**Acceptance Scenarios**:

1. **Given** a running session, **When** a fault occurs while rendering or while
   handling a single user action, **Then** the fault is contained, an error
   message is shown to the user, and the session remains interactive.
2. **Given** a background task (e.g. a file transfer) faults, **When** the fault
   occurs, **Then** only that task is affected; the rest of the application stays
   fully usable and the failure is reported against that task.
3. **Given** an expected error condition during a normal operation (permission
   denied, missing path, unreadable file), **When** it occurs, **Then** it is
   presented as an in-app error rather than terminating the application.

---

### User Story 3 - Crash reports a developer can actually act on (Priority: P2)

After a crash, a developer (or the user filing a report) opens the saved
crash-report file and finds everything needed to understand the failure without
having to reproduce it: when it happened, which version of the app, the platform,
the precise fault message and source location, a captured backtrace, and a short
trail of the most recent actions the user took leading up to the crash.

**Why this priority**: Turns "it crashed" into a diagnosable event. Depends on
US1's crash interception (where the report is written) but adds the richer
context — especially the recent-action trail, which the existing warning-only log
cannot provide. P2 because a minimal report already ships with US1; this story
makes it genuinely actionable.

**Independent Test**: Trigger a crash after performing a known sequence of
actions; open the resulting report and confirm it contains version, platform,
fault message + location, a backtrace, and the recent actions in order.

**Acceptance Scenarios**:

1. **Given** a crash has occurred, **When** the crash report is opened, **Then**
   it contains a timestamp, the application version, the operating system and
   architecture, the fault message, and the source location of the fault.
2. **Given** a crash has occurred, **When** the crash report is opened, **Then**
   it contains a backtrace that is present regardless of any environment-variable
   configuration on the user's machine.
3. **Given** the user performed a sequence of actions before crashing, **When**
   the crash report is opened, **Then** it lists the most recent actions/events
   in chronological order up to the crash.
4. **Given** repeated crashes over time, **When** new reports are written,
   **Then** old reports do not accumulate without bound.

---

### User Story 4 - Knowing what you're running and who made it (Priority: P2)

A user wants to confirm which version of cargonaut they have, who wrote it, and
under what license — to cite it in a bug report, verify an upgrade, or check
attribution and licensing. They can find this both from inside the running app
and from the command line, without hunting through source files.

**Why this priority**: Independent, self-contained, and quick. It carries real
value (attribution, support, licensing clarity) and shares nothing with the
crash work, so it can be built and shipped in isolation. P2 because survivability
is the more urgent need, but this could ship first without risk.

**Independent Test**: Open the in-app About view and run the CLI version command;
confirm both show the app name, version, author, copyright, and license.

**Acceptance Scenarios**:

1. **Given** cargonaut is running, **When** the user opens the in-app About
   information, **Then** they see the application name, version, author,
   copyright notice, and license identifier.
2. **Given** a shell, **When** the user runs the version command, **Then** the
   output includes the version, copyright, and license.
3. **Given** the user is on the main view, **When** they open the help screen,
   **Then** the enriched About information is reachable from there.
4. **Given** the user is on the main view, **When** they open the application
   menu, **Then** a dedicated About entry opens a view showing the same identity
   details.

---

### Edge Cases

- **Fault before the UI is entered** (during startup/config load): no terminal
  state to restore yet; the app still writes a crash report and exits with a
  clear message rather than a bare panic dump.
- **Fault during teardown** (while restoring the terminal): restoration is
  best-effort and idempotent so a second fault cannot re-scramble the terminal.
- **Fault inside the crash handler itself** (re-entrancy): the handler must not
  loop or deadlock; a second fault degrades to the simplest possible clean exit.
- **Crash-report directory not writable / disk full**: the terminal is still
  restored and the app still exits cleanly; the user is told the report could not
  be saved instead of suffering a secondary crash.
- **Not a real terminal** (output piped, or accessibility/plain-text mode):
  crash handling must not emit control sequences that corrupt a non-TTY stream.
- **Very large backtrace or non-text fault message**: the report is still
  written and readable (bounded/sanitized as needed).
- **Many rapid recoverable faults**: repeated in-session errors must not wedge
  the UI in an error-dialog loop or exhaust resources.
- **Secrets in context**: a crash report or log must never expose credentials
  (e.g. an SFTP password held in memory).

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: On any abnormal (fault-induced) termination, the system MUST
  restore the terminal to a usable state — normal/cooked input mode, primary
  (non-alternate) screen, visible cursor, and released mouse capture — before the
  process exits.
- **FR-002**: On any **fatal/unrecovered** fault-induced termination, the system
  MUST write a crash-report file to a single, documented per-user location.
  (Recovered, non-fatal faults are handled by FR-007 and do not each produce a
  crash-report file.)
- **FR-003**: Each crash report MUST include a timestamp, the application
  version, the operating system and architecture, the fault message, and the
  source location of the fault.
- **FR-004**: Each crash report MUST include a captured backtrace that is present
  regardless of the user's environment-variable configuration.
- **FR-005**: Each crash report MUST include a trail of the most recent in-app
  actions/events preceding the crash, in chronological order.
- **FR-006**: After a fatal fault-induced termination, the system MUST inform the
  user, on the restored terminal, of the crash report's location.
- **FR-006a**: On the next launch after a crash, if a crash report exists that
  the user has not yet been notified about, the system MUST surface a one-time
  notice of its location, and MUST NOT repeat that notice once seen.
- **FR-007**: A recoverable failure during interactive operation (while drawing
  or while handling a single user action) MUST NOT terminate the session; it MUST
  be logged at error level, surfaced to the user as a dismissible message, and
  leave the application interactive. Recovered faults MUST NOT each create a
  separate crash-report file.
- **FR-008**: A failure in a background task MUST NOT terminate the application;
  the failure MUST be isolated to that task and reported against it.
- **FR-009**: Expected error conditions during normal operations (e.g.
  permission denied, missing path, unreadable file) MUST be handled as in-app
  errors rather than aborting the application.
- **FR-010**: Users MUST be able to view, from within the running application,
  the application name, version, author, copyright notice, and license
  identifier.
- **FR-011**: The command-line version output MUST include the copyright notice
  and license identifier in addition to the version.
- **FR-012**: The in-app About information MUST be reachable BOTH from the help
  screen (an enriched "About" section) AND as a dedicated About view reachable
  from the application menu.
- **FR-013**: Crash-report writing MUST be failure-tolerant: if the report
  cannot be written, the terminal MUST still be restored and the process MUST
  still exit cleanly without a secondary crash.
- **FR-014**: Crash reports MUST NOT accumulate without bound; the system MUST
  retain only a bounded number of the most recent reports.
- **FR-015**: Crash reports and logs MUST NOT contain credentials or other
  secrets held by the application.
- **FR-016**: Crash handling MUST be safe on a non-terminal output stream and in
  accessibility/plain-text output mode (no control sequences written to a
  non-TTY stream).
- **FR-017**: The release distribution MUST remain within the existing binary
  size ceiling (≤ 8 MiB stripped, NFR-001) after this feature.

### Key Entities *(include if feature involves data)*

- **Crash Report**: A self-contained, human-readable record of one fault —
  timestamp, app version, platform (OS + architecture), fault message, source
  location, backtrace, and the recent-action trail. Lives as a file in the
  per-user data location; subject to bounded retention.
- **Recent-Action Trail**: A small, fixed-capacity, in-memory record of the most
  recent user actions / dispatched commands / events. Continuously overwritten
  during normal operation (oldest dropped first); captured into a crash report
  when a fault occurs.
- **About Information**: The static identity of the build — application name,
  version, author, copyright notice, and license identifier — surfaced both
  in-app and at the command line.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: After a forced fault, the terminal is left usable (no manual
  `reset` required: cooked input, primary screen, visible cursor) in 100% of
  automated pseudo-terminal test runs.
- **SC-002**: After a forced fault, a crash report containing — at minimum —
  version, platform, fault location, and a backtrace exists at the documented
  location in 100% of test runs.
- **SC-003**: A forced recoverable fault in a single screen/operation does not
  end the session in 100% of test runs; the application remains interactive
  afterward.
- **SC-004**: A forced fault in a background task leaves the rest of the
  application fully usable in 100% of test runs.
- **SC-005**: A user can read the version, author, copyright, and license both
  in-app (within two keystrokes of the main view) and from a single CLI command.
- **SC-006**: A crash report identifies the failing source location precisely
  enough that a developer can locate the responsible source area without
  reproducing the crash (validated by inspection).
- **SC-007**: The stripped release binary remains ≤ 8 MiB after the change.
- **SC-008**: With a credential configured in the session, a forced crash
  produces a report that contains no occurrence of that credential (verified by
  test).
- **SC-009**: After a crash, the next launch surfaces exactly one notice of the
  report's location, and that notice does not reappear on subsequent launches
  once the report has been seen (verified by test).

## Assumptions

- **Recovery is in scope (US2) — confirmed.** Per the 2026-06-21 clarification,
  "prevent total crash gracefully" includes in-session recovery, not only
  clean-exit-then-die. This requires allowing the runtime to unwind on fault
  rather than aborting immediately; the resulting binary-size cost is accepted
  given the current large headroom (release binary ≈ 2.97 MiB against the 8 MiB
  ceiling, NFR-001).
- **Recovered faults are logged, not filed.** Per clarification, a caught,
  non-fatal fault is recorded to `debug.log` at error level and shown on screen;
  only fatal/unrecovered crashes produce a `crash-<timestamp>` report file.
- **About appears in two places.** Per clarification, the identity details are
  surfaced both in the F1 Help "About" section and in a dedicated menu-reachable
  About view.
- **Crash notice is shown twice.** Per clarification, the report path is printed
  on exit and a one-time notice is surfaced on the next launch if unseen.
- **Crash-report and log location** is the existing per-user data directory used
  for `debug.log` (XDG data dir, e.g. `~/.local/share/cargonaut/`).
- **Recent-action trail capacity** is a small fixed number (order of dozens);
  the exact size is a tuning detail for planning.
- **Crash-report retention** keeps a bounded number of the most recent reports;
  the exact count/rotation policy is a planning detail.
- **Author / copyright / license** are taken from the existing source headers:
  author/copyright "© 2024–2026 Mohiuddin Khan Inamdar", license
  "MIT OR Apache-2.0".
- **Recovery scope** covers the interactive UI loop (rendering + input handling)
  and background tasks. Faults during initial startup, before the UI is entered,
  exit cleanly with a crash report rather than attempting recovery.
- **Testing** reuses the project's existing gated pseudo-terminal harness pattern
  for the crash/restore integration test, alongside unit tests for the pure
  report-formatting and terminal-restore helpers.
- **No new heavy dependencies**: diagnostics should be built from the standard
  library and existing crates where practical (evaluated in research).

### Out of Scope

- Automatic crash-report upload, telemetry, or any network transmission of
  diagnostics.
- Persisting and restoring full interactive session state (panes, tabs, cursor)
  across a crash.
- Surviving OS-level kills (e.g. `SIGKILL`); resumable transfers already cover
  the data-safety case for that (Feature 037) and are not revisited here.
- Localizing or themability of the About / crash messages beyond existing
  conventions.

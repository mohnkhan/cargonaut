# Feature Specification: In-Session Mouse Capture Toggle

**Feature Branch**: `041-mouse-capture-toggle`

**Created**: 2026-06-15

**Status**: Draft

**Input**: User description: "In-session mouse capture toggle key. Add a runtime keymap binding that toggles mouse capture on/off without restarting the app, complementing the existing `--no-mouse` launch flag and `ui.mouse` config. Pressing the key when capture is active suspends it (restoring terminal-native text selection); pressing it again re-enables capture. The current capture state is tracked in UiState and surfaced to the user. Document the terminal hold-modifier (Shift) bypass for one-off text selection. This implements deferred FR-013 from Feature 031 (tracked as issue #38)."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Suspend mouse capture mid-session to copy text (Priority: P1)

A user is working in the running file manager with mouse support active (the default). They want to select and copy a file path (or a chunk of on-screen text) using their terminal's native text selection — but while the app captures the mouse, click-and-drag is consumed by the app instead of the terminal, so native selection doesn't work. The user presses a single key to suspend mouse capture, performs their text selection/copy with the terminal as usual, then presses the same key again to resume in-app mouse interaction — all without quitting and relaunching.

**Why this priority**: This is the core value of the feature and the exact gap left by Feature 031 (FR-013 deferred). Without it, a user who wants native text selection must relaunch with `--no-mouse`, losing their navigation state. It is the minimal viable slice: a working toggle that flips capture and is independently demonstrable.

**Independent Test**: Launch the app with mouse capture on (default). Confirm click-to-focus works. Press the toggle key; confirm in-app mouse interaction no longer responds and the terminal's native click-drag text selection works. Press the toggle key again; confirm in-app mouse interaction responds again.

**Acceptance Scenarios**:

1. **Given** the app is running with mouse capture active, **When** the user presses the toggle key, **Then** mouse capture is suspended and subsequent mouse clicks/scrolls are no longer consumed by the app (terminal-native selection works).
2. **Given** mouse capture has been suspended via the toggle, **When** the user presses the toggle key again, **Then** mouse capture is resumed and in-app mouse interaction (click-to-focus, wheel-scroll, double-click-to-enter) works again.
3. **Given** the app is running, **When** the user toggles capture off and then on repeatedly, **Then** each press reliably flips the state with no residual or stuck state (the toggle is idempotent per press).

---

### User Story 2 - See the current capture state (Priority: P2)

A user who has toggled mouse capture wants to know, at a glance, whether the mouse is currently captured by the app or released to the terminal — so they are not confused about why clicks do or don't respond.

**Why this priority**: Discoverability and feedback. A toggle whose state is invisible leads to "why isn't my mouse working?" confusion. It builds on US1 but is not required for the toggle itself to function, so it is P2.

**Independent Test**: Toggle capture off and confirm the UI communicates that mouse capture is suspended; toggle it back on and confirm the UI communicates that mouse capture is active.

**Acceptance Scenarios**:

1. **Given** the user presses the toggle key, **When** capture changes state, **Then** the UI surfaces a clear, immediate indication of the new state (active vs. suspended).
2. **Given** mouse capture is suspended, **When** the user looks at the interface, **Then** the suspended state is discoverable (not silently changed).

---

### User Story 3 - Toggle is a no-op signal when mouse is disabled by config/flag (Priority: P3)

A user who launched with `--no-mouse` (or has `ui.mouse = false` in config) presses the toggle key. Because mouse support was disabled for the whole session by configuration, the runtime toggle should behave predictably and communicate why nothing changed, rather than silently doing nothing or appearing broken.

**Why this priority**: Edge-case robustness. It prevents a confusing dead-key experience for users who disabled the mouse up front, but it is secondary to the main toggle behavior.

**Independent Test**: Launch with `--no-mouse`. Press the toggle key. Confirm the app does not crash, mouse stays uncaptured, and the user receives feedback explaining the state.

**Acceptance Scenarios**:

1. **Given** the app was launched with mouse support disabled by flag or config, **When** the user presses the toggle key, **Then** the app communicates that mouse support is disabled for this session and capture remains off.

---

### Edge Cases

- **Toggling while a modal/dialog is open**: pressing the toggle key while a dialog (e.g., quick-cd, filter prompt, tasks panel) has focus must not corrupt the dialog; the toggle either applies cleanly or is deferred to normal mode in a predictable way.
- **Toggling, then shelling out to an external program (F3/F4 pager/editor)**: when the app suspends the TUI to run an external tool and then restores, the restored mouse capture state must match the user's last toggle choice, not the original launch state.
- **Toggling off, then quitting**: on exit, the terminal must be left in a clean state (mouse capture released, terminal-native input restored) regardless of the last toggle state.
- **Rapid repeated presses**: alternating presses must not desynchronize the tracked state from the actual terminal capture state.
- **Terminal that does not support mouse capture**: toggling must degrade gracefully (no crash) even if the terminal silently ignores capture control sequences.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide a runtime control (a keymap binding) that toggles mouse capture between active and suspended without restarting the application.
- **FR-002**: When mouse capture is active and the user invokes the toggle, the system MUST suspend mouse capture so that mouse events are released to the terminal (enabling terminal-native text selection/copy).
- **FR-003**: When mouse capture is suspended and the user invokes the toggle, the system MUST resume mouse capture so that in-app mouse interaction works again.
- **FR-004**: The system MUST track the current mouse capture state in the running UI state so that all parts of the app observe a single, consistent capture state.
- **FR-005**: The system MUST surface the current capture state to the user when it changes (active vs. suspended), so the change is discoverable and not silent.
- **FR-006**: When mouse support is disabled for the session by launch flag (`--no-mouse`) or configuration (`ui.mouse = false`), invoking the toggle MUST NOT capture the mouse, and the system MUST communicate that mouse support is disabled for the session.
- **FR-007**: The system MUST keep the tracked capture state and the actual terminal capture state synchronized across operations that suspend and restore the TUI (e.g., shelling out to an external pager/editor), restoring the user's last toggled choice rather than the launch-time state.
- **FR-008**: On application exit, the system MUST leave the terminal in a clean state (mouse capture released) regardless of the last toggle state.
- **FR-009**: The toggle binding MUST NOT collide with any existing keymap binding (including the orthodox-FM-compat `--mc-keys` map) in a way that overrides established navigation.
- **FR-010**: The feature MUST document, in user-facing help/docs, the terminal hold-modifier bypass (commonly Shift) that lets users perform a one-off native text selection without toggling capture off.
- **FR-011**: Toggling MUST degrade gracefully (no crash, no terminal corruption) when the underlying terminal does not honor mouse capture control.

### Key Entities *(include if feature involves data)*

- **Mouse capture state**: A runtime, in-memory boolean-like state representing whether the application is currently capturing the mouse. Distinct from the session-level "mouse support enabled" setting derived from config/flag. Lives for the duration of the running session and is reset/released on exit.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can suspend mouse capture and perform a terminal-native text selection, then resume in-app mouse interaction, in a single session without restarting — completing the full cycle with at most two key presses.
- **SC-002**: After any sequence of toggle presses, the observable mouse behavior (captured vs. released) matches the tracked state 100% of the time (no desynchronization).
- **SC-003**: 100% of toggle state changes produce a visible indication of the new state.
- **SC-004**: When mouse support is disabled by flag/config, pressing the toggle never captures the mouse and always produces an explanatory indication (0 silent no-ops).
- **SC-005**: Exiting the app after any toggle sequence leaves the terminal usable (native input restored) in 100% of cases.
- **SC-006**: User-facing documentation includes the hold-modifier (Shift) bypass instruction.

## Assumptions

- **Default-on baseline**: Mouse capture is enabled by default per Feature 031 (FR-013); this feature adds only the runtime toggle on top of that baseline.
- **Single dedicated binding**: A single key/chord toggles capture (rather than separate enable/disable keys). The exact key is finalized during planning; it must not collide with existing bindings and should be discoverable via the help overlay.
- **Scope is the interactive TUI**: The toggle applies to the running terminal UI session only; it does not change persisted config and does not affect non-interactive subcommands.
- **No persistence**: The toggle affects the current session only; it does not write the user's choice back to the config file. Relaunching starts from the config/flag default.
- **Hold-modifier bypass is terminal-provided**: The Shift (or terminal-specific) hold-modifier text-selection bypass is a function of the user's terminal emulator, not implemented by the app; this feature documents it rather than implementing it.
- **Reuses existing capture mechanics**: The enable/disable of capture reuses the same terminal control the app already uses at startup/teardown and around external-program suspension.

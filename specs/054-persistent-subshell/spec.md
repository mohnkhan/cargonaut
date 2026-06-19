# Feature Specification: Persistent Subshell (Ctrl-o)

**Feature Branch**: `054-persistent-subshell`

**Created**: 2026-06-19

**Status**: Draft

**Resolves**: GitHub issue #44 — Persistent subshell integration (Ctrl-o), deferred from Feature 031 (FR-029)

## Clarifications

### Session 2026-06-19

- Q: When the subshell panel is visible and file-manager focus is active, does `Ctrl-o` hide the panel or transfer focus into the shell? → A: Three-state cycle — `Ctrl-o` advances: hidden → visible+FM-focus → visible+shell-focus → hidden. One key handles all transitions; no separate focus-entry mechanism is required.
- Q: Does the subshell panel require full ANSI/VT100 terminal emulation, or simplified raw-line display? → A: Full ANSI/VT100 terminal emulation — cursor-addressing programs (`vim`, `htop`, `less`) must work correctly within the subshell panel. A terminal-emulator crate (e.g. `vt100`) is the expected implementation vehicle alongside `portable-pty`.
- Q: When the user switches tabs within the active side (Feature 053 `[`/`]` keys), does the subshell cwd sync to the newly active tab's directory? → A: Yes — a tab-switch is treated as a directory-change event and triggers cwd-sync to the new active tab's cwd. FR-007 and FR-008 are extended to cover this case explicitly.

## Overview

Today Cargonaut launches an external viewer/editor by fully suspending the TUI (leaving the alternate screen, restoring the terminal to cooked mode) and blocking until the process exits. There is no persistent shell session that the user can toggle open and closed. This feature adds a **persistent subshell panel** at the bottom of the screen: a PTY-backed shell process that is spawned once at startup, kept alive for the session, and toggled visible or hidden with `Ctrl-o`. When visible, the shell occupies roughly the lower third of the terminal; both file-manager panes shrink to share the upper portion. When the subshell is hidden, the layout returns to the normal dual-panel view. The shell's working directory is kept in sync with the active panel's current directory whenever the panel changes directories. This mirrors the "Ctrl-o" subshell toggle in the reference manager (Midnight Commander) and resolves the deferral recorded in Feature 031.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Toggle subshell open and closed (Priority: P1)

A user is browsing their project directory in the left panel. They press `Ctrl-o`: the terminal splits, with the dual-panel view occupying the upper portion and an interactive shell prompt appearing in the lower portion. The shell's cwd is the active panel's current directory. The user runs `git log --oneline` to check recent history, reads the output, then presses `Ctrl-o` again. The shell panel collapses and the file manager returns to full-screen dual-panel mode. The shell process is still alive in the background; pressing `Ctrl-o` again restores the same session, preserving shell history and environment.

**Why this priority**: Toggle-in / toggle-out is the fundamental interaction. Nothing else in this feature is valuable if the shell can't be shown and hidden reliably.

**Independent Test**: Press `Ctrl-o`; confirm a shell prompt appears in the lower portion of the screen; type a command and observe output; press `Ctrl-o` again; confirm the shell panel disappears and the dual-panel view returns to full height.

**Acceptance Scenarios**:

1. **Given** the file manager is in normal dual-panel mode, **When** the user presses `Ctrl-o`, **Then** a shell panel appears in the lower portion of the screen and the dual panels are visible above it.
2. **Given** the subshell panel is visible, **When** the user presses `Ctrl-o`, **Then** the shell panel hides and the dual panels expand back to full-screen height.
3. **Given** the subshell was previously shown and commands were run, **When** the user presses `Ctrl-o` to show it again, **Then** the same shell session is resumed (history, environment variables, and cwd are preserved from the hidden state).
4. **Given** the subshell is showing, **When** the user types a shell command and presses Enter, **Then** the command runs inside the shell and output appears within the subshell panel.
5. **Given** the subshell is running a command, **When** the user presses `Ctrl-o`, **Then** the subshell is hidden but the running command continues in the background; toggling back reveals the resumed output.

---

### User Story 2 — Shell cwd stays in sync with the active panel (Priority: P1)

A user opens the subshell (`Ctrl-o`). The shell prompt shows `~/projects/cargonaut` — the same as the active panel. The user navigates the left panel into `~/projects/cargonaut/src` (by pressing Enter on the directory). Immediately the shell's working directory updates to `~/projects/cargonaut/src`. The user switches focus to the right panel (which is in `/tmp`), and the shell cwd updates to `/tmp`. When the user runs `ls` in the shell, results match the focused panel's directory.

**Why this priority**: cwd-sync is what makes the subshell genuinely useful versus just having a floating terminal. Without it, the user would constantly have to `cd` manually.

**Independent Test**: Open subshell; navigate the active panel to a new directory; confirm the shell prompt shows the updated cwd (either via prompt string or by running `pwd`).

**Acceptance Scenarios**:

1. **Given** the subshell is open, **When** the user navigates the active panel into a child directory, **Then** the subshell's cwd updates to match that directory.
2. **Given** the subshell is open, **When** the user switches focus between left and right panels, **Then** the subshell's cwd updates to the newly focused panel's current directory.
3. **Given** the subshell is hidden, **When** the user navigates the panel and then opens the subshell, **Then** the subshell's cwd reflects the panel's current directory at the moment of opening.
4. **Given** the user typed `cd ~/other` in the shell manually (changing cwd from within the shell), **When** the user then navigates to a new directory in the file manager panel, **Then** the subshell's cwd is overridden to the panel's new directory (the panel is authoritative on navigation events).
5. **Given** the target directory does not exist (e.g., was deleted), **When** a cwd-sync is attempted, **Then** the sync fails gracefully — the shell is told to `cd` to the best available ancestor; no crash or frozen state.

---

### User Story 3 — Keyboard focus and input routing (Priority: P2)

A user opens the subshell with `Ctrl-o` (first press — panel appears, FM focus retained). Arrow keys still move the file manager cursor. A second `Ctrl-o` transfers keyboard focus into the shell; the user types commands and sees output. A third `Ctrl-o` hides the panel entirely and restores full file-manager mode. The cycle repeats cleanly. The user can also click inside the shell panel to jump directly into shell-focus without extra `Ctrl-o` presses.

**Why this priority**: Input-routing defines usability. The three-state cycle must be crisp and predictable — any ambiguity about which mode is active breaks the user's mental model.

**Independent Test**: Press `Ctrl-o` (panel appears, FM focus); press arrow keys (FM cursor moves); press `Ctrl-o` again (focus enters shell); type text (appears in shell); press `Ctrl-o` (panel hides, FM focus restored).

**Acceptance Scenarios**:

1. **Given** the subshell panel is visible and file-manager focus is active (state 2), **When** the user presses arrow keys, **Then** the file manager cursor moves (FM receives input).
2. **Given** state 2 (visible+FM-focus), **When** the user presses `Ctrl-o`, **Then** keyboard focus transfers to the subshell (state 3); file-manager panels remain visible.
3. **Given** state 3 (visible+shell-focus), **When** the user presses `Ctrl-o`, **Then** the subshell panel hides (state 1) and file-manager focus is restored.
4. **Given** state 2 (visible+FM-focus), **When** the user clicks inside the subshell panel (mouse mode), **Then** keyboard focus immediately transfers to the subshell (state 3).
5. **Given** state 3 (visible+shell-focus), **When** the user clicks on either file-manager pane (mouse mode), **Then** keyboard focus returns to the file manager (state 2).
6. **Given** keyboard focus is in the subshell (state 3), **When** the user types a command including special characters (pipes, redirects, quoting), **Then** all characters are forwarded verbatim to the PTY.
7. **Given** the subshell has keyboard focus (state 3), **When** the shell exits (user types `exit` or `Ctrl-d`), **Then** the subshell panel closes (state 1), a fresh shell is spawned ready for the next `Ctrl-o`, and file-manager focus is restored.

---

### Edge Cases

- **Ctrl-o while a modal dialog is open**: `Ctrl-o` is consumed by the modal and must NOT toggle the subshell. Subshell state MUST remain unchanged while any modal (copy-confirm, delete-confirm, rename, find-file, filter, hex viewer, user menu) is active.
- **Terminal resize while subshell is open**: The subshell panel and file manager panels must reflow to the new dimensions; the PTY is resized via `TIOCSWINSZ` so that the shell's programs (e.g. `vim`, `less`) redraw correctly.
- **Very narrow or very short terminal**: If the terminal is too small to show both the dual-panel view and the subshell (e.g., fewer than 8 lines available for subshell), the subshell opens but shows a truncated view; the file manager panes always keep a minimum of 5 lines. Below an absolute minimum terminal size, the subshell toggle is a no-op with a status-bar notice.
- **Shell exits unexpectedly** (crash, SIGKILL): The panel shows a "Shell exited — press Ctrl-o to restart" message. The next `Ctrl-o` spawns a fresh shell in the active panel's current directory.
- **File manager loses the active pane's directory** (e.g., the directory was deleted): cwd-sync sends the active pane's last known valid path; the shell receives a `cd` to the best available ancestor.
- **Multiple rapid Ctrl-o presses**: Debounced — only the first in a burst of <50 ms toggles; subsequent rapid presses are ignored to prevent flickering layout.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST spawn a PTY-backed persistent shell process at application startup (or on the first `Ctrl-o`, whichever is lazier). The shell MUST be determined by the `$SHELL` environment variable, falling back to `/bin/sh`.
- **FR-002**: `Ctrl-o` advances the subshell through a three-state cycle in order: (1) **hidden** → (2) **visible, file-manager focus** → (3) **visible, shell focus** → back to (1) **hidden**. Each `Ctrl-o` press advances exactly one step. The cycle is the sole keyboard mechanism for showing, entering, and hiding the subshell; no separate focus-entry key is required.
- **FR-003**: When advancing from state 1 → 2 (hidden → visible+FM-focus), the subshell panel MUST appear in the lower portion of the screen (~33% of content-area rows by default; FR-013). The dual panels remain visible and fully interactive above it. File-manager focus is retained — arrow keys and all file-manager bindings remain active.
- **FR-004**: When advancing from state 2 → 3 (visible+FM-focus → visible+shell-focus), keyboard focus transfers to the subshell. All subsequent keystrokes (except `Ctrl-o`) are forwarded verbatim to the PTY. The file-manager panels remain visible but do not receive input.
  When advancing from state 3 → 1 (visible+shell-focus → hidden), the subshell panel hides and file-manager focus is restored. The shell process remains alive in the background.
  A mouse click inside the subshell panel area MUST also transfer keyboard focus to the shell (jumping directly to state 3) regardless of current state. A mouse click outside the subshell panel (on either file-manager pane) MUST return focus to the file manager (state 2 if panel is visible).
- **FR-005**: The subshell panel MUST act as a full ANSI/VT100 terminal emulator — cursor-addressing programs (`vim`, `htop`, `less`, `top`) MUST render and respond to input correctly within the panel. The visible region shows the terminal's current screen state (not a raw log of output lines); the user MAY scroll up through scrollback history while focus is in the subshell (state 3). A terminal-emulator library paired with `portable-pty` provides this capability; raw-line display is insufficient.
- **FR-006**: When keyboard focus is in the subshell, ALL keystrokes (including special characters, Ctrl sequences, and escape codes) MUST be forwarded verbatim to the PTY. The only exception is `Ctrl-o`, which returns focus to the file manager (FR-004).
- **FR-007**: Whenever the **active file-manager panel** changes its current directory (via navigation, cd-popup, bookmark jump, tab-switch via `[`/`]`, or any other mechanism), the subshell's cwd MUST be updated by sending `cd <new-path>\n` to the shell PTY. This sync MUST occur regardless of whether the subshell panel is currently visible. Tab-switch (Feature 053) is explicitly a cwd-change trigger: switching from tab A (in `/foo`) to tab B (in `/bar`) on the active side MUST send `cd /bar` to the shell.
- **FR-008**: Whenever the **focused file-manager side** changes (Tab or M-1/M-2 focus-swap), the subshell's cwd MUST be updated to the active tab of the newly focused side's current directory.
- **FR-009**: If the shell process exits (any cause), the subshell panel MUST show a "Shell exited — press Ctrl-o to restart" notice rather than crashing or freezing. The next `Ctrl-o` MUST spawn a fresh shell in the active panel's directory and restore normal subshell behavior.
- **FR-010**: When the terminal is resized, the PTY MUST be resized to match the new subshell panel dimensions (`TIOCSWINSZ` / equivalent portable API). The file manager and subshell panel MUST reflow to the new dimensions within one frame.
- **FR-011**: The tab bar (Feature 053), status bar, function-key bar, and menu bar MUST remain visible and functional when the subshell panel is open. The subshell panel occupies only the central content area rows between the chrome above and the function-key bar below.
- **FR-012**: While any modal dialog is active, `Ctrl-o` MUST be consumed by the modal without modifying subshell visibility or focus state.
- **FR-013**: The subshell panel height MUST be configurable via the Cargonaut configuration file (key: `ui.subshell_height_pct`, type: `u8`, valid range: 10–60, default: 33). Height is expressed as a percentage of the available content area rows, rounded to the nearest whole row, with a minimum of 3 rows enforced.
- **FR-014**: The `open-subshell` action MUST be registered in `design/contracts/keymap.toml` (already present) and handled by the `Command` dispatch in `cargonaut-ui-tui`. A `subshell` mode entry MUST exist in the keymap for keys active only when shell focus is held (at minimum `C-o` to return focus).
- **FR-015**: The binary size budget (NFR-001, ≤8 MiB stripped) MUST remain satisfied after this feature. The PTY dependency (`portable-pty`) is already in the workspace `[workspace.dependencies]`; no new heavy dependencies may be added.

### Key Entities

- **SubshellState**: Top-level struct (in `cargonaut-ui-tui`) owning the PTY master, child process handle, output ring buffer, scroll offset, and visible/focus flags. Managed by `UiState`.
- **PtyOutput**: A fixed-capacity ring buffer of lines/bytes received from the PTY master fd, used to paint the subshell panel without keeping unlimited history.
- **SubshellFocus**: Boolean flag on `UiState`; when true, input is routed to the PTY instead of the file manager dispatch table.
- **SubshellHeight**: Resolved pixel-row count for the subshell panel, derived from `ui.subshell_height_pct` (FR-013) and current terminal height.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can toggle the subshell open and closed with `Ctrl-o` without visible flicker or layout glitch, and the dual panels return to their original heights within one frame of toggling closed.
- **SC-002**: The shell's cwd reflects the active file-manager panel's current directory within 100 ms of every panel navigation or focus-switch event.
- **SC-003**: All existing features (copy, move, delete, filter, sort, viewer, find-file, bulk-rename, bookmarks) continue to work identically whether the subshell panel is open or closed.
- **SC-004**: After pressing `Ctrl-o` to reveal the shell, then running a command, then pressing `Ctrl-o` to hide, then pressing `Ctrl-o` again, the shell session (history, environment, last prompt) is fully preserved — the user does not need to re-run any setup.
- **SC-005**: A terminal resize (e.g., dragging the window) while the subshell is open causes the PTY and both panel regions to reflow within one frame; programs running in the subshell (e.g. `top`, `less`) redraw correctly for the new dimensions.
- **SC-006**: The stripped release binary size remains at or below 8 MiB (NFR-001) after the feature is merged.
- **SC-007**: The CI benchmark for keypress latency (NFR-002, ≤16 ms) passes with the subshell feature compiled in (even when the subshell is closed).
- **SC-008**: 100% of the CI test suite (≥80% coverage on core crates per NFR-007) continues to pass after merge.

## Assumptions

- The shell to spawn is `$SHELL` if set and executable, falling back to `/bin/sh`. No shell-picker UI is provided in this feature.
- The subshell panel is always at the bottom of the content area, below both file manager panes. A horizontally-split layout (subshell beside a pane) is out of scope.
- Lazy spawn (on first `Ctrl-o`) is preferred over eager spawn (at startup) to avoid needless process creation when the user never uses the subshell; either is acceptable as an implementation decision in planning.
- The subshell panel scrollback is bounded (ring buffer); a very large shell session's earlier output may be truncated. The exact ring-buffer size is an implementation detail for planning.
- cwd-sync is one-directional: the file manager pushes its cwd to the shell. The shell's own `cd` changes are NOT reflected back to the file manager panels. This avoids a complex feedback loop.
- Mouse events inside the subshell panel (when mouse capture is active) transfer keyboard focus to the shell. Mouse events outside it (on the file manager panels) transfer focus back.
- The subshell feature does not require `unsafe` code; `portable-pty` provides a safe abstraction over PTY creation and `TIOCSWINSZ`.

## Out of Scope

- Shell output search (grep-within-panel).
- Saving/restoring shell sessions across application restarts.
- Multiple simultaneous subshell panels or a shell tab bar.
- Horizontal layout (subshell beside a pane instead of below both).
- Shell-picker UI (always uses `$SHELL`/`/bin/sh`).
- Capturing shell output and panelizing it in a file manager pane (handled by external-panelize FR-205, a separate existing feature).

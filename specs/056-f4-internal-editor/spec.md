# Feature Specification: Internal F4 Editor

**Feature Branch**: `056-f4-internal-editor`

**Created**: 2026-06-19

**Status**: Draft

**Input**: Replace the F4 `$EDITOR` shell-out with a built-in full-screen TUI text editor. When the user presses F4 on a file in either pane, Cargonaut opens the file in an internal editing mode inside the same terminal window — no subprocess, no TUI teardown. Closes #40.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Edit and Save a Plain-Text File (Priority: P1)

A user in the file manager presses F4 on a configuration file or script. The file opens instantly in a full-screen editor that fills the entire terminal. The user navigates by line and column with arrow keys, makes changes by typing, and presses F2 (or Ctrl-S) to save. The file is written back to disk and the editor closes, returning to the file manager with the pane refreshed.

**Why this priority**: This is the entire purpose of the feature — in-process editing without spawning an external program. Every other user story depends on the core editing loop being correct.

**Independent Test**: Press F4 on a plain-text file; type new text; press F2; confirm the file on disk contains the new text; confirm the file manager pane is active again.

**Acceptance Scenarios**:

1. **Given** the file manager is open and a plain-text file is focused, **When** the user presses F4, **Then** the editor opens full-screen showing the file's content with a cursor at line 1, column 1.
2. **Given** the editor is open, **When** the user types characters, **Then** the characters are inserted at the cursor position, existing text shifts right, and the display updates immediately.
3. **Given** the editor has unsaved changes, **When** the user presses F2 or Ctrl-S, **Then** the file is written to disk, the modified indicator clears, and the editor remains open.
4. **Given** the file has been saved (or no changes were made), **When** the user presses F10, Esc, or Q, **Then** the editor closes and the file manager is restored with the pane refreshed.
5. **Given** the editor is open, **When** the user presses arrow keys, Home, End, Page Up, or Page Down, **Then** the cursor moves accordingly, the viewport scrolls to keep the cursor visible, and the status line reflects the current line:column.

---

### User Story 2 — Unsaved-Changes Guard on Exit (Priority: P2)

A user edits a file but decides to exit without saving. The editor detects unsaved changes and presents a confirmation prompt: Save, Discard, or Cancel. Choosing Discard exits without writing; Cancel returns to editing.

**Why this priority**: Accidental data loss on a misplaced key press is a critical usability concern. Without this guard, any exit key (F10, Esc, Q) silently discards edits.

**Independent Test**: Open a file in the editor, make a change, press F10. A confirmation prompt must appear with at least two options (discard / cancel). Choosing discard must return to the file manager without writing the file; the original content must be unchanged on disk.

**Acceptance Scenarios**:

1. **Given** the editor has unsaved changes, **When** the user presses F10, Esc, or Q, **Then** a modal confirmation prompt appears ("Unsaved changes — Save, Discard, or Cancel?").
2. **Given** the confirmation prompt is showing, **When** the user chooses Discard, **Then** the editor closes without writing, returning to the file manager; the file on disk is unchanged.
3. **Given** the confirmation prompt is showing, **When** the user chooses Save, **Then** the file is saved and the editor closes normally.
4. **Given** the confirmation prompt is showing, **When** the user chooses Cancel (or presses Esc), **Then** the prompt dismisses and the user resumes editing.
5. **Given** the editor has no unsaved changes, **When** the user presses F10, Esc, or Q, **Then** the editor closes immediately without prompting.

---

### User Story 3 — Safety Limits for Uneditable Files (Priority: P3)

A user accidentally presses F4 on a binary file, a very large file, or a directory. The editor declines to open it with a clear status message, returning immediately to the file manager. No corrupt content is displayed.

**Why this priority**: Without this guard, the editor would display garbled binary content or hang on a multi-gigabyte file. This guard prevents data corruption and unresponsive UI.

**Independent Test**: Press F4 on a binary file (e.g. a compiled executable) and on a file larger than the size limit. Both must return to the file manager with an informative status message; neither must open the editor view.

**Acceptance Scenarios**:

1. **Given** the focused file is detected as binary (contains null bytes or non-UTF-8 sequences), **When** the user presses F4, **Then** the editor does not open; a status message reads "Cannot edit: binary file".
2. **Given** the focused file exceeds the size limit (default: 10 MiB), **When** the user presses F4, **Then** the editor does not open; a status message reads "Cannot edit: file too large (>10 MiB)".
3. **Given** the focused entry is a directory or the cursor is on the `..` row, **When** the user presses F4, **Then** nothing happens (consistent with current behaviour for F4 on directories).

---

### Edge Cases

- **Write failure**: If saving fails (disk full, permission denied), the editor must remain open, the modified indicator must stay set, and a status message must describe the error. The in-memory content is not lost.
- **File changed on disk between open and save**: Overwrite silently (same as most terminal editors) — out of scope for this feature; a future "reload prompt" can be added.
- **Very long lines**: Lines exceeding the terminal width are wrapped visually (or scrolled horizontally — see Assumptions); they are stored and saved without truncation.
- **Undo/Redo**: Out of scope for this feature. A single undo buffer is a viable future addition (tracked separately).
- **Syntax highlighting**: Out of scope; the editor renders plain text.
- **Tab characters**: Displayed as a configurable number of spaces (default: 4) for visual consistency; saved as literal tab bytes on disk.

---

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Pressing F4 on a focused plain-text file MUST open the built-in editor full-screen, occupying the entire terminal area (no file-manager chrome visible during editing).
- **FR-002**: The editor header MUST display the filename and a modified indicator (e.g. `*`) when unsaved changes exist; the footer MUST display the current line number and column number.
- **FR-003**: The editor MUST support the following navigation keys: arrow keys (character/line), Home/End (line start/end), Page Up/Page Down (viewport scroll), Ctrl-Home/Ctrl-End (file start/end).
- **FR-004**: The editor MUST support: character insertion at cursor, Backspace (delete left), Delete (delete right), Enter (split line), and line-level operations do not need to be atomic across multiple lines in P1.
- **FR-005**: F2 or Ctrl-S MUST save the current buffer to the original file path. On success, the modified indicator clears. On failure, the error is shown in the footer and the editor remains open.
- **FR-006**: F10, Esc, or Q MUST exit the editor. If no unsaved changes exist, exit is immediate. If unsaved changes exist, a confirmation prompt (FR-007) is shown first.
- **FR-007**: The unsaved-changes confirmation prompt MUST offer at least Save, Discard, and Cancel actions, accessible by keyboard. It MUST be implemented using the existing shared dialog/modal system (Constitution §III).
- **FR-008**: If the focused file is binary or exceeds 10 MiB, F4 MUST decline to open it and display a descriptive status message. The threshold is configurable via the existing config system.
- **FR-009**: After the editor closes (save or discard), the originating pane MUST be refreshed so any file-size or timestamp change is visible.
- **FR-010**: The existing `$EDITOR` shell-out path for F4 MUST be removed; the internal editor replaces it entirely. (The `$PAGER` shell-out for F3 is unaffected — F3 uses the built-in viewer from Feature 051.)

### Key Entities

- **EditorState**: In-memory representation of the file being edited. Holds the line buffer, cursor position (line, column), scroll offset, the original file path, and the dirty/modified flag.
- **EditorDialog**: The full-screen TUI widget that renders the editor — header bar (filename + modified indicator), scrollable content area, footer (line:col status and key hints). Follows the existing dialog widget conventions.
- **UnsavedChangesDialog**: A modal confirmation widget (Save / Discard / Cancel) shown when the user exits with a dirty buffer. Reuses the shared dialog infrastructure.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: F4 on a plain-text file opens the editor and renders the first screenful of content within one rendered frame (≤16 ms, consistent with the existing frame budget NFR-002).
- **SC-002**: Keypress-to-visible-update latency for character insertion, deletion, and cursor movement is ≤16 ms (same frame budget as the rest of the TUI — enforced by the existing keypress-latency bench).
- **SC-003**: All existing tests continue to pass; no regressions in the file-manager, viewer, or transfer features.
- **SC-004**: At least two automated tests cover the editor: one verifying that inserting text and saving produces the correct file content on disk, and one verifying that the unsaved-changes guard is triggered on exit with a dirty buffer.
- **SC-005**: The editor correctly round-trips all valid UTF-8 text files (content read equals content written when no edits are made).
- **SC-006**: F4 on a binary file and on a file >10 MiB both return to the file manager without opening the editor — verified by automated tests.

## Assumptions

- The editor targets **plain-text files only**. Binary detection uses the presence of null bytes or UTF-8 decode failures in the first 8 KiB of the file.
- **Long-line display**: Lines wider than the terminal are wrapped visually (soft-wrap). Horizontal scrolling is deferred to a future feature.
- **Encoding**: Only UTF-8 is supported. Files with a BOM or non-UTF-8 encoding are treated as binary and declined.
- **Line endings**: The file is read and written with its original line endings preserved (LF or CRLF detected on first read; saved identically).
- **File size limit default**: 10 MiB. This is configurable through the existing `cargonaut-config` system (a new `ui.editor_max_file_mib` key with default 10).
- **No undo in P1**: Undo/redo is a known gap — deferred to a follow-up feature. The spec notes this explicitly so users set expectations correctly.
- **Tab display**: Tabs are displayed as 4 spaces visually; written back as literal `\t` bytes. Tab width is not configurable in this feature.
- **Concurrency**: The file is opened by the TUI event loop. No background I/O thread is needed for saving (files ≤10 MiB save synchronously in ≪1 ms on local storage).
- **The existing F3 built-in viewer (Feature 051) is unaffected**: only the F4 code path changes.

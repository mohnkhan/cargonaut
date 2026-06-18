# Feature Specification: Internal File Viewer F3 (Text + Hex + Search)

**Feature Branch**: `051-f3-file-viewer`

**Created**: 2026-06-18

**Status**: Draft

**Input**: User description: "Internal file viewer F3 (text + hex + search, closes #39). Feature 031 ships F3 as an external-pager shell-out ($PAGER). This feature replaces that with a built-in viewer that runs entirely inside the TUI without shelling out. Capabilities: (1) text mode — stream/display file content with line numbers, word-wrap toggle, and vertical scrolling; (2) hex mode — display raw bytes in classic 16-bytes-per-row hex+ASCII layout with scrolling; (3) incremental search — forward and backward search across visible content, highlight matches; (4) goto line/offset — jump to a specific line number (text) or byte offset (hex); (5) keyboard navigation compatible with the existing keymap (arrow keys, PgUp/PgDn, Home/End, /, n, N, g, G, q/Esc to close). The viewer handles files up to a configurable size limit (default 10 MB for text, no limit with streaming for hex); larger files stream on demand rather than loading into memory all at once. The viewer opens on F3 or Enter on a file entry and closes with q or Esc, returning the pane to its pre-viewer state."

## Overview

Feature 031 wired F3 to shell out to `$PAGER`, suspending the TUI around an external process. This feature replaces that external dependency with a built-in viewer that runs entirely within the TUI without shelling out. After this feature ships, pressing F3 on a highlighted file (or pressing Enter on a file entry) opens a full-screen viewer overlay. The viewer supports text mode (UTF-8 content with line numbers, word-wrap toggle, and scrolling), hex mode (raw byte display in classic 16-byte-per-row hex+ASCII layout), incremental search (forward and backward, with match highlighting), and position navigation (goto line number, goto byte offset, jump to end). Files within a fixed threshold (10 MiB, a named constant — not user-configurable in this feature) are loaded entirely; larger files stream content on demand. Pressing `q` or `Esc` closes the viewer and returns the pane to its exact pre-viewer state.

## Clarifications

### Session 2026-06-19

- Q: How should ANSI escape sequences in file content be handled in text mode? → A: Strip ANSI sequences silently — display plain text without rendering colours or showing literal escape characters.
- Q: Should the 10 MiB streaming threshold be a hardcoded constant or a user-configurable setting? → A: Hardcoded named constant in the viewer module; no config-crate changes in this feature. Can be promoted to a user setting in a follow-up.
- Q: Should the viewer indicate when a search covers only a partial (streaming) buffer? → A: Yes — the status bar annotates search results with the loaded portion when the file is streaming, so users know results may be incomplete.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Open a file in text mode and scroll (Priority: P1)

A user navigating a directory sees a text file and presses F3. The TUI does not suspend or shell out; instead, a full-screen overlay opens immediately, showing the file's content in UTF-8 text mode with line numbers in the left margin. The user scrolls down with the arrow keys and Page Down, and up with Page Up. Home jumps to line 1. The status bar shows the current line and total line count (e.g., `Line 42/350`). The user presses `q` to dismiss the overlay; the pane cursor is unchanged.

**Why this priority**: This is the core deliverable — a file viewer that works entirely in-process, replacing the fragile `$PAGER` shell-out. All other capabilities (hex, search, goto) are extensions of this base.

**Independent Test**: Press F3 on any text file; confirm an overlay opens with numbered lines, scrollable with arrow keys; press `q` and confirm the pane is unchanged.

**Acceptance Scenarios**:

1. **Given** a file is focused in the active pane, **When** the user presses F3, **Then** a full-screen modal overlay opens showing the file content in text mode with line numbers, without suspending or shelling out.
2. **Given** the viewer is open, **When** the user presses Down/Up arrows, **Then** the content scrolls one line.
3. **Given** the viewer is open, **When** the user presses Page Down/Page Up, **Then** the content scrolls by approximately one screen height.
4. **Given** the viewer is open, **When** the user presses Home, **Then** the view jumps to line 1.
5. **Given** the viewer is open, **When** the user presses `q` or Esc, **Then** the overlay closes and the pane cursor and selection are exactly as they were before F3.
6. **Given** the viewer is open, **When** the user presses any key that is not a viewer navigation or command key, **Then** that keypress is swallowed — it does not trigger underlying pane commands.
7. **Given** F3 is pressed on a directory entry, **Then** the viewer does not open and the status bar shows a brief "Not a file" message.

---

### User Story 2 — View binary content in hex mode (Priority: P1)

A user opens a binary file with F3. Because the file is not valid UTF-8, the viewer opens automatically in hex mode. The user sees rows of 16 bytes each, with hex values on the left and their ASCII equivalents (printable chars or `.` for non-printable) on the right. Each row shows the byte offset in the leftmost column. The user can also toggle hex mode manually in any viewer session using `Ctrl-x X`. The status bar shows the current byte offset and total file size.

**Why this priority**: Hex mode is essential for binary files and is the primary reason the built-in viewer is more valuable than a generic pager. A pager renders binary garbage; the built-in viewer detects non-UTF-8 content and presents it usefully.

**Independent Test**: Press F3 on a binary file (e.g., the compiled cargonaut binary); confirm the viewer opens in hex mode automatically and shows offset + hex + ASCII columns without crashing.

**Acceptance Scenarios**:

1. **Given** a file whose content is not valid UTF-8, **When** F3 opens the viewer, **Then** hex mode is active automatically.
2. **Given** a UTF-8 text file is open in text mode, **When** the user presses `Ctrl-x X`, **Then** the viewer switches to hex mode; pressing `Ctrl-x X` again switches back to text mode.
3. **Given** the viewer is in hex mode, **When** the user scrolls, **Then** content advances in multiples of the row width (16 bytes per row).
4. **Given** the viewer is in hex mode, **When** the status bar is visible, **Then** the current byte offset and total file size are shown.
5. **Given** the viewer is in hex mode, **When** the file is a symlink, **Then** the target file's raw bytes are displayed (symlink is followed).

---

### User Story 3 — Search within the file (Priority: P2)

A user wants to find all occurrences of a pattern in a large log file. They press `/` to open an incremental search prompt at the bottom of the overlay, type a search string, and press Enter. The viewer jumps to the first match below the current line. All visible matches are highlighted. The user presses `n` to advance to the next match and `N` to go back. When searching backward with `?`, the direction is reversed. If no match is found, the status bar shows "Pattern not found".

**Why this priority**: Search converts the viewer from a passive reader into a useful tool for log analysis and code inspection. Without search, users who need to find content must exit and use external tools.

**Independent Test**: Open a text file with a known repeating token via F3; press `/`, type the token, press Enter; confirm the view jumps to the first match; press `n` to advance to the next.

**Acceptance Scenarios**:

1. **Given** the viewer is open, **When** the user presses `/`, **Then** a search prompt appears at the bottom of the overlay.
2. **Given** a search prompt is open, **When** the user types a pattern and presses Enter, **Then** the viewer jumps to the first match below the current top line and highlights all visible matches.
3. **Given** a search is active with matches, **When** the user presses `n`, **Then** the viewer advances to the next match.
4. **Given** a search is active with matches, **When** the user presses `N`, **Then** the viewer goes to the previous match.
5. **Given** a `?` search, **When** the user types a pattern and presses Enter, **Then** the first match above the current line is found, and `n`/`N` directions are reversed.
6. **Given** the search pattern has no matches, **When** the user presses Enter, **Then** the status bar shows "Pattern not found" and the view does not scroll.
7. **Given** a search is active, **When** the user presses Esc at the search prompt, **Then** the prompt closes and no search is performed; existing highlights are cleared.
8. **Given** the viewer switches between text and hex mode, **When** a search is active, **Then** the search state is cleared (patterns only apply within a single mode).

---

### User Story 4 — Goto a specific position (Priority: P2)

A user wants to jump to line 1200 in a long config file. They press `g`, type `1200`, and press Enter. The viewer jumps to that line. In hex mode, they press `g`, type `0x1f00`, and the view jumps to byte offset 8192. Pressing `G` (capital) jumps to the last line (text) or last block (hex) regardless of file size. Pressing `Home` always returns to position 1/offset 0.

**Why this priority**: Goto is a productivity multiplier for large files. Without it, reaching line 5000 requires holding Page Down for minutes.

**Independent Test**: Open a file with more than 100 lines; press `g`, type `50`, press Enter; confirm the view jumps to line 50; press `G` and confirm the last line is visible.

**Acceptance Scenarios**:

1. **Given** the viewer is open in text mode, **When** the user presses `g`, **Then** a goto prompt appears at the bottom asking for a line number.
2. **Given** the goto prompt is open, **When** the user types a valid line number and presses Enter, **Then** the viewer scrolls to show that line at the top.
3. **Given** the goto prompt is open with a line number beyond the last line, **Then** the viewer jumps to the last line (clamped).
4. **Given** the viewer is open in hex mode, **When** the user presses `g`, **Then** the goto prompt accepts a decimal or `0x`-prefixed hex byte offset.
5. **Given** the viewer is open (either mode), **When** the user presses `G`, **Then** the viewer jumps to the last line or last block.
6. **Given** the goto prompt, **When** the user presses Esc, **Then** the prompt closes without scrolling.

---

### User Story 5 — Large file streaming and Enter-on-file shortcut (Priority: P2)

A user presses Enter on a 500 MiB log file. The viewer opens within 150 ms and displays the first screenful. Scrolling works immediately. Memory usage stays bounded; the viewer does not load the entire file before displaying content. The user can also use word-wrap toggle (`w`) so long lines wrap at the terminal width rather than being truncated.

**Why this priority**: Without streaming, the viewer would be unusable on large files and potentially crash the session. Word-wrap makes the viewer usable for files with very long lines (e.g., JSON on one line, minified JS).

**Independent Test**: Open a file larger than 10 MiB with F3; confirm it opens within 150 ms; scroll several pages; confirm RSS stays below 64 MiB.

**Acceptance Scenarios**:

1. **Given** a file focused in the pane, **When** the user presses Enter (not on the `..` parent row), **Then** the viewer opens as if F3 was pressed (file entries open the viewer; the `..` row still ascends to the parent directory).
2. **Given** a file larger than 10 MiB, **When** F3 opens the viewer in text mode, **Then** the viewer displays the first screenful within 150 ms by streaming on demand, without loading the whole file first.
3. **Given** the viewer is open in text mode, **When** the user presses `w`, **Then** long lines wrap at the terminal width; pressing `w` again disables wrapping.
4. **Given** the viewer is streaming a large file, **When** the user scrolls quickly to a distant position, **Then** the viewer correctly displays the content at that offset within one second.
5. **Given** the viewer has been streaming a large file with scrolling, **When** the user closes the viewer, **Then** the process memory returns to near the pre-viewer baseline (no retained buffers).

---

### Edge Cases

- **File disappears while open**: If the backing file is deleted or moved while the viewer is open, the already-loaded content remains displayed; new scroll requests that require reading more content show a "File no longer readable" status in the header.
- **File is a symlink**: The viewer follows the symlink and reads the target's content. The title bar shows the symlink name.
- **Empty file**: Viewer opens showing "(empty file)" instead of content. No crash.
- **Terminal narrower than 80 columns**: Line numbers and content are truncated to fit; no wrapping artifacts or panics.
- **Terminal shorter than 4 rows**: The viewer renders what it can in the available area; at minimum the title bar and one content row are displayed.
- **File with mixed UTF-8 and binary bytes**: The file is treated as binary (hex mode) when any non-UTF-8 byte is encountered in the initial detection scan (first 4096 bytes).
- **Search pattern is a regex metacharacter sequence**: Search is literal string matching (no regex); special characters are treated as literal. A future feature may add regex search.
- **F3 pressed while viewer is already open**: The keypress is swallowed (viewer does not re-open on top of itself).
- **Viewer open while another dialog is already active**: F3 is swallowed; viewers do not stack over other modals.
- **Word-wrap in hex mode**: The `w` key has no effect in hex mode (rows are fixed-width); a brief status explains this.
- **File with ANSI escape sequences**: In text mode, all ANSI/terminal control sequences (CSI, OSC, and other escape sequences) are stripped from content before rendering; the display shows plain text only. This applies regardless of whether the file is otherwise valid UTF-8. Stripping prevents terminal corruption from crafted files.

## Requirements *(mandatory)*

### Functional Requirements

#### Viewer entry and exit

- **FR-001**: Pressing F3 on a focused file entry in the active pane MUST open the internal viewer overlay; the application MUST NOT shell out to `$PAGER` or any external program.
- **FR-002**: Pressing Enter on a focused file entry (not the `..` parent row) MUST open the internal viewer, identical to pressing F3. Pressing Enter on the `..` parent row MUST continue to ascend to the parent directory.
- **FR-003**: Pressing `q` or Esc while the viewer is open MUST close it; the pane cursor position, selection set, and active filter MUST be identical to their state before the viewer opened.
- **FR-004**: F3 MUST be swallowed (ignored) when any other modal dialog is already active.
- **FR-005**: F3 pressed on a directory entry MUST NOT open the viewer; the status bar MUST display a brief "Not a file" message.

#### Text mode

- **FR-006**: The viewer MUST open in text mode by default for files that are valid UTF-8 (determined by inspecting the first 4096 bytes).
- **FR-007**: Text mode MUST display content with a line-number gutter on the left, separated from content by a single space.
- **FR-008**: The viewer MUST support vertical scrolling with Up/Down arrows (1 line), Page Up/Down (1 page), Home (line 1), and End/`G` (last line).
- **FR-009**: The viewer MUST display a status line showing the current top line number and total line count (e.g., `Line 1/350`).
- **FR-010**: The viewer MUST support a word-wrap toggle activated by `w`; when wrap is on, lines longer than the terminal width wrap to the next row; when wrap is off, long lines are truncated at the right edge.

#### Hex mode

- **FR-011**: Files that are not valid UTF-8 MUST open in hex mode automatically.
- **FR-012**: Pressing `Ctrl-x X` MUST toggle between text and hex mode; the scroll position is reset to the top on each toggle.
- **FR-013**: Hex mode MUST display rows of 16 bytes, each row showing: an 8-digit byte offset (hex), 16 space-separated two-hex-digit byte values, and 16 ASCII characters (printable chars shown as-is; non-printable shown as `.`).
- **FR-014**: Hex mode MUST display a status line showing the current byte offset and total file size (e.g., `Offset 0x0000 / 524288 bytes`).

#### Search

- **FR-015**: Pressing `/` MUST open a search prompt at the bottom of the viewer; typing a pattern and pressing Enter MUST jump to the first match at or below the current top line.
- **FR-016**: Pressing `?` MUST open a backward search prompt; the first match at or above the current top line is found.
- **FR-017**: Pressing `n` MUST advance to the next match in the current search direction; `N` MUST go to the previous match.
- **FR-018**: All visible matches in the current view MUST be highlighted with a distinct style.
- **FR-019**: Search MUST use literal string matching (no regex); all characters in the pattern are treated literally.
- **FR-020**: If the pattern has no matches, the status bar MUST show "Pattern not found" and the view MUST NOT scroll.
- **FR-021**: Pressing Esc at the search prompt MUST close the prompt without searching; any existing highlights MUST be cleared.
- **FR-022**: Switching between text and hex mode MUST clear the active search state.
- **FR-033**: When a search is performed on a file in streaming mode (i.e. the file exceeds `STREAMING_THRESHOLD` and only a portion is loaded), the status bar MUST annotate both the match count and the "Pattern not found" message with the loaded coverage (e.g. `"1 match (searched 10 MiB of 512 MiB)"` or `"Pattern not found (searched 10 MiB of 512 MiB)"`), so users know the search did not cover the full file.

#### Goto

- **FR-023**: Pressing `g` MUST open a goto prompt at the bottom of the viewer. In text mode, the prompt accepts a decimal line number. In hex mode, it accepts a decimal integer or a `0x`-prefixed hexadecimal byte offset.
- **FR-024**: After a valid goto input, the viewer MUST scroll so the target line or offset is at the top of the visible area.
- **FR-025**: A goto value beyond the end of the file MUST be clamped to the last line or last full row (no crash, no error; the viewer simply shows the end).
- **FR-026**: Pressing Esc at the goto prompt MUST close the prompt without scrolling.

#### Memory and streaming

- **FR-027**: Files whose decoded line count (text mode) or byte count (hex mode) fits within the fixed threshold (`STREAMING_THRESHOLD = 10 MiB`, a named constant in the viewer module) MUST be loaded entirely into an in-process buffer before display. This threshold is not user-configurable in this feature.
- **FR-028**: Files exceeding 10 MiB MUST be streamed on demand: only the bytes needed to render the current screen (plus a small read-ahead buffer) are held in memory at any time.
- **FR-029**: The viewer's resident memory contribution MUST remain below 64 MiB for any file size during normal scrolling operations.

#### Content sanitisation

- **FR-032**: In text mode, the viewer MUST strip ANSI/terminal escape sequences (CSI sequences, OSC sequences, and any `ESC`-prefixed control sequence) from file content before rendering; the displayed text MUST be the plain, de-colourised content. Stripping MUST occur before search matching so that search patterns match the visible plain text, not raw escape bytes.

#### Keymap integration

- **FR-030**: All viewer-mode bindings (search, hex toggle, goto, word-wrap) MUST be defined in `design/contracts/keymap.toml` under `mode = "preview"` before any implementation. The existing FR-209 bindings (`Ctrl-x X`, `/`, `?`, `n`, `N`) MUST remain unchanged.
- **FR-031**: The `g`, `G`, `w`, and `q` bindings in preview mode MUST be added to `design/contracts/keymap.toml` as part of this feature.

### Key Entities

- **ViewerState**: active file path, current display mode (text or hex), top scroll offset (line number or byte offset), optional active search (pattern, direction, last match position), word-wrap flag. Owned by the UI layer (not persisted across sessions).
- **ViewBuffer**: in-process content cache; for files ≤10 MiB (the `STREAMING_THRESHOLD` constant) holds the full decoded content; for larger files holds only the pages needed to render the current screen plus a fixed read-ahead window.
- **SearchState**: current pattern string, search direction (forward/backward), index of the last matched position within the buffer.
- **ViewMode**: `Text` | `Hex` — governs rendering, scrolling granularity, search encoding, and goto prompt format.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The viewer opens and displays the first screenful of a ≤1 MiB text file within 150 ms of the F3 keypress (measured from key event to TUI repaint).
- **SC-002**: Every keypress within the viewer (scroll, search advance, mode toggle) produces a visible repaint within 16 ms (NFR-002 compliance, enforced by the existing `benches/keypress_latency.rs` harness).
- **SC-003**: Resident memory usage with a 1 GiB file open and fully scrolled remains below 64 MiB (SC-003 compliance; verified by the existing `benches/rss_headroom.rs` harness).
- **SC-004**: The stripped release binary remains ≤8 MiB after the viewer is added (NFR-001; enforced by `scripts/check-binary-size.sh`).
- **SC-005**: All existing tests pass without modification; the viewer adds at least 30 new unit or integration tests.

## Assumptions

- Files opened by the viewer are readable by the current process user; world-readable or user-readable files are the assumed case. Files with permission errors show "Cannot open file: permission denied" in the viewer body and close after one keypress.
- The terminal emulator supports at least 256 colors or basic ANSI highlighting for match highlighting; on monochrome terminals the viewer falls back to reversed-video for highlights.
- Search operates only on the content already in the `ViewBuffer` (i.e., the loaded portion). For streaming files, only content loaded so far is searched; the status bar annotates results with loaded coverage (FR-033). Full-file search across streaming content is a future enhancement.
- `Enter` on a file entry opening the viewer is a behavior change from the current "no-op with status message" (`descend_into_focused` comment: "T1.21 will open via $EDITOR / openers"). This feature implements that intended open-on-enter behavior specifically for the built-in viewer; the comment should be updated when the change ships.
- The `g` (goto) binding is added to the existing `preview` mode in `keymap.toml`; it uses an inline prompt rather than a modal dialog, identical in style to the search prompt.
- The viewer is scoped to local VFS files; files on remote VFS backends (future Feature 048) are out of scope.

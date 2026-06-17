# Feature Specification: User Menu (F2) + Scrollable Hypertext Help (F1)

**Feature Branch**: `047-user-menu-help`

**Created**: 2026-06-18

**Status**: Draft

**Input**: User description: "User menu (F2) + built-in hypertext help content (Feature 031 follow-up, closes #50). Feature 031 shipped a minimal F1 overlay and F2 placeholder. This feature implements: (1) user-defined scriptable action menu activated by F2 — reads a user config file defining named actions with shell commands, optional conditions, and keyboard shortcuts; (2) full scrollable hypertext F1 help overlay — expands the current minimal F1 overlay into a paginated/scrollable viewer showing all keybindings, feature descriptions, and navigation hints."

## Overview

Feature 031 shipped the chrome layer (function-key bar, menu bar, themed panes) and wired most F-key commands live, but intentionally deferred two subsystems: the **F2 user menu** (a scriptable, user-defined action menu) and the **F1 help content** (a scrollable reference of all keybindings and features). Both appear on the function-key bar, but pressing them today produces a "not yet available" status line message rather than a live overlay.

This feature closes that gap. After this feature ships, pressing F1 opens a scrollable, multi-section help viewer compiled into the binary. Pressing F2 opens a modal menu whose items are loaded from the user's `~/.config/cargonaut/menu.toml`; each item runs a shell command with the selected entry's path as an argument. Both overlays are navigable by keyboard (and mouse where relevant) and dismissable with Esc.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Scrollable help viewer (F1) (Priority: P1)

A user presses F1 at any time during normal operation and a full-screen modal overlay appears, showing a comprehensive, well-organized reference of all keybindings, features, and navigation hints. The content is divided into named sections (Navigation, File Operations, Panels, Bookmarks, Attributes, …). The user scrolls through the content with the arrow keys or Page Up/Down, jumps between sections, and dismisses the overlay with Esc or F1 again. The overlay content reflects the current state of the application — every binding that is live in the application is described.

**Why this priority**: The help overlay is the primary discoverability mechanism for users who do not already know every keybinding. The existing overlay dismisses on the first keypress and contains only a dozen lines of content, which is inadequate for a tool with 30+ bindings. Expanding it into a scrollable viewer with organized sections directly addresses the "I don't know what this app can do" discovery problem and is self-contained within the UI crate with no external dependencies.

**Independent Test**: Press F1; confirm a multi-section, scrollable overlay appears. Scroll past the bottom of the first section. Press Esc; confirm the overlay closes and the underlying state is unchanged.

**Acceptance Scenarios**:

1. **Given** the app is running in any pane state, **When** the user presses F1, **Then** a modal help overlay opens covering the full terminal area with a title, bordered sections, and visible scroll indicators.
2. **Given** the help overlay is open, **When** the user presses Down/Up or Page Down/Page Up, **Then** the content scrolls to reveal more keybindings and sections.
3. **Given** the help overlay is open and scrolled down, **When** the user presses Home, **Then** the overlay scrolls back to the top.
4. **Given** the help overlay is open, **When** the user presses Esc or F1, **Then** the overlay closes and the application state (active pane, cursor position, selection) is exactly as it was before F1 was pressed.
5. **Given** the help overlay is open, **When** the user presses any unrecognized key (not a navigation key or Esc/F1), **Then** the overlay stays open and the keypress is swallowed (does not trigger underlying commands).
6. **Given** the help overlay content, **When** reviewed, **Then** every live keybinding documented in `design/contracts/keymap.toml` appears in the overlay with a description.
7. **Given** a narrow or short terminal, **When** the help overlay renders, **Then** text is wrapped and the overlay is legible without overflowing or panicking.

---

### User Story 2 — User action menu (F2) with a config file (Priority: P1)

A user creates `~/.config/cargonaut/menu.toml` with named action items, each specifying a label, a shell command template, and optionally a condition and a keyboard shortcut. When the user presses F2 in the application, a modal menu overlay appears listing the user's defined actions. The user navigates with arrow keys, presses Enter to run the selected action, or Esc to dismiss. The chosen action runs with the currently highlighted entry's absolute path substituted into the command template (safely shell-quoted). The result of the action (exit code, stderr) is reported in the status bar.

**Why this priority**: The F2 key slot has been reserved since Feature 031 with a "not yet available" stub. The user action menu is the primary extensibility surface: it lets users integrate custom scripts (e.g., open in editor, compress, git status, rsync) without modifying the application. Ranked equal priority with F1 because both are simple modal overlays over the existing dialog infrastructure.

**Independent Test**: Create a `~/.config/cargonaut/menu.toml` with at least one action. Press F2; confirm the menu lists the action. Select it; confirm the command runs with the highlighted file's path.

**Acceptance Scenarios**:

1. **Given** a `~/.config/cargonaut/menu.toml` with at least one action, **When** the user presses F2, **Then** a modal menu overlay appears listing the user's defined action labels with their optional shortcut hints.
2. **Given** the F2 menu is open, **When** the user navigates with Up/Down and presses Enter, **Then** the selected action's command runs, with the currently highlighted entry's absolute path shell-quoted and substituted for the `{path}` placeholder in the command template.
3. **Given** the F2 menu is open, **When** the user presses Esc, **Then** the menu closes without running any action and the application state is unchanged.
4. **Given** an action whose command exits with a non-zero status, **When** it runs, **Then** the exit code and stderr (truncated to one line) are displayed in the status bar as a non-fatal error; the application continues normally.
5. **Given** `~/.config/cargonaut/menu.toml` does not exist or is empty, **When** the user presses F2, **Then** the menu opens showing a single informational placeholder row ("No actions defined — see docs for menu.toml format") and dismisses cleanly.
6. **Given** a `menu.toml` that is present but contains TOML syntax errors, **When** the user presses F2, **Then** the overlay reports the parse error (file + line number) in the menu body and the application continues without crashing.
7. **Given** an action with an `only_if` condition field set to a shell expression, **When** the expression evaluates to non-zero (false), **Then** that action is hidden from the F2 menu; when the condition evaluates to zero (true), the action appears.
8. **Given** multiple actions defined, **When** the user opens F2 and presses an action's shortcut key (single character), **Then** that action executes immediately as if the user had navigated to it and pressed Enter.

---

### User Story 3 — Config file format and documentation (Priority: P2)

A user who wants to add custom actions to F2 can read a short built-in help snippet (visible in the F1 viewer and as a comment in the generated default menu.toml) and understand the full menu.toml schema: label, command, optional `only_if`, optional `key` shortcut. The application ships a commented example `menu.toml` in `examples/` and the F1 overlay includes a brief description of the F2 user menu and a pointer to the config path.

**Why this priority**: Without documentation, the F2 menu is undiscoverable even if it works. A commented example and an F1 mention reduce support burden. Lower priority than the working overlay because the overlays must exist before documentation of them is useful.

**Independent Test**: Locate `examples/menu.toml`; confirm it is a valid, commented TOML file that the application parses without error. Confirm the F1 overlay mentions F2 and the config file path.

**Acceptance Scenarios**:

1. **Given** `examples/menu.toml` exists in the repo, **When** it is parsed by the application, **Then** it loads without errors and its actions appear in the F2 menu.
2. **Given** the F1 help overlay, **When** the user reads it, **Then** a section describing F2 and the location of `menu.toml` is present.

---

### Edge Cases

- **`{path}` placeholder absent from command**: the command runs as-is without substitution (useful for actions that do not need a file argument, e.g., `git status`).
- **Command contains no shell metacharacters**: prefer `Command::new(prog).arg(arg)` over `sh -c` to avoid shell injection entirely; only fall back to `sh -c` when the command string contains shell operators (`|`, `;`, `&&`, `$`, etc.).
- **Highlighted entry is the `..` parent row**: `{path}` resolves to the parent directory's absolute path.
- **Highlighted entry is a symlink**: `{path}` is the symlink path itself, not the resolved target.
- **User runs the action on a path containing spaces, quotes, or backslashes**: the path MUST be shell-quoted (via the `shell-quote` crate or equivalent) so the shell sees it as a single token.
- **Action command hangs indefinitely**: the UI must remain responsive; the command runs in a background task and can be interrupted (or the user can dismiss with Esc).
- **Very long action label**: truncated with ellipsis to fit the menu column width.
- **`menu.toml` is readable but the command binary does not exist**: the error is captured and shown in the status bar; no crash.
- **Help overlay on a terminal smaller than the minimum renderable size**: the overlay renders as much as fits; no panic.
- **F2 pressed while a dialog is already open**: F2 is swallowed (ignored) while another modal (confirm, input, tasks panel, etc.) is active — menus must not stack over other modals.
- **F1 pressed while F2 menu is open**: F2 menu closes first; F1 overlay is not opened simultaneously.

## Requirements *(mandatory)*

### Functional Requirements

#### F1 — Scrollable help overlay (US1)

- **FR-001**: Pressing F1 MUST open a full-screen modal help overlay that covers the entire terminal area.
- **FR-002**: The help overlay MUST present keybinding content in named sections (e.g., Navigation, File Operations, Panels & Modes, Bookmarks, File Attributes, Theme & Config, About).
- **FR-003**: The help overlay MUST be scrollable with Up/Down arrows and Page Up/Page Down; the visible region MUST update accordingly.
- **FR-004**: The help overlay MUST display a scroll indicator (e.g., line N/M or a scrollbar glyph) so the user knows their position within the content.
- **FR-005**: Pressing Esc or F1 while the overlay is open MUST close it; no other application state changes.
- **FR-006**: Any key that is not a navigation key (Up, Down, PageUp, PageDown, Home, End) and not a dismiss key (Esc, F1) MUST be swallowed by the overlay — it MUST NOT trigger commands in the underlying application while the overlay is open.
- **FR-007**: The help overlay content MUST include every live keybinding defined in `design/contracts/keymap.toml`, each with a plain-language description of its effect.
- **FR-008**: The help overlay content MUST be compiled into the binary (not read from an external file at runtime), so it is always available offline and is versioned with the code.
- **FR-009**: The help overlay MUST render legibly on terminals of at least 80×24; on smaller terminals it MUST truncate or wrap gracefully without panicking.

#### F2 — User action menu (US2)

- **FR-010**: Pressing F2 MUST open a modal menu overlay listing the actions defined in `~/.config/cargonaut/menu.toml`.
- **FR-011**: The menu overlay MUST be navigable with Up/Down arrow keys; pressing Enter on a highlighted item MUST execute that action's command.
- **FR-012**: Pressing Esc while the F2 menu is open MUST close it without executing any action.
- **FR-013**: When an action's command contains the literal placeholder `{path}`, the application MUST substitute the absolute path of the currently highlighted entry, shell-quoted, before execution.
- **FR-014**: Shell-quoting of the substituted path MUST use the `shell-words` crate (`shell_words::quote()`) or `Command::new(prog).arg(arg)` without a shell when the command has no shell operators — raw string interpolation into shell strings is forbidden (constitution macro-safety rule).
- **FR-015**: The action's command MUST run asynchronously so the TUI remains responsive during execution; the user MUST NOT be blocked from interacting with the app while the command runs.
- **FR-016**: After an action completes, its exit code MUST be shown in the status bar: success (exit 0) shows a brief confirmation; non-zero exit shows the exit code and the first line of stderr.
- **FR-017**: When `~/.config/cargonaut/menu.toml` does not exist or is empty, the F2 overlay MUST open showing a single informational placeholder row; it MUST NOT crash or show an error.
- **FR-018**: When `menu.toml` exists but contains a TOML parse error, the F2 overlay MUST open and display the parse error (filename and line number); the application MUST continue without crashing.
- **FR-019**: If an action defines an `only_if` field containing a shell expression, the action MUST be shown only when that expression exits 0; actions whose condition exits non-zero MUST be hidden.
- **FR-020**: If an action defines a `key` field (single printable character), pressing that character while the F2 menu is open MUST execute that action immediately.
- **FR-021**: F2 MUST be ignored (swallowed) while any other modal dialog is already open (confirm, input, tasks panel, hotlist, filter prompt, quick-cd). Menus MUST NOT stack.

#### Config format (US3)

- **FR-022**: The `menu.toml` format MUST support at minimum these per-action fields: `label` (string, required), `command` (string, required), `only_if` (string, optional), `key` (single character string, optional).
- **FR-023**: The application MUST ship `examples/menu.toml` — a valid, commented TOML file demonstrating the schema with at least three example actions.
- **FR-024**: The F1 help overlay MUST include a section describing the F2 user menu, the menu.toml config file location, and the `{path}` placeholder syntax.

### Key Entities

- **HelpContent**: The compiled-in multi-section keybinding reference. Structured as an ordered list of sections, each with a title and a list of (key, description) rows. Rendered as a scrollable overlay.
- **UserMenu**: The runtime-loaded list of `MenuItem` values parsed from `menu.toml`. Loaded on each F2 open (so edits to the file take effect without restarting the application).
- **MenuItem**: One entry in the user menu: `label`, `command`, optional `only_if`, optional `key`.
- **MenuExecution**: The async task that runs the action command, captures its exit code and stderr, and feeds a result back to the status bar.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user presses F1 and a help overlay appears in under 100 ms from keypress; the overlay is scrollable and remains open until the user dismisses it.
- **SC-002**: Every live keybinding in `design/contracts/keymap.toml` appears in the F1 help content; a CI test asserts this programmatically.
- **SC-003**: A user with a valid `menu.toml` presses F2 and sees their actions listed; selecting one runs the command with the correct path within 500 ms of the Enter keypress (command launch latency, not command completion time).
- **SC-004**: A path containing spaces, single quotes, and backslashes is passed to an action command without shell injection — the command receives the path as a single argument.
- **SC-005**: With no `menu.toml` present, pressing F2 shows a placeholder row rather than crashing; the application continues normally after dismissal.
- **SC-006**: A `menu.toml` with a TOML syntax error shows the error in the F2 overlay rather than crashing; the application continues normally after dismissal.
- **SC-007**: The stripped release binary size does not increase by more than 32 KiB above the Feature 046 baseline (help content is text; the user menu parser is small).

## Assumptions

- The `menu.toml` config directory (`~/.config/cargonaut/`) follows XDG Base Directory conventions; the path is resolved via `std::env::var("XDG_CONFIG_HOME")` with `$HOME/.config` fallback — the same pattern already used for `config.toml` and `themes/` resolution. No `dirs` crate dependency is needed.
- The `shell-words 1.1` crate will be added to `cargonaut-ui-tui`'s `[dependencies]` (T001) for safe shell tokenization and quoting.
- The existing `centered_rect` / dialog infrastructure in `cargonaut-ui-tui` is sufficient for both overlays; no new layout primitives are required.
- The `only_if` condition is evaluated synchronously at menu-open time using `std::process::Command` with a short timeout (≤200 ms); conditions that exceed the timeout are treated as false (hidden).
- Actions that produce no output and exit 0 show a brief "Done" confirmation in the status bar for 2 seconds, then revert to the normal status line.
- The F1 overlay uses a simple line-based scroll model (scroll offset in lines); no hyper-links or interactive elements are needed in this feature.
- Mouse support for the F2 menu (clicking to select and activate items) is included as a natural extension of the existing mouse infrastructure; clicking outside the overlay closes it.
- Keyboard navigation in the F2 menu uses the same pattern as the existing `TasksPanelDialog` (Up/Down to move, Enter to act, Esc to dismiss).

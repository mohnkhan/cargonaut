# Feature Specification: Visual & Interactive Parity Layer

**Feature Branch**: `031-visual-interactive-parity`

**Created**: 2026-06-14

**Status**: Draft

**Input**: User description: "Close the gap between Cargonaut and the reference orthodox dual-pane terminal file manager it is modeled on. Three user-reported defects drive it: (1) no mouse support, (2) the color theme looks off, (3) lots of functionality is simply missing."

## Overview

Cargonaut 0.1.0 ships a working engine — virtual filesystem, resumable transfer engine, keymap parser, command dispatch, and directory history — but its interactive surface is a deliberate Phase-1 MVP: two bordered panes and a one-line status bar rendered in monochrome, with the mouse disabled and most keymap commands inert. To a user familiar with the classic blue-background, dual-pane terminal file manager that Cargonaut is modeled on (referred to here as "the reference manager"), the application reads as unfinished and unfamiliar.

This feature delivers the **visual and interactive parity layer** that makes Cargonaut look and feel like the reference manager, without building the largest deferred subsystems (full internal viewer/editor, find-file, remote/archive filesystems). It addresses the three reported defects directly: a real color theme, a clickable mouse-driven interface, and the screen chrome plus the cheap-to-wire commands that users expect to be present.

## Clarifications

### Session 2026-06-14

- Q: Should mouse be ON by default now, or stay opt-in (current `ui.mouse=false`)? → A: **Default ON** — mouse capture is enabled by default so the fix is visible on first launch; configuration/flag can disable it, and a runtime toggle plus a hold-modifier bypass keep terminal-native text selection available.
- Q: Which panel listing modes ship this feature? → A: **Brief + Full + Quick-view** — quick-view makes the passive panel live-preview the highlighted file (bounded text preview); info-panel and tree modes are deferred.
- Q: What should F3 (View) and F4 (Edit) do, given the full internal viewer/editor are deferred? → A (recommended default, not overridden): **Shell out to external tools** — F3 launches the external pager (`$PAGER`, fallback `less`/`more`), F4 launches the external editor (`$EDITOR`, fallback `vi`/`nano`); the full *internal* viewer/editor remain deferred.
- Q: Load external/user-authored skin files, or built-in themes only? → A (recommended default, not overridden): **Built-in themes only** — ship 2+ compiled-in themes selectable by name; external skin-file format + loader are deferred to a tracked follow-up.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - The application looks like the reference manager (Priority: P1)

A user launches Cargonaut and immediately recognizes the familiar look: colored panels with a distinct background, directories visually distinct from files, executables and symlinks distinguishable at a glance, a clearly highlighted cursor row, and a clearly marked set of tagged files. The user can switch the color scheme by name (via a launch flag or the config file) and the choice takes effect.

**Why this priority**: Highest visual return on investment and the most direct fix for "the color theme looks off." Today the app paints with the terminal's default foreground/background and conveys every state purely through inverse video, so it looks washed-out and nothing resembles the reference manager. This story is self-contained in the presentation layer and unblocks the perception that the product is finished.

**Independent Test**: Launch the app with a known theme; confirm the panels render with the theme's panel background, that directory / executable / symlink / hidden entries each render in their distinct colors, that the cursor row and tagged rows are visually distinct, and that selecting a different theme by name changes the palette.

**Acceptance Scenarios**:

1. **Given** the app is launched with the default theme, **When** a directory listing is displayed, **Then** the panel renders with the theme's panel background color and directories, executables, symlinks, and regular files each appear in visually distinct colors.
2. **Given** a directory listing with a cursor and some tagged files, **When** the user looks at the panel, **Then** the cursor row is shown as a distinct colored bar and tagged rows are shown in the theme's "marked" color, distinguishable from each other and from normal rows.
3. **Given** the user specifies a valid theme name via the launch flag, **When** the app starts, **Then** that theme's palette is applied throughout (panels, borders, status bar, dialogs).
4. **Given** the user specifies an unknown theme name, **When** the app starts, **Then** the app falls back to a built-in default theme and continues without crashing (and surfaces a non-fatal notice).
5. **Given** the user's terminal supports only 16 colors, **When** a 256-color or truecolor theme is selected, **Then** the app still renders legibly by degrading colors rather than failing.

---

### User Story 2 - Screen chrome: function-key bar and menu bar (Priority: P1)

A user sees, at the bottom of the screen, the familiar numbered function-key button bar (e.g. `1 Help  2 Menu  3 View  4 Edit  5 Copy  6 RenMov  7 Mkdir  8 Delete  9 PullDn  10 Quit`), and at the top a pull-down menu bar with the standard menu titles. Each function-key label corresponds to a real action; pressing the function key (or, with mouse support, clicking the label) invokes it. The pull-down menu can be opened and its items selected to invoke the same commands. Each panel shows a one-line mini-status describing the highlighted entry.

**Why this priority**: The function-key bar and menu bar are the single most recognizable structural elements of the reference manager and the primary discoverability mechanism — without them, the available actions are invisible. They are also the on-screen targets that mouse support (US3) needs, so they are sequenced together. This story turns the large set of "silently dead" keymap commands into a discoverable, labeled surface.

**Independent Test**: Launch the app; confirm the function-key bar renders at the bottom with correct labels, the menu bar renders at the top, opening a menu shows its items, choosing an item (or pressing the corresponding function key) invokes the action, and each panel shows a mini-status line for the highlighted file.

**Acceptance Scenarios**:

1. **Given** the app is running, **When** the main view is displayed, **Then** a function-key bar is visible at the bottom with numbered, labeled buttons and a menu bar is visible at the top with menu titles.
2. **Given** the function-key bar is visible, **When** the user presses a function key whose action is implemented, **Then** the corresponding action runs; for actions that are deferred, the bar still shows the label and the app reports that the action is not yet available rather than doing nothing silently.
3. **Given** the menu bar, **When** the user opens a menu and selects an item, **Then** the associated command is invoked, identical to pressing its key binding.
4. **Given** a panel with a highlighted entry, **When** the entry changes, **Then** that panel's mini-status line updates to show details (name, size, modification time, permissions) of the highlighted entry.
5. **Given** a narrow terminal, **When** the chrome is rendered, **Then** labels are abbreviated or truncated gracefully without breaking the layout or panicking.

---

### User Story 3 - Mouse support (Priority: P1)

With the mouse enabled, a user can click a file to move the cursor to it and focus that panel, double-click a directory to enter it (or a file to open it via its associated action), scroll the wheel to move through a listing, and click the function-key bar buttons and menu bar titles to invoke them.

**Why this priority**: Directly fixes the reported "missing mouse support" defect. Mouse support is **enabled by default** (per clarification) so the fix is visible on first launch, with a runtime toggle and a hold-modifier bypass so terminal-native text selection remains available. It depends on the chrome from US2 (the function-key/menu bars are the click targets) and on lifting the on-screen layout regions into shared state so clicks can be mapped to what was clicked. Sequenced with US2.

**Independent Test**: With mouse enabled, click a row in each panel and confirm the cursor moves and focus follows; double-click a directory and confirm descent; scroll the wheel and confirm the listing scrolls; click a function-key button and a menu title and confirm the actions fire.

**Acceptance Scenarios**:

1. **Given** mouse support is enabled, **When** the user single-clicks a row in a panel, **Then** that panel becomes the active panel and the cursor moves to the clicked row.
2. **Given** mouse support is enabled, **When** the user double-clicks a directory row, **Then** the panel descends into that directory; double-clicking a file invokes its open/descend action.
3. **Given** mouse support is enabled, **When** the user scrolls the wheel over a panel, **Then** the listing scrolls / the cursor advances in the scroll direction.
4. **Given** mouse support is enabled, **When** the user clicks a function-key bar button or a menu bar title, **Then** the corresponding action or menu is invoked.
5. **Given** mouse support is disabled in configuration (or suspended via the runtime toggle / hold-modifier bypass), **When** the user interacts, **Then** the app behaves exactly as the keyboard-only build and terminal-native text selection/copy continues to work.
6. **Given** the app is launched with no mouse configuration set, **When** it starts, **Then** mouse support is active by default.

---

### User Story 4 - Richer panel listing: columns, parent entry, sorting, listing modes (Priority: P2)

A user sees more than just file names and sizes: a modification-time column and a permissions column, with a `..` parent entry as the first row for one-step ascent. The user can cycle the sort order (by name, extension, size, modification time, and reverse), switch between listing layouts (a compact multi-name "brief" mode, a detailed "full" mode, and a "quick-view" mode in which the passive panel live-previews the highlighted file), and request the recursive size of a highlighted directory on demand.

**Why this priority**: These are high-value, low-cost capabilities — the underlying metadata is already available in the filesystem layer, so this is mostly presentation and dispatch wiring. They make the panels informative and navigable in the way the reference manager's panels are. Lower priority than the visual identity and chrome because the app is usable without them.

**Independent Test**: Open a directory; confirm modification-time and permission columns are shown and a `..` row is present and ascends when activated; cycle the sort order and confirm the listing reorders; switch listing mode and confirm the layout changes; trigger recursive directory size and confirm the computed size appears for the highlighted directory.

**Acceptance Scenarios**:

1. **Given** a directory listing, **When** it is displayed, **Then** each entry shows at least name, size, modification time, and permissions, and a `..` entry is present as the first row (except at a filesystem root).
2. **Given** the cursor is on the `..` entry, **When** the user activates it, **Then** the panel ascends to the parent directory.
3. **Given** a listing, **When** the user cycles the sort order, **Then** the listing reorders accordingly and the active sort order is indicated; a reverse toggle inverts the order.
4. **Given** a listing, **When** the user switches the listing mode, **Then** the panel layout changes among the available modes: a compact "brief" layout, a detailed "full" layout, and a "quick-view" mode.
5. **Given** quick-view mode is active and the cursor is on a text-readable file, **When** the highlighted entry changes, **Then** the passive panel updates to show a bounded text preview of that file; a non-previewable or binary file shows a graceful placeholder rather than garbage.
6. **Given** the cursor is on a directory, **When** the user requests its recursive size, **Then** the computed total size is displayed for that directory in place of the directory placeholder.

---

### User Story 5 - Core operation parity: mkdir, pattern selection, transfer progress (Priority: P2)

A user can create a new directory (F7), select or unselect groups of files by typing a wildcard pattern (`+` / `-`), and — when a copy or move is running — see a progress dialog showing per-operation and total progress, throughput, and estimated time remaining, with the ability to cancel.

**Why this priority**: Completes the "missing functionality" complaint for the operations users reach for daily. Mkdir and pattern selection are cheap dispatch wiring over existing capabilities; the transfer progress dialog surfaces data the engine already emits (running state, throughput, ETA) but which the UI currently shows only as a count in the status bar. Lower priority than visual identity but essential for the app to feel complete.

**Independent Test**: Press the create-directory key, enter a name, and confirm the directory is created and appears in the listing; use the select-by-pattern action with a wildcard and confirm matching files become tagged (and unselect-by-pattern untags them); start a copy of a large file and confirm a progress dialog shows progress, throughput, and ETA, and that cancelling stops the transfer.

**Acceptance Scenarios**:

1. **Given** a panel showing a writable directory, **When** the user invokes create-directory and supplies a name, **Then** the directory is created and appears in the listing; an invalid name or a permission error is reported without crashing.
2. **Given** a listing, **When** the user invokes select-by-pattern and enters a wildcard, **Then** all entries matching the pattern become tagged; unselect-by-pattern untags matching entries.
3. **Given** a copy or move of a sizeable file or set is running, **When** it is in progress, **Then** a progress dialog displays the current file, per-operation and overall progress, throughput, and estimated time remaining.
4. **Given** a transfer is shown in the progress dialog, **When** the user cancels it, **Then** the transfer stops promptly and the dialog dismisses, consistent with the engine's existing cancellation behavior.
5. **Given** a transfer completes, **When** it finishes, **Then** the progress dialog dismisses and the destination panel reflects the new content.

---

### Edge Cases

- **Unknown / malformed theme name**: fall back to a built-in default; never crash; surface a non-fatal notice.
- **Low color-depth terminal**: degrade 256/truecolor themes to the nearest legible colors rather than failing.
- **No-TTY / non-interactive environment**: mouse capture and rendering must remain best-effort; teardown must always restore the terminal even on error.
- **Mouse click outside any known region** (between bars, on a border): ignored without side effect.
- **Click on an empty area below the last file** in a panel: focuses the panel but does not move the cursor past the last entry.
- **Double-click detection**: two clicks on the same row within a short interval count as a double-click; clicks on different rows do not.
- **`..` at a filesystem root**: no parent entry is shown (or it is inert) so the user cannot ascend above the root.
- **Recursive directory size on a very large tree**: the computation must not freeze the interface; results appear when ready and the rest of the UI stays responsive.
- **Pattern selection with a pattern matching nothing**: no entries are tagged and the user is informed the pattern matched zero files.
- **Narrow terminal**: chrome (menu bar, function-key bar, columns) abbreviates/truncates rather than overflowing or panicking.
- **Resize during interaction**: the next render recomputes all on-screen regions; a click arriving between a resize and the next render is handled against the most recent known layout without misbehaving.
- **Mkdir / pattern-select dialog cancelled**: no change is made.

## Requirements *(mandatory)*

### Functional Requirements

#### Theme / color (US1)

- **FR-001**: The system MUST render the interface using a named color theme rather than the terminal's default foreground/background alone.
- **FR-002**: The theme MUST define, at minimum, distinct colors for: panel background and foreground, directory entries, executable entries, symlink entries, hidden entries, the cursor (highlight) row, tagged ("marked") entries, focused vs. unfocused panel borders, the menu bar, the function-key bar, the status bar, and dialogs.
- **FR-003**: Directory listing rows MUST be colored according to entry type (directory, executable, symlink, regular, hidden) so that types are distinguishable at a glance.
- **FR-004**: The system MUST ship at least two built-in themes, including a default that evokes the reference manager's signature look (a distinct panel background with high-contrast entry and selection colors).
- **FR-005**: Users MUST be able to select the theme by name via the launch flag and via the configuration file; the launch flag MUST take effect (it is currently parsed but ignored).
- **FR-006**: When an unknown or invalid theme name is supplied, the system MUST fall back to a built-in default and continue, surfacing a non-fatal notice.
- **FR-007**: The system MUST render legibly on terminals limited to 16 colors by degrading richer (256/truecolor) themes.

#### Chrome: function-key bar, menu bar, mini-status (US2)

- **FR-008**: The system MUST display a function-key button bar along the bottom of the screen with numbered, labeled buttons reflecting the current context's actions.
- **FR-009**: The system MUST display a pull-down menu bar along the top of the screen with the standard menu titles, and MUST allow opening a menu and selecting an item to invoke the associated command.
- **FR-010**: Each panel MUST display a one-line mini-status describing the currently highlighted entry (at minimum name, size, modification time, permissions).
- **FR-011**: Function-key and menu actions that are implemented MUST invoke the corresponding command; actions that are deferred MUST still be labeled and MUST report "not yet available" rather than silently doing nothing.
- **FR-012**: The chrome MUST degrade gracefully on narrow terminals (abbreviate/truncate labels) without breaking layout.

#### Mouse (US3)

- **FR-013**: The system MUST enable mouse input **by default**, and MUST allow it to be disabled via configuration/launch flag and suspended at runtime via a toggle and a hold-modifier bypass; when disabled or suspended the mouse MUST NOT be captured (preserving terminal-native text selection).
- **FR-014**: A single left-click on a panel row MUST make that panel active and move the cursor to the clicked row.
- **FR-015**: A double-click on a row MUST invoke the open/descend action for that entry (enter a directory; open a file via its associated action).
- **FR-016**: Wheel scrolling over a panel MUST move through the listing in the scroll direction.
- **FR-017**: Clicking a function-key bar button or a menu bar title MUST invoke the corresponding action or open the menu.
- **FR-018**: Clicks outside any actionable region MUST be ignored without side effects, and the terminal MUST be restored correctly on exit regardless of mouse state.

#### Panel listing (US4)

- **FR-019**: Directory listings MUST display, for each entry, at least name, size, modification time, and permissions.
- **FR-020**: Listings MUST include a `..` parent entry as the first row that ascends to the parent directory when activated, except at a filesystem root where ascent above the root MUST NOT be possible.
- **FR-021**: Users MUST be able to cycle the sort order among at least name, extension, size, and modification time, with a reverse toggle; the active sort order MUST be discoverable.
- **FR-022**: Users MUST be able to switch the panel listing mode among a compact "brief" layout, a detailed "full" layout, and a "quick-view" mode in which the passive panel live-previews the highlighted file (bounded text preview; graceful placeholder for non-text/binary/oversized files).
- **FR-023**: Users MUST be able to request the recursive size of a highlighted directory on demand, and the result MUST be displayed without freezing the interface.

#### Operations (US5)

- **FR-024**: Users MUST be able to create a new directory by name, with invalid-name and permission errors reported without crashing.
- **FR-025**: Users MUST be able to tag and untag entries by wildcard pattern (select-by-pattern / unselect-by-pattern), with a zero-match pattern reported as matching nothing.
- **FR-026**: While a copy or move is running, the system MUST display a progress dialog showing the current item, per-operation and overall progress, throughput, and estimated time remaining.
- **FR-027**: Users MUST be able to cancel a transfer from the progress dialog, consistent with the engine's existing cancellation guarantees; the dialog MUST dismiss on completion or cancellation and the affected panel MUST refresh.

#### View / Edit via external tools (US2 function-key actions)

- **FR-030**: Invoking the View action (F3) on the highlighted file MUST launch the external pager (`$PAGER`, falling back to `less`/`more`), suspending and correctly restoring the terminal around the external process; the full *internal* viewer remains deferred.
- **FR-031**: Invoking the Edit action (F4) on the highlighted file MUST launch the external editor (`$EDITOR`, falling back to `vi`/`nano`), suspending and correctly restoring the terminal around the external process, and refreshing the panel afterward; the full *internal* editor remains deferred.

#### Cross-cutting

- **FR-028**: All newly surfaced actions MUST be invokable through their existing key bindings (no regressions to current keyboard behavior) in addition to any new mouse/menu affordances.
- **FR-029**: The set of capabilities explicitly NOT delivered by this feature (see Out of Scope) MUST each be tracked by a documented deferral (issue + roadmap entry) so they are not silently lost.

### Key Entities *(include if feature involves data)*

- **Theme**: a named collection of colors for each themable interface element (panel, entry types, cursor, marked, borders, menu, function-key bar, status, dialog). Resolvable by name to a built-in palette; degrades by color depth.
- **Screen layout regions**: the on-screen rectangles for each interactive area (left panel, right panel, status line, menu bar, function-key bar) used to map a click position to the thing clicked.
- **Function-key binding**: the association between a numbered function key, its displayed label, and the command it invokes in the current context.
- **Listing column set**: the fields shown per entry for a given listing mode (name, size, modification time, permissions), plus the synthetic `..` parent entry. The listing mode is one of brief / full / quick-view.
- **Quick-view preview**: a bounded text projection of the highlighted file shown in the passive panel; capped in size and degraded to a placeholder for non-text/binary/oversized files.
- **Sort order**: the active ordering key (name/extension/size/mtime) and direction (ascending/reverse) for a panel.
- **Transfer progress view**: the user-facing projection of an in-flight transfer (current item, bytes done/total, throughput, ETA, cancel affordance) derived from the engine's existing progress events.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: On first launch, a user familiar with the reference manager identifies Cargonaut as the same class of tool — colored dual panels, a top menu bar, and a bottom function-key bar are all visible without any interaction.
- **SC-002**: In the default theme, directories, executables, symlinks, regular files, hidden files, the cursor row, and tagged rows are each visually distinguishable from one another.
- **SC-003**: A user can move the cursor, change the active panel, enter a directory, and scroll a listing using only the mouse.
- **SC-004**: A user can invoke at least the implemented function-key actions both by pressing the function key and by clicking its button on the bar.
- **SC-005**: Of the commands targeted by this feature, 100% either perform their action or report a clear "not yet available" message — none silently do nothing.
- **SC-006**: A directory listing shows modification time and permissions for every entry and a working `..` parent entry.
- **SC-007**: A user can create a directory, tag files by wildcard pattern, and complete a copy while watching live progress with throughput and ETA, then see the result reflected in the target panel.
- **SC-008**: Selecting a different built-in theme by name changes the on-screen palette, and an invalid theme name does not prevent the app from starting.
- **SC-009**: Every capability listed as out of scope has a corresponding tracked deferral (issue + roadmap entry) at merge time.
- **SC-010**: The feature introduces no regression to existing keyboard-driven behavior (all existing keybindings continue to work) and the existing test suite continues to pass.

## Out of Scope *(deferred — each requires a tracked deferral per FR-029)*

The following reference-manager capabilities are intentionally **not** delivered by this feature and are to be tracked as deferrals (GitHub issue + ROADMAP row) per the project's deferral policy:

- Internal file viewer (text + hex + search). *(External-pager shell-out ships via FR-030; the full internal viewer is deferred.)*
- Internal full-screen editor. *(External-editor shell-out ships via FR-031; the full internal editor is deferred.)*
- External skin-file format and loader for user-authored themes. *(Built-in themes ship via FR-004; external skins deferred.)*
- Find-file (by name and by content) and external panelize.
- Directory hotlist / bookmarks.
- Compare-directories and diff-two-files.
- Persistent subshell integration (drop-to-shell toggle).
- Tabs (multiple panels per side).
- File attribute operations: chmod / chown, symlink / hardlink creation.
- Bulk rename via editor; undo of file operations.
- Virtual filesystem backends: archives-as-directories and remote (SFTP/FTP/shell) filesystems (already roadmapped for a later phase).
- User menu (F2 scriptable action menu) and the built-in help viewer content (F1) — the labels appear on the bar per FR-011 but the full subsystems are deferred.

## Assumptions

- The default theme should change from the current inert `"solarized-dark"` string to a built-in palette that evokes the reference manager's signature look; the exact default name is an implementation detail to be confirmed in planning.
- Mouse support is **on by default** (clarified); the existing `ui.mouse` flag/launch flag can disable it and a runtime toggle + hold-modifier bypass preserve terminal-native text selection for users who rely on it.
- The reference manager is referred to generically throughout; no trademarked product name appears in user-facing strings, code, or documentation.
- The filesystem layer already exposes the metadata (size, modification time, permission bits, entry kind, hidden flag) needed for richer columns and type-based coloring; no new metadata sources are required.
- The transfer engine already emits running-state events with throughput and ETA and supports cancellation; the progress dialog is a presentation of existing data, not new engine work.
- Existing key bindings and the command vocabulary defined in the keymap contract are the source of truth for which actions exist; this feature wires the currently-inert ones rather than inventing a new key scheme.
- Work proceeds on the `031-visual-interactive-parity` feature branch and merges via pull request with the mandatory README and Learnings documentation updates, consistent with project policy.
- Theme palettes and built-in themes are bundled with the binary; user-authored external skin files are **deferred** (clarified) and tracked as a follow-up.
- Quick-view preview reads a bounded amount of the highlighted file as text (e.g. a capped byte/line budget) to avoid coupling to the deferred full internal viewer and to keep the UI responsive on large files.
- F3/F4 shell out to external pager/editor (clarified); this requires suspending the alternate screen and raw mode around the child process and restoring them afterward, reusing the existing terminal teardown/setup discipline.

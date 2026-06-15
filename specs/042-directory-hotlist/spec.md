# Feature Specification: Directory Hotlist / Bookmarks

**Feature Branch**: `042-directory-hotlist`

**Created**: 2026-06-15

**Status**: Draft

**Input**: User description: "Directory hotlist / bookmarks. Named shortcuts to frequently-used directories with an add-current hotkey and optional grouping. Persist the hotlist to a file under the user config directory so bookmarks survive across sessions. A popup dialog (reusing the existing shared dialog widgets) lists the bookmarks; selecting one navigates the active pane to that directory. Provide a key to add the active pane's current directory as a new bookmark (prompting for a name), and a way to remove a bookmark. Bind the hotlist popup to Ctrl-b. This implements the deferred directory-hotlist capability from Feature 031 (§Out of Scope), tracked as issue #42."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Bookmark the current directory and jump back to it (Priority: P1)

A user navigates deep into a project directory they visit often. Rather than re-typing or re-navigating that path every time, they add it to their hotlist under a memorable name. Later — even after moving the active pane elsewhere — they open the hotlist, pick that name, and the active pane jumps straight to the bookmarked directory.

**Why this priority**: This is the core value of the feature: turn a frequently-visited path into a one-or-two-keystroke jump. The add→list→select→navigate loop is the minimal viable slice and is independently demonstrable in a single session.

**Independent Test**: From a pane at some directory, press the add-bookmark key, give it a name. Move the pane elsewhere. Open the hotlist (Ctrl-b), select the bookmark, and confirm the active pane is now at the bookmarked directory.

**Acceptance Scenarios**:

1. **Given** the active pane is at a directory, **When** the user invokes "add bookmark" and supplies a name, **Then** a new bookmark with that name and the pane's current directory is created and appears in the hotlist.
2. **Given** at least one bookmark exists, **When** the user opens the hotlist (Ctrl-b), **Then** a popup lists each bookmark by name (and its target directory).
3. **Given** the hotlist popup is open, **When** the user selects a bookmark, **Then** the active pane navigates to that bookmark's directory and the popup closes.
4. **Given** the hotlist popup is open, **When** the user cancels (Esc), **Then** the popup closes with no change to either pane and no change to the hotlist.

---

### User Story 2 - Bookmarks persist across sessions (Priority: P2)

A user who has curated a set of bookmarks expects them to still be there the next time they launch the application — they should not have to rebuild the list every session.

**Why this priority**: Persistence is what makes a hotlist worth curating; without it the feature is only a within-session convenience. It builds on US1 but is separable (US1 is fully usable in-session before persistence is wired).

**Independent Test**: Add a bookmark, quit the application, relaunch, open the hotlist, and confirm the bookmark is still present with the correct name and target.

**Acceptance Scenarios**:

1. **Given** the user has added one or more bookmarks, **When** the application exits and is relaunched, **Then** the previously added bookmarks appear in the hotlist with their names and targets intact.
2. **Given** no hotlist has ever been saved, **When** the application launches, **Then** it starts with an empty hotlist and does not error.
3. **Given** a saved hotlist file is malformed or unreadable, **When** the application launches, **Then** it starts with an empty (or last-valid) hotlist and surfaces a non-fatal notice rather than crashing.

---

### User Story 3 - Remove a bookmark (Priority: P2)

A user's bookmarked directory becomes obsolete (project archived, path renamed). They want to delete that entry from the hotlist so it stops cluttering the list.

**Why this priority**: Curation requires removal as well as addition; a list that only grows becomes useless. Secondary to the add/jump loop but needed for the feature to be maintainable by the user.

**Independent Test**: With at least one bookmark present, open the hotlist, remove a bookmark, and confirm it disappears from the list and stays gone after the popup is reopened (and after relaunch).

**Acceptance Scenarios**:

1. **Given** the hotlist popup is open with a bookmark highlighted, **When** the user invokes "remove", **Then** that bookmark is deleted from the hotlist and no longer appears.
2. **Given** a bookmark was removed, **When** the hotlist is next opened (including after relaunch), **Then** the removed bookmark does not reappear.

---

### Edge Cases

- **Bookmark target no longer exists / is inaccessible**: selecting it must fail gracefully with a clear message, leave the panes unchanged, and NOT silently delete the bookmark (the user may want to fix the path or remount the location).
- **Empty hotlist**: opening Ctrl-b with no bookmarks still opens the popup and clearly indicates there are none (and offers no dead-end), rather than doing nothing.
- **Duplicate / blank name**: adding a bookmark with an empty name, or a name that already exists, must be handled predictably (defined in Requirements/Assumptions).
- **Bookmarking the same directory twice** under different names: allowed; both entries coexist.
- **Concurrent edits**: the hotlist is per-user, single-application; last-write-wins on save is acceptable.
- **Very long list / very long names**: the popup must remain usable (scroll/truncate) without breaking layout.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Users MUST be able to open a hotlist popup with a single binding (`Ctrl-b`).
- **FR-002**: The hotlist popup MUST list each saved bookmark, showing its name and target directory.
- **FR-003**: Selecting a bookmark MUST navigate the active pane to that bookmark's target directory and close the popup.
- **FR-004**: Users MUST be able to add the active pane's current directory as a new bookmark, supplying a name for it.
- **FR-005**: Users MUST be able to remove an existing bookmark.
- **FR-006**: The hotlist MUST be persisted to a file under the user configuration directory so it survives across sessions.
- **FR-007**: On startup the system MUST load the persisted hotlist if present; absence of the file MUST yield an empty hotlist without error.
- **FR-008**: Selecting a bookmark whose target is missing or inaccessible MUST fail gracefully (clear message, panes unchanged, bookmark retained).
- **FR-009**: The hotlist popup MUST use the shared dialog/widget conventions (consistent keyboard navigation: move, select, cancel) rather than an ad-hoc layout.
- **FR-010**: Opening the hotlist when it is empty MUST still present the popup with a clear empty-state indication.
- **FR-011**: Adding a bookmark with a blank name MUST be rejected (or defaulted) predictably; the system MUST define behavior for a name that duplicates an existing one.
- **FR-012**: Cancelling the popup (Esc) MUST close it with no change to panes or to the hotlist.
- **FR-013**: A malformed or unreadable hotlist file MUST NOT crash the application; it MUST degrade to an empty/last-valid list with a non-fatal notice.

### Key Entities *(include if feature involves data)*

- **Bookmark**: a named shortcut to a directory. Attributes: a user-visible **name** and a **target directory** path. Optionally a **group/category** label (see Assumptions — grouping scope to be confirmed).
- **Hotlist**: the ordered collection of bookmarks for the user, persisted as a single file under the user configuration directory. Lifecycle: loaded at startup, mutated by add/remove, saved back to disk.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can add the current directory as a bookmark and later jump back to it within the same session, completing the round trip in a single, discoverable flow.
- **SC-002**: Bookmarks added in one session are present, with correct names and targets, after quitting and relaunching (100% of saved entries survive a clean exit).
- **SC-003**: A user can jump to an existing bookmark in at most two interactions (open hotlist, choose entry).
- **SC-004**: Selecting a bookmark whose target no longer exists never crashes and never loses the bookmark (100% of such attempts produce a message and leave state intact).
- **SC-005**: Removing a bookmark removes it permanently — it is absent on the next open and after relaunch (100%).
- **SC-006**: Opening an empty hotlist always presents a clear empty state (0 silent no-ops).

## Assumptions

- **Binding**: `Ctrl-b` opens the hotlist popup (the keymap already reserves `C-b` → `bookmarks-menu`); the add-bookmark action uses a distinct, currently-unbound key, finalized in planning.
- **Grouping is out of MVP scope**: the issue mentions "optional grouping"; the core feature ships a single flat list. Grouping/categories are treated as a deferred enhancement unless confirmed in-scope during clarification. (If deferred, it is tracked per the project's deferral discipline.)
- **Persistence format/location**: a single human-diffable file under the user config directory (alongside the existing config), following the project's existing config conventions. No database.
- **Scope is the interactive TUI**: bookmarks affect the running pane navigation only; they do not alter unrelated config and have no effect on non-interactive subcommands.
- **Navigation reuse**: jumping to a bookmark reuses the existing pane navigation path, so directory-history recording and invalid-path rejection behave the same as manual navigation.
- **Naming**: a bookmark name is a short free-text label; blank names are rejected. A new bookmark whose name duplicates an existing one is allowed to coexist OR replaces it — finalized in clarification (default: allow coexistence, since two names can point at related paths).
- **Removal scope**: removal deletes the hotlist entry only; it never touches the directory on disk.
- **Single user / single instance**: last-write-wins on save; no multi-instance merge.

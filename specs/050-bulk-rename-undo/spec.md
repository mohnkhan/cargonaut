# Feature Specification: Bulk Rename via Editor + Undo of File Operations

**Feature Branch**: `050-bulk-rename-undo`

**Created**: 2026-06-18

**Status**: Draft

**Input**: User description: "Bulk rename via editor + undo of file operations: dump tagged filenames to a temp file, open $EDITOR (reuse the F4 shell-out suspend/resume), read back the edited names, validate them (no-op for unchanged, error for duplicate/empty/invalid), apply as atomic renames, then support undo of the last file operation (rename, copy, move, delete) via a single keypress. Closes issue #47."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Bulk Rename via Editor (Priority: P1)

The user has a set of tagged files in the active panel that they want to rename in a coordinated way — for example, adding a common prefix, stripping a date suffix, or normalising casing across a batch. They press a key, and the app dumps the tagged filenames (one per line) into a temporary file, then suspends the TUI and opens their configured editor with that file. They edit the names freely in the editor, save, and exit. The app reads back the modified file, validates every renamed line (no duplicates, no empty lines, no path separators), and applies the renames atomically within the directory. Unchanged lines are silently skipped.

**Why this priority**: Bulk rename is the primary user-facing value of this feature. Without it, the undo subsystem has nothing meaningful to reverse. It is also the most-requested missing action in orthodox file managers.

**Independent Test**: Tag 3+ files in a local directory; invoke bulk rename; edit two names in the editor; confirm those two files are renamed, the unchanged file is untouched, and no temp files are left on disk.

**Acceptance Scenarios**:

1. **Given** two or more files are tagged, **When** the user invokes "Bulk rename", **Then** the TUI suspends, the editor opens with a temp file containing exactly one filename per line (basenames only, no path), the filenames are in the same order as the panel listing, and no other files or directories are listed.
2. **Given** the user edits some names and saves, **When** the editor exits, **Then** all edited names are renamed atomically in the source directory, unchanged names are left untouched, and the TUI repaints with the updated listing.
3. **Given** the user leaves all names unchanged, **When** the editor exits, **Then** no renames are performed and a status message confirms "No changes — nothing renamed".
4. **Given** the user introduces a duplicate name (two lines with the same value), **When** the editor exits, **Then** no renames are applied, and an error message identifies the duplicate.
5. **Given** the user leaves a line blank or deletes a line, **When** the editor exits, **Then** no renames are applied, and an error message identifies the empty/missing entry.
6. **Given** the user introduces a name containing a path separator (`/`), **When** the editor exits, **Then** no renames are applied and an error message identifies the offending name.
7. **Given** a rename target name already exists in the directory (a non-tagged file with that name), **When** the editor exits, **Then** no renames are applied and an error message identifies the collision.
8. **Given** no files are tagged, **When** the user invokes "Bulk rename", **Then** a status message says "Tag at least one file to bulk rename" and no editor is opened.

---

### User Story 2 — Undo Last File Operation (Priority: P2)

After performing any file-modifying operation — rename (including bulk), copy, move, or delete — the user can press a single key to reverse it. The undo reverses the most recent reversible operation in the active session. Undo is a single level only (one undo per session; subsequent undos are no-ops with a status message).

**Why this priority**: Undo provides a safety net for the bulk rename and for destructive operations (delete, overwrite). It dramatically reduces the cost of mistakes. It is explicitly listed in issue #47 as part of the scope.

**Independent Test**: Rename a file via the standard rename dialog; confirm the original name is restored after undo. Copy a file; confirm the copy is removed after undo. Move a file; confirm it is moved back. Delete a file; confirm the file is restored from the operation log (or a status message explains it cannot be undone).

**Acceptance Scenarios**:

1. **Given** a rename was just performed (single file or bulk), **When** the user presses the undo key, **Then** all renamed files are returned to their original names and the panel listing updates.
2. **Given** a copy was just performed, **When** the user presses the undo key, **Then** the destination copies are deleted and the panel listing updates.
3. **Given** a move was just performed, **When** the user presses the undo key, **Then** all moved files are moved back to their original location and both panels update.
4. **Given** a delete was just performed, **When** the user presses the undo key, **Then** a status message explains that delete is not undoable and suggests manual recovery; no change is made to the filesystem.
5. **Given** no operation has been performed in this session, **When** the user presses the undo key, **Then** a status message says "Nothing to undo".
6. **Given** an undo was already performed, **When** the user presses the undo key again, **Then** a status message says "Nothing to undo" (single-level undo only).
7. **Given** a bulk rename was undone, **When** the panel listing refreshes, **Then** all original names are restored and the selection state is cleared.

---

### Edge Cases

- What happens if the editor exits with a non-zero exit code? The rename is aborted and a status message reports the editor exited with an error.
- What happens if the temp file cannot be created (e.g., disk full)? The rename is aborted with a descriptive error; no editor is opened.
- What happens if the number of lines in the edited file differs from the number of tagged files? The rename is aborted with an error — adding or deleting lines is not supported.
- What happens if a source file was deleted between tagging and the rename being applied? That rename is skipped and reported in the status.
- What happens if an undo copy-delete fails (e.g., the destination is now read-only)? The undo is partially applied; the status reports which files could not be undone.
- What happens if the user tags directories alongside files for bulk rename? Directories are included in the rename the same as files — name-only rename, no recursion.
- What happens if the editor is not configured (`$EDITOR` is unset)? The bulk rename is aborted with a message explaining how to configure the editor, matching F4 behaviour.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST provide a "Bulk rename" action (bound to a keymap entry) that, when at least one entry is tagged, dumps all tagged entry basenames to a temporary file (one per line), suspends the TUI, and opens `$EDITOR` with that file, using the same TUI-suspend/resume mechanism as F4.
- **FR-002**: The temp file MUST contain exactly the basenames of the tagged entries (no path, no trailing newline after the last entry), in the same order they appear in the panel listing.
- **FR-003**: On editor exit, the app MUST read back the temp file and compare it line-by-line with the original names. Lines that are unchanged MUST be silently skipped. All changed lines are candidates for rename.
- **FR-004**: Before applying any renames, the app MUST validate the full set of proposed names: no empty names, no names containing `/`, no duplicate names within the set, no collision with existing non-tagged entries in the directory. Any validation failure MUST abort all renames and show a descriptive error message.
- **FR-005**: If the line count in the edited file does not match the number of tagged entries, the rename MUST be aborted with an error message.
- **FR-006**: If all proposed names are unchanged (no edits), the rename MUST be a no-op and a status message MUST confirm "No changes — nothing renamed".
- **FR-007**: Renames MUST be applied atomically within the directory — either all succeed or an error message identifies which rename failed and no further renames are attempted.
- **FR-008**: After a successful bulk rename, the operation MUST be recorded in the session undo log so it can be reversed with the undo action.
- **FR-009**: The temp file MUST be deleted after the rename attempt completes (success, validation failure, or editor error), leaving no temp files on disk.
- **FR-010**: The app MUST provide an "Undo last operation" action (bound to a keymap entry) that reverses the most recent reversible file operation in the active session.
- **FR-011**: The following operations MUST be reversible via undo: rename (single and bulk), copy (delete the destination copies), move (move files back to their original locations).
- **FR-012**: Delete MUST NOT be reversible; pressing undo after a delete MUST display a clear message explaining this limitation.
- **FR-013**: Undo MUST be single-level only: after one undo, the undo log is exhausted and subsequent undo presses show "Nothing to undo".
- **FR-014**: The undo log MUST be session-scoped — it does not persist across app restarts.
- **FR-015**: After undo completes, both panels MUST refresh their listings and the selection/tag state MUST be cleared.

### Key Entities

- **Tagged entry set**: The set of panel entries currently tagged (selected) by the user; the input to bulk rename.
- **Temp rename file**: A transient file (system temp directory) containing the original basenames; written before editor launch, deleted after rename completes.
- **Rename proposal**: A pair `(original_name, new_name)` for each tagged entry; derived by comparing the edited temp file against the original list.
- **Undo log**: A single-entry session record capturing the operation type and enough information to reverse it (original paths, destination paths). Stored in memory; not persisted.
- **Operation type**: One of `Rename`, `Copy`, `Move`, `Delete` — determines undo behaviour.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A bulk rename of 50 tagged files completes (editor → validation → rename applied) within 500 ms of editor exit, measured from the moment the editor exits to the moment the panel listing is repainted.
- **SC-002**: Zero validation false positives — a valid rename set (all names unique, no path separators, no collisions) is never incorrectly rejected.
- **SC-003**: Zero validation false negatives — an invalid rename set (duplicate, empty, collision, line count mismatch, path separator) is always rejected before any filesystem change is made.
- **SC-004**: Undo of a rename (single or bulk) completes and the panel listing is repainted within 500 ms of the undo keypress for up to 50 files.
- **SC-005**: The temp rename file is never left on disk after the bulk rename action completes, regardless of success or failure path.
- **SC-006**: After undo, the filesystem state matches the pre-operation state for all reversible operations (rename, copy, move) — verified by comparing file listings before and after.

## Clarifications

### Session 2026-06-18

- Q: Should bulk rename include directories as well as files in the tagged set, or files only? → A: Both files and directories are included — name-only rename, no recursion into directory contents.
- Q: Should the undo log record only the most recent operation across all operation types, or maintain separate undo stacks per operation type? → A: A single shared undo log (one entry) — the most recent reversible operation across all types. Subsequent operations overwrite the previous undo entry.
- Q: Should undo of a copy remove the destination copies even if they have been modified since the copy? → A: Yes — undo removes the destination copies unconditionally. The user is responsible for understanding that any post-copy edits to the copies will be lost. A status message warns of this when undo is initiated on a copy operation.

## Assumptions

- Bulk rename uses the same `$EDITOR` environment variable and TUI suspend/resume mechanism as the F4 "Edit" action. If `$EDITOR` is unset, the same error path as F4 applies.
- The temp file is written to the system temporary directory (e.g., `/tmp`) with a unique name to avoid conflicts between concurrent sessions.
- Renames are within a single directory (the active panel's directory). Cross-directory renames are not supported in this feature — they require a move, which is already implemented.
- The undo log holds at most one entry. It is an in-memory structure, not persisted across restarts.
- Undo of a copy does not verify whether the copies were modified after the copy; it removes them unconditionally and warns the user.
- Line order in the temp file matches the panel's current listing order (same order visible to the user). Re-ordering lines in the editor has no effect — only the names, not the order, matter.
- The temp file uses newline (`\n`) as the line separator. Files with newlines in their names cannot be bulk renamed via this mechanism (they are excluded from the tagged set with a status warning).

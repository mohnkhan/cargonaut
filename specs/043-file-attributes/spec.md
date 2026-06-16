# Feature Specification: File Attribute Operations (chmod / chown / links)

**Feature Branch**: `043-file-attributes`

**Created**: 2026-06-17

**Status**: Draft

**Input**: User description: "File attribute operations: change permissions/ownership and create links. Add the reference orthodox-FM file-attribute operations to the focused/tagged files: change Unix permissions (chmod, via both symbolic like u+x and octal like 0644), change ownership (chown user/group), and create symbolic links and hard links. Provide new VFS backend operations (chmod/chown/symlink/link) implemented for the LocalFs backend, surfaced through dialogs that reuse the existing shared dialog widgets, and reachable from the File menu (and a keybinding). Operations apply to the current selection (tagged files, or the focused entry if none tagged), require confirmation for destructive/ownership changes, refresh the affected pane, and report errors (e.g. permission denied) without crashing. This implements the deferred file-attributes capability from Feature 031 (§Out of Scope, FR-029), tracked as issue #46."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Change permissions of selected files (Priority: P1)

A user wants to make a script executable, or lock a file down to read-only. They tag one or more files (or just highlight one), invoke "change permissions", and enter the new mode either as an octal value (e.g. `755`, `0644`) or as a symbolic change (e.g. `u+x`, `go-w`, `a=r`). The permissions of every selected file change accordingly, and the pane immediately shows the updated permission column.

**Why this priority**: chmod is by far the most common file-attribute operation and the headline of this feature. It is the minimal viable slice and is independently demonstrable end to end (select → set mode → see new perms).

**Independent Test**: Highlight a file showing `rw-r--r--`, invoke change-permissions, enter `755` (or `u+x`), confirm the listing now shows `rwxr-xr-x`.

**Acceptance Scenarios**:

1. **Given** a file is highlighted, **When** the user sets an octal mode (e.g. `644`), **Then** the file's permission bits become exactly that mode and the listing reflects it.
2. **Given** a file is highlighted, **When** the user enters a symbolic change (e.g. `u+x`), **Then** only the indicated bits change relative to the current mode and the listing reflects it.
3. **Given** several files are tagged, **When** the user applies one mode, **Then** all tagged files receive that change in a single action.
4. **Given** an invalid mode string (e.g. `xyz`, `999`), **When** the user submits it, **Then** no file is changed and an inline error is shown.

---

### User Story 2 - Create symbolic and hard links (Priority: P2)

A user wants a shortcut to a file or directory in another location, or a second hard link to a file. With a file highlighted, they invoke "create symlink" (or "create hardlink"), are prompted for the new link's name/path, and a link is created pointing at the highlighted item. The new link appears in the listing.

**Why this priority**: Link creation is the second pillar of the reference attribute operations and is independently useful, but secondary to chmod in everyday use.

**Independent Test**: Highlight `file.txt`, invoke create-symlink, accept/enter a link name, confirm a symlink to `file.txt` now appears in the pane and resolves to the target.

**Acceptance Scenarios**:

1. **Given** a file is highlighted, **When** the user creates a symbolic link with a given name, **Then** a symlink with that name pointing at the source appears in the listing.
2. **Given** a file is highlighted, **When** the user creates a hard link with a given name, **Then** a new directory entry referring to the same file content is created.
3. **Given** a link name that already exists, **When** the user submits it, **Then** the operation is refused with a clear message and nothing is overwritten.
4. **Given** a hard-link request that the filesystem cannot satisfy (e.g. crossing filesystems, or linking a directory), **When** the user submits it, **Then** the failure is reported without crashing.

---

### User Story 3 - Change ownership (Priority: P3)

A user (typically with the necessary privileges) wants to change a file's owning user and/or group. They select files, invoke "change owner", enter a user and/or group, and ownership changes where permitted.

**Why this priority**: chown is part of the reference attribute set but is the least-used in single-user contexts and usually requires elevated privileges, so it is lowest priority. It must still fail gracefully when not permitted.

**Independent Test**: With sufficient privilege, change a file's group to one the user belongs to and confirm the listing/owner reflects it; without privilege, confirm the attempt reports "permission denied" and changes nothing.

**Acceptance Scenarios**:

1. **Given** the user has permission, **When** they set a new user and/or group on the selection, **Then** ownership changes for each file.
2. **Given** the user lacks permission, **When** they attempt a change, **Then** the operation reports the error and leaves the files unchanged.
3. **Given** an unknown user or group name, **When** submitted, **Then** the operation is refused with a clear message and nothing changes.

---

### Edge Cases

- **The `..` parent row** must be excluded from all attribute operations (consistent with copy/move/delete) — it is never a target.
- **Confirmation**: ownership changes and recursive changes require explicit confirmation before applying; a plain chmod of a single file MAY proceed without a second confirmation (defined in Requirements/Assumptions).
- **Partial failure across a multi-file selection**: if some files succeed and others fail (e.g. mixed ownership), the user is told which failed; successes are not rolled back.
- **chmod/chown on a symlink**: behavior on the link vs. its target must be predictable (defined in Assumptions).
- **Symlink to a non-existent target** (dangling link) is allowed; **hard link to a directory or across filesystems** is rejected by the OS and surfaced as an error.
- **Recursive application** to a directory tree (if offered) must handle large trees without hanging the UI and report per-entry failures.
- **Non-local backends**: on a backend that does not support an attribute operation, the action reports "not supported" rather than failing opaquely.
- **Invalid input**: malformed octal, malformed symbolic spec, blank link name — all rejected with no state change.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST let users change Unix permissions (chmod) of the current selection, entered as **octal** (e.g. `644`, `0755`) or **symbolic** (e.g. `u+x`, `go-w`, `a=r`).
- **FR-002**: The system MUST let users create a **symbolic link** to the focused entry, with a user-supplied link name/location.
- **FR-003**: The system MUST let users create a **hard link** to the focused entry, with a user-supplied link name/location.
- **FR-004**: The system MUST let users change **ownership** (user and/or group) of the current selection.
- **FR-005**: Attribute operations MUST apply to the **current selection** — all tagged files, or the focused entry if none are tagged — excluding the synthetic `..` row.
- **FR-006**: The system MUST provide the underlying file-attribute operations (change-permissions, change-ownership, create-symlink, create-hard-link) for the local filesystem; on a backend that does not support a given operation it MUST report it as unsupported rather than crashing.
- **FR-007**: Ownership changes (and any recursive attribute change) MUST require explicit user confirmation before being applied.
- **FR-008**: After a successful operation the affected pane MUST refresh so the new permissions / ownership / link are visible.
- **FR-009**: Invalid input (malformed octal/symbolic mode, unknown user/group, blank or duplicate link name) MUST be rejected with a clear inline message and MUST NOT change any file.
- **FR-010**: Operational failures (e.g. permission denied, cross-filesystem hard link, link target exists) MUST be reported to the user without crashing; for a multi-file selection, partial failures MUST be surfaced (which items failed) without rolling back the successes.
- **FR-011**: The operations MUST be reachable from the **File menu** and from a **keybinding**, and their dialogs MUST reuse the shared dialog widgets (consistent navigation: edit, confirm, cancel).
- **FR-012**: Cancelling any attribute dialog MUST close it with no change to the files or panes.

### Key Entities *(include if feature involves data)*

- **Permission set (mode)**: the Unix permission bits of a file (owner/group/other × read/write/execute, plus special bits). Expressible as octal or as symbolic deltas.
- **Ownership**: the owning **user** and **group** of a file (by name or id).
- **Link**: a new directory entry referring to a target — a **symbolic link** (a path pointer, may dangle) or a **hard link** (a second name for the same file content, same filesystem only).
- **Selection**: the set of real entries an operation targets (tagged files, else the focused entry; never the `..` row) — the existing selection concept, reused.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can change a file's permissions via octal or symbolic input and see the updated permission column in the listing, in a single dialog interaction.
- **SC-002**: A user can create a symbolic link and a hard link to a file, and both new entries appear in the listing.
- **SC-003**: A single change-permissions action applies to every tagged file in the selection (verified with a multi-file selection).
- **SC-004**: Invalid mode/owner/link input never alters any file and always produces a clear error (0 silent failures, 0 partial mutations from bad input).
- **SC-005**: Permission-denied and other OS-level failures never crash the application and are always reported, including which items failed in a multi-file batch (100%).
- **SC-006**: A user with sufficient privilege can change a file's owning user and/or group and see it reflected; without privilege the attempt is reported and leaves files unchanged.
- **SC-007**: Every attribute operation is reachable from the File menu and a keybinding, and cancelling leaves all state unchanged.

## Assumptions

- **Selection semantics reuse the existing model**: "tagged files, else focused entry, excluding `..`" is the same selection logic used by copy/move/delete; no new selection concept is introduced.
- **chmod input forms**: both octal (3–4 digits) and the common symbolic grammar (`[ugoa][+-=][rwx]`, comma-separated) are accepted; the symbolic form is applied relative to each file's current mode.
- **Symlink vs. hardlink semantics**: symbolic links may point at a non-existent target (dangling allowed); hard links are restricted to the same filesystem and to non-directory targets (OS-enforced; surfaced as errors).
- **Default chmod/chown on a symlink** operates per the platform's default (typically following the link for chmod); operating on the link itself vs. the target is not separately configurable in this feature.
- **Link location**: by default a new link is created in the active pane's current directory with a user-supplied name (prefilled with a sensible default such as the target's name); the user may type a different name/path.
- **Confirmation scope**: a single-file chmod proceeds from its dialog without a second confirmation; ownership changes and any recursive (whole-subtree) application require an explicit confirm (the exact recursion offering is finalized in planning).
- **Scope is the interactive TUI on the local filesystem**: this feature targets the local backend; remote/archive backends are out of scope (they advertise the operation as unsupported until their own features land).
- **No new persisted state**: attribute operations mutate the filesystem directly; nothing is written to config/state files.
- **Keybinding & menu placement** follow the project's single-source-of-truth keymap and the existing File menu; the exact key is finalized in planning.

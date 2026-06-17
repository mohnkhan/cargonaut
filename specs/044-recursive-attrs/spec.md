# Feature Specification: Recursive chmod / chown into Subtrees

**Feature Branch**: `044-recursive-attrs`

**Created**: 2026-06-17

**Status**: Draft

**Input**: User description: "Recursive chmod/chown into directory subtrees. Extend the Feature 043 file-attribute operations so that changing permissions (chmod) or ownership (chown) can optionally be applied recursively to a selected directory and its entire subtree, not just the directory entry itself — the reference orthodox-FM "recurse into subdirectories" option. When the selection includes a directory, the user can opt into recursion; recursion always requires explicit confirmation before applying. The subtree is walked with a bounded traversal (so a huge tree cannot wedge the UI), applying the operation to every entry; symbolic chmod is applied per-file relative to each entry's current mode; per-entry failures (e.g. permission denied deep in the tree) are aggregated and reported without rolling back the successes. Symlinked directories are not traversed into (no following links across the tree). The affected pane refreshes afterward. This implements the deferred recursion capability from Feature 043 (issue #65, tracked as a follow-up)."

## Clarifications

### Session 2026-06-17

- Q: How should the user opt into a recursive chmod/chown (vs the existing shallow `C-x c` / `C-x o`)? → A: **Dedicated recursive chords** — add `C-x C` (recursive chmod) and `C-x O` (recursive chown) plus File-menu entries, parallel to the shallow `C-x c` / `C-x o`. Each reuses the same mode/owner input, then a single confirmation where Cancel = abort. (Both chords are free; case-sensitive like the existing `C-x X`.)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Recursively change permissions of a directory tree (Priority: P1)

A user wants to make an entire project directory readable by their group, or strip world access from a whole tree. They highlight (or tag) a directory, invoke change-permissions, and choose to apply it recursively. After confirming, every file and subdirectory inside the tree receives the change, and the pane reflects the directory's own updated permissions.

**Why this priority**: Recursive chmod is the headline of this follow-up and the most common bulk-permission task. It is the minimal viable slice and is independently demonstrable (change a tree, verify a file deep inside changed).

**Independent Test**: Create a directory with nested subdirectories and files. Recursively chmod it to a mode, confirm, and verify a file several levels deep has the new mode.

**Acceptance Scenarios**:

1. **Given** a directory is selected, **When** the user applies a permission change recursively and confirms, **Then** every entry in the subtree (files and subdirectories) receives the change.
2. **Given** a symbolic mode (e.g. `g+r`) and a recursive apply, **When** it runs, **Then** each entry's change is computed relative to that entry's own current mode.
3. **Given** the user declines the confirmation, **When** the prompt is dismissed, **Then** nothing is changed.
4. **Given** some entries deep in the tree cannot be changed (e.g. permission denied), **When** the operation runs, **Then** the successes are kept, the failures are reported (count and/or which), and the operation does not abort the whole tree.

---

### User Story 2 - Recursively change ownership of a directory tree (Priority: P2)

A user with the necessary privileges wants to reassign ownership of an entire directory tree to a user and/or group. They select a directory, invoke change-owner, choose recursive, confirm, and ownership changes throughout the subtree where permitted.

**Why this priority**: Recursive chown completes the parity with the reference manager's recursive attribute operations, but is less frequently used (and usually needs privilege) than chmod, so it is P2.

**Independent Test**: With sufficient privilege, recursively chgrp a tree to an owned group and verify a nested file's group changed; without privilege, verify the attempt reports failures and changes nothing it cannot.

**Acceptance Scenarios**:

1. **Given** a directory is selected and the user has permission, **When** they apply an ownership change recursively and confirm, **Then** every entry in the subtree gets the new owner/group.
2. **Given** insufficient privilege, **When** a recursive ownership change runs, **Then** the failures are reported and no entry is left partially corrupted.

---

### Edge Cases

- **Confirmation is mandatory**: a recursive change is never applied without an explicit confirmation step (distinct from a non-recursive single-file chmod, which may apply directly).
- **Huge tree**: traversal is bounded by a node cap; if the cap is reached the operation still completes on what it walked and the user is told the result was truncated, rather than hanging the UI.
- **Symlinked directory inside the tree**: the traversal does not descend into it (no following links out of the subtree); the link entry itself is treated as a leaf.
- **Unreadable subdirectory**: if a directory cannot be listed (permission denied), its subtree is skipped and the failure is surfaced; sibling branches still process.
- **Recursion requested on a plain file** (no directory in the selection): behaves as the ordinary non-recursive change (there is nothing to recurse into).
- **The `..` parent row** is never a target (consistent with all other operations).
- **Apply order**: the change must not lock the traversal out of the tree it is still walking (e.g. removing read/execute from a directory before its contents are visited) — entries are collected before restrictive changes take effect.
- **Mixed selection** (a directory plus loose files): recursion applies within the directory; the loose files get the single change.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST provide dedicated recursive operations — `C-x C` (recursive chmod) and `C-x O` (recursive chown), plus File-menu entries — that apply a permission/ownership change to a selected directory's entire subtree. These are distinct from the existing shallow `C-x c` / `C-x o`.
- **FR-002**: A recursive change MUST require an explicit confirmation before any entry is modified; declining the confirmation (Cancel) aborts with no change.
- **FR-003**: A recursive permission change MUST apply to every entry in the subtree (files and subdirectories); a symbolic mode MUST be applied to each entry relative to that entry's own current mode.
- **FR-004**: A recursive ownership change MUST apply the new user and/or group to every entry in the subtree (where permitted).
- **FR-005**: Subtree traversal MUST be bounded so that an arbitrarily large tree cannot wedge the interface; if the bound is reached, the system MUST report that the result was truncated.
- **FR-006**: Traversal MUST NOT descend into symbolic-link directories (it MUST NOT follow links out of the selected subtree).
- **FR-007**: Per-entry failures MUST be aggregated and reported (a count, and/or which entries failed) without rolling back entries that succeeded.
- **FR-008**: The existing non-recursive attribute operations MUST remain available and unchanged (recursion is opt-in).
- **FR-009**: Requesting recursion when the selection contains no directory MUST behave as the ordinary non-recursive change.
- **FR-010**: After a recursive operation the affected pane MUST refresh so the (top-level) result is visible.
- **FR-011**: A recursive change MUST NOT lock itself out of the tree mid-traversal (the set of entries to change is determined before restrictive changes take effect).

### Key Entities *(include if feature involves data)*

- **Subtree**: the set of all entries reachable from a selected directory by walking child directories, excluding entries reached through symbolic links; the target set of a recursive operation.
- **Recursive operation**: a permission or ownership change parameterized by the same inputs as the non-recursive operation (mode spec / owner spec) plus a "recurse" intent, applied to every entry in the subtree.
- **Result summary**: the outcome of a batch — how many entries succeeded, which failed, and whether the walk was truncated by the bound.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can recursively change permissions of a directory and a file several levels deep ends up with the new mode (verified at depth).
- **SC-002**: A user (with privilege) can recursively change ownership of a directory and a nested entry reflects the new owner/group.
- **SC-003**: A recursive change is applied only after explicit confirmation — declining changes nothing (100%).
- **SC-004**: A tree exceeding the traversal bound completes without hanging and reports truncation (no unbounded operation).
- **SC-005**: A symbolic-link directory within the tree is not descended into — its link target outside the subtree is unchanged.
- **SC-006**: When some entries cannot be changed, the operation reports the failures and preserves the successful changes (0 whole-tree aborts from a single failure).

## Assumptions

- **Extends Feature 043**: this builds directly on the shipped file-attribute operations (chmod octal/symbolic, chown by name/id, the selection model, the per-entry-failure status). Non-recursive behavior is unchanged.
- **Opt-in mechanism**: recursion is invoked through dedicated chords `C-x C` (chmod) and `C-x O` (chown) and matching File-menu entries (clarified), parallel to the shallow `C-x c` / `C-x o`; each reuses the existing mode/owner input dialog and then always shows a confirmation (FR-002), where Cancel aborts.
- **Collect-then-apply**: the subtree's entry set is enumerated first and the change applied afterward, so a restrictive mode/owner change cannot prevent the walk from completing (FR-011). Order within apply favors children before their parent directory for restrictive changes.
- **Bounded walk**: traversal reuses the project's existing bounded-walk convention (a node cap, as used by recursive directory size); the exact cap is an implementation detail.
- **No symlink following**: links are treated as leaves; the operation applies to the link entry per the platform default and never traverses through it (FR-006), preventing cycles and tree-escape.
- **Scope is the local filesystem**: recursion targets the local backend only (consistent with Feature 043); remote/archive backends are out of scope.
- **No new persisted state**: the operation mutates the filesystem directly; nothing is written to config/state.

# Feature Specification: Compare Directories + Diff Tagged Files

**Feature Branch**: `049-compare-dirs`

**Created**: 2026-06-18

**Status**: Draft

**Input**: User description: "Compare directories + diff two tagged files: highlight files that differ between the two panels (by name/size/content hash), mark differing entries via the existing selection/tag set, and diff two tagged files via external tool. Closes issue #43."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Compare Two Panels (Priority: P1)

The user has two different directories open in the left and right panels (e.g., a source directory vs. a backup). They trigger "Compare directories" and the app immediately highlights every entry that differs — files present only on one side, or files present on both sides but with different sizes or content. The user can then navigate and act on the highlighted items (copy, delete, inspect).

**Why this priority**: This is the core value of the feature. Without it the diff-tagged-files story has no context. It is also the primary reason issue #43 was raised.

**Independent Test**: Open two directories containing a mix of identical files, size-only-different files, content-different files, and unique files on each side; trigger compare; verify highlighting matches expectations on all four categories.

**Acceptance Scenarios**:

1. **Given** both panels show directories, **When** the user triggers "Compare directories", **Then** every file that is absent on one side is marked/highlighted, every file that is present on both sides but differs by size or content hash is marked/highlighted, and every identical file is left unmarked.
2. **Given** both panels show the same directory (identical contents), **When** the user triggers "Compare directories", **Then** no entries are marked and a status message confirms "all entries identical".
3. **Given** compare has run and highlighted some entries, **When** the user navigates and manually untags an entry, **Then** that entry reverts to the normal untagged state (compare does not re-assert its mark).
4. **Given** one or both panels is empty, **When** the user triggers "Compare directories", **Then** all entries on the non-empty side are marked and the empty side shows no entries.

---

### User Story 2 — Diff Two Tagged Files (Priority: P2)

After a compare (or any other session activity that tagged files), the user has exactly two files tagged across the two panels. They invoke "Diff tagged files" and the app launches an external diff tool with the two tagged files as arguments. The user reviews the diff in the external tool and returns to the app.

**Why this priority**: The compare-highlight alone (US1) covers 80% of the use case. The external diff deepens the workflow for users who need line-level inspection of specific pairs.

**Independent Test**: Tag exactly one file in each panel, invoke "Diff tagged files", observe the external tool opens with the correct two paths.

**Acceptance Scenarios**:

1. **Given** exactly two files are tagged (one per panel), **When** the user invokes "Diff tagged files", **Then** the TUI suspends, the configured external diff tool is launched with both file paths as arguments occupying the full terminal, and the TUI resumes cleanly when the tool exits.
2. **Given** fewer than two files are tagged, **When** the user invokes "Diff tagged files", **Then** the app shows an error message "Diff requires exactly 2 tagged files" and does not launch any external process.
3. **Given** more than two files are tagged, **When** the user invokes "Diff tagged files", **Then** the app shows an error message "Diff requires exactly 2 tagged files" and does not launch any external process.
4. **Given** no external diff tool is configured, **When** the user invokes "Diff tagged files", **Then** the app shows a clear error explaining that no diff tool is configured and how to set one.
5. **Given** the external tool binary is configured but missing from PATH, **When** the user invokes "Diff tagged files", **Then** the app reports the launch failure with the tool name, without crashing.

---

### Edge Cases

- What happens when one panel is not showing a local filesystem directory (e.g., a virtual or empty panel)? Compare should be restricted to local filesystem directories; a clear error is shown if the panel type is unsupported.
- What happens when a file in the listing is a symbolic link? The comparison checks the symlink target's metadata, not the symlink itself.
- What happens when a directory entry is a subdirectory (not a file)? Subdirectories are compared by name-presence only (not recursively) — a subdirectory present on both sides is considered "same" even if their contents differ.
- What happens when a file is unreadable (permissions error) during hash computation? The file is marked as "differing" with a status indicator distinguishing it from a normal content diff.
- What happens when both panels show the same path? The app detects this and shows a warning "Both panels point to the same directory — compare would mark nothing."

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST provide a "Compare directories" action (bound to a keymap entry) that, when invoked with two local-filesystem panels open, compares the visible file listings of both panels and marks all differing entries.
- **FR-002**: Two entries are considered "differing" when: (a) the name exists on only one side, OR (b) the name exists on both sides and the file sizes differ, OR (c) sizes match but a fast content hash (e.g. first+last block or full file up to a threshold) differs.
- **FR-003**: Marked entries MUST use the existing selection/tag visual style so that the highlighting is consistent with other tagged entries (no new colour or indicator system introduced).
- **FR-004**: Compare MUST only add tags — it MUST NOT clear or overwrite any existing tags. After compare, the user can remove compare-applied marks using the existing "untag all" action; no compare-specific clear command is needed.
- **FR-005**: The app MUST provide a "Diff tagged files" action (bound to a keymap entry) that splits the configured diff tool string on whitespace into argv and appends the two tagged file paths as the final two positional arguments, invoking the tool without a shell.
- **FR-006**: If no external diff tool is configured, "Diff tagged files" MUST display a descriptive error message indicating the missing configuration.
- **FR-007**: If the count of tagged files is not exactly two, "Diff tagged files" MUST display an error message without launching any external process.
- **FR-008**: When "Diff tagged files" is invoked, the TUI MUST suspend (raw-mode released, screen restored), the diff tool MUST be given full terminal control, and the TUI MUST resume cleanly (screen repainted, raw-mode re-engaged) after the tool exits. Both terminal-based tools (vimdiff, diff) and GUI tools (meld) MUST be supported through this mechanism.
- **FR-009**: The compare action MUST complete within a perceptible time for directories up to 1,000 entries; for larger directories a progress indicator MUST be shown.
- **FR-010**: The compare action MUST be restricted to entries visible in the active listing (not recursive); subdirectories are compared by name-presence only.
- **FR-011**: The keymap for both actions MUST be defined in the single keymap source of truth (`design/contracts/keymap.toml`) and MUST not conflict with existing bindings.

### Key Entities

- **Panel listing**: The set of file-system entries currently displayed in one panel — each entry has a name, size, type (file/dir/link), and an optional tag bit.
- **Compare result**: A transient, per-entry classification: `left-only`, `right-only`, `size-differ`, `hash-differ`, `identical`, `unreadable`. Stored ephemerally; not persisted.
- **Diff tool config**: The user-configured external command string stored in the app's configuration file; defaults to unset. The string is split on whitespace into argv; the two file paths are appended as the final two arguments (e.g., config `diff -u` → invoked as `diff -u <path1> <path2>`). No shell expansion is performed.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: "Compare directories" completes and visually marks all differing entries within 2 seconds for a panel containing up to 1,000 files on a local filesystem.
- **SC-002**: Zero false positives — identical files (same name + size + content) are never marked after a compare run.
- **SC-003**: Zero false negatives — every differing file (name-only, size-different, or content-different) is marked after a compare run.
- **SC-004**: "Diff tagged files" suspends the TUI and hands control to the external tool within 500 ms of the keypress for two locally-accessible files.
- **SC-005**: After the external diff tool exits, the TUI repaints fully and is interactive within 200 ms.
- **SC-006**: Error messages for misconfigured or missing diff tool are shown within one frame of the keypress.

## Clarifications

### Session 2026-06-18

- Q: Should "Diff tagged files" support terminal-based tools (vimdiff, diff) by suspending the TUI, or is support limited to GUI/detached tools only? → A: TUI suspends and hands full terminal control to the diff tool; resumes cleanly on exit. Both terminal-based and GUI tools are supported through this mechanism.
- Q: When "Compare directories" runs, what should happen to entries that are already manually tagged? → A: Compare is additive — it only adds tags to differing entries and never clears or overwrites existing manual tags on any entry.
- Q: Should the diff tool config accept a full command string with flags (e.g., `diff -u`) or a bare binary name only? → A: Full argv string — split on whitespace, two file paths appended as last two args; no shell invoked.

## Assumptions

- Both panels must be showing local filesystem directories; compare is not defined for virtual panels or non-directory listings in this feature.
- Content hashing uses a fast strategy: for files ≤4 MiB, full CRC32 hash; for larger files, CRC32 of the first 512 KiB (head only). This balances accuracy against interactive latency and can be refined later.
- The diff tool config is a full argv string (e.g., `diff -u`, `vimdiff -O2`). It is split on whitespace into argv; the two file paths are appended as the final two positional arguments. No shell is invoked — this avoids shell-injection risk and handles paths with spaces correctly via direct exec.
- Subdirectory comparison is name-presence only — recursive compare is a separate, larger feature (deferred, see issue #43 notes).
- Compare is additive: it only sets tags on differing entries. It never clears or overwrites existing tags on any entry. Users wishing to compare from a clean state can "untag all" before running compare.
- Symbolic links: the stat (not lstat) is used — target size and type are compared, not the link itself.
- The diff tool is given full terminal control via TUI suspend/resume. Both terminal-based tools (vimdiff, diff) and GUI tools (meld) are supported through this same mechanism — the TUI suspends, yields the terminal, and resumes after the tool exits regardless of tool type.

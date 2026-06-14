# Feature Specification: Quick-CD Popup with Tab-Completion

**Feature Branch**: `038-quick-cd-popup`

**Created**: 2026-06-15

**Status**: Draft

**Input**: User description: "Full quick-cd popup with tab-completion (FR-012, issue #31, follow-up to Feature 028). Today Alt-c only shows a status-bar placeholder plus a keymap binding — the actual feature is not implemented. Build the full inline quick-cd prompt: Alt-c opens an inline text-input dialog; Tab completes against the focused pane's VFS plus its recent-directory history; Enter navigates via the existing :cd dispatch path; Escape cancels. Requires a reusable text-input dialog widget (shared dialog widgets per Constitution §III) that also unblocks #32 and #33. Include an injected-input test per the original T1.25."

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Jump to a directory by typing its path (Priority: P1)

A user navigating files wants to move the active pane to a directory whose path
they already know, without walking the tree one level at a time. They press the
quick-cd shortcut (Alt-c), an inline prompt appears, they type the target
directory path, press Enter, and the active pane is now showing that directory.

**Why this priority**: This is the core value of the feature and the minimum
that makes the long-advertised Alt-c binding actually do something. Typing a
known path is the single most common reason to reach for quick-cd, and it
delivers value even before completion exists. It is the MVP.

**Independent Test**: Open the prompt, type a valid absolute directory path,
press Enter; assert the active pane's current directory changed to that path and
the prompt closed. Fully testable with injected key input against the
application state, with no dependency on tab-completion.

**Acceptance Scenarios**:

1. **Given** the active pane is showing directory A, **When** the user presses
   Alt-c, types the path of an existing directory B, and presses Enter, **Then**
   the active pane navigates to B, the prompt closes, and the inactive pane is
   unchanged.
2. **Given** the quick-cd prompt is open, **When** the user types characters and
   uses Backspace, **Then** the prompt's visible text reflects the edits
   in order.
3. **Given** the user typed a path and pressed Enter and navigation succeeded,
   **Then** the directory they left is recorded in the active pane's recent
   directory history exactly as a normal directory change would record it.

---

### User Story 2 - Complete a partially typed path with Tab (Priority: P2)

A user begins typing a directory path but does not want to type the whole thing.
They press Tab and the prompt completes the path against the directories that
actually exist under the focused pane's filesystem, plus the directories they
have recently visited in that pane. Repeated Tab presses cycle through the
matching candidates.

**Why this priority**: Completion is what makes quick-cd fast rather than just
functional, and it is the headline ask in issue #31 (FR-012). It builds directly
on US1 and is independently demonstrable, but US1 must exist first because
completion only has value once typing-and-navigating works.

**Independent Test**: Open the prompt, type a prefix that uniquely identifies one
child directory, press Tab; assert the prompt's text is completed to that
directory. Type a prefix matching several directories, press Tab repeatedly;
assert each press advances to the next candidate. Testable with injected input.

**Acceptance Scenarios**:

1. **Given** the prompt contains a partial path whose last segment uniquely
   matches one existing child directory, **When** the user presses Tab, **Then**
   the prompt's text is completed to that directory's path.
2. **Given** the partial path's last segment matches several existing child
   directories, **When** the user presses Tab repeatedly, **Then** each press
   advances the prompt text to the next matching candidate, wrapping around after
   the last.
3. **Given** the partial path matches one or more directories the user recently
   visited in the active pane, **When** the user presses Tab, **Then** those
   recently-visited directories appear among the completion candidates.
4. **Given** the partial path matches no existing directory and no recent
   directory, **When** the user presses Tab, **Then** the prompt text is left
   unchanged and the user receives a non-disruptive indication that there is
   nothing to complete.
5. **Given** the prompt's last segment matches a file rather than a directory,
   **When** the user presses Tab, **Then** the file is not offered as a
   completion candidate.

---

### User Story 3 - Cancel or recover from a bad path (Priority: P3)

A user who opened the prompt by mistake, or typed a path that does not exist,
must be able to back out cleanly. Pressing Escape closes the prompt and leaves
both panes exactly as they were. Pressing Enter on a path that is not a reachable
directory does not silently move them somewhere wrong — it tells them the path is
invalid and keeps them in control.

**Why this priority**: Safety and reversibility. Without it the feature is
hostile to mistakes, but the happy paths (US1, US2) deliver value on their own,
so this is lowest priority while still required for a shippable feature.

**Independent Test**: Open the prompt, type anything, press Escape; assert the
prompt closed and neither pane's directory nor history changed. Separately, type
a non-existent path, press Enter; assert no navigation occurred and the user is
informed.

**Acceptance Scenarios**:

1. **Given** the quick-cd prompt is open with any text typed, **When** the user
   presses Escape, **Then** the prompt closes, no navigation occurs, and neither
   pane's current directory or history is modified.
2. **Given** the prompt contains a path that does not exist or is not a
   directory, **When** the user presses Enter, **Then** no navigation occurs and
   the user is shown an error message; the user is not left in a broken state.
3. **Given** the prompt is empty, **When** the user presses Enter, **Then**
   nothing is navigated and the prompt either stays open or closes without error
   (no spurious navigation).

---

### Edge Cases

- **Path that disappeared**: A recent-directory candidate that has since been
  deleted is selected/entered → treated as an invalid path (US3 scenario 2),
  not a crash.
- **Relative vs absolute input**: A relative path is resolved against the active
  pane's current directory; an absolute path is used as-is.
- **Trailing separator**: A path typed with a trailing separator (e.g.
  `foo/bar/`) is treated the same as without it.
- **Completion at an intermediate segment**: Tab completes the final, partial
  segment of the typed path; earlier segments are taken as the directory to list
  candidates from.
- **No candidates**: Tab with nothing to complete is a no-op with gentle
  feedback (US2 scenario 4).
- **Prompt already open**: The quick-cd shortcut while the prompt is open does
  not stack a second prompt.
- **Another modal open**: Quick-cd cannot be opened on top of a different modal;
  only one modal is active at a time.
- **Symlink to a directory**: Followed as a directory target (consistent with
  normal navigation behavior).
- **Permission denied on the target**: Reported as an error (US3 scenario 2);
  no navigation.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST open an inline quick-cd prompt in the active pane's
  context when the user invokes the quick-cd action (Alt-c), replacing the
  current placeholder behavior.
- **FR-002**: Users MUST be able to type, edit (including delete the last
  character), and clear a directory path within the prompt while it is open.
- **FR-003**: While the prompt is open, it MUST capture keyboard input so that
  typing, completion, accept, and cancel keys act on the prompt rather than
  triggering other application shortcuts.
- **FR-004**: On accept (Enter) with a path that resolves to an existing,
  reachable directory, the system MUST navigate the active pane to that directory
  through the same code path used by ordinary directory changes, and MUST close
  the prompt.
- **FR-005**: A successful quick-cd navigation MUST update the active pane's
  recent-directory history identically to any other directory change.
- **FR-006**: On accept with a path that does not resolve to a reachable
  directory (non-existent, not a directory, or permission denied), the system
  MUST NOT navigate, MUST inform the user of the error, and MUST NOT leave the
  application in a broken or inconsistent state.
- **FR-007**: Pressing Tab MUST complete the final segment of the typed path
  against candidate directories; when the segment uniquely matches one candidate
  the text MUST be completed to it, and when it matches several, repeated Tab
  presses MUST cycle through the candidates in a stable order, wrapping after the
  last.
- **FR-008**: Completion candidates MUST be drawn from (a) the directories that
  exist under the directory implied by the typed path within the active pane's
  filesystem, and (b) the active pane's recent-directory history; non-directory
  entries MUST be excluded.
- **FR-009**: When Tab is pressed and there are no matching candidates, the
  system MUST leave the typed text unchanged and give a non-disruptive
  indication that there is nothing to complete.
- **FR-010**: Pressing Escape MUST close the prompt without navigating and
  without modifying either pane's current directory or history.
- **FR-011**: The system MUST allow only one modal prompt at a time; invoking
  quick-cd while the prompt (or another modal) is open MUST NOT stack a second
  prompt.
- **FR-012**: A relative path entered in the prompt MUST be resolved against the
  active pane's current directory; an absolute path MUST be used as entered.
- **FR-013**: The quick-cd prompt MUST operate only on the active pane; the
  inactive pane MUST be unaffected by opening, completing in, accepting, or
  cancelling the prompt.

### Key Entities *(include if feature involves data)*

- **Quick-CD Prompt**: The transient modal state representing an open quick-cd
  session. Holds the user's current text, the cursor/edit position, and the
  current completion cycle position. Exists only while the prompt is open.
- **Completion Candidate**: A directory eligible to complete the current input.
  Sourced from the active pane's filesystem listing of the relevant directory and
  from the active pane's recent-directory history; always a directory, never a
  file.
- **Recent-Directory History**: The existing per-pane record of previously
  visited directories (delivered by T1.24), consumed here as one source of
  completion candidates.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can change the active pane to any reachable directory using
  only the keyboard via the quick-cd prompt — open, type the full path, accept —
  in a single uninterrupted prompt session.
- **SC-002**: When a typed prefix uniquely identifies a directory, a single Tab
  press completes the path to that directory (one keystroke, no retyping).
- **SC-003**: 100% of offered completion candidates are valid directories — no
  files and no non-existent paths are ever offered or navigated to.
- **SC-004**: Cancelling the prompt with Escape leaves both panes' current
  directory and history byte-for-byte identical to their state before the prompt
  opened (zero side effects on cancel).
- **SC-005**: Entering a non-existent or non-directory path and accepting it
  never changes the active pane's directory; the user is informed in 100% of
  such cases.
- **SC-006**: The end-to-end behavior (open → type → complete → accept, and
  open → cancel) is covered by an automated injected-input test that passes in
  CI.

## Assumptions

- The application already exposes a single, canonical directory-change operation
  for a pane (used by descend/ascend/sync today); quick-cd routes accepts through
  it rather than introducing a parallel navigation path.
- Per-pane recent-directory history (T1.24) is already populated and readable;
  this feature consumes it and does not redefine how history is recorded, beyond
  the normal recording that a successful navigation performs.
- The quick-cd prompt is built on a reusable modal text-input building block
  (shared dialog widgets per Constitution §III). The same building block is
  intended to serve the deferred tasks/jobs panel (#32) and panel filter prompt
  (#33); making it reusable is in scope, but implementing #32 and #33 themselves
  is out of scope for this feature.
- Completion operates against the active pane's filesystem backend (the local
  filesystem in the current phase). Remote/archive backends are out of scope;
  the design should not preclude them but need not be exercised here.
- The active pane is always well-defined (exactly one of the two panes is
  focused at any time).

## Out of Scope

- Implementing the tasks/jobs panel (#32) or the panel filter prompt (#33) — only
  the shared input building block they will reuse is delivered here.
- Fuzzy / substring matching of completion candidates beyond prefix matching of
  the final path segment (may be a future enhancement).
- Persisting quick-cd input history across application restarts.
- Bookmark/alias expansion (e.g. named shortcuts) within the prompt.
- Completion against remote or archive filesystem backends.

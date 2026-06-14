# Feature Specification: Panel Filter Prompt Dialog

**Feature Branch**: `033-panel-filter-prompt`

**Created**: 2026-06-15

**Status**: Draft

**Input**: User description: "Panel filter prompt dialog (GitHub issue #33, Feature 022 / FR-013 follow-up). Today the `TogglePanelFilter` command (Alt-!) is clear-only: it can drop an active filter but cannot prompt the user for a new pattern. Implement a prompt dialog that lets the user type a filter pattern for the focused pane."

## Why This Feature

FR-013 shipped in Feature 022 as **clear-only**: the filter command can remove an
active filter but offers no way to *set* one — the prompt was deferred until a shared
text-input dialog existed. That widget now exists (the caller-driven text-input dialog
delivered by Feature 038 / issue #31). This feature closes the gap so the user can
actually narrow a pane to the entries they care about.

## Clarifications

### Session 2026-06-15

- Q: How should a typed filter pattern be matched against entry names? → A: Glob +
  auto-substring — compile as a glob; if the pattern contains no glob metacharacters
  (`* ? [ ] { }`), auto-wrap it as `*pattern*` so a bare word matches any name containing
  it. Patterns that fail to compile surface an inline error.
- Q: Should filter matching be case-sensitive? → A: Case-insensitive.
- Q: What happens to an active filter when the user navigates into a different directory?
  → A: The filter persists until explicitly cleared.

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Set a filter on the focused pane (Priority: P1)

A user viewing a directory with many entries wants to see only the entries whose names
match a pattern (for example, only the `.rs` source files). They invoke the filter
command, a prompt appears, they type a pattern, press Enter, and the pane immediately
shows only the matching entries with the cursor at the top of the narrowed list.

**Why this priority**: This is the entire point of the feature — without it the filter
command remains clear-only and FR-013 stays half-delivered. It is the MVP.

**Independent Test**: Open the app, focus a pane with mixed entries, invoke the filter
command, type a pattern, press Enter, and confirm only matching entries are visible and
the cursor is at the top. Fully testable on its own.

**Acceptance Scenarios**:

1. **Given** the focused pane shows a directory with both matching and non-matching
   entries and no active filter, **When** the user invokes the filter command, types a
   valid pattern, and presses Enter, **Then** the pane shows only entries whose names
   match the pattern and the cursor is reset to the first visible entry.
2. **Given** the focused pane already has an active filter, **When** the user invokes the
   filter command, **Then** the prompt opens prefilled with the current pattern so it can
   be edited rather than retyped.
3. **Given** a filter is being set on the focused pane, **When** the filter is applied,
   **Then** the other (non-focused) pane's visible entries are unchanged.

---

### User Story 2 - Clear the filter via the prompt (Priority: P1)

A user with an active filter wants to see the full listing again. They invoke the filter
command and submit an empty pattern (clearing the prefilled text first), and the pane
restores its full, unfiltered listing.

**Why this priority**: Preserving the existing clear behavior is a hard requirement —
the command was clear-capable before this feature and must remain so. Clearing is the
natural inverse of setting and shares the same entry point, so it ships in the MVP.

**Independent Test**: With an active filter, invoke the command, clear the input to empty,
press Enter, and confirm the full listing returns.

**Acceptance Scenarios**:

1. **Given** the focused pane has an active filter, **When** the user invokes the filter
   command, clears the input to empty, and presses Enter, **Then** the filter is removed
   and all entries (subject to the hidden-files toggle) are visible again.
2. **Given** the focused pane has no active filter, **When** the user invokes the filter
   command and submits an empty pattern, **Then** the pane remains unfiltered (no error,
   a no-op clear).

---

### User Story 3 - Recover from an invalid pattern (Priority: P2)

A user types a pattern the system cannot compile (a malformed glob). Instead of losing
their work or corrupting the pane, the prompt stays open showing an inline error so they
can correct the pattern and try again, or cancel out entirely.

**Why this priority**: Robustness against bad input is important for a polished
experience, but the core value (set/clear) is delivered by P1. This guards the edges.

**Independent Test**: Invoke the command, type a pattern known to be invalid, press Enter,
and confirm the prompt remains open with an error message and the pane state is unchanged.

**Acceptance Scenarios**:

1. **Given** the filter prompt is open, **When** the user enters a pattern that fails to
   compile and presses Enter, **Then** the prompt stays open, an inline error is shown,
   and the focused pane's filter and listing are unchanged.
2. **Given** the prompt is showing an inline error, **When** the user edits the input,
   **Then** the error clears so the corrected pattern can be submitted.
3. **Given** the filter prompt is open (with or without an error), **When** the user
   cancels (Esc), **Then** the prompt closes and the focused pane's filter is left exactly
   as it was before the prompt opened.

---

### Edge Cases

- **Cancel after editing**: editing the prefilled pattern and then cancelling must not
  alter the pane — cancel always reverts to the pre-prompt filter state.
- **Filter that matches nothing**: a valid pattern matching zero entries yields an empty
  visible list (cursor clamped); this is a successful filter, not an error.
- **Whitespace-only input**: treated the same as empty input (clears the filter).
- **Filter interaction with hidden files**: filtering composes with the existing
  hidden-files toggle — both constraints apply to the visible set.
- **Navigating while filtered**: see Assumptions for the chosen behavior on directory
  change.
- **Re-opening the prompt on a pane whose filter was cleared**: opens empty.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The filter command MUST open a modal text-input prompt for the focused pane
  instead of immediately clearing the filter.
- **FR-002**: The prompt MUST be prefilled with the focused pane's current filter pattern
  when one is active, and empty when no filter is active.
- **FR-003**: On submit with a non-empty, valid pattern, the system MUST apply the pattern
  as the focused pane's filter so that only entries whose names match remain visible.
- **FR-003a**: A pattern that contains glob metacharacters (`* ? [ ] { }`) MUST be matched
  as a glob against the full entry name. A pattern with no glob metacharacters MUST be
  matched as a substring (equivalent to wrapping it as `*pattern*`).
- **FR-003b**: Pattern matching MUST be case-insensitive (e.g. `*.RS` matches `lib.rs`).
- **FR-003c**: An applied filter MUST persist across directory navigation in that pane
  until it is explicitly cleared (it is not auto-dropped when entering a new directory).
- **FR-004**: On applying a filter, the focused pane's cursor MUST be reset to the first
  visible entry of the narrowed list.
- **FR-005**: On submit with an empty (or whitespace-only) pattern, the system MUST clear
  the focused pane's filter, restoring the full listing (preserving prior clear-on-empty
  behavior).
- **FR-006**: On submit with a pattern that fails to compile, the system MUST keep the
  prompt open, display an inline error, and leave the focused pane's filter and listing
  unchanged.
- **FR-007**: Editing the prompt input after an error MUST clear the displayed error.
- **FR-008**: Cancelling the prompt MUST close it and leave the focused pane's filter
  exactly as it was before the prompt opened.
- **FR-009**: Setting or clearing the filter MUST affect only the focused pane; the other
  pane's filter and visible entries MUST be unchanged.
- **FR-010**: The feature MUST reuse the existing shared text-input dialog widget rather
  than introducing a new prompt widget.

### Key Entities *(include if feature involves data)*

- **Pane filter**: the per-pane pattern that constrains which entries are visible. It is
  either absent (no filtering) or present (a compiled matcher applied to entry names).
- **Filter prompt**: a transient modal capturing the user's pattern text, surfacing an
  inline error on invalid input, dismissed on submit or cancel.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can narrow a populated pane to a chosen subset of entries in a single
  prompt interaction (open → type → Enter).
- **SC-002**: A user can restore the full listing of a filtered pane in a single prompt
  interaction (open → empty → Enter).
- **SC-003**: 100% of patterns that fail to compile leave the pane unchanged and surface
  an inline error rather than closing the prompt or corrupting the view.
- **SC-004**: Filtering or clearing one pane never changes the other pane's visible
  entries.
- **SC-005**: Automated tests cover the set, clear, and invalid-pattern paths.

## Assumptions

- **Pattern semantics** are resolved (see Clarifications): glob with auto-substring
  fallback for metacharacter-free patterns, case-insensitive, matched against the entry
  name only (not the full path). Glob compilation uses the existing globset dependency.
- **Filter persistence across navigation** is resolved (see Clarifications): the per-pane
  filter persists until explicitly cleared.
- The shared caller-driven text-input dialog from Feature 038 is available and suitable
  for reuse (completions are optional and may be omitted for this prompt).
- The filter command keybinding (Alt-!) and its routing already exist; only the behavior
  behind it changes.
- Scope is limited to the focused pane; multi-pane or saved/named filters are out of
  scope.

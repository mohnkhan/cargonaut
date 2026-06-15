# Feature Specification: Panel `..` Parent Entry as First Row

**Feature Branch**: `040-parent-row`

**Created**: 2026-06-15

**Status**: Clarified

**Input**: User description: "Panel `..` parent entry as first row (issue #37, Feature 031 follow-up, FR-020). Every non-root directory listing shows a synthetic `..` row as the first row; activating it (Enter / double-click) ascends to the parent directory. At a filesystem root (no parent) the `..` row is not shown and ascent above root is impossible. The `..` row is not a real entry: it cannot be selected/tagged, is excluded from copy/move/delete and select-by-pattern/invert, and always appears regardless of the hidden-file toggle or active name filter. The challenge is the index model: cursor and selection are index-based into the listing, so the design introduces a synthetic-row-aware cursor where cursor row 0 maps to ascend while real-entry indices used by the selection set stay stable."

## Clarifications

### Session 2026-06-15

- Q: When a non-root directory is displayed (after descending or ascending into
  it), where should the cursor land by default — on the new `..` row or on the
  first real entry? → A: **First real entry.** The cursor starts on the first
  real entry; the `..` row sits above it and is reached by pressing Up. This
  preserves Cargonaut's current "cursor on first entry" behavior and muscle
  memory, and means Enter never accidentally ascends. (FR-014)

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Go up a directory by activating the `..` row (Priority: P1)

A user browsing a subdirectory wants to step up to its parent. They see a `..`
row at the very top of the listing, move to it (or click it), and activate it —
the pane navigates to the parent directory, exactly as the existing "ascend"
action would. This gives the long-expected clickable/visible affordance that
orthodox file managers have always had, alongside the existing ascend key.

**Why this priority**: This is the feature's core value and the headline of
FR-020 — the `..` row that ascends on activation. It is the minimum that makes
the affordance real and is independently demonstrable.

**Independent Test**: In a non-root directory, assert a `..` row is the first
row; move the cursor to it and press Enter (and separately, double-click it);
assert the pane navigated to the parent directory in both cases.

**Acceptance Scenarios**:

1. **Given** the active pane shows a non-root directory, **When** the listing is
   displayed, **Then** a `..` row appears as the first row, above all real
   entries.
2. **Given** the cursor is on the `..` row, **When** the user activates it
   (Enter), **Then** the active pane navigates to the parent directory, via the
   same path as the existing ascend action.
3. **Given** the active pane shows a non-root directory, **When** the user
   double-clicks the `..` row, **Then** the active pane navigates to the parent
   directory.
4. **Given** the cursor is on the first real entry, **When** the user moves the
   cursor up, **Then** it lands on the `..` row; **When** they move up again,
   **Then** the cursor stays on `..` (nothing is above it).

---

### User Story 2 - The `..` row can never be selected or operated on (Priority: P2)

A user tagging files for a copy/move/delete must never accidentally include the
parent directory. The `..` row cannot be tagged; bulk selection operations skip
it; and it can never end up in the set of items a copy/move/delete acts on.

**Why this priority**: Safety. A `..` that could be tagged and deleted/copied
would be dangerous and surprising. The happy-path ascent (US1) delivers value on
its own, so this is second, but it is required for a shippable feature.

**Independent Test**: Focus the `..` row and toggle selection; assert nothing is
selected. Select-all / invert / select-by-pattern in a non-root directory;
assert the `..` row is never part of the resulting selection and that a
subsequent copy/move/delete set never contains `..`.

**Acceptance Scenarios**:

1. **Given** the cursor is on the `..` row, **When** the user toggles selection,
   **Then** nothing is added to the selection (no-op).
2. **Given** a non-root directory with several entries, **When** the user inverts
   the selection or selects by a pattern that would textually match `..`,
   **Then** the `..` row is not included in the selection.
3. **Given** any selection state in a non-root directory, **When** the user
   triggers copy, move, or delete, **Then** the set of operated-on items never
   contains the parent (`..`).

---

### User Story 3 - The `..` row is shown consistently, and never above a root (Priority: P3)

The `..` affordance must be predictable: it is always there in a non-root
directory — even when a name filter is active or hidden files are toggled — and
it is absent at a filesystem root, where there is nothing to ascend to.

**Why this priority**: Consistency and correctness at the boundaries. The core
ascent (US1) works without these guarantees, but without them the affordance
flickers in/out confusingly or offers an impossible ascent at the root.

**Independent Test**: Apply a name filter that matches no entries in a non-root
directory; assert the `..` row is still shown. Toggle hidden files; assert the
`..` row is unaffected. Navigate to a filesystem root; assert no `..` row is
shown and activating ascent does nothing harmful.

**Acceptance Scenarios**:

1. **Given** a non-root directory with an active name filter (even one matching
   zero real entries), **When** the listing is displayed, **Then** the `..` row
   is still present as the first row.
2. **Given** a non-root directory, **When** the user toggles hidden-file
   visibility, **Then** the `..` row remains present and unaffected.
3. **Given** the active pane is at a filesystem root, **When** the listing is
   displayed, **Then** no `..` row is shown, and any ascent attempt leaves the
   pane unchanged (cannot go above the root).

---

### Edge Cases

- **Empty non-root directory**: A directory with no real entries still shows the
  `..` row as its only row, and activating it ascends.
- **Filter hides everything**: With a filter that matches no real entries, the
  `..` row is still shown (it is not subject to filtering); the cursor rests on
  it.
- **Cursor clamping**: Cursor movement keeps the cursor within the rows that
  exist (the `..` row plus visible real entries); it never points past the end or
  before `..`.
- **Selection survives a listing that gains/loses the `..` row**: Moving between
  a root and a non-root directory (so the `..` row appears or disappears) must not
  cause the selection set to refer to the wrong real entries.
- **Pattern that matches `..` textually**: A select-by-pattern whose glob would
  match the literal string `..` still must not select the parent row.
- **Pane focus**: The `..` affordance operates on the pane it belongs to;
  activating it in one pane does not affect the other.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: Every non-root directory listing MUST display a `..` row as the
  first row, above all real entries.
- **FR-002**: At a filesystem root (a directory with no parent), the `..` row
  MUST NOT be displayed.
- **FR-003**: Activating the focused `..` row (the same activation used to open a
  directory — Enter) MUST ascend the pane to its parent directory, using the same
  navigation path as the existing ascend action (so history and side effects are
  identical).
- **FR-004**: Double-clicking the `..` row MUST ascend the pane to its parent
  directory.
- **FR-005**: The cursor MUST be able to rest on the `..` row; moving up from the
  first real entry lands on `..`, and moving up while on `..` is a no-op (nothing
  is above it). Cursor movement MUST stay within the existing rows at all times.
- **FR-006**: The `..` row MUST NOT be selectable/taggable; toggling selection
  while it is focused MUST be a no-op.
- **FR-007**: Bulk selection operations (invert selection, select-by-pattern,
  unselect-by-pattern) MUST NOT include the `..` row, even if a pattern would
  textually match `..`.
- **FR-008**: Copy, move, and delete MUST never operate on the parent (`..`); the
  set of items these operations act on MUST never contain it.
- **FR-009**: The `..` row MUST be present in a non-root listing regardless of the
  hidden-file toggle or the active name filter; it is not subject to filtering.
- **FR-010**: Introducing the `..` row MUST NOT change which real entries the
  selection set refers to; real-entry identity/selection MUST remain stable
  whether or not the `..` row is present.
- **FR-011**: Keyboard activation, mouse activation, and rendering MUST treat the
  `..` row consistently through a single model (no divergence where, e.g., the
  keyboard and mouse disagree about which row is `..`).
- **FR-012**: The `..` row MUST be visually identifiable as the parent affordance
  (labeled `..`) and rendered as the first row of the pane.
- **FR-013**: Any user-facing count of directory entries MUST NOT count the `..`
  row as a real entry.
- **FR-014**: When a non-root directory is displayed (whether reached by
  descending or ascending), the cursor MUST default to the first real entry, not
  the `..` row; the `..` row is reached by moving up from the first real entry.
  When a non-root directory has no real entries, the cursor rests on the `..` row.

### Key Entities *(include if feature involves data)*

- **Parent Row (`..`)**: A synthetic, non-selectable first row present in every
  non-root listing. It represents "ascend to parent"; it is not a filesystem
  entry, carries no metadata to operate on, and is never part of a selection or a
  copy/move/delete set.
- **Listing Cursor**: The pane's current highlighted row. Its addressable range
  is the `..` row (when present) plus the visible real entries. Row 0 is the `..`
  row in a non-root directory.
- **Selection Set**: The set of tagged real entries. It refers only to real
  entries and is unaffected by the presence or absence of the `..` row.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: In any non-root directory a user can ascend to the parent using
  only the `..` row — by keyboard (focus + activate) and by double-click — without
  using the dedicated ascend key or the menu.
- **SC-002**: At a filesystem root, no `..` row is ever shown and no action can
  navigate above the root.
- **SC-003**: The parent (`..`) is never selected, never tagged, and never
  appears in a copy/move/delete set — 100% of attempts to do so are no-ops.
- **SC-004**: The `..` row is present in 100% of non-root listings regardless of
  filter or hidden-file state.
- **SC-005**: Adding the `..` row introduces zero regressions in which real
  entries get selected/operated on (the same real entry is selected before and
  after the change for the same user actions).
- **SC-006**: The behavior (presence, ascent on activate by key and mouse,
  non-selectability, root suppression) is covered by automated tests that pass in
  CI.

## Assumptions

- The application already exposes a single canonical ascent operation (used by the
  ascend key, the menu, and mouse today); the `..` row routes activation through
  it rather than introducing a parallel path.
- The application already detects whether a directory has a parent (used today to
  block ascent above a root); the `..` row's presence is driven by that same
  detection.
- Cursor position is currently expressed relative to the visible rows; the design
  extends that addressable range to include the leading `..` row when present,
  while the selection set continues to refer to real entries only.
- **Default cursor position on entering a directory**: resolved in
  Clarifications — the cursor starts on the first real entry (not on the `..`
  row), preserving today's behavior; see FR-014.
- The current phase targets the local filesystem; the affordance is
  backend-agnostic and not specific to any backend.

## Out of Scope

- Landing the cursor on the directory you just came from after ascending (a
  separate navigation nicety).
- A `..` entry in any non-pane listing (e.g. completion popups or dialogs).
- Showing `.` (current directory) or other synthetic rows beyond `..`.
- Changing the existing ascend key, menu item, or their behavior (the `..` row is
  an additional affordance over the same operation).
- Drag-and-drop onto the `..` row (e.g., "move into parent" by dragging).

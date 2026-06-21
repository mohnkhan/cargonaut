# Feature Specification: Click-on-dropdown-item support for the pull-down menu bar

**Feature Branch**: `065-menu-dropdown-mouse-click`

**Created**: 2026-06-22

**Status**: Draft

**Input**: User description: "Click-on-dropdown-item support for the pull-down menu bar. Currently clicking a menu title opens its dropdown, but items inside the open dropdown can only be selected with the keyboard (arrows/hjkl + Enter). Add mouse support so that: clicking a dropdown item invokes its command; hovering/moving the mouse over a dropdown item highlights it (updates selection); clicking outside the open dropdown closes the menu; clicking a different menu title switches to that menu. This mirrors Norton Commander / Midnight Commander mouse behavior. Must respect the existing mouse-capture toggle (Alt-m / --no-mouse / ui.mouse config)."

## Clarifications

### Session 2026-06-22

- Q: When a menu is open and the user clicks a file panel (outside the dropdown), what should
  that single click do? → A: Close + act (pass-through) — the click closes the menu AND
  performs the normal panel action (focus pane + move cursor; double-click descends).
- Q: Should hover-to-highlight (US3) ship in this feature or be deferred? → A: Include hover
  now (US3 is in scope, degrading gracefully where the terminal sends no motion events).

## User Scenarios & Testing *(mandatory)*

### User Story 1 - Click a dropdown item to run it (Priority: P1)

A user opens a pull-down menu (by clicking its title in the menu bar) and sees a list
of commands. They click directly on one of those commands. The command runs immediately
and the menu closes, exactly as if they had navigated to it with the keyboard and pressed
Enter.

**Why this priority**: This is the core gap. Today a mouse user can open a menu but then
must switch to the keyboard to choose anything — a jarring, half-finished interaction that
violates the "click to open, click to choose" expectation set by every comparable file
manager (Norton Commander, Midnight Commander). Delivering only this story already makes
the menus fully usable by mouse.

**Independent Test**: Open any menu via a title click, then click a known item (e.g.
"Mkdir" in the File menu) and confirm the corresponding command is dispatched and the menu
closes. Fully testable on its own with no dependency on the other stories.

**Acceptance Scenarios**:

1. **Given** the File menu is open, **When** the user left-clicks the row showing "Mkdir",
   **Then** the Mkdir command is dispatched and the menu closes.
2. **Given** a menu is open, **When** the user clicks the first item row, **Then** that
   item's command runs (no off-by-one against the dropdown border).
3. **Given** a menu is open, **When** the user clicks the last visible item row, **Then**
   that item's command runs.
4. **Given** a menu is open, **When** the user clicks the dropdown border or an empty area
   inside the dropdown frame that is not an item row, **Then** no command runs and the menu
   stays open.

---

### User Story 2 - Close or switch menus with the mouse (Priority: P2)

A user who has opened a menu changes their mind. They click somewhere outside the open
dropdown — on a panel, on empty chrome, anywhere that is not the dropdown or a menu title —
and the menu closes without running anything. Alternatively they click a *different* menu
title and the open menu switches to that one.

**Why this priority**: Dismissing a menu by clicking away is a deeply ingrained convention;
without it a mouse user feels trapped once a menu is open. Switching between menu titles by
clicking is the natural companion. Builds on US1 but is independently valuable.

**Independent Test**: Open a menu, click on a file panel row, and confirm the menu closes
(and, per the existing panel behavior, the click does not also trigger an unintended action
that the user would find surprising). Separately, open one menu and click another title;
confirm the second menu is now the open one.

**Acceptance Scenarios**:

1. **Given** the File menu is open, **When** the user clicks the Options menu title,
   **Then** the File menu closes and the Options menu opens.
2. **Given** a menu is open, **When** the user left-clicks outside both the menu bar and the
   dropdown (e.g. on a file panel row), **Then** the menu closes AND the click also performs
   the normal panel action (focuses that pane and moves the cursor to the clicked row).
3. **Given** a menu is open, **When** the user clicks the already-open menu's own title,
   **Then** the menu closes (toggle behavior consistent with current title-click handling).

---

### User Story 3 - Hover to highlight the item under the pointer (Priority: P3)

While a menu is open, the user moves the mouse pointer over its items without clicking. The
item currently under the pointer becomes the highlighted/selected item, so a subsequent
click (or Enter) acts on it. This gives continuous visual feedback that the menu is
mouse-live.

**Why this priority**: A polish layer on top of click-to-invoke. It improves discoverability
and feel but is not required for the menus to be fully operable by mouse — clicking an item
in US1 already selects-and-invokes in one action. Lowest priority and explicitly degradable
(see Edge Cases) because some terminals do not deliver pointer-motion events.

**Independent Test**: With a menu open, move the pointer over a non-selected item row and
confirm the highlight moves to that row, with no command dispatched until a click.

**Acceptance Scenarios**:

1. **Given** a menu is open with item 0 highlighted, **When** the pointer moves over item 2's
   row, **Then** item 2 becomes highlighted and no command runs.
2. **Given** a menu is open, **When** the pointer moves over the dropdown border or outside
   the item rows, **Then** the highlight does not change and no command runs.
3. **Given** the pointer is hovering item 2, **When** the user left-clicks, **Then** item 2's
   command runs (hover selection and click invocation agree).

---

### Edge Cases

- **Mouse disabled for the session** (`--no-mouse` / `ui.mouse = false`): no menu mouse
  interaction occurs because no mouse events are delivered. The keyboard path is unchanged.
- **Mouse suspended at runtime** (toggled off via Alt-m): same as disabled until re-enabled;
  no clicks or hovers affect menus while suspended.
- **Terminal does not report pointer-motion events**: US3 hover silently does nothing;
  US1/US2 (button clicks) still work. Hover is an enhancement, never a requirement.
- **Dropdown clamped by a short terminal**: when the dropdown is truncated to fit the screen,
  only the rows actually rendered are clickable; a click below the rendered area counts as
  "outside" and closes the menu.
- **Click on a menu title row while a dropdown is open but the click is on a different
  title**: switches menus (US2), does not close-then-ignore.
- **Click exactly on the dropdown's top/bottom border**: treated as a non-item click inside
  the frame — the menu stays open and nothing is invoked (no off-by-one into the first/last
  item).
- **Double-click on an item**: the first click already invokes and closes the menu; a second
  click lands on whatever is now underneath and must not re-trigger the (now-gone) item.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST dispatch the command of a dropdown item when the user
  left-clicks that item's row in an open menu, and MUST close the menu afterward (same
  end-state as keyboard Enter on that item).
- **FR-002**: The system MUST hit-test clicks against the *rendered, visible* item rows only,
  mapping a click position to the correct item with no off-by-one error relative to the
  dropdown's border/frame. Because the dropdown is drawn with a one-cell border, item *N* sits
  one row below the dropdown's top edge; only rows that are actually visible (after any
  short-terminal clamping) are clickable.
- **FR-003**: The system MUST treat a left-click inside the dropdown frame that is not on an
  item row (e.g. the border) as a no-op that leaves the menu open.
- **FR-004**: The system MUST close the open menu when the user left-clicks outside both the
  menu bar titles and the open dropdown, without invoking any menu item. When the click lands
  on a file panel, the click MUST ALSO perform the normal panel action (focus pane + move
  cursor; double-click descends) — i.e. close-and-pass-through, not swallow.
- **FR-005**: The system MUST switch the open menu when the user left-clicks a different menu
  title while a menu is already open.
- **FR-006**: The system MUST toggle the open menu closed when the user left-clicks the title
  of the menu that is already open (consistent with existing title-click behavior).
- **FR-007**: The system SHOULD update the highlighted item to the item under the pointer when
  the pointer moves over an item row in an open menu (hover-to-highlight), without dispatching
  any command. Because pointer-motion events can arrive at high frequency, handling each one
  MUST be cheap enough to stay within the normal frame budget (no perceptible lag).
- **FR-008**: Pointer movement over the dropdown border, or outside item rows, MUST NOT change
  the current highlight and MUST NOT dispatch a command.
- **FR-009**: All new mouse interactions MUST respect the existing mouse-capture state: when
  the session has mouse disabled (`--no-mouse` / `ui.mouse = false`) or runtime-suspended
  (Alt-m), menu mouse interactions MUST NOT occur and keyboard behavior MUST be unchanged.
- **FR-010**: When the terminal does not deliver pointer-motion events, the feature MUST
  degrade gracefully — click-to-invoke (FR-001) and click-to-close/switch (FR-004/005) MUST
  still function; only hover (FR-007) is forgone.
- **FR-011**: Existing keyboard navigation of the menu (arrows/hjkl, Enter, Esc, open via F9)
  MUST continue to work unchanged.
- **FR-012**: The click-to-invoke path MUST route through the same command-dispatch mechanism
  as keyboard selection, so a menu item behaves identically regardless of input method.

### Key Entities *(include if feature involves data)*

- **Menu bar / dropdown**: the on-screen pull-down menu — a bar of titles, at most one of
  which is "open", showing a vertical list of named items each bound to a command. Has a
  rendered geometry (title positions, dropdown rectangle, per-item rows) that mouse positions
  are tested against.
- **Mouse event**: a user pointer action with a screen position and kind (left-button press,
  pointer movement). Already captured by the application subject to the mouse-capture state.
- **Mouse-capture state**: whether the session supports mouse at all and whether capture is
  currently active at runtime; gates all mouse interaction.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A mouse-only user can open any of the menus and run any of its items using only
  the mouse — open, then click the item — with zero keyboard input.
- **SC-002**: 100% of dropdown items are invokable by a single click on their visible row,
  with no row mapping to the wrong command and no clickable "dead row" caused by the border.
- **SC-003**: A user can dismiss an open menu with a single click outside it in 100% of cases,
  and can move between menus by clicking titles without first closing the current one.
- **SC-004**: When hover is available, the highlighted item always matches the item under the
  pointer; when hover is unavailable, the user notices no broken behavior — clicking still
  works.
- **SC-005**: With mouse disabled or suspended, menu behavior is identical to the pre-feature
  behavior (keyboard-only), verified by the existing keyboard interactions continuing to pass.

## Assumptions

- The dropdown is rendered with a one-cell border on all sides (current behavior), so item
  row *N* sits one row below the dropdown's top edge; hit-testing accounts for this offset.
- Left mouse button is the invocation button; right/middle buttons are out of scope.
- Hover uses pointer-motion ("moved") events; the application already enables mouse capture
  when the session permits, and motion delivery is terminal-dependent (hence FR-010).
- "Clicking outside" includes clicks on file panels; the existing panel-click behavior
  (focus/cursor/descend) is unchanged. Per the 2026-06-22 clarification, the same click that
  closes the menu ALSO performs the panel action (close-and-pass-through); the menu closing is
  not a separate consuming click.
- No new menu items, themes, or configuration keys are introduced by this feature.

## Out of Scope

- Right-click / context menus anywhere in the app.
- Mouse interaction with the user-menu dialog (F2/menu.toml) — that is a separate dialog.
- Keyboard shortcut letters / mnemonics within menus.
- Scroll-wheel navigation within an open dropdown.
- Any change to which commands appear in which menu.

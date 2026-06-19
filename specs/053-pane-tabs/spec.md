# Feature Specification: Pane Tabs — Multiple Panels Per Side

**Feature Branch**: `053-pane-tabs`

**Created**: 2026-06-19

**Status**: Draft

**Input**: User description: "Tabs: multiple panels per side, closes #45. Currently PaneId is Left/Right only; this feature generalizes it to support multiple directory tabs per side (left pane can have tabs 1..N, right pane can have tabs 1..N). Capabilities: (1) Ctrl-t opens a new tab in the active side, inheriting the current directory; (2) Ctrl-w closes the current tab (no-op if it's the only tab on that side); (3) a tab bar widget renders above each pane showing tab index + directory name, with the active tab highlighted; (4) Alt-Left/Alt-Right (or [ / ] keys) cycles through tabs on the active side; (5) tab state is per-side — each side independently tracks its own list of tabs and which is active; (6) cross-pane operations (copy, move, diff) continue to work between the active tab of each side; (7) PaneId/PaneState generalization — replace Left/Right enum with a side+index concept internally, keeping the public API stable for callers that only care about active left vs active right. The tab bar is compact: shows truncated directory basename, max ~20 chars per tab, scrolls horizontally if tabs exceed pane width. Minimum viable: 2+ tabs per side, all existing features (filter, search, sort, viewer, find-file) work within a tab."

## Overview

Today `PaneId` is a two-variant enum (`Left`/`Right`) and `App` holds exactly one `PaneState` per side. This feature introduces per-side tab lists: each side (left, right) independently manages an ordered list of directory tabs, with one active tab at a time. A compact tab bar renders above each pane column showing the basename (truncated to ~20 chars) of each tab's working directory, with the active tab visually distinguished. Users create tabs with `Ctrl-t` (inheriting the current directory), close them with `Ctrl-w` (no-op when only one tab remains), and cycle between them with `[` / `]`. Every existing feature — filter, search, sort, hex viewer, find-file, panelize, file ops — continues to function identically within each individual tab. Cross-pane operations (copy, move, compare) act between the *active* tab on the left side and the *active* tab on the right side, preserving existing semantics.

## Clarifications

### Session 2026-06-19

- Q: Should tabs persist across sessions (saved to config/state file)? → A: No persistence in this feature — tabs are session-only; reopening the application starts with one tab per side.
- Q: What is the maximum number of tabs per side? → A: No hard cap enforced in this feature; the tab bar scrolls horizontally when tabs exceed pane width. Practical limit is screen width / minimum tab label width (~4 chars).
- Q: Should Ctrl-t inherit the active tab's directory, or open a user-configured home/root? → A: Inherit the active tab's current working directory so the new tab opens in the same place.
- Q: How does the PaneId public API generalization work — does a new `PaneSide` Rust type get introduced, or does `PaneId` stay unchanged? → A: `PaneId::Left`/`Right` stay unchanged (FR-009 honoured). `PaneSide` is a spec conceptual term, not a new Rust type. Internally, `App.panes: [PaneState; 2]` becomes `App.sides: [SideState; 2]` where `SideState` holds `Vec<PaneState>` + `active_tab: usize`. The existing `pane(PaneId) -> &PaneState` method now returns `sides[idx].tabs[active_tab]` — identical signature, identical semantics ("the visible state for this side"), zero call-site changes outside `cargonaut-core`.
- Q: Should the tab bar be rendered when only one tab exists on a side? → A: Yes — always render the tab bar on both sides regardless of tab count, to prevent a layout jump when the first extra tab is added or removed (pane content height stays constant).
- Q: Should `[` / `]` tab-cycle keys be swallowed by an open modal dialog (like `Ctrl-t` is), or allowed to switch tabs while a modal is active? → A: Swallowed — all tab management keys (`Ctrl-t`, `Ctrl-w`, `[`, `]`) are consumed by any active modal; tab state MUST NOT change while a dialog is open, so the dialog's context (source/destination pane, confirmation target) cannot shift mid-interaction.
- Q: When `Ctrl-w` closes a tab, which tab becomes active and do indices renumber? → A: The tab to the right becomes active (wrapping to the last tab if closing the rightmost); tab indices always reflect 1-based position and renumber continuously — closing tab 2 of [1,2,3] yields [1,2] with the former tab 3 (now at position 2) becoming active.
- Q: While the F3 viewer is open, are `[`/`]` tab-cycle keys allowed through or consumed by the viewer? → A: Consumed — the F3 viewer is treated as a full modal (consistent with its `ActiveDialog::FileViewer` implementation in feature 051); FR-013 applies. All four tab management keys are blocked while viewer is open. User must press `q`/Esc to close the viewer before switching tabs.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Open and switch between multiple tabs (Priority: P1)

A developer is comparing files across three different directories in the left pane. They press `Ctrl-t` twice to create two additional tabs. Each new tab opens in the same directory as the previous active tab. A tab bar appears above the left pane showing three entries — each displaying the basename of its working directory. Pressing `]` moves to the next tab and `[` moves to the previous tab. Each tab maintains its own cursor position and selection independently. The right pane is unaffected.

**Why this priority**: Tab creation and switching is the entire raison d'être of this feature. Nothing else works until this does.

**Independent Test**: Press `Ctrl-t` from the left pane; confirm a tab bar appears with two entries; press `]` to switch; confirm the active tab indicator moves; press `[` to go back; press `Ctrl-w` and confirm the closed tab disappears and the tab bar shows one entry again.

**Acceptance Scenarios**:

1. **Given** one tab per side, **When** the user presses `Ctrl-t`, **Then** a new tab opens on the active side in the same directory and becomes the active tab; a tab bar is visible above that pane column.
2. **Given** multiple tabs on the active side, **When** the user presses `]`, **Then** the next tab (wrapping from last to first) becomes active.
3. **Given** multiple tabs on the active side, **When** the user presses `[`, **Then** the previous tab (wrapping from first to last) becomes active.
4. **Given** multiple tabs on the active side, **When** the user presses `Ctrl-w`, **Then** the current tab closes; the tab to the right (or left if it was the last) becomes active; the tab bar updates.
5. **Given** exactly one tab on a side, **When** the user presses `Ctrl-w`, **Then** nothing happens (no crash, no state change).
6. **Given** multiple tabs, **When** the user switches tabs, **Then** each tab's cursor position, selection, sort order, and filter are independently preserved.
7. **Given** the active side has tabs and the other side does not, **When** the user presses `Tab` to focus the other side, **Then** the other side shows a tab bar with one entry.
8. **Given** a tab bar with more entries than fit horizontally, **Then** the tab bar scrolls so the active tab is always visible.

---

### User Story 2 — File operations act between active tabs (Priority: P1)

A user has the left pane on `~/Downloads` tab 1 and a second left tab on `~/Documents`. The right pane shows `/tmp`. They press `Tab` to focus the right pane, select a file, and press `F5` to copy. The copy destination is the *active* left tab (`~/Downloads` tab 1), not the other left tab. The cross-pane semantics are unchanged: source = active tab on focused side, destination = active tab on the other side.

**Why this priority**: File operations are core functionality. If the active-tab semantics for cross-pane ops are wrong, users will accidentally copy to the wrong directory.

**Independent Test**: Create two left tabs in different directories; focus the right pane; copy a file with F5; confirm the dialog shows the active left tab's directory as the destination.

**Acceptance Scenarios**:

1. **Given** multiple tabs on the left side, **When** the user initiates a copy from the right pane, **Then** the copy destination is the active left tab's cwd.
2. **Given** multiple tabs on the right side, **When** the user initiates a copy from the left pane, **Then** the copy destination is the active right tab's cwd.
3. **Given** both sides have multiple tabs, **When** the user switches tabs on the destination side before confirming the copy, **Then** the destination does not change mid-dialog (dialog captured the destination at open time).
4. **Given** a compare-directories operation, **When** triggered, **Then** it compares the active tab on the left side against the active tab on the right side.

---

### User Story 3 — All existing pane features work per-tab (Priority: P2)

A user opens a tab in a large directory, applies a name filter (Alt-!), sorts by date (Ctrl-s), then opens a second tab. The second tab has no filter and default sort — each tab maintains its own independent filter, sort, cursor, and selection state. Opening the hex viewer (F3) in one tab does not affect the other tab. The find-file popup (Alt-?) opens relative to the active tab's directory.

**Why this priority**: Tabs are only useful if they are fully isolated state-wise. A regression that makes filter or sort leak between tabs would be a critical bug.

**Independent Test**: Apply a filter in tab 1; switch to tab 2; confirm the filter is absent; switch back to tab 1 and confirm the filter is still active.

**Acceptance Scenarios**:

1. **Given** a filter is applied in tab 1, **When** the user switches to tab 2, **Then** tab 2 has no filter (unless tab 2 independently has one).
2. **Given** a sort order is set in tab 1, **When** the user switches to tab 2, **Then** tab 2 uses its own sort order.
3. **Given** the hex viewer is open in tab 1, **When** the user presses `Ctrl-t`, **Then** the new tab opens in pane mode (not in viewer mode).
4. **Given** a find-file result is panelized in tab 1, **When** the user switches to tab 2, **Then** tab 2 shows its own listing independently.
5. **Given** tab 1 and tab 2 exist on the right side, **When** the user presses `M-i` (sync other panel path) from the left pane, **Then** the active right tab's path is synced to the left pane (the inactive right tab is unaffected).

---

### Edge Cases

- What happens if any tab management key (`Ctrl-t`, `Ctrl-w`, `[`, `]`) is pressed while a modal dialog (copy confirm, delete confirm, filter prompt, rename dialog, etc.) is open? All four keys are consumed by the dialog; tab state MUST NOT change while any modal is active. This ensures that the dialog's captured context (source pane, destination pane, selected entries) cannot shift mid-interaction.
- What happens when a tab is closed while its directory no longer exists (e.g., was deleted)? The tab closes normally; no crash; the new active tab navigates to its own cwd as usual.
- What happens when all tabs on a side navigate to the same directory? Each tab is still independent state (cursor, selection, filter may differ).
- What happens when only one tab exists and `[` or `]` is pressed? The action is a no-op (cursor stays on the only tab).
- What is the minimum tab label width in the tab bar when many tabs are open? Each label is at minimum 4 characters (1 for index, ellipsis, separator) — exact truncation is an implementation detail not a spec constraint.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The active side MUST support opening a new tab (`Ctrl-t`) that starts in the active tab's current working directory.
- **FR-002**: The active side MUST support closing the current tab (`Ctrl-w`); the action MUST be a no-op when only one tab remains on that side. After closing, the tab to the right of the closed tab becomes active; if the closed tab was the rightmost, the new rightmost tab becomes active. Tab indices MUST renumber continuously (1-based positional), so after closing tab 2 of [1,2,3] the tabs display as [1,2] with tab 2 being the former tab 3.
- **FR-003**: The user MUST be able to cycle to the next tab on the active side (`]`) and to the previous tab (`[`), wrapping around.
- **FR-004**: A tab bar MUST be rendered above each pane column at all times — including when only one tab exists on that side — so pane content height is constant and no layout jump occurs on the first `Ctrl-t`. The tab bar shows the basename of each tab's cwd, truncated to ~20 characters. The active tab MUST be visually distinguished (e.g., bold, highlighted background, or underline).
- **FR-005**: When tabs exceed the available horizontal width, the tab bar MUST scroll so the active tab is always visible.
- **FR-006**: Each tab MUST maintain its own independent state: cursor position, selection set, sort order, name filter, show-hidden flag, and directory history.
- **FR-007**: Cross-pane file operations (copy, move, compare, sync-path) MUST use the active tab of each side as the source/destination — the same semantics as the current two-pane model.
- **FR-008**: All existing pane features (name filter, sort, directory history, hex viewer, find-file, panelize, file attributes, bookmarks, recursive-dir-size) MUST work correctly within an individual tab.
- **FR-009**: The `PaneId` public API MUST remain stable — `PaneId::Left`, `PaneId::Right`, `App::pane(PaneId)`, `App::active_pane()`, and `App::active_pane_state()` keep their current signatures. Internally, `App.panes: [PaneState; 2]` is replaced by `App.sides: [SideState; 2]` (private); `pane(PaneId)` returns `sides[idx].tabs[active_tab]`. No call sites outside `cargonaut-core` require modification.
- **FR-010**: `Tab` key (focus-swap-pane) and `M-1`/`M-2` (focus-left-pane/focus-right-pane) MUST continue to work — they switch focus between sides, not between tabs within a side.
- **FR-011**: When a new tab is opened, it MUST not inherit an active filter or selection from the source tab (it starts clean in the same directory).
- **FR-012**: The tab bar MUST show a 1-based numeric index or a visible indicator so users can orient themselves (e.g., `[1] src  [2] tests  [3*] docs` where `*` marks active, or highlighted styling).
- **FR-013**: While any modal dialog is active (copy confirm, delete confirm, filter prompt, viewer, find-file, rename, etc.), all four tab management keys (`Ctrl-t`, `Ctrl-w`, `[`, `]`) MUST be consumed by the dialog without modifying tab state.

### Key Entities

- **PaneSide** (spec conceptual term, not a new Rust type): Left or Right — identifies which of the two pane columns is being referred to. In Rust, this concept continues to be represented by the existing `PaneId` enum (`PaneId::Left` / `PaneId::Right`). No new `PaneSide` type is introduced; `PaneId` retains its current name and variants (FR-009).
- **SideState** (new internal Rust struct, private to `cargonaut-core`): A side's complete state: an ordered `Vec<PaneState>` plus `active_tab: usize`. Replaces the two `PaneState` slots in `App.panes: [PaneState; 2]`, which becomes `App.sides: [SideState; 2]`. `App::pane(PaneId)` continues to return `&PaneState` by indexing `sides[idx].tabs[active_tab]`.
- **TabIndex**: Zero-based integer index identifying a tab within a `SideState.tabs` Vec.
- **TabBar**: Read-only view model used by the renderer: list of (basename, is-active) pairs for one side's tabs, produced by a new additive public method `App::tab_bar_view(PaneId) -> Vec<TabBarEntry>`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can open, switch between, and close tabs with `Ctrl-t`, `]`, `[`, `Ctrl-w` without any unintended state mutation in non-active tabs — verified by an integration test exercising all four bindings on both sides.
- **SC-002**: All 80%+ coverage thresholds (Constitution §II) continue to pass after this feature — no coverage regression in `cargonaut-core`.
- **SC-003**: Keypress-to-first-paint latency remains ≤16 ms (Constitution §IV NFR-002) in the presence of a tab bar with 5 tabs per side — verified by the existing latency bench or an updated one.
- **SC-004**: Memory overhead of 5 additional empty tabs per side adds ≤1 MiB to RSS — verified by the existing RSS bench or an updated one targeting the new cap (SC-003 of the original set: ≤64 MiB total).
- **SC-005**: Every existing feature-level integration test (filter, sort, viewer, find-file, compare-dirs, bulk-rename, file-ops) passes without modification in a single-tab configuration.

## Assumptions

- Tab state is session-only; no persistence to disk in this feature.
- The maximum number of visible tabs is bounded only by screen width; no hard cap is imposed.
- The tab bar height is exactly one terminal row per pane column; it does not resize.
- `Alt-Left` / `Alt-Right` (arrow-key form of tab cycle) are not used in this feature — `[` and `]` are the tab-cycle keys, consistent with editor convention — because `M-Left`/`M-Right` would conflict with word-navigation conventions in some terminals.
- A new tab does not inherit the source tab's filter, selection, or viewer state; it starts with default pane state in the inherited cwd. The F3 viewer is treated as a full modal (FR-013): all tab management keys (`Ctrl-t`, `Ctrl-w`, `[`, `]`) are consumed while the viewer is open — the user must close it with `q`/Esc before switching or creating tabs.
- This feature does not add tab persistence, tab reorder (drag), or named/labeled tabs — those are follow-up scope.
- The `before_specify` git hook already created branch `053-pane-tabs`; no branch creation action is needed here.

# Feature Specification: F2 User-Menu Mouse-Click Support

**Feature Branch**: `048-f2-mouse-click`

**Created**: 2026-06-18

**Status**: Draft

**Input**: Feature 047 follow-up — wire and integration-test mouse-click support for the F2 user-menu button in the function-key bar. When the user left-clicks the on-screen F2 key button, the app should open the UserMenu dialog (ActiveDialog::UserMenu), identical to pressing the F2 key. GitHub issue #70.

## Overview

Feature 047 wired the F2 keyboard trigger (`Command::ShowUserMenu`) and the full user-menu dialog, but did not add an integration test that verifies the on-screen F2 button also responds to a mouse left-click. The fkey-bar click routing may already dispatch `Command::ShowUserMenu` for button index 1 (F2), but this has never been covered by an automated test.

This feature closes that gap. The deliverable is primarily a test — verify (and fix if needed) that a left-click on the on-screen F2 button opens the UserMenu dialog exactly as a keyboard F2 keypress does.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Mouse click on F2 button opens the user menu (Priority: P1)

A user who prefers mouse-driven navigation clicks the on-screen **F2** button in the function-key bar. The user-menu dialog opens immediately, identical to pressing the F2 key on the keyboard. The user can then navigate the menu with keyboard or mouse and dismiss it with Esc.

**Why this priority**: The function-key bar is a mouse-clickable affordance that users reasonably expect to be fully functional. Keyboard and mouse paths for the same UI element should behave identically. Issue #70 confirms the keyboard path is tested and green; the mouse path is not integration-tested and may silently not work.

**Independent Test**: Simulate a left-click at the on-screen F2 button position; confirm `ActiveDialog::UserMenu` is set. This can be tested independently of any keyboard interaction.

**Acceptance Scenarios**:

1. **Given** the application is running with the user-menu feature available, **When** the user left-clicks the F2 button in the function-key bar, **Then** `ActiveDialog::UserMenu` is set and the user-menu dialog renders.
2. **Given** the F2 button is clicked and the user-menu dialog is open, **When** the user presses Esc, **Then** the dialog closes and the application returns to its prior state.
3. **Given** the user-menu dialog is not open, **When** the user left-clicks any fkey-bar button that is NOT F2, **Then** the user-menu dialog does NOT open (no cross-button interference).

---

### Edge Cases

- **Click lands outside the F2 button bounds**: no dialog opens (existing mouse routing handles this).
- **`menu.toml` is absent**: the dialog still opens (showing the "no actions" placeholder row), identical to the keyboard F2 path — the routing test does not depend on menu content.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: A left-click on the on-screen F2 button in the fkey bar MUST set `ActiveDialog::UserMenu`, opening the user-menu dialog. If another dialog is already active, the click MUST be ignored (no-op), identical to the keyboard F2 guard behavior.
- **FR-002**: The mouse-click path MUST produce the same outcome as pressing the F2 keyboard key — no behavioral divergence between the two input modes. "Same outcome" is defined as: `ActiveDialog::UserMenu { .. }` is set in both cases (same dialog variant; sub-fields such as `entry_path` are not required to be identical across paths).
- **FR-003**: If the fkey-bar click routing does NOT already dispatch `Command::ShowUserMenu` for the F2 button, the routing code MUST be updated to do so.
- **FR-004**: An integration test MUST exist that simulates a left-click at the F2 button's rendered position and asserts `ActiveDialog::UserMenu` is the resulting dialog state.
- **FR-005**: The new test MUST pass in the standard CI pipeline without requiring any special environment variable gate (unlike PTY tests which require `CARGONAUT_PTY_TESTS=1`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: The integration test covering mouse-click on F2 passes green in `cargo test --workspace` with no special flags.
- **SC-002**: Clicking the F2 button and pressing the F2 key both result in `ActiveDialog::UserMenu { .. }` being set — measurable by asserting the same enum variant in the mouse-click integration test as in the existing keyboard tests.
- **SC-003**: No existing tests regress as a result of this change — verified by `make ci-local` completing all five pipeline steps (clippy → test → build → check-pr-body → docs-gate) green.

## Assumptions

- The fkey-bar renders F-key buttons at predictable, calculable positions based on terminal width; the test can compute the click coordinates from the render output without requiring a real PTY.
- The existing `handle_mouse` implementation in `cargonaut-ui-tui` already routes fkey-bar clicks to the appropriate `Command` variant for other buttons; this feature either confirms or fixes the F2 routing specifically.
- No behavioral changes are needed in `cargonaut-core` or `cargonaut-config` — this is a UI routing + test-coverage fix only.
- The `ActiveDialog::UserMenu` state is already exercised by keyboard tests; the new test can use the same assertion patterns.

## Out of Scope

- Mouse navigation within the user-menu dialog (already handled by the existing dialog widget).
- Changes to `menu.toml` loading, action execution, or any other user-menu behavior.
- PTY-level end-to-end mouse tests — the test uses the app's internal event handling, not a real terminal emulator.

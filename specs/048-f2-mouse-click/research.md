# Research: F2 User-Menu Mouse-Click Support

**Date**: 2026-06-18

## R-001: Is the fkey-bar → ShowUserMenu routing already correct?

**Decision**: Yes — the routing is fully wired and correct. No production code changes are needed.

**Evidence**:
- `chrome.rs:229` — the fkey-bar button array includes `("User menu", Command::ShowUserMenu)` as button index 1 (F2).
- `chrome.rs:131` — `FunctionKeyBar::command_at(area, x, y)` maps a click coordinate to the button's `Command`.
- `lib.rs:1246` — `handle_mouse` calls `ui.fkeybar.command_at(ui.layout.fkeys, x, y)` and on success routes to `dispatch_ui_command(cmd, …)`.
- `lib.rs:1032` — `dispatch_ui_command` handles `Command::ShowUserMenu` by setting `*active_dialog = Some(ActiveDialog::UserMenu { … })`.

**Rationale**: The issue description in #70 said "the wiring is technically feasible but requires integration testing" — confirmed.

**Alternatives considered**: Fixing the routing. Not needed.

## R-002: Why does the existing T-MOUSE-5 test not catch the gap?

**Decision**: The `mouse()` test helper discards `active_dialog`. The assertion is weak (negative string check only).

**Evidence**: `lib.rs:2441–2466` — `mouse()` creates a local `let mut dlg: Option<ActiveDialog> = None;` that is never returned. The existing T-MOUSE-5 (`click_fkey_button_dispatches_command`) calls `mouse(…)` and only checks `!status.contains("not yet available")`.

**Rationale**: When Feature 047 shipped, the test confirmed the old "not yet available" stub was removed, but stopped short of asserting the *new* behavior. The positive assertion that `ActiveDialog::UserMenu` is set is missing.

## R-003: What is the minimal fix?

**Decision**: Add a `mouse_with_dlg()` helper that returns `(String, Option<ActiveDialog>)`, and add a new `#[tokio::test] async fn f2_mouse_click_opens_user_menu()` that uses it and asserts `matches!(dlg, Some(ActiveDialog::UserMenu { .. }))`.

**Rationale**:
- Keeps the original `mouse()` helper intact (other tests depend on it).
- A new helper avoids changing existing test signatures.
- A new test (rather than modifying T-MOUSE-5) preserves T-MOUSE-5's original intent while adding a stronger T-MOUSE-5b.

**Alternatives considered**:
- Modify T-MOUSE-5 to use `mouse_with_dlg`: valid, but adds churn to an existing test label.
- Add `active_dialog` return to `mouse()`: would require updating every caller.

## R-004: What click coordinates represent the F2 button?

**Decision**: In a 100-wide fkey-bar with 10 buttons, F2 occupies slot 1 (0-indexed), covering x=10..20. `x=15, y=23` (used in existing T-MOUSE-5) is correct and lands within that slot.

**Evidence**: `lib.rs:2639–2649` — the existing test sets `ui.layout.fkeys = Rect { x: 0, y: 23, width: 100, height: 1 }` and clicks `left_click(15, 23)`. The fkey layout divides width evenly: 100 / 10 = 10 px per button; button index 1 → x in [10, 20).

## R-005: Does the no-menu.toml case (empty actions) still produce ActiveDialog::UserMenu?

**Decision**: Yes — `lib.rs:1064–1071` sets `*active_dialog = Some(ActiveDialog::UserMenu { widget: UserMenuDialog::new_error("…"), entry_path })` even when filtered actions are empty. The test will reliably observe `ActiveDialog::UserMenu` without any `menu.toml` present.

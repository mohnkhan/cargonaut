# Implementation Plan: F2 User-Menu Mouse-Click Support

**Branch**: `048-f2-mouse-click` | **Date**: 2026-06-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/048-f2-mouse-click/spec.md`

## Summary

The fkey-bar click routing already dispatches `Command::ShowUserMenu` when the user
left-clicks the F2 button (`chrome.rs:229` maps F2 → `ShowUserMenu`; `lib.rs:1246`
routes all fkey-bar clicks through `dispatch_ui_command`). The existing T-MOUSE-5 test
(`click_fkey_button_dispatches_command`) exists but uses a throwaway `active_dialog`
variable — it only asserts the negative "not yet available" string is absent. The full
positive assertion (`ActiveDialog::UserMenu { .. }` is set) is missing.

**This feature is test-only.** No routing code changes are required.

## Technical Context

**Language/Version**: Rust (edition 2021), same toolchain as workspace

**Primary Dependencies**: `tokio` (async test runtime), `cargonaut-ui-tui` (dialog state,
`handle_mouse`, `ActiveDialog`, `UiState`), `cargonaut-core` (`App`), `tempfile` (test dirs)

**Storage**: N/A

**Testing**: `cargo test --workspace` — standard `#[tokio::test]` inside `crates/cargonaut-ui-tui/src/lib.rs` (where all T-MOUSE-* tests live)

**Target Platform**: Linux / CI-identical environment

**Performance Goals**: N/A (test-only)

**Constraints**: Test must pass without `CARGONAUT_PTY_TESTS=1` or any env gate. Must not
break T-MOUSE-1 through T-MOUSE-6 or any adjacent test.

**Scale/Scope**: 1 test function added; ~20–40 lines of test code.

## Constitution Check

| Gate | Status | Notes |
|------|--------|-------|
| §I Code Quality: `clippy -D warnings` | ✅ Pass | Test-only addition; no new warnings expected |
| §I Code Quality: `cargo fmt` | ✅ Pass | Format before commit |
| §II TDD: red commit before green commit | ✅ Required | Write failing `#[tokio::test]` first |
| §II Coverage ≥80% on core crates | ✅ Maintained | New test increases coverage on `dispatch_ui_command` / `handle_mouse` |
| §III UX: shared dialog macro | ✅ N/A | No new dialog widget |
| §IV Performance benches | ✅ N/A | No performance-path changes |
| §V SSD: `make check-tmpfs` | ✅ Required | All builds via `make test` |

No violations. Complexity Tracking not needed.

## Project Structure

### Documentation (this feature)

```text
specs/048-f2-mouse-click/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output (N/A for test-only — omitted)
├── quickstart.md        # Phase 1 output
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code (touched files)

```text
crates/cargonaut-ui-tui/src/lib.rs
  └── #[cfg(test)] mod tests
        ├── mouse_with_dlg()          # new helper returning (status, Option<ActiveDialog>)
        ├── click_fkey_button_dispatches_command()   # T-MOUSE-5: strengthen assertion
        └── f2_mouse_click_opens_user_menu()         # T-MOUSE-5b: new dedicated test
```

No other files are changed.

## Phase 0: Research

See [research.md](research.md).

## Phase 1: Design

### Data model

No new data model. `ActiveDialog::UserMenu { widget, entry_path }` already exists
(Feature 047). The test observes it; it does not define it.

### Contracts

No external interface change. The fkey-bar click → dialog wiring is purely internal.

### Implementation approach

#### Existing routing (already correct, do not change)

```
Left-click at (x=15, y=23)
  → handle_mouse (lib.rs:1216)
  → fkeybar.command_at(layout.fkeys, 15, 23) → Some(Command::ShowUserMenu)   [chrome.rs:131]
  → dispatch_ui_command(ShowUserMenu, …)  [lib.rs:827]
  → active_dialog = Some(ActiveDialog::UserMenu { … })  [lib.rs:1032–1074]
```

#### Gap to close

The `mouse()` test helper (lib.rs:2442) creates a **local** throwaway `dlg: Option<ActiveDialog>` and discards it — the caller never sees what dialog was set.

**Fix**: Add a `mouse_with_dlg()` helper that returns `(String, Option<ActiveDialog>)`:

```rust
async fn mouse_with_dlg(
    m: MouseEvent,
    app: &mut App,
    ui: &mut UiState,
    l: &PaneView,
    r: &PaneView,
) -> (String, Option<ActiveDialog>) {
    let mut status = String::new();
    let mut mode = Mode::Pane;
    let mut dlg: Option<ActiveDialog> = None;
    let mut quit = false;
    handle_mouse(m, app, ui, l, r, &mut status, &mut mode, &mut dlg, &mut quit)
        .await
        .unwrap();
    (status, dlg)
}
```

Then write a dedicated test that uses `mouse_with_dlg` and asserts on the dialog variant:

```rust
// T-MOUSE-5b (issue #70 / FR-001): left-click on the on-screen F2 button
// opens ActiveDialog::UserMenu — verifying the mouse path is equivalent
// to the keyboard F2 path.
#[tokio::test]
async fn f2_mouse_click_opens_user_menu() {
    let td_l = TempDir::new().unwrap();
    let td_r = TempDir::new().unwrap();
    let mut app = app_with(&td_l, &td_r).await;
    let mut ui = fresh_ui(
        Rect { x: 0, y: 1, width: 40, height: 10 },
        Rect { x: 50, y: 1, width: 40, height: 10 },
        true,
    );
    ui.layout.fkeys = Rect { x: 0, y: 23, width: 100, height: 1 };
    let (l, r) = synced_views(&app);
    // F2 button occupies slot 1 (0-indexed) in a 100-wide, 10-button bar → x 10..20.
    let (_status, dlg) = mouse_with_dlg(left_click(15, 23), &mut app, &mut ui, &l, &r).await;
    assert!(
        matches!(dlg, Some(ActiveDialog::UserMenu { .. })),
        "left-click on F2 must open UserMenu dialog; got {dlg:?}"
    );
}
```

#### Strengthen the existing T-MOUSE-5 assertion (optional but desirable)

The existing `click_fkey_button_dispatches_command` can have its assertion replaced with
the stronger `ActiveDialog::UserMenu` check by switching it to use `mouse_with_dlg`.

### Key lookups for implementation

| Symbol | File | Line |
|--------|------|------|
| `handle_mouse` | `crates/cargonaut-ui-tui/src/lib.rs` | 1216 |
| `dispatch_ui_command` | `crates/cargonaut-ui-tui/src/lib.rs` | 827 |
| `Command::ShowUserMenu` handler | `crates/cargonaut-ui-tui/src/lib.rs` | 1032 |
| `ActiveDialog::UserMenu` enum variant | `crates/cargonaut-ui-tui/src/lib.rs` | 154 |
| `FunctionKeyBar::command_at` | `crates/cargonaut-ui-tui/src/chrome.rs` | 131 |
| F2 → `ShowUserMenu` mapping | `crates/cargonaut-ui-tui/src/chrome.rs` | 229 |
| `mouse()` test helper | `crates/cargonaut-ui-tui/src/lib.rs` | 2442 |
| `click_fkey_button_dispatches_command` (T-MOUSE-5) | `crates/cargonaut-ui-tui/src/lib.rs` | 2620 |
| `left_click` helper | `crates/cargonaut-ui-tui/src/lib.rs` | 2417 |
| `synced_views` helper | `crates/cargonaut-ui-tui/src/lib.rs` | 2426 |
| `fresh_ui` helper | `crates/cargonaut-ui-tui/src/lib.rs` | 1907 |

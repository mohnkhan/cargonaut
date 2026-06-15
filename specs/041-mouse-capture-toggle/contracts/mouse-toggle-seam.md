# Contract: Mouse Capture Toggle Seam

**Feature**: 041-mouse-capture-toggle | **Date**: 2026-06-15

The interfaces this feature exposes and the invariants tests pin down.

## 1. Keymap contract (`design/contracts/keymap.toml`)

New binding (single source of truth, loaded at startup):

```toml
[[binding]]
mode = "global"
key = "M-m"
action = "toggle-mouse-capture"  # FR-001 (Feature 031 FR-013 follow-up, #38)
```

- `action = "toggle-mouse-capture"` maps to `Command::ToggleMouseCapture`
  (kebab-case → variant via `#[serde(rename_all = "kebab-case")]`).
- **Contract test** (`keymap.rs`): `Keymap::load(DEFAULT_KEYMAP_TOML)` succeeds
  and `lookup(Mode::Global, M-m)` resolves to `Command::ToggleMouseCapture`.
- **Non-collision test**: no other binding resolves `M-m`; loading both the
  default and `--mc-keys` maps does not bind `M-m` to anything else (FR-009).

## 2. Command enum (`crates/cargonaut-ui-tui/src/keymap.rs`)

```rust
pub enum Command {
    // …existing…
    /// Toggle runtime mouse capture on/off (FR-001).
    ToggleMouseCapture,
}
```

## 3. Pure decision function (`crates/cargonaut-ui-tui/src/lib.rs`)

```rust
/// Outcome of a mouse-capture toggle request. Pure — no terminal I/O.
pub(crate) enum MouseToggleOutcome { Disabled, EnabledNow, SuspendedNow }

/// Decide what a toggle should do, given session support and current capture.
/// `supported` = `config.ui.mouse`; `currently` = `UiState.mouse_enabled`.
pub(crate) fn plan_mouse_toggle(supported: bool, currently: bool) -> MouseToggleOutcome;
```

**Truth table (the contract tests assert exactly this):**

| `supported` | `currently` | outcome |
|-------------|-------------|---------|
| false | false | `Disabled` |
| false | true  | `Disabled` (defensive; should not occur) |
| true  | false | `EnabledNow` |
| true  | true  | `SuspendedNow` |

## 4. Dispatch wiring (`dispatch_ui_command`)

`Command::ToggleMouseCapture` arm:
1. `match plan_mouse_toggle(app.config().ui.mouse, ui.mouse_enabled)`:
   - `Disabled` → set `*status` to the disabled message; **no** `execute!`,
     **no** flag change.
   - `EnabledNow` → `execute!(stdout(), EnableMouseCapture)?`;
     `ui.mouse_enabled = true`; set `*status` to "Mouse capture: on".
   - `SuspendedNow` → `execute!(stdout(), DisableMouseCapture)?`;
     `ui.mouse_enabled = false`; set `*status` to the suspended+Shift message.
2. `return Ok(())` (UI-only command, no core command dispatch).

**Invariants:**
- After the arm runs, `ui.mouse_enabled` and the terminal capture mode agree
  (SC-002).
- The arm performs at most one `execute!` (idempotent per press, SC-001).
- `handle_mouse`'s existing `if !ui.mouse_enabled { return }` guard means a
  suspended session ignores mouse events with no further change (FR-002).
- `run_external(_, _, ui.mouse_enabled)` (existing) restores the *current*
  flag after an external program, not the launch value (FR-007).

## 5. Persistent indicator (`chrome.rs` / `draw_frame`)

A render-only helper produces the menu-bar-right label:

```rust
/// "[mouse:on]" | "[mouse:susp]" | "[mouse:off]" from the two booleans.
pub fn mouse_indicator(session_supported: bool, captured: bool) -> &'static str;
```

- **Render test** (`TestBackend`): with `captured=true` the frame contains
  `[mouse:on]`; after a `SuspendedNow` toggle it contains `[mouse:susp]`;
  with `session_supported=false` it contains `[mouse:off]` (FR-005 persistent).
- Drawn with typed theme colors only (Constitution III).

## 6. Help text (`lib.rs` help overlay)

The help overlay's mouse line MUST mention the `M-m` toggle and the Shift-drag
bypass (FR-010 / SC-006). **Test**: help string contains "M-m" and "Shift".

# Data Model: In-Session Mouse Capture Toggle

**Feature**: 041-mouse-capture-toggle | **Date**: 2026-06-15

This feature is almost entirely behavioral; the only "data" is small in-memory
session state and one new enum. No persisted entities, no serialization changes.

## Entities

### Mouse capture state (runtime)

| Field | Type | Owner | Meaning |
|-------|------|-------|---------|
| `mouse_enabled` | `bool` | `UiState` (existing, `crates/cargonaut-ui-tui/src/lib.rs`) | Whether the app is **currently** capturing the mouse. Initialized from `config.ui.mouse`; flipped by the toggle. |

- **Lifecycle**: Created when `run()` builds `UiState` (from `config.ui.mouse`);
  flipped in place by `Command::ToggleMouseCapture`; released on exit by the
  existing unconditional `DisableMouseCapture` teardown.
- **Invariant (SC-002)**: Whenever `mouse_enabled` changes, the terminal's
  actual capture mode is updated in the same dispatch (one `execute!`), so the
  flag and the terminal never desync.

### Session mouse support (read-only, derived)

| Field | Type | Source | Meaning |
|-------|------|--------|---------|
| `config.ui.mouse` | `bool` | `cargonaut-config` (existing) | Whether mouse support is enabled for the **whole session**. Set false by `--no-mouse` or `ui.mouse=false`. Immutable at runtime. |

- Not new state — read in `dispatch_ui_command` to gate the toggle (FR-006).

## New type: `MouseToggleOutcome`

The pure decision returned by `plan_mouse_toggle(supported, currently)`:

| Variant | Precondition | Effect the caller applies |
|---------|--------------|----------------------------|
| `Disabled` | `supported == false` | No capture change; status: "Mouse support disabled for this session (--no-mouse / ui.mouse=false)". |
| `EnabledNow` | `supported && !currently` | `execute!(EnableMouseCapture)`; `mouse_enabled = true`; status: "Mouse capture: on". |
| `SuspendedNow` | `supported && currently` | `execute!(DisableMouseCapture)`; `mouse_enabled = false`; status: "Mouse capture: suspended — Shift+drag to select text". |

- **Pure**: depends only on its two boolean inputs; no I/O. This is the unit-
  tested core (FR-002/003/006, SC-001/004).

## State transition diagram

```text
                 M-m (supported)              M-m (supported)
   [captured] ───────────────────► [suspended] ───────────────────► [captured]
       ▲                                                                  │
       └──────────────────────────────────────────────────────────────--┘

   [session mouse disabled]  ──M-m──►  [session mouse disabled]   (no state change,
                                                                   explanatory status)
```

## Persistent indicator (render-only projection)

The menu-bar indicator is a pure function of `mouse_enabled` + session support:

- `config.ui.mouse == false` → `[mouse:off]` (session-disabled; shown dimmed)
- `mouse_enabled == true`     → `[mouse:on]`
- `mouse_enabled == false`    → `[mouse:susp]`

No stored field — computed at render time from the two booleans above.

# Research: In-Session Mouse Capture Toggle

**Feature**: 041-mouse-capture-toggle | **Date**: 2026-06-15

All Technical Context items are resolved (no NEEDS CLARIFICATION remained after
`/speckit-clarify`). This file records the design decisions and the existing-code
findings that shaped the plan.

## R-001: Where the runtime capture flag already lives

- **Decision**: Reuse the existing `UiState.mouse_enabled: bool`
  (`crates/cargonaut-ui-tui/src/lib.rs:101`) as the single runtime capture
  state. Do **not** add a parallel flag.
- **Rationale**: `mouse_enabled` is already (a) initialized from
  `config.ui.mouse` in `run()` (lib.rs:60), (b) honored by `handle_mouse`
  via an early-return when false (lib.rs:784), and (c) read by `run_external`
  to restore capture after shelling out (lib.rs:316/750). One flag = no
  desync risk (SC-002), and FR-007 (preserve state across external programs)
  is already satisfied by the existing `run_external(_, _, ui.mouse_enabled)`
  call — no new work needed for that FR beyond a regression test.
- **Alternatives considered**: A second "captured" flag distinct from
  "enabled" — rejected as redundant; the existing flag already means exactly
  "currently capturing".

## R-002: Distinguishing "session mouse disabled" from "currently suspended"

- **Decision**: Treat `app.config().ui.mouse` as the immutable **session
  support** signal and `ui.mouse_enabled` as the **runtime capture** signal.
  The toggle is a no-op (with an explanatory status message) when
  `config.ui.mouse == false` (FR-006); otherwise it flips `mouse_enabled`.
- **Rationale**: `--no-mouse`/`ui.mouse=false` merge into `config.ui.mouse`
  in `main.rs` (main.rs:91-93), so `config.ui.mouse` is the authoritative
  session-level switch. Reading it in `dispatch_ui_command` (which already
  has `&mut App`) avoids adding session-level state to `UiState`.
- **Alternatives considered**: Storing a separate `mouse_supported` on
  `UiState` — rejected; the config is already reachable and is the single
  source of truth for the session setting.

## R-003: Testable seam for the toggle decision

- **Decision**: Factor the decision into a pure function
  `plan_mouse_toggle(supported: bool, currently: bool) -> MouseToggleOutcome`
  where `MouseToggleOutcome ∈ { Disabled, EnabledNow, SuspendedNow }`, each
  carrying the user-facing status string. `dispatch_ui_command` calls it and
  performs the thin `execute!(EnableMouseCapture|DisableMouseCapture)` + status
  assignment based on the outcome.
- **Rationale**: Constitution II (Test-First) requires a CI-gated test per FR.
  The `execute!` macro writes control sequences to the real terminal, which is
  awkward to assert on. A pure decision function makes FR-002/003/006 logic
  unit-testable with zero I/O, leaving only a 3-line I/O wiring shim untested
  (acceptable — it is a direct passthrough mirroring `run()`'s existing calls).
- **Alternatives considered**: (a) a `pending_mouse_resync` flag drained by
  `run_loop` (like `pending_external`) — more moving parts than needed for a
  synchronous flip; (b) asserting on captured stdout bytes — brittle and
  terminal-dependent. Pure function is the simplest testable design.

## R-004: Key binding choice — `M-m`

- **Decision**: Bind `M-m` (Alt-m) in `mode = "global"`.
- **Rationale**: Verified unbound in `design/contracts/keymap.toml` (M-m / M-S-m
  / C-x m all free). Mnemonic ("mouse"), single keystroke, consistent with the
  Alt-letter convention used by other UI toggles (`M-c` quick-cd, `M-!` filter,
  `M-t`). `global` mode so it works whether a pane or preview has focus, like
  `F12`/`F10`. Does not collide with the `--mc-keys` orthodox map (which adds
  function-key/Ctrl bindings, not `M-m`).
- **Alternatives considered**: `C-x m` chord (slower, less discoverable);
  `M-S-m` (more awkward, reserves plain `M-m` for nothing in particular).
  Chosen `M-m` per clarification.

## R-005: Surfacing capture state (transient + persistent)

- **Decision**: (a) Transient — set the run-loop `status` string on each toggle
  ("Mouse capture: on" / "Mouse capture: suspended — Shift+drag to select text").
  (b) Persistent — render a short indicator (e.g. `🖱on`/`🖱off` rendered as
  ASCII `[mouse:on]`/`[mouse:off]` to stay within the no-Unicode-escape spirit)
  in the **right side of the top menu-bar row**, which currently renders only
  left-aligned menu titles and has empty space on the right (chrome.rs:348-367).
- **Rationale**: Matches the existing transient-feedback pattern (status line
  already shows "Chord: …", "Returned from …"). The menu-bar row is always
  visible, already themed, and has free horizontal space, so the persistent
  indicator needs no new layout rows (no NFR-001/002 cost). Typed theme colors
  (`theme.menu_fg/bg`) keep Constitution III (no hardcoded ANSI).
- **Alternatives considered**: A new dedicated status row (wastes a line);
  putting it in the function-key bar (already dense with F1–F10 labels). The
  menu-bar right gutter is the lowest-impact home.

## R-006: Hold-modifier (Shift) bypass — documentation only

- **Decision**: Document the Shift-drag terminal bypass in the in-app help
  overlay and in user docs (README/quickstart); do not implement terminal
  behavior.
- **Rationale**: Shift-to-bypass-mouse-reporting is provided by the terminal
  emulator (xterm, GNOME Terminal, kitty, etc.), not the application. FR-010
  is a documentation requirement (SC-006). The help overlay already has a
  "Mouse:" line (lib.rs:1256) to extend.
- **Alternatives considered**: None — implementing terminal-side selection is
  out of scope and impossible from the app.

## R-007: Clean teardown on exit

- **Decision**: No change needed. `run()` already issues an unconditional
  `DisableMouseCapture` on teardown (lib.rs:73) regardless of the runtime flag,
  satisfying FR-008. Add a regression test note rather than new code.
- **Rationale**: Teardown is best-effort and state-independent by design.

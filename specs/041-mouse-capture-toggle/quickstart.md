# Quickstart / Validation: In-Session Mouse Capture Toggle

**Feature**: 041-mouse-capture-toggle | **Date**: 2026-06-15

How to validate the feature end-to-end. Implementation details live in
[plan.md](./plan.md), [data-model.md](./data-model.md), and
[contracts/mouse-toggle-seam.md](./contracts/mouse-toggle-seam.md).

## Prerequisites

- tmpfs target active: `make tmpfs-status` (set up with `make tmpfs-setup` if not).
- A terminal emulator that supports mouse reporting and a Shift-drag bypass
  (xterm, GNOME Terminal, kitty, iTerm2, etc.).

## Automated validation

```bash
# Full local CI (fmt, clippy -D warnings, test, release build, docs gate)
make ci-local

# Or just this feature's tests
cargo test -p cargonaut-ui-tui mouse
cargo test -p cargonaut-ui-tui keymap
```

Expected: the new tests pass —
- `plan_mouse_toggle` truth table (Disabled / EnabledNow / SuspendedNow),
- `M-m` resolves to `Command::ToggleMouseCapture` in global mode and collides
  with nothing,
- the menu-bar indicator renders `[mouse:on]` / `[mouse:susp]` / `[mouse:off]`,
- the help overlay mentions `M-m` and `Shift`.

## Manual validation (the SC walkthrough)

1. **Default capture (baseline)** — launch with mouse on:
   ```bash
   cargo run -p cargonaut-bin
   ```
   Click a pane row: focus follows the click. Indicator shows `[mouse:on]`.

2. **Suspend (SC-001, FR-002)** — press **Alt-m**. Status line shows
   "Mouse capture: suspended — Shift+drag to select text"; indicator shows
   `[mouse:susp]`. Now click-drag selects text with your *terminal's* native
   selection (the app no longer consumes the drag).

3. **Resume (FR-003)** — press **Alt-m** again. Status line shows
   "Mouse capture: on"; indicator `[mouse:on]`; click-to-focus works again.

4. **Round-trip integrity (SC-002)** — toggle several times quickly; behavior
   matches the indicator every time (no stuck state).

5. **External program preserves state (FR-007)** — suspend with Alt-m, then
   press **F3** (pager) / **F4** (editor) on a file and exit the external tool.
   Back in Cargonaut, capture is still suspended (`[mouse:susp]`), not silently
   re-enabled.

6. **Disabled session (FR-006, SC-004)** — relaunch with mouse off:
   ```bash
   cargo run -p cargonaut-bin -- --no-mouse
   ```
   Press **Alt-m**: status shows "Mouse support disabled for this session
   (--no-mouse / ui.mouse=false)"; indicator stays `[mouse:off]`; mouse is never
   captured.

7. **Clean exit (FR-008, SC-005)** — after any toggle sequence, quit (**F10**).
   Your terminal's mouse + text selection behave normally (capture released).

8. **Docs (FR-010, SC-006)** — open help (**F1**); the mouse line documents the
   Alt-m toggle and the Shift-drag bypass.

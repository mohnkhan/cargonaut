# Quickstart: Quick-CD Popup with Tab-Completion

**Feature**: 038-quick-cd-popup

## What it does

Press **Alt-c** in a file pane to open an inline "quick cd" prompt. Type a
directory path, press **Tab** to complete it against the current pane's
directories and your recently-visited dirs, and press **Enter** to jump there.
**Esc** cancels.

## Try it (manual)

```bash
make build            # tmpfs-guarded debug build
./target/debug/cargonaut   # or: make run
```

1. In either pane, press **Alt-c**. A prompt opens, prefilled with the pane's
   current directory, cursor at the end.
2. Clear it (or edit the tail) and type a partial subdirectory name, e.g. `sr`.
3. Press **Tab** → it completes to `…/src` (or cycles `src`, `scripts`, … if
   several match — Tab again to advance, wrapping at the end).
4. Press **Enter** → the active pane navigates there. The pane you left is now in
   that pane's back-history (Alt-← / history-prev returns to it).
5. Re-open with **Alt-c**, type a path that doesn't exist, press **Enter** →
   an inline error appears and the prompt stays open so you can fix it.
6. Press **Esc** any time → the prompt closes and nothing changed.

## Try it (automated — the SC-006 gate)

Core behavior runs without a terminal:

```bash
cargo test -p cargonaut-core quick_cd       # completion + navigation logic
cargo test -p cargonaut-ui-tui path_input   # widget key-handling + render
```

Expected: open→type→complete→accept changes the active pane's cwd; open→cancel
leaves both panes untouched; accept of a bad path returns an error and does not
navigate.

## Where it lives

- Logic: `crates/cargonaut-core/src/lib.rs` — `App::complete_cd`, `App::quick_cd`
  (routes through `navigate_to`).
- Widget: `crates/cargonaut-ui-tui/src/dialog.rs` — `PathInputDialog`
  (shared; also intended for #32 tasks panel and #33 filter prompt).
- Wiring: `crates/cargonaut-ui-tui/src/lib.rs` — `ActiveDialog::QuickCd`,
  `dispatch_ui_command`, `handle_key` dialog branch, render arm.
- Binding: `design/contracts/keymap.toml` — `M-c → quick-cd-popup` (already
  present; unchanged).

## Notes / limits (this feature)

- Completion is prefix-based on the final path segment (no fuzzy match).
- Completion runs against the active pane's local filesystem backend.
- Quick-cd input is not remembered across restarts.
- The tasks panel (#32) and filter prompt (#33) reuse `PathInputDialog` but are
  not implemented here.

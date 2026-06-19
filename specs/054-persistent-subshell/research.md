# Research: Feature 054 — Persistent Subshell (Ctrl-o)

**Date**: 2026-06-19
**Branch**: `054-persistent-subshell`

---

## R-001 PTY crate selection

**Decision**: Use `portable-pty = "0.8"` (already in `[workspace.dependencies]`).

**Rationale**: Already present in workspace; used in `cargonaut-bin`. Provides `native_pty_system()`, `openpty(PtySize)`, `spawn_command()`, `try_clone_reader()` (blocking `Read`), `take_writer()` (blocking `Write`), and `master.resize(PtySize)` for SIGWINCH delivery. No additional workspace change needed; only `cargonaut-ui-tui/Cargo.toml` needs `portable-pty = { workspace = true }` added.

**Alternatives considered**:
- `pty-process` — simpler API but fewer platforms; portable-pty is already present.
- Raw `nix::pty::openpty` — too low-level; nix is already in workspace but for unrelated user/group resolution.

---

## R-002 VT100 / ANSI terminal-emulator state machine

**Decision**: Add `vt100 = "0.16"` as a workspace dependency; use it in `cargonaut-ui-tui`.

**Rationale**: `vt100::Parser::new(rows, cols, scrollback)` creates a stateful VT100 emulator. `parser.process(&bytes)` feeds raw PTY bytes and updates the internal screen state. `parser.screen()` exposes the current cell grid (`cell(row, col) -> Option<Cell>`), cursor position, hide-cursor flag, and alternate-screen flag. `parser.screen_mut().set_size(rows, cols)` resizes the virtual terminal. This is the canonical Rust VT100 library and handles all ANSI, SGR, cursor-addressing escape codes needed for `vim`, `htop`, `less`, `top`.

**Alternatives considered**:
- `alacritty_terminal` — battle-tested, but pulls in a large transitive dependency tree; binary size risk.
- `termwiz` — WezTerm's library; very capable but also heavyweight.
- Custom VT100 parser — `design/contracts/requirements.toml` mentions "own VT100" for Phase 4; for this Phase-1 subshell panel, the `vt100` crate gives correct semantics without premature complexity.

---

## R-003 Terminal rendering widget

**Decision**: Add `tui-term = "0.3"` as a direct dependency in `cargonaut-ui-tui`.

**Rationale**: `tui_term::widget::PseudoTerminal::new(parser.screen())` produces a ratatui-compatible widget that maps `vt100::Cell` → `ratatui::style::Style` + `Buffer` symbols correctly, including wide characters, inverse video, and all SGR attributes. Saves ~50 lines of manual mapping and is actively maintained for ratatui 0.27+.

**Manual rendering fallback** (if `tui-term` is rejected for binary-size reasons): iterate `screen.cell(row, col)` and write to `Buffer` directly using `map_color(vt100::Color) -> ratatui::style::Color`. The mapping is ~25 lines; cursor rendering is 10 more lines. The fallback avoids the `tui-term` dependency at the cost of duplicating what `tui-term` already does.

Binary size estimate: `vt100` ≈ 120 KB, `tui-term` ≈ 60 KB compiled. Total new addition ≈ 180 KB, well within the 8 MiB NFR-001 budget (current binary ~2.72 MiB leaving ~5.28 MiB headroom).

---

## R-004 Async PTY read pattern

**Decision**: `tokio::task::spawn_blocking` + `tokio::sync::mpsc::channel::<Vec<u8>>(64)`.

**Rationale**: `portable-pty`'s `try_clone_reader()` returns a blocking `Box<dyn Read + Send>`. The blocking thread sends byte chunks via `pty_tx.blocking_send(...)` to a `pty_rx` receiver polled in the `run_loop`'s `tokio::select!` alongside crossterm key events. The `vt100::Parser` lives on the main task (no `Arc<Mutex>` needed), fed by the receive arm.

Alternative (lower latency): `AsyncFd` on the raw master fd with `O_NONBLOCK` set via `fcntl`. Avoids an OS thread but requires unsafe-adjacent `from_raw_fd` plumbing. Not worth the complexity for a panel that renders at 60 Hz — `spawn_blocking` is sufficient.

---

## R-005 Three-state Ctrl-o focus model

**Decision**: `SubshellPhase` enum with three variants on `UiState`.

```
SubshellPhase::Hidden
SubshellPhase::VisibleFmFocus
SubshellPhase::VisibleShellFocus
```

Each `OpenSubshell` command dispatch advances the phase:
- `Hidden` → `VisibleFmFocus` (show panel; FM retains input)
- `VisibleFmFocus` → `VisibleShellFocus` (transfer input to PTY)
- `VisibleShellFocus` → `Hidden` (hide panel; FM retains input; shell process lives)

A mouse click inside the subshell rect jumps to `VisibleShellFocus` regardless of current state (if panel is visible). A mouse click outside the subshell rect while in `VisibleShellFocus` moves to `VisibleFmFocus`.

---

## R-006 Layout integration strategy

**Decision**: Extend `FrameLayout` with `subshell: Option<Rect>` and teach `draw_frame` to insert a subshell region in the vertical constraint list when the phase is not `Hidden`.

Current vertical split: `[Length(1), Min(3), Length(1), Length(1)]` → menu, panes, status, fkeys.

When subshell visible, split becomes: `[Length(1), Min(3), Length(subshell_rows), Length(1), Length(1)]` → menu, panes, subshell, status, fkeys.

`subshell_rows` = `max(3, (content_height * pct) / 100)` where `pct = config.ui.subshell_height_pct` (default 33).

The `subshell` field of `FrameLayout` is set to `Some(rect)` when the panel is visible; used for hit-testing mouse clicks.

`draw_frame` receives `subshell_phase: SubshellPhase` and `subshell_screen: Option<&vt100::Screen>` (or the `SubshellState` reference) as new parameters. Rendering the `PseudoTerminal` widget happens inside `draw_frame` using the pre-computed subshell area.

---

## R-007 cwd-sync mechanism

**Decision**: Send `cd <path>\r` (carriage return, not newline) to the PTY writer on every qualifying event.

**Events that trigger sync** (from FR-007, FR-008):
1. Panel directory navigation (enter dir, cd-popup, bookmark, quick-cd, `..` row)
2. Panel focus swap (Tab / M-1 / M-2)
3. Tab-switch within active side (`[` / `]`) — Feature 053 clarification

Shell-quoting: Use `shell_words::quote()` (already in workspace via `shell-words` crate) on the path before sending to avoid breakage on paths with spaces or special characters. The command sent is `cd <shell-quoted-path>\r`.

This is a fire-and-forget write to `SubshellState::writer`. If the shell is busy (running a command), the `cd` goes into the PTY input buffer and executes after the current command finishes — acceptable behavior.

If the subshell phase is `Hidden`, cwd-sync still fires (FR-007: "MUST occur regardless of whether the subshell panel is currently visible"). The shell process is alive and will execute the `cd` immediately.

---

## R-008 PTY resize handling

**Decision**: On `crossterm::event::Event::Resize(cols, rows)`, compute the new subshell panel dimensions and call both:
1. `subshell.parser.screen_mut().set_size(new_rows, new_cols)` — update vt100 state
2. `subshell.master.resize(PtySize { rows: new_rows, cols: new_cols, pixel_width: 0, pixel_height: 0 })` — send SIGWINCH to child

The subshell is only resized when it is visible (`SubshellPhase != Hidden`). When hidden, the PTY is not resized (the shell's concept of terminal size becomes stale, but that's acceptable — it's hidden). On becoming visible again, an immediate resize is applied using the current dimensions.

---

## R-009 Crossterm KeyCode → PTY byte mapping

The `handle_key` function, when `SubshellPhase::VisibleShellFocus`, routes all key events to `subshell.write_key(key_event)`. The mapping:

| crossterm KeyCode | PTY bytes |
|---|---|
| `Char(c)` | UTF-8 bytes of `c`; apply Ctrl modifier with `0x1f & byte` |
| `Enter` | `\r` |
| `Backspace` | `\x7f` |
| `Delete` | `\x1b[3~` |
| `Up` | `\x1b[A` (or `\x1bOA` if `screen.application_cursor()`) |
| `Down` | `\x1b[B` (or `\x1bOB`) |
| `Left` | `\x1b[D` (or `\x1bOD`) |
| `Right` | `\x1b[C` (or `\x1bOC`) |
| `Home` | `\x1b[1~` |
| `End` | `\x1b[4~` |
| `PageUp` | `\x1b[5~` |
| `PageDown` | `\x1b[6~` |
| `F(n)` | `\x1b[11~` through `\x1b[24~` |
| `Esc` | `\x1b` |
| `Tab` | `\x09` |
| `BackTab` | `\x1b[Z` |

`Ctrl-o` specifically (`Char('o')` + `CONTROL`) is intercepted BEFORE the PTY write and advances the phase to `Hidden` instead.

---

## R-010 Shell exit detection

**Decision**: The `spawn_blocking` reader task detects EOF (0-byte read) and sends a sentinel `Vec::<u8>::new()` (empty vec) over the channel. The `run_loop` receive arm checks `bytes.is_empty()` and, if so, sets `SubshellState::dead = true` and transitions the phase to `Hidden` (if `VisibleShellFocus`) or leaves it wherever it is (displaying "Shell exited" in the panel).

The next `Ctrl-o` (advancing from `Hidden`) spawns a fresh PTY + shell process by calling `SubshellState::respawn(cwd)`.

---

## R-011 Minimum terminal size guard

**Decision**: If `content_height < 8` (fewer than 8 rows for the pane + subshell combined), the `VisibleFmFocus` advance is blocked: the command is silently discarded and a status-bar message "Terminal too small for subshell" is set. The threshold of 8 rows gives 5 rows to the pane region and 3 rows to the subshell (the minimum enforced by FR-013).

---

## R-012 Shell launch

**Decision**: Lazy spawn — `SubshellState` is `None` in `UiState` at startup. The first advance from `Hidden` (first `Ctrl-o`) spawns the PTY + shell process and stores the `SubshellState` in `UiState`. Subsequent toggles reuse the existing state. This avoids creating a shell process for users who never press `Ctrl-o`.

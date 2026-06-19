# Data Model: Feature 054 — Persistent Subshell (Ctrl-o)

**Date**: 2026-06-19

---

## Enums

### `SubshellPhase` (in `cargonaut-ui-tui`)

The three-state Ctrl-o cycle state, owned by `UiState`.

```
SubshellPhase::Hidden
  — Subshell panel is not visible. Shell process may or may not exist yet (lazy).
  — Ctrl-o advances to VisibleFmFocus.
  — Input routing: file manager.

SubshellPhase::VisibleFmFocus
  — Subshell panel is visible; keyboard focus is in the file manager.
  — Ctrl-o advances to VisibleShellFocus.
  — Input routing: file manager.
  — Mouse click in subshell rect → jump to VisibleShellFocus.

SubshellPhase::VisibleShellFocus
  — Subshell panel is visible; keyboard focus is in the subshell.
  — Ctrl-o advances to Hidden.
  — Input routing: PTY writer (all keys except Ctrl-o forwarded to shell).
  — Mouse click outside subshell rect → move to VisibleFmFocus.
```

Transitions:
```
Hidden ──(Ctrl-o)──→ VisibleFmFocus
VisibleFmFocus ──(Ctrl-o)──→ VisibleShellFocus
VisibleShellFocus ──(Ctrl-o)──→ Hidden
VisibleFmFocus ──(mouse click in shell rect)──→ VisibleShellFocus
VisibleShellFocus ──(mouse click outside shell rect)──→ VisibleFmFocus
```

Invariant: advancing from `Hidden` when `subshell` is `None` triggers lazy spawn.

---

## Structs

### `SubshellState` (in `cargonaut-ui-tui/src/subshell.rs`)

Owns the PTY and all associated runtime state for the subshell panel.

| Field | Type | Description |
|---|---|---|
| `master` | `Box<dyn MasterPty + Send>` | PTY master fd; call `master.resize(PtySize)` on terminal resize |
| `writer` | `Box<dyn Write + Send>` | Write end of PTY (keystrokes, cwd-sync `cd` commands) |
| `parser` | `vt100::Parser` | ANSI/VT100 state machine; `parser.process(&bytes)` on PTY output |
| `pty_rx` | `tokio::sync::mpsc::Receiver<Vec<u8>>` | Receives PTY output bytes from the `spawn_blocking` reader task |
| `scroll_offset` | `u16` | How many rows the user has scrolled up in scrollback (0 = latest output) |
| `dead` | `bool` | True when the shell process has exited; panel shows restart notice |
| `current_size` | `PtySize` | Last-known PTY size (rows/cols); used to detect size drift on show |

Behavior:
- `SubshellState::spawn(cwd: &Path, rows: u16, cols: u16) -> Result<SubshellState>`: creates PTY pair, spawns `$SHELL` (or `/bin/sh`), starts `spawn_blocking` reader task.
- `SubshellState::write_key(key: KeyEvent)`: maps crossterm `KeyCode` + modifiers to PTY bytes and writes to `writer`.
- `SubshellState::sync_cwd(path: &Path)`: shell-quotes path via `shell_words::quote()`, writes `cd <quoted>\r` to `writer`. No-op if `dead`.
- `SubshellState::resize(rows: u16, cols: u16)`: calls `master.resize()` and `parser.screen_mut().set_size()`. Sends `sync_cwd(current_cwd)` to force shell prompt redraw in the new size.
- `SubshellState::poll_output(&mut self)`: drains all pending bytes from `pty_rx` (non-blocking; uses `try_recv`) and feeds each chunk to `parser.process()`. Called once per `run_loop` iteration.
- `SubshellState::screen(&self) -> &vt100::Screen`: returns `parser.screen()` for rendering.
- `SubshellState::respawn(cwd: &Path, rows: u16, cols: u16) -> Result<()>`: drops old PTY, spawns fresh shell; resets `dead`, `scroll_offset`.

---

### `FrameLayout` extension (in `cargonaut-ui-tui/src/chrome.rs`)

Add one field to the existing struct:

| New Field | Type | Description |
|---|---|---|
| `subshell` | `Option<Rect>` | The screen rect allocated for the subshell panel; `None` when `SubshellPhase::Hidden` |

Layout algorithm when subshell is visible:
```
Vertical constraints:
  Length(1)            ← menu bar
  Min(5)               ← pane band (always ≥ 5 rows)
  Length(subshell_rows) ← subshell panel
  Length(1)            ← status line
  Length(1)            ← function-key bar

subshell_rows = max(3, (available_content_rows * subshell_height_pct) / 100)
```

---

### `UiConfig` extension (in `cargonaut-config/src/lib.rs`)

Add one field:

| New Field | Type | Default | Valid Range | Description |
|---|---|---|---|---|
| `subshell_height_pct` | `u8` | `33` | `10..=60` | Percentage of content-area rows allocated to the subshell panel |

Config file key: `ui.subshell_height_pct`. Values outside 10–60 are clamped at load time (a tracing warning is emitted).

---

### `PtyMessage` (internal, conceptual)

The type sent over the `mpsc` channel from the blocking reader task to `run_loop`:
- Non-empty `Vec<u8>`: PTY output bytes to feed into `parser.process()`.
- Empty `Vec<u8>`: sentinel indicating the shell process has exited (EOF on master fd).

This is not a named struct; it's just `Vec<u8>`. The receiver checks `bytes.is_empty()` to detect exit.

---

## State Invariants

1. `SubshellPhase::Hidden` ⟹ `UiState.subshell` may be `None` (not yet spawned) or `Some(_)` (live process in background).
2. `SubshellPhase::VisibleFmFocus | VisibleShellFocus` ⟹ `UiState.subshell` is always `Some(_)`.
3. `SubshellState.dead == true` ⟹ panel shows restart notice; no key writes to `writer`.
4. `SubshellState.parser.screen().size()` is always consistent with `SubshellState.current_size`.
5. `scroll_offset == 0` ⟹ latest output is showing (default).

---

## Keymap additions (design/contracts/keymap.toml)

The `subshell` mode binding already exists in the keymap file:
```toml
[[binding]]
mode = "pane"
key = "C-o"
action = "open-subshell"
```

The `subshell` mode entry needs one binding added:
```toml
[[binding]]
mode = "subshell"
key = "C-o"
action = "open-subshell"   # advances from VisibleShellFocus → Hidden
```

The `subshell` mode exists in `keymap.rs` (`Mode::Subshell`) but has no bindings today. The `open-subshell` action in subshell mode is the exit-focus trigger.

---

## New Files

| Path | Purpose |
|---|---|
| `crates/cargonaut-ui-tui/src/subshell.rs` | `SubshellState`, `SubshellPhase`, key-to-PTY mapping, async reader task, cwd-sync |

## Modified Files

| Path | Changes |
|---|---|
| `crates/cargonaut-ui-tui/src/lib.rs` | `UiState`: add `subshell: Option<SubshellState>`, `subshell_phase: SubshellPhase`; `run_loop`: poll PTY output, route keys, fire cwd-sync; `draw_frame`: new `subshell` params + render widget |
| `crates/cargonaut-ui-tui/src/chrome.rs` | `FrameLayout`: add `subshell: Option<Rect>`; layout logic for subshell band |
| `crates/cargonaut-ui-tui/Cargo.toml` | Add `portable-pty`, `vt100`, `tui-term` |
| `crates/cargonaut-config/src/lib.rs` | `UiConfig`: add `subshell_height_pct: u8` with default + clamping |
| `Cargo.toml` (workspace) | Add `vt100 = "0.16"` to `[workspace.dependencies]` |
| `design/contracts/keymap.toml` | Add `subshell` mode binding for `C-o` → `open-subshell` |
| `design/contracts/requirements.toml` | Add FR-054-001 through FR-054-015 entries |

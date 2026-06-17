# Data Model: PTY Binary-Level Navigation Smoke Test

**Feature**: 045-pty-nav-smoke-test | **Date**: 2026-06-17

This feature adds no production data models. The entities below are
test-infrastructure types that live exclusively in `crates/cargonaut-bin/tests/`.

---

## TempDirFixture

A prepared pair of temporary directories used as the binary's left and right
pane arguments.

| Field | Type | Description |
|-------|------|-------------|
| `left` | `tempfile::TempDir` | Left pane root — contains `aaa/`, `bbb/`, `ccc/` subdirectories |
| `right` | `tempfile::TempDir` | Right pane root — empty (tests only assert against the left pane) |

**Invariants**:
- `aaa`, `bbb`, `ccc` are directories (not files), so they serve as descent
  targets for `nav_descend_enter` and `nav_ascend_backspace` without needing a
  separate fixture.
- Sorted alphabetically, they appear in that order in the pane listing (after the
  `..` parent row at index 0).
- Created via `tempfile::tempdir()` and cleaned up automatically on drop.

---

## PtyHandle

An active cargonaut process running under a PTY. Defined in `tests/common/mod.rs`.

| Field | Type | Description |
|-------|------|-------------|
| `child` | `Box<dyn portable_pty::Child + Send + Sync>` | The spawned process; holds the process ID and `wait()` handle |
| `writer` | `Box<dyn Write + Send>` | PTY master writer — key sequences are injected here |
| `sink` | `Arc<Mutex<Vec<u8>>>` | Accumulated raw PTY output; drained by a background thread |

**Lifecycle**: Created by `spawn(exe, left, right)`. Cleaned up by sending F10
(`\x1b[21~`) to request a clean exit, then calling `child.wait()`. If the binary
does not exit within the deadline, `sigkill(pid)` is called.

---

## DeltaSnapshot

A lightweight cursor into the PTY output buffer, used by `delta_contains` to
isolate only bytes written since the last action.

| Field | Type | Description |
|-------|------|-------------|
| `prev_len` | `usize` | Buffer length at the moment the snapshot was taken |

**Usage**: Call `sink.lock().unwrap().len()` before injecting a key to capture
`prev_len`. Pass `prev_len` to `delta_contains(sink, prev_len, needle)` to
assert that new bytes (bytes after `prev_len`) contain `needle`.

---

## Key Sequences (constants)

Defined as `&[u8]` constants in `tests/common/mod.rs`.

| Constant | Bytes | Action |
|----------|-------|--------|
| `KEY_DOWN` | `b"\x1b[B"` | Cursor down (crossterm ANSI sequence) |
| `KEY_UP` | `b"\x1b[A"` | Cursor up |
| `KEY_ENTER` | `b"\r"` | Enter / descend-or-open |
| `KEY_BACKSPACE` | `b"\x7f"` | Backspace / ascend-parent |
| `KEY_F10` | `b"\x1b[21~"` | Quit |

These match the crossterm sequences exercised by the existing `resume_sigkill`
PTY test and the crossterm key mappings in `cargonaut-ui-tui/src/lib.rs`.

---

## Observable Signals

The test assertions scan raw PTY output for these text strings.

| Signal | Observable text | When present |
|--------|----------------|--------------|
| TUI ready | `"Quit"` | Appears in function-key bar on every rendered frame |
| Cursor on entry X | `"  X"` (entry name at end of mini-status line, preceded by permissions + size + mtime) | After the cursor moves to entry X; detected via delta-buffer |
| CWD changed to subdir | The subdirectory name as a path segment in the pane title | After a successful descend |
| CWD returned to parent | Parent directory name in the pane title | After a successful ascend |

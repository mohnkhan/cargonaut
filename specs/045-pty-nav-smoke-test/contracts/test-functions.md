# Test Function Contracts: PTY Navigation Smoke Test

**Feature**: 045-pty-nav-smoke-test | **Date**: 2026-06-17

These contracts describe the behavioural obligations of the three test functions
and the shared helper module. They are the acceptance-test interface — any
implementation that satisfies all contracts passes the feature.

---

## Shared Helper: `tests/common/mod.rs`

### `fn spawn(exe: &str, left: &Path, right: &Path) -> PtyHandle`

| Obligation | Detail |
|------------|--------|
| Launches the binary in a real PTY | Uses `portable_pty::native_pty_system()`, `PtySize { rows: 40, cols: 120, .. }` |
| Passes both directory paths as arguments | `cmd.arg(left)`, `cmd.arg(right)` |
| Sets `TERM=xterm-256color` | Ensures crossterm renders in colour mode |
| Drains PTY output to sink | A background thread appends all bytes to the `Arc<Mutex<Vec<u8>>>` |
| Returns `PtyHandle` | `(child, writer, sink)` |

### `fn wait_until(deadline: Duration, cond: impl Fn() -> bool) -> bool`

Polls `cond` every 50 ms until it returns `true` or `deadline` elapses.
Returns `true` if the condition was satisfied before the deadline, `false` otherwise.

### `fn output_contains(sink: &Arc<Mutex<Vec<u8>>>, needle: &str) -> bool`

Returns `true` if the entire accumulated PTY output (UTF-8 lossy) contains `needle`.

### `fn delta_contains(sink: &Arc<Mutex<Vec<u8>>>, prev_len: usize, needle: &str) -> bool`

Returns `true` if the bytes at indices `prev_len..` of the accumulated output
(UTF-8 lossy) contain `needle`. Used to isolate assertions to bytes written after
a specific action.

### `fn sigkill(pid: u32)`

Sends SIGKILL to the given process ID. Used as a last-resort cleanup if the binary
does not exit cleanly within the test deadline.

### Constants

`KEY_DOWN`, `KEY_UP`, `KEY_ENTER`, `KEY_BACKSPACE`, `KEY_F10` — see data-model.md.

---

## Test Function: `nav_cursor_arrow_keys`

**Location**: `crates/cargonaut-bin/tests/local_navigation.rs`
**Gate**: self-skips when `CARGONAUT_PTY_TESTS != "1"`
**Platform**: `#[cfg(unix)]`

### Obligations

| Step | Obligation |
|------|------------|
| Setup | Create a `TempDirFixture` with `aaa/`, `bbb/`, `ccc/` in `left` |
| Spawn | Call `spawn(exe, left.path(), right.path())` |
| Wait for TUI ready | `wait_until(5s, \|\| output_contains(&sink, "Quit"))` — assert `true` |
| Snapshot before first key | `prev = sink.lock().unwrap().len()` |
| Send Down | Write `KEY_DOWN` to writer, flush |
| Assert cursor on `aaa` | `wait_until(5s, \|\| delta_contains(&sink, prev, "aaa"))` — assert `true` |
| Snapshot, send Down again | As above |
| Assert cursor on `bbb` | `wait_until(5s, \|\| delta_contains(&sink, prev2, "bbb"))` — assert `true` |
| Snapshot, send Up | As above |
| Assert cursor back on `aaa` | `wait_until(5s, \|\| delta_contains(&sink, prev3, "aaa"))` — assert `true` |
| Quit | Send `KEY_F10`, await `child.wait()` within 5s or `sigkill(pid)` |

---

## Test Function: `nav_descend_enter`

**Location**: `crates/cargonaut-bin/tests/local_navigation.rs`
**Gate**: self-skips when `CARGONAUT_PTY_TESTS != "1"`
**Platform**: `#[cfg(unix)]`

### Obligations

| Step | Obligation |
|------|------------|
| Setup | Create a `TempDirFixture` with `aaa/`, `bbb/`, `ccc/` in `left` |
| Spawn | Call `spawn(exe, left.path(), right.path())` |
| Wait for TUI ready | `wait_until(5s, \|\| output_contains(&sink, "Quit"))` — assert `true` |
| Navigate to `bbb` | Send `KEY_DOWN` twice (past `..` to `aaa`, then to `bbb`) with short flush delays |
| Snapshot | `prev = sink.lock().unwrap().len()` |
| Send Enter | Write `KEY_ENTER`, flush |
| Assert CWD changed | `wait_until(5s, \|\| delta_contains(&sink, prev, "bbb"))` — assert `true` (pane title now contains the path to `bbb/`) |
| Quit | Send `KEY_F10`, await clean exit or kill |

---

## Test Function: `nav_ascend_backspace`

**Location**: `crates/cargonaut-bin/tests/local_navigation.rs`
**Gate**: self-skips when `CARGONAUT_PTY_TESTS != "1"`
**Platform**: `#[cfg(unix)]`

### Obligations

| Step | Obligation |
|------|------------|
| Setup | Create a `TempDirFixture` with `aaa/`, `bbb/`, `ccc/` in `left`; record `left_name` (last path component of `left.path()`) |
| Spawn | Call `spawn(exe, left.path(), right.path())` |
| Descend into `aaa` | Wait for TUI ready; send `KEY_DOWN` (cursor to `aaa`), then `KEY_ENTER`; wait until `delta_contains("aaa")` |
| Snapshot | `prev = sink.lock().unwrap().len()` |
| Send Backspace | Write `KEY_BACKSPACE`, flush |
| Assert CWD returned | `wait_until(5s, \|\| delta_contains(&sink, prev, &left_name))` — assert `true` (pane title returns to the parent path) |
| Quit | Send `KEY_F10`, await clean exit or kill |

---

## Non-Regression Contract

`cargo test -- --ignored` (no env var) MUST report **zero** ignored tests in the
`cargonaut-bin` crate. The presence of any `#[ignore]` on the three functions is
a contract violation.

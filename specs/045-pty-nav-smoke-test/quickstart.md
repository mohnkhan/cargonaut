# Quickstart: PTY Navigation Smoke Test

**Feature**: 045-pty-nav-smoke-test | **Date**: 2026-06-17

## Prerequisites

- Rust toolchain matching the workspace MSRV (1.76+)
- Linux or macOS (PTY tests are `#[cfg(unix)]`)
- `make tmpfs-setup` run once for this checkout (SSD preservation)
- `cargonaut` binary built: `cargo build --bin cargonaut` (or `make build`)

## Running locally

### Full test suite (fast, PTY tests skipped)

```sh
cargo test --workspace
```

All three navigation tests self-skip and are silent. `cargo test -- --ignored`
shows them explicitly as ignored.

### PTY tests enabled

```sh
CARGONAUT_PTY_TESTS=1 cargo test --workspace --tests 2>&1 | grep -E "nav_|resume"
```

Expected output (all passing):

```
test nav_cursor_arrow_keys ... ok
test nav_descend_enter ... ok
test nav_ascend_backspace ... ok
test resume_sigkill_smoke ... ok
```

### Running a single navigation test

```sh
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_cursor_arrow_keys -- --nocapture
```

### Verifying no ignored tests remain

```sh
cargo test -p cargonaut -- --ignored 2>&1 | grep -c "ignored"
# Expected: 0
```

## CI behaviour

CI sets `CARGONAUT_PTY_TESTS=1` in the env block of the `unit-test` job
(`.github/workflows/ci.yml`). All four PTY tests (three navigation + one
resume) run automatically. No configuration change is needed.

## Validating the assertion strategy

To manually confirm the observable signals work before running the full tests:

```sh
# 1. Build the binary
cargo build --bin cargonaut

# 2. Create a quick fixture
mkdir -p /tmp/nav-test/{aaa,bbb,ccc} /tmp/nav-right

# 3. Launch in a real terminal to observe rendered output
./target/debug/cargonaut /tmp/nav-test /tmp/nav-right

# 4. Press Down to navigate; observe that the mini-status line at the bottom
#    of the left pane shows the focused entry name (aaa → bbb → ccc)
# 5. Press Enter on a subdirectory; observe pane title changes to that dir
# 6. Press Backspace; observe pane title returns to /tmp/nav-test
# 7. Press F10 to quit
```

## What a failing test looks like

If an assertion deadline expires, the test panics with a message such as:

```
thread 'nav_cursor_arrow_keys' panicked at 'cursor did not reach aaa within 5s'
```

Common failure causes:
- Binary not built (stale `target/`): run `cargo build --bin cargonaut`
- TUI startup took longer than 5 s on the runner: increase the startup deadline
- Entry names not sorted as expected: check that `aaa`, `bbb`, `ccc` are created
  as directories (not files) and that no hidden entries sort before them

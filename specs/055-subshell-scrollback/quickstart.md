# Quickstart Validation Guide: Subshell Scrollback Rendering (Feature 055)

## Prerequisites

- `make tmpfs-status` shows `target/` is a tmpfs symlink
- Cargonaut builds cleanly: `make build`

## Automated Tests

```bash
# Run the subshell-specific tests (includes new scrollback test)
cargo test -p cargonaut-ui-tui subshell -- --nocapture

# Full workspace
make test
```

Expected: all tests pass including `render_vt100_screen_scrollback_offset_changes_content`.

## Manual Validation

### Setup

```bash
make run   # or: cargo run --bin cargonaut
```

### SC-001 / SC-002: Scroll through subshell output

1. Press `Ctrl-o` to open the subshell panel (cycles: Hidden → VisibleFmFocus → VisibleShellFocus).
2. Press `Ctrl-o` again if needed to reach VisibleShellFocus (shell has keyboard input).
3. Run a command that produces many lines:
   ```
   seq 1 100
   ```
4. The output fills the panel. Scroll the mouse wheel **UP** inside the panel.
5. **Expected**: Earlier lines scroll into view. Line numbers decrease as you scroll further up.
6. Scroll mouse wheel **DOWN** inside the panel.
7. **Expected**: View returns toward the live terminal bottom. Fully scrolling down shows the prompt.

### SC-005: Scroll past maximum history

1. With the panel open, scroll UP many times past the bottom of the scrollback buffer.
2. **Expected**: View stops at the oldest available line; no crash or garbled output.

### Regression check

1. Verify `Ctrl-o` panel cycling still works (Hidden → Visible → ShellFocus → Hidden).
2. Verify file manager panes still respond to keyboard and mouse normally when subshell is not in shell-focus mode.
3. Verify the cursor is **not** displayed in the subshell panel when scrolled into history (no reversed-video block at wrong position).

## CI Gate

```bash
make ci-local
```

Expected: all steps pass (clippy, test, build, check-pr-body, docs-gate).

# Quickstart Validation Guide: F2 User-Menu Mouse-Click Support

**Feature**: 048-f2-mouse-click  
**Date**: 2026-06-18

## Prerequisites

- Rust toolchain matching workspace `rust-toolchain.toml`
- `make tmpfs-status` confirms `target/` is a tmpfs symlink (run `make tmpfs-setup` if not)

## Validation Scenarios

### VS-1: New integration test passes (SC-001)

```bash
# Run only the TUI crate's tests; filter to mouse tests
make test 2>&1 | grep -E "test.*mouse|PASSED|FAILED|ok|FAILED"
# OR
cargo test -p cargonaut-ui-tui f2_mouse_click_opens_user_menu -- --nocapture
```

**Expected outcome**: `f2_mouse_click_opens_user_menu ... ok`

### VS-2: Full test suite still green (SC-003)

```bash
make ci-local
```

**Expected outcome**: All five pipeline steps pass — `clippy`, `cargo test --workspace`,
`cargo build --release`, `check-pr-body`, `docs-gate`.

### VS-3: Keyboard path still opens UserMenu (SC-002 — regression guard)

The keyboard path is covered by the existing `user_menu_opens_on_f2` or equivalent test
in the TUI crate. Confirm it still passes:

```bash
cargo test -p cargonaut-ui-tui user_menu -- --nocapture
```

**Expected outcome**: All matching tests pass; no ActiveDialog mismatch.

### VS-4: Manually confirm the routing (optional)

If you want to observe the behavior interactively:

```bash
cargo run --bin cargonaut -- /tmp /tmp
```

- Click the on-screen `F2 User menu` button in the function-key bar.
- Confirm the user-menu dialog appears (it will show "No menu actions available…" since no `menu.toml` is present).
- Press Esc to close. Verify the underlying pane state is unchanged.

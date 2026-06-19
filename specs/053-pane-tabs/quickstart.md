# Quickstart: Pane Tabs Validation (Feature 053)

## Prerequisites

- `make tmpfs-setup` completed (SSD preservation)
- Rust toolchain installed
- Working directory: `/home/main/MyOS-2026/cargonaut`

## Running the test suite

```bash
make test
# or directly:
cargo test --workspace
```

## Key validation scenarios

### Scenario 1: Tab creation and switching (US1 core)

**Test name**: `tab_new_opens_in_same_cwd` (cargonaut-core integration tests)

```bash
cargo test -p cargonaut-core tab_new
```

Expected: new tab has same `cwd`, clean `filter=None`, `selected` empty, `cursor` at default.

---

### Scenario 2: Tab close — successor and renumbering (FR-002)

**Test names**: `tab_close_noop_on_single_tab`, `tab_close_selects_right_successor`, `tab_close_wraps_to_last_when_rightmost`

```bash
cargo test -p cargonaut-core tab_close
```

Expected:
- Single-tab: no state change, no panic
- Close tab 1 of [0,1,2]: active becomes index 1 (former tab 2)
- Close tab 2 of [0,1,2]: active becomes index 1 (wraps to new last)

---

### Scenario 3: State isolation (US3 / FR-006)

**Test name**: `tab_state_is_independent`

```bash
cargo test -p cargonaut-core tab_state
```

Expected: setting a filter on tab 0 does not affect tab 1; switching tabs shows each tab's own state.

---

### Scenario 4: Cross-pane ops use active tab (US2 / FR-007)

**Test name**: `cross_pane_copy_targets_active_tab`

```bash
cargo test -p cargonaut-core cross_pane
```

Expected: `confirm_copy` sources from `active_tab.cwd` on the focused side, destinations to `active_tab.cwd` on the other side.

---

### Scenario 5: Tab bar view model (FR-004 / FR-012)

**Test name**: `tab_bar_view_correct_labels_and_active_marker`

```bash
cargo test -p cargonaut-core tab_bar_view
```

Expected: returns one `TabBarEntry` per tab; `is_active` set only for the current tab; labels are basename-truncated.

---

### Scenario 6: Tab bar rendering (TUI)

**Test name**: `draw_pane_renders_tab_bar_row` (cargonaut-ui-tui tests)

```bash
cargo test -p cargonaut-ui-tui tab_bar
```

Expected: the tab bar row appears above the list block in the rendered buffer; active tab has distinct style.

---

### Scenario 7: Existing features unchanged (US3 / FR-008)

```bash
cargo test --workspace
```

All existing tests pass without modification. No regressions in filter, sort, viewer, find-file, compare-dirs, bulk-rename, or file-ops.

---

### Scenario 8: Clippy + fmt clean

```bash
make clippy
cargo fmt --check
```

Expected: zero warnings, zero formatting issues.

---

### Scenario 9: Coverage gate

```bash
make ci-local
```

The CI pipeline runs tarpaulin and enforces ≥80% line coverage on `cargonaut-core`. New tab-ops code must be covered by new tests.

---

## Manual TUI validation (for `find` feature correctness)

1. `cargo run --release -- /tmp /tmp` (or `make run`)
2. Press `Ctrl-t` — verify a tab bar appears with `[1*]tmp  [2]tmp`
3. Navigate left pane to a different dir; press `Ctrl-t` again — tab bar shows `[1]tmp  [2]new  [3*]new`
4. Press `[` — tab bar shows active on `[2]`
5. Press `Ctrl-w` on the active tab — tab disappears, successor becomes active
6. Press `Ctrl-w` when one tab remains — nothing changes (no crash)
7. Press `F3` to open viewer; while viewer is open, press `[` — viewer stays, tab does NOT change
8. Copy a file with `F5`; verify the destination is the right pane's active tab directory

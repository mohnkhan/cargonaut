# Quickstart / Validation: Click-on-dropdown-item support

**Feature**: 065-menu-dropdown-mouse-click | **Date**: 2026-06-22

How to prove the feature works end-to-end. Build artifacts go to tmpfs — use the `make`
wrappers (Constitution §V); do not run `cargo clean`.

## Prerequisites

```bash
make tmpfs-status   # confirm target/ is the tmpfs symlink; run `make tmpfs-setup` if not
```

## Automated validation (authoritative)

```bash
# Unit tests for the new MenuBar geometry/selection methods
cargo test -p cargonaut-ui-tui menu_bar_item_hit_test
cargo test -p cargonaut-ui-tui menu_bar_select
cargo test -p cargonaut-ui-tui menu_hover

# Integration tests for the handle_mouse wiring
cargo test -p cargonaut-ui-tui T-MENU-MOUSE

# Full gate (mirrors CI): fmt → clippy -D warnings → test → release build → doc/size checks
make ci-local
```

Expected: all listed tests pass; `make ci-local` is green. Each FR maps to at least one test
(see contracts/menubar-mouse.md).

## Manual validation (in a real terminal)

```bash
make build
./target/release/cargonaut    # mouse capture on by default
```

| # | Action | Expected (FR) |
|---|--------|----------------|
| 1 | Click the "File" title, then click the "Mkdir" row | Mkdir runs; menu closes (FR-001) |
| 2 | Open a menu, click the very first item row | First item's command runs, not the second (FR-002) |
| 3 | Open a menu, click the dropdown's border line | Nothing happens; menu stays open (FR-003) |
| 4 | Open a menu, click a row in the opposite file panel | Menu closes AND that pane focuses with the cursor on the clicked row (FR-004 pass-through) |
| 5 | Open "File", then click the "Options" title | File closes, Options opens (FR-005) |
| 6 | Click the title of the already-open menu | Menu closes (FR-006) |
| 7 | Open a menu, move the pointer down the items without clicking | Highlight follows the pointer; nothing runs (FR-007) |
| 8 | Move the pointer onto the border / off the items | Highlight does not change (FR-008) |
| 9 | Press `Alt-m` to suspend mouse, then try 1–8 | No menu mouse effect; keyboard (F9, arrows, Enter, Esc) still works (FR-009, FR-011) |
| 10 | Restart with `--no-mouse`, repeat | Same as #9 — mouse inert, keyboard unchanged (FR-009) |

> Terminals that do not report pointer motion: step 7/8 simply do nothing while clicks
> (1–6) still work — that is the intended graceful degradation (FR-010).

## Success criteria mapping

| SC | Validated by |
|----|--------------|
| SC-001 mouse-only operation | Manual #1, #5; `T-MENU-MOUSE` click-dispatch test |
| SC-002 every item invokable, no dead row | Manual #2, #3; `menu_bar_item_hit_test` (first/last/border) |
| SC-003 dismiss + switch by mouse | Manual #4, #5, #6; close/switch integration tests |
| SC-004 hover matches pointer / degrades | Manual #7, #8; `menu_hover_*` tests |
| SC-005 disabled = pre-feature behavior | Manual #9, #10; disabled-mouse harness test |

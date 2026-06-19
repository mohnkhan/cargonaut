# Quickstart: Find-File and Panelize — Validation Guide

**Feature**: 052-find-file-panelize | **Date**: 2026-06-19

This guide describes the runnable validation scenarios that prove the feature works end-to-end. It is not an implementation guide — see `tasks.md` for task breakdown.

---

## Prerequisites

```bash
make tmpfs-status          # Confirm target/ → tmpfs (Constitution §V)
make build                 # Clean baseline build
make test                  # All existing tests pass
which rg                   # Confirm ripgrep on PATH for content-search scenarios
```

---

## Scenario 1 — US1: Filename glob search and panelize

**Goal**: Find all `*.rs` files under `crates/`, panelize, confirm bulk ops work.

**Steps**:

1. Launch: `cargo run -p cargonaut-bin -- crates/`
2. Navigate to any subdirectory (e.g., `cargonaut-ui-tui/src/`).
3. Press `Alt-?` → find-file dialog opens; input field focused; count reads `0 matches`.
4. Type `*.rs` → press `Enter`.
5. Walk starts; result list populates incrementally; count increases live.
6. Walk completes; count shows `N matches`; result list is scrollable (arrow keys, PgDn).
7. Press `Enter` on the result list → dialog closes; active panel shows flat synthetic listing.
8. Status bar shows `[Find: *.rs]`.
9. Press `Space` on a file → file is tagged (marked with `*`).
10. Press `F5` (copy) → copy dialog opens with the tagged file ready.
11. Press `Esc` to cancel the copy → return to panelized listing.

**Expected**: Steps 3–11 complete without errors. Panel lists only `*.rs` files.

---

## Scenario 2 — US2: Content search via ripgrep

**Goal**: Find files containing `TODO` and panelize them.

**Steps**:

1. Launch: `cargo run -p cargonaut-bin -- .`
2. Press `Alt-?` → dialog opens in Name mode.
3. Press `Tab` → mode switches to `Content`; input hint updates.
4. Type `TODO` → press `Enter`.
5. Result list shows file paths (one per file, not line numbers).
6. Press `Enter` → panel shows panelized content-search results.

**Expected**: Results match `rg TODO --files-with-matches .` output (same set of files).

---

## Scenario 3 — US3: Cancel in-progress walk

**Goal**: Verify `Esc` aborts the walk and leaves panel unchanged.

**Steps**:

1. Launch: `cargo run -p cargonaut-bin -- /`  (large directory tree)
2. Press `Alt-?` → dialog opens.
3. Type `*` → press `Enter` → walk starts (count climbs).
4. Press `Esc` within 2 seconds.
5. Dialog closes; active panel shows its previous listing (unchanged).
6. App is fully responsive to keystrokes immediately after.

**Expected**: Walk stops within ≤300 ms of `Esc`; no stale results in memory; no crash.

---

## Scenario 4 — Empty result set

**Goal**: Verify no-results path keeps dialog open with a notice.

**Steps**:

1. Press `Alt-?` → dialog opens.
2. Type `__nonexistent_pattern_xyzzy__` → press `Enter`.
3. Walk completes; count reads `0 matches`.
4. Notice reads "No files found matching `__nonexistent_pattern_xyzzy__`".
5. Dialog stays open; input field is focused for retry.
6. Clear input, type `*.toml` → press `Enter` → results appear normally.

**Expected**: Step 4 notice shown; step 5 dialog remains; step 6 works from same dialog open.

---

## Scenario 5 — ripgrep unavailable

**Goal**: Verify graceful degradation when `rg` is not found.

**Setup**: Temporarily set `config.search.ripgrep_path = "rg_does_not_exist"` or remove `rg` from PATH.

**Steps**:

1. Launch with rg unavailable.
2. Press `Alt-?` → dialog opens; Name mode active.
3. Press `Tab` → mode does NOT switch; notice "Content search unavailable: rg not found" shown.
4. Type `*.toml` → press `Enter` → name-search works normally.

**Expected**: No crash; Name mode fully functional; Content tab visibly dimmed.

---

## Scenario 6 — Result truncation at max_results

**Goal**: Verify cap at `config.search.max_results` (5000).

**Setup**: Set `search.max_results = 10` in config (test only); search a large directory.

**Steps**:

1. Launch with `max_results = 10`.
2. Press `Alt-?` → type `*` → `Enter`.
3. Walk stops at 10 results; header reads `10 matches (truncated)`.
4. `Enter` to panelize → 10 files shown.

**Expected**: Exactly 10 entries in the panel; truncated notice in header.

---

## Scenario 7 — Navigation from panelized listing

**Goal**: Verify returning to real directory navigation clears the find label.

**Steps**:

1. Panelize any search result (any of scenarios 1–2).
2. Status bar shows `[Find: <pattern>]`.
3. Press `Enter` on a directory entry in the panelized list (if any).
4. Panel loads real directory; status bar reverts to directory path.
5. Press Backspace / parent-dir navigation.
6. Returns to the real parent directory, NOT to the panelized listing.

**Expected**: `[Find: …]` label absent after any real navigation; synthetic listing is not in history stack.

---

## Automated test verification

```bash
make test              # all unit + integration tests pass
make ci-local          # full pipeline: fmt, clippy, test, build, docs-gate
```

Key test modules to check:
- `crates/cargonaut-ui-tui/src/dialog.rs` — `FindFileDialog` unit tests (phase transitions, enter/esc/tab)
- `crates/cargonaut-ui-tui/src/keymap.rs` — `M-?` → `FindFilePopup` binding test
- `crates/cargonaut-ui-tui/src/lib.rs` — panelize integration test (temp dir fixture)

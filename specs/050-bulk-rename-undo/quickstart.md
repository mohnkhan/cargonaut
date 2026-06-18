# Quickstart Validation Guide: Bulk Rename + Undo

**Feature**: 050-bulk-rename-undo | **Date**: 2026-06-18

## Prerequisites

- `EDITOR` environment variable set (e.g. `export EDITOR=nano` or `export EDITOR=vim`)
- A local directory with several files
- Cargonaut built: `make build`
- tmpfs symlink active: `make tmpfs-status` (dev host only)

## Scenario 1 — Bulk Rename via Editor (US1 happy path)

**Setup**:
```sh
tmpdir=$(mktemp -d)
touch "$tmpdir/file_a.txt" "$tmpdir/file_b.txt" "$tmpdir/file_c.txt"
```

**Steps**:
1. Launch: `cargo run -- "$tmpdir"`
2. Navigate to the directory with the three files
3. Tag all three with `Insert` three times
4. Press `C-x r` (Ctrl-x then r)
5. The TUI suspends; editor opens with three lines: `file_a.txt`, `file_b.txt`, `file_c.txt`
6. Change `file_a.txt` → `renamed_a.txt` and `file_c.txt` → `renamed_c.txt`. Leave `file_b.txt` unchanged.
7. Save and exit the editor
8. TUI resumes; panel shows `renamed_a.txt`, `file_b.txt`, `renamed_c.txt`

**Expected status message**: `"2 entries renamed"`

**Verify**:
```sh
ls "$tmpdir"   # should show: file_b.txt  renamed_a.txt  renamed_c.txt
```

## Scenario 2 — No-op Bulk Rename (all names unchanged)

**Setup**: same as Scenario 1 (3 tagged files)

**Steps**: Open editor, save without changing any name, exit.

**Expected**: Status `"No changes — nothing renamed"`. No filesystem changes.

## Scenario 3 — Validation Failure (duplicate name)

**Setup**: 2 tagged files: `alpha.txt`, `beta.txt`

**Steps**: In editor, change both lines to `same.txt`. Save and exit.

**Expected**: Status `"Duplicate name 'same.txt' on lines 1 and 2"`. No renames applied. Both files still exist with original names.

## Scenario 4 — Validation Failure (line count mismatch)

**Setup**: 3 tagged files

**Steps**: In editor, delete one line entirely. Save and exit.

**Expected**: Status `"Line count changed: expected 3, got 2 — do not add or delete lines"`. No renames applied.

## Scenario 5 — No Tagged Files

**Steps**: Press `C-x r` with nothing tagged.

**Expected**: Status `"Tag at least one entry to bulk rename"`. No editor opens.

## Scenario 6 — Undo Rename (US2)

**Setup**: Perform Scenario 1 (two files renamed successfully).

**Steps**: Press `C-z`.

**Expected**:
- `renamed_a.txt` → `file_a.txt`
- `file_b.txt` → unchanged
- `renamed_c.txt` → `file_c.txt`
- Status: `"Undo: 2 renames reversed"`

**Verify**:
```sh
ls "$tmpdir"   # should show: file_a.txt  file_b.txt  file_c.txt
```

## Scenario 7 — Undo Copy (US2)

**Setup**: Two local panes. Tag `file_a.txt` in left pane. Press `F5` (copy), confirm.

**Steps**: Press `C-z`.

**Expected**: Right pane's `file_a.txt` copy is deleted. Status: `"Undo: 1 copy removed"`.

## Scenario 8 — Undo is Single-Level

**Setup**: Perform Scenario 6 (undo rename).

**Steps**: Press `C-z` again.

**Expected**: Status `"Nothing to undo"`. No filesystem changes.

## Scenario 9 — Undo Delete (not reversible)

**Setup**: Tag a file, press F8 (delete), confirm.

**Steps**: Press `C-z`.

**Expected**: Status `"Undo: delete cannot be reversed — no in-session recovery available"`. No filesystem changes.

## Running Unit Tests

```sh
# All tests
make test

# Core unit tests only (fast)
cargo test -p cargonaut-core -- validate_rename bulk_rename undo_last

# UI tests only
cargo test -p cargonaut-ui-tui -- queue_bulk_rename
```

## Running Benchmarks (SC-001, SC-004)

```sh
cargo bench -p cargonaut-core -- bulk_rename_50 undo_rename_50
```

Expected outputs per spec:
- SC-001: bulk rename of 50 files ≤ 500 ms
- SC-004: undo of 50-file rename ≤ 500 ms

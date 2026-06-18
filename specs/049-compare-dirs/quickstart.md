# Quickstart Validation Guide: Compare Directories + Diff Tagged Files (Feature 049)

**Date**: 2026-06-18  
**Branch**: `049-compare-dirs`

## Prerequisites

- Cargonaut built: `make build`
- tmpfs symlink active: `make tmpfs-status` (see CLAUDE.md §V)
- Two directories prepared for testing (see setup below)

## Test Setup

```bash
# Create two scratch dirs with known content
mkdir -p /tmp/cn-test/{left,right}

# Identical file (should NOT be marked)
echo "same content" > /tmp/cn-test/left/same.txt
echo "same content" > /tmp/cn-test/right/same.txt

# Size-different file (should be marked on both sides)
echo "hello" > /tmp/cn-test/left/size-diff.txt
echo "hello world" > /tmp/cn-test/right/size-diff.txt

# Content-different (same size, different content — should be marked)
echo "aaa" > /tmp/cn-test/left/hash-diff.txt
echo "bbb" > /tmp/cn-test/right/hash-diff.txt

# Left-only file (should mark left side only)
echo "left only" > /tmp/cn-test/left/left-only.txt

# Right-only file (should mark right side only)
echo "right only" > /tmp/cn-test/right/right-only.txt
```

## Scenario 1: Compare directories (US1 P1)

1. Launch: `cargo run -- /tmp/cn-test/left` (left panel opens at left dir)
2. Navigate right panel to `/tmp/cn-test/right`
3. Press `C-x d` (compare-directories)
4. **Expected**: `same.txt` is NOT highlighted on either side; `size-diff.txt`, `hash-diff.txt` highlighted on both sides; `left-only.txt` highlighted on left; `right-only.txt` highlighted on right.
5. **Status bar**: "N entries differ" (N = 4)

## Scenario 2: Identical directories (US1 acceptance scenario 2)

1. Navigate both panels to the same directory (e.g., both at `/tmp/cn-test/left`)
2. Press `C-x d`
3. **Expected**: No entries highlighted; status bar: "Both panels point to the same directory — compare would mark nothing"

## Scenario 3: Additive tagging (US1 acceptance scenario 3)

1. In either panel, manually tag `same.txt` (press `Insert`)
2. Run compare (`C-x d`)
3. **Expected**: `same.txt` remains tagged (compare does not clear it); differing files are additionally tagged.

## Scenario 4: Diff two tagged files (US2 P2)

1. Configure diff tool (one-time):
   ```toml
   # ~/.config/cargonaut/config.toml
   [diff]
   tool = "diff -u"
   ```
2. Launch cargonaut with two dirs
3. Tag `hash-diff.txt` in the left panel (Insert key)
4. Tab to right panel, tag `hash-diff.txt` in right panel
5. Press `C-x C-d` (diff-two-tagged-files)
6. **Expected**: TUI suspends; `diff -u /tmp/cn-test/left/hash-diff.txt /tmp/cn-test/right/hash-diff.txt` output visible in terminal; pressing Enter (or Ctrl-d) returns to TUI cleanly.

## Scenario 5: Diff error paths

| Setup | Action | Expected |
|---|---|---|
| No `[diff]` tool configured | `C-x C-d` | Status: "No diff tool configured…" |
| Only 1 file tagged | `C-x C-d` | Status: "Diff requires exactly 2 tagged files (1 tagged)" |
| 3 files tagged | `C-x C-d` | Status: "Diff requires exactly 2 tagged files (3 tagged)" |
| Diff tool binary not on PATH | `C-x C-d` | Status: "Failed to launch diff tool: …" |

## Automated test commands

```bash
# Unit tests (core compare logic + config)
cargo test -p cargonaut-core compare
cargo test -p cargonaut-config diff

# TUI integration tests (feature 049 — suspend/resume)
cargo test -p cargonaut-ui-tui diff

# Full workspace
cargo test --workspace

# SC-001 bench (compare ≤2 s for 1000 entries)
cargo bench --bench compare-dirs

# Clippy (must be clean)
cargo clippy --workspace --all-targets -- -D warnings
```

## Cleanup

```bash
rm -rf /tmp/cn-test
```

## References

- Data model: [data-model.md](data-model.md)
- Config contract: [contracts/config-diff.md](contracts/config-diff.md)
- Spec: [spec.md](spec.md)
- Success criteria: SC-001, SC-002, SC-003, SC-004, SC-005, SC-006

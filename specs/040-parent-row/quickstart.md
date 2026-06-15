# Quickstart / Validation: Panel `..` Parent Entry as First Row

**Feature**: 040-parent-row

## Prerequisites

- Branch `040-parent-row`. tmpfs target active (`make tmpfs-status`; else `make
  tmpfs-setup`) — Constitution §V.

## Build & test

```bash
make ci-local                 # clippy -D warnings → test → release → docs-gate
# iterating:
make test
cargo test -p cargonaut-core parent          # cursor-model unit tests
cargo test -p cargonaut-ui-tui parent        # PaneView render + mouse tests
```

Expected: all green, including the new `..`-row tests and the updated
index-coupled tests.

## Automated validation scenarios (authoritative — see contracts for names)

1. **SC-001 — ascend via the row**: cursor up onto `..`, `Descend` → cwd is the
   parent; also double-click row 0 → cwd is the parent.
2. **SC-002 — root suppression**: at a filesystem root, `has_parent()` is false,
   no `..` row renders, ascent is a no-op.
3. **SC-003 — never operable**: toggle selection on `..` → nothing selected;
   invert / select-by-pattern (incl. a pattern matching `..`) → `..` excluded;
   copy/move/delete set never contains `..`.
4. **SC-004 — always present**: a non-root dir with a zero-matching filter, and
   after toggling hidden files, still renders `..` as row 0.
5. **SC-005 — selection stable**: the same real entry is selected before and
   after the change for the same key actions (no off-by-one).
6. **FR-014 — default cursor**: entering a non-root dir focuses the first real
   entry (not `..`); an empty non-root dir focuses `..`.

## Manual smoke (optional)

```bash
make build && ./target/debug/cargonaut <dirA> <dirB>
```
1. Descend into a subdirectory → a `..` row sits at the top; the cursor is on the
   first real entry.
2. Press Up → cursor moves onto `..`; press Up again → it stays on `..`.
3. Press Enter on `..` → the pane goes up to the parent.
4. Double-click the `..` row → the pane goes up.
5. Tag a few files (Insert/Space), then move to `..` and try to tag it → nothing
   happens; `..` is never marked.
6. Navigate to `/` → no `..` row; you cannot go higher.

## References

- Behavior: [spec.md](./spec.md) (FR-001…FR-014, SC-001…SC-006)
- Decisions: [research.md](./research.md)
- Model: [data-model.md](./data-model.md)
- API: [contracts/pane-cursor-model.md](./contracts/pane-cursor-model.md)

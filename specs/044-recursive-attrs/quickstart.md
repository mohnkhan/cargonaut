# Quickstart / Validation: Recursive chmod / chown

**Feature**: 044-recursive-attrs | **Date**: 2026-06-17

How to validate end-to-end. Design: [plan.md](./plan.md),
[data-model.md](./data-model.md),
[contracts/recursive-attrs-seam.md](./contracts/recursive-attrs-seam.md).

## Prerequisites

- tmpfs target active: `make tmpfs-status` (`make tmpfs-setup` if not).

## Automated validation

```bash
make ci-local                              # fmt, clippy -D warnings, test, release, docs-gate
# focused:
cargo test -p cargonaut-core recursive     # collect_subtree + chmod/chown_recursive
cargo test -p cargonaut-ui-tui recursive   # C-x C / C-x O dispatch + confirm chain
```

Expected new tests pass: depth apply, symbolic-per-entry, deepest-first (no
lock-out), symlink-not-followed, partial-failure aggregation, truncation,
file-only = shallow, invalid input rejected, and the UI confirm-chain.

## Manual validation (the SC walkthrough)

```bash
mkdir -p /tmp/rtest/a/b/c && printf x > /tmp/rtest/a/b/c/deep.txt
cargo run -p cargonaut-bin -- /tmp/rtest /tmp
```

1. **Recursive chmod (US1, SC-001)** — highlight `a`, press **`C-x C`**, enter
   `700`, **confirm** → `a/b/c/deep.txt` is now `0700` (check with `ls -l`).
2. **Symbolic recursive (FR-003)** — `C-x C` on `a`, enter `g-rwx`, confirm →
   each entry loses group bits relative to its own mode.
3. **Confirm gating (SC-003)** — `C-x C`, enter a mode, but **Cancel** the
   confirmation → nothing changes.
4. **Deepest-first / no lock-out (FR-011)** — `C-x C` on `a`, enter `0`, confirm
   → even the deepest entries are changed (the walk wasn't locked out by the
   parent losing `x`).
5. **Symlink not followed (SC-005)** — `ln -s /tmp/outside /tmp/rtest/a/link`
   (with `/tmp/outside` populated); recursive chmod `a` → `/tmp/outside` is
   unchanged (the link wasn't descended).
6. **Partial failure (SC-006)** — make one deep entry unwritable by another owner
   (or run unprivileged against a root-owned entry); recursive chown/chmod →
   status reports the failure, other entries still changed, no crash.
7. **Recursive chown (US2, SC-002)** — highlight `a`, **`C-x O`**, enter a group
   you belong to, confirm → a nested entry's group changes.
8. **Menu + shallow unchanged (FR-008)** — **F9 → File** lists "Chmod -R" /
   "Chown -R"; the shallow `C-x c` / `C-x o` still apply only to the selected
   entry.
9. **Help** — **F1** mentions the recursive keys (`C-x C` / `C-x O`).
```

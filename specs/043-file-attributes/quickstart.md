# Quickstart / Validation: File Attribute Operations

**Feature**: 043-file-attributes | **Date**: 2026-06-17

How to validate end-to-end. Design: [plan.md](./plan.md),
[data-model.md](./data-model.md),
[contracts/attr-ops-seam.md](./contracts/attr-ops-seam.md).

## Prerequisites

- tmpfs target active: `make tmpfs-status` (`make tmpfs-setup` if not).

## Automated validation

```bash
make ci-local                          # fmt, clippy -D warnings, test, release, docs-gate
# focused:
cargo test -p cargonaut-vfs mode       # ModeSpec parse/apply truth table (SC-004)
cargo test -p cargonaut-vfs chmod      # + chown/symlink/hard_link LocalFs ops
cargo test -p cargonaut-core attr      # chmod_selection / links / partial-failure
cargo test -p cargonaut-ui-tui chmod   # dispatch + dialog wiring
```

## Manual validation (the SC walkthrough)

```bash
cargo run -p cargonaut-bin -- ~ /tmp
```

1. **chmod octal (US1, SC-001)** — highlight a file showing `rw-r--r--`, press
   **`C-x c`** (or File → Chmod), the prompt is prefilled with `644`; type `755`,
   Enter → the perms column now reads `rwxr-xr-x`.
2. **chmod symbolic (US1)** — on a file, `C-x c`, enter `u+x` → only the owner
   execute bit is added relative to the current mode.
3. **chmod multi-file (SC-003)** — tag several files (Insert), `C-x c`, enter one
   mode → all tagged files change in one action.
4. **Invalid mode (SC-004)** — `C-x c`, enter `xyz` or `999` → inline error, no
   file changes; Esc closes with no change.
5. **Symlink (US2, SC-002)** — highlight `file.txt`, press **`C-x s`** (File →
   Symlink), accept/enter a link name → a symlink to `file.txt` appears and
   resolves. Try an existing name → refused, nothing overwritten.
6. **Hardlink (US2)** — highlight a file, **`C-x l`**, enter a name → a second
   hard link appears. Try linking a directory → reported failure, no crash.
7. **chown (US3, SC-006)** — highlight a file, **`C-x o`** (File → Chown), enter
   a group you belong to (e.g. your own), confirm in the dialog → ownership
   updates; entering an **unknown** user/group → error, no change; on a file you
   don't own → "permission denied", no crash (SC-005).
8. **Menu + cancel (SC-007)** — open **F9 → File**: Chmod/Chown/Symlink/Hardlink
   are listed and invoke the same flows; Esc on any dialog leaves state unchanged.
9. **Help** — **F1** mentions the attribute keys (`C-x c` chmod, …).
```

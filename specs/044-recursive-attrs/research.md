# Research: Recursive chmod / chown into Subtrees

**Feature**: 044-recursive-attrs | **Date**: 2026-06-17

All Technical Context items resolved (no NEEDS CLARIFICATION after
`/speckit-clarify`). Decisions + existing-code findings.

## R-001: No new VFS methods — recursion is a core-level walk over existing ops

- **Decision**: Reuse the per-path `VfsBackend::chmod`/`chown` shipped in Feature
  043. Recursion is a `cargonaut-core` walk that enumerates the subtree and calls
  the existing op per entry. No `cargonaut-vfs` changes.
- **Rationale**: The atomic operation (change one path) already exists and is
  tested; recursion is an orchestration concern (which paths, what order, error
  aggregation) that belongs in core next to `chmod_selection`/`chown_selection`.
- **Alternatives considered**: a `chmod_recursive` VFS method — rejected; pushes
  traversal policy (cap, symlink rule, order) into the backend where it doesn't
  belong and would have to be reimplemented per backend.

## R-002: Bounded BFS collector, descending only into `VfsKind::Dir`

- **Decision**: A `collect_subtree(roots) -> (Vec<VfsPath>, truncated)` helper
  BFS-walks from each selected directory using `local_fs.list`, pushing only
  `VfsKind::Dir` children onto the stack, capped at `NODE_CAP` (reuse the
  `recursive_dir_size` value, 200_000). Returns every entry path (roots +
  descendants) and whether the cap truncated the walk.
- **Rationale**: `list` uses `symlink_metadata`, so a symlink-to-directory has
  kind `VfsKind::Symlink` (a distinct variant), **not** `Dir`. Matching only
  `Dir` for descent therefore never follows links out of the subtree (FR-006)
  with no extra code, and prevents symlink cycles. The cap satisfies FR-005/
  SC-004 (no unbounded walk). This mirrors the proven `recursive_dir_size` walk.
- **Alternatives considered**: following symlinks with a visited-set cycle guard
  — rejected; tree-escape + complexity for no user benefit (orthodox FMs don't
  follow links in recursive attribute ops).

## R-003: Apply deepest-first (collect-then-apply) to avoid lock-out (FR-011)

- **Decision**: Collect the full path list first, then apply the change in
  **reverse BFS order** (descendants before their ancestors).
- **Rationale**: chmod/chown of `/a/b/c` needs traverse (`x`) on `/a` and `/a/b`.
  A top-down apply that strips `x` from `/a` first would make `/a/b/c`
  unreachable. BFS collects ancestors before descendants; applying in reverse
  means a directory's permissions are still original (traversable — we just
  listed it) when we change everything beneath it, and the directory itself is
  changed last. This is the standard safe ordering for restrictive recursive
  chmod.
- **Alternatives considered**: top-down apply (the naïve `chmod -R` order) —
  rejected; self-locks on restrictive modes. Applying without collecting first —
  same lock-out risk mid-walk.

## R-004: Trigger = dedicated chords `C-x C` / `C-x O` (clarified)

- **Decision**: `C-x C` (recursive chmod) and `C-x O` (recursive chown), plus
  File-menu entries, parallel to the shallow `C-x c` / `C-x o`. Both are free and
  case-sensitive (like the existing `C-x X`). Each reuses the mode/owner
  `TextInputDialog`, then chains a `ConfirmDialog` (FR-002); Cancel aborts.
- **Rationale**: A dedicated trigger makes the recursion intent unambiguous and
  keeps Cancel meaning "abort" (intuitive), versus overloading the shallow
  command's confirm to mean recurse-vs-shallow. Matches the project's
  one-command-per-operation style and the existing `C-x`-family convention.
- **Alternatives considered**: a recurse-or-shallow confirm on `C-x c`/`C-x o`
  (Cancel = shallow is confusing); a `-R` flag in the input (poor
  discoverability). Both rejected in clarification.

## R-005: Confirm chain reuses Feature 043's `ConfirmDialog → core Command` seam

- **Decision**: Add core `Command::ChmodRecursive(String)` /
  `ChownRecursive(String)`; the recursive input's submit opens a `ConfirmDialog`
  whose `on_confirm` is the matching core command, which `dispatch` routes to
  `App::chmod_recursive`/`chown_recursive`.
- **Rationale**: Feature 043 already routes chown through
  `ConfirmDialog{on_confirm: Command::Chown(String)}`; reusing that seam for the
  two recursive ops needs no new dialog machinery — just two more core commands
  and two `InputKind` variants.
- **Alternatives considered**: a bespoke recursive-confirm dialog — unnecessary.

## R-006: Recursive chmod applies the literal mode to every entry (`chmod -R`)

- **Decision**: The same `ModeSpec` is applied to every entry — octal absolutely,
  symbolic relative to each entry's current bits (the existing `apply`). No
  special directory handling.
- **Rationale**: Matches universal `chmod -R` semantics and is predictable; a
  user who wants directory-smart execute bits can use the symbolic `X` form (a
  possible future enhancement). Keeps scope tight.
- **Alternatives considered**: auto dir-vs-file modes (like `a+rX`) — out of
  scope; documented as a future option, not silently applied.

## R-007: Parse before walk; reuse `attr_status` for aggregation

- **Decision**: Parse the mode/owner up front (`ModeSpec::parse` / `parse_owner`,
  `BadAttr` on invalid) before any traversal — invalid input never starts a walk.
  Per-entry failures accumulate into the existing `attr_status("chmod -R", ok,
  &failures)` status; a truncated walk appends a "(truncated)" note.
- **Rationale**: Reuses Feature 043's validation gate and status formatting; fail
  fast on bad input; partial failures surfaced without rollback (FR-007/SC-006).

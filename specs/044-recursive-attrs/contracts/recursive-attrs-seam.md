# Contract: Recursive Attribute Operations Seam

**Feature**: 044-recursive-attrs | **Date**: 2026-06-17

Interfaces this feature exposes and the invariants tests pin down. Builds on the
Feature 043 seam (`ModeSpec`, `parse_owner`, per-path `chmod`/`chown`, `attr_status`).

## 1. Subtree collector (`cargonaut-core`, private)

```rust
async fn collect_subtree(&self, roots: &[VfsPath]) -> (Vec<VfsPath>, bool);
```

**Invariants (tested via the public recursive methods):**
- Includes each root and every descendant reached by descending into
  `VfsKind::Dir` entries only.
- **Never descends into a `VfsKind::Symlink`** (no-follow, FR-006/SC-005).
- Bounded by `NODE_CAP`; on overflow returns `truncated = true` (FR-005/SC-004).
- Order is shallow→deep (ancestors before descendants).

## 2. Recursive App operations (`cargonaut-core`)

```rust
impl App {
    pub async fn chmod_recursive(&mut self, spec: &str) -> Result<Vec<Event>, AppError>;
    pub async fn chown_recursive(&mut self, owner: &str) -> Result<Vec<Event>, AppError>;
}
// Command (core) gains: ChmodRecursive(String), ChownRecursive(String)
```

**Invariants / tests (nested tempdir trees):**
- `chmod_recursive("700")` on a dir ⇒ a file **several levels deep** ends up `0o700` (SC-001).
- Symbolic `chmod_recursive("g+r")` ⇒ each entry's change is relative to its own mode (FR-003).
- `chown_recursive("<uid>:<gid>")` (current owner) ⇒ a nested entry shows those ids (SC-002).
- Apply is **deepest-first**: `chmod_recursive("0")` (strips all perms) on a deep
  tree still changes the deepest entries (no lock-out, FR-011) — assert a leaf
  changed even though ancestors were also targeted.
- A `VfsKind::Symlink` directory inside the tree is **not** descended: its target
  (outside the subtree) is unchanged (SC-005).
- One unwritable entry deep in the tree ⇒ reported in the status, other entries
  still changed (FR-007/SC-006); no whole-tree abort.
- Invalid `spec`/`owner` ⇒ `Err(AppError::BadAttr)` and **no walk / no change** (R-007).
- A file-only selection ⇒ behaves as a shallow change (FR-009).
- Truncation: with a lowered cap (test seam) a large tree ⇒ status contains
  "truncated" (SC-004).
- Empty selection (cursor on `..`) ⇒ `Ok` with "No files selected", no change.

## 3. UI wiring (`cargonaut-ui-tui`)

- `keymap::Command::{ChmodRecursive, ChownRecursive}` exist; `keymap.toml` binds
  `C-x C` / `C-x O` (pane) to them — parse + `lookup_sequence` test, no collision
  with `C-x c` / `C-x o`.
- `dispatch_ui_command(Command::ChmodRecursive, …)` opens a prefilled
  `TextInputDialog` (`InputKind::ChmodRecursive`), `Mode::Dialog`.
- On submit, the loop opens a `ConfirmDialog` ("Recursively chmod N item(s)?")
  with `on_confirm = AppCommand::ChmodRecursive(text)`; **confirm** dispatches it,
  **Cancel** aborts with no change (FR-002/SC-003). Same for chown.
- File menu lists "Chmod -R" / "Chown -R" → the same commands.

## 4. Help text

The F1 help overlay mentions the recursive keys. **Test:** help string contains
"C-x C" (and "recursive" or "-R").

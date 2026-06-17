# Data Model: Recursive chmod / chown into Subtrees

**Feature**: 044-recursive-attrs | **Date**: 2026-06-17

Behavioral feature; no persisted entities and no new VFS types. Reuses Feature
043's `ModeSpec`, `parse_owner`, `AppError::BadAttr`, `attr_status`, and the
per-path `VfsBackend::chmod`/`chown`.

## Reused types (no change)

- `ModeSpec` (`cargonaut-vfs`) — octal/symbolic; `apply(current_bits)` per entry.
- `parse_owner` (`cargonaut-vfs`) — `(Option<uid>, Option<gid>)`.
- `VfsKind` (`cargonaut-vfs`) — `File` / `Dir` / `Symlink{target}`. Descent
  matches **only** `Dir`, so `Symlink` dirs are leaves (FR-006).
- `App::selection_or_focused` — the root set (tagged, else focused; excludes `..`).
- `attr_status(op, ok, failures)` — batch status line.

## Core additions (`cargonaut-core`)

### Subtree collector (private helper)

```rust
/// BFS-enumerate every entry under `roots`, descending only into real
/// directories (never `Symlink`), capped at NODE_CAP. Returns paths in
/// shallow→deep order plus whether the cap truncated the walk.
async fn collect_subtree(&self, roots: &[VfsPath]) -> (Vec<VfsPath>, bool);
```

- Each root is included; for a root that is a directory, its descendants are
  appended level by level. A root that is a file contributes only itself (FR-009).
- `NODE_CAP` (reuse `recursive_dir_size`'s 200_000); on overflow → `truncated = true`.

### Recursive App methods

| Method | Behavior |
|--------|----------|
| `chmod_recursive(&mut self, spec: &str) -> Result<Vec<Event>, AppError>` | `ModeSpec::parse` (BadAttr on fail); `roots = selection_or_focused`; `collect_subtree(roots)`; apply chmod to each path **deepest-first** (reverse order), `apply` per entry's current bits; aggregate failures; refresh; status `chmod -R` (+truncated note). |
| `chown_recursive(&mut self, owner: &str) -> Result<Vec<Event>, AppError>` | `parse_owner` (BadAttr on fail); same walk; chown each path deepest-first; aggregate; refresh; status `chown -R`. |

- **Apply order**: reverse of the shallow→deep collection (descendants before
  ancestors) so a restrictive change can't lock the apply out of a child (FR-011).
- Empty root set (cursor on `..`, nothing tagged) → `Ok(Status("No files selected"))`.

### Core commands

`Command` (cargonaut-core) gains `ChmodRecursive(String)` and
`ChownRecursive(String)`; `dispatch` routes them to the methods above. These are
constructed only by the UI confirm chain.

## UI surface (`cargonaut-ui-tui`)

- `keymap::Command::{ChmodRecursive, ChownRecursive}` (kebab `chmod-recursive` /
  `chown-recursive`).
- `keymap.toml` (pane): `C-x C` → `chmod-recursive`, `C-x O` → `chown-recursive`.
- `InputKind::{ChmodRecursive, ChownRecursive}` driving the existing prefilled
  `TextInputDialog` (chmod=current octal, chown=current `uid:gid`).
- Dispatch: `C-x C` / `C-x O` open the input; **on submit** → `ConfirmDialog`
  ("Recursively chmod/chown <N> item(s)?") with `on_confirm =
  AppCommand::ChmodRecursive(text)` / `ChownRecursive(text)`; confirm dispatches
  it (FR-002, Cancel aborts).
- File menu: "Chmod -R", "Chown -R".

## Flow

```text
C-x C ─► mode input (prefill current octal) ─► Confirm "recursively chmod N?" ─► chmod_recursive ─► refresh
C-x O ─► owner input (prefill uid:gid) ─► Confirm "recursively chown N?" ─► chown_recursive ─► refresh
   (invalid input ─► BadAttr ─► inline status, no walk; Cancel at confirm ─► abort, no change)
   collect subtree (Dir-only descent, NODE_CAP) → apply deepest-first → aggregate failures (+truncated)
```

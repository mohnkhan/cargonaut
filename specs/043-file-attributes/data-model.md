# Data Model: File Attribute Operations

**Feature**: 043-file-attributes | **Date**: 2026-06-17

Mostly behavioral; the only new "data" is the pure mode representation and the
new VFS operation surface. No persisted entities.

## Existing types reused (no change)

- `FileMode { bits: u32, uid: Option<u32>, gid: Option<u32> }` (`cargonaut-vfs`)
  — already carries permission bits **and** ownership. chmod sets `bits`; chown
  sets `uid`/`gid`; both are re-read by the pane refresh.
- `VfsMetadata { mode: Option<FileMode>, … }` — the per-entry metadata the
  listing already shows (perms column via `chrome::perms_string`).
- Selection: the existing "tagged files, else focused entry, excluding `..`"
  model via `App::selection_or_focused` — no new selection concept.

## New type: `ModeSpec` (`cargonaut-vfs::mode`)

The parsed, validated representation of a chmod request — pure, no I/O.

```rust
pub enum ModeSpec {
    /// Absolute octal mode (low 12 bits), e.g. `0644`, `755`.
    Octal(u32),
    /// One or more symbolic clauses applied relative to the current mode.
    Symbolic(Vec<SymClause>),   // SymClause { who: WhoMask, op: +|-|=, perms: PermMask }
}

pub enum ModeError { Empty, BadOctal, BadSymbolic }   // -> AppError::BadAttr

impl ModeSpec {
    pub fn parse(input: &str) -> Result<ModeSpec, ModeError>;
    pub fn apply(&self, current_bits: u32) -> u32;   // Octal ignores current; Symbolic mutates it
}
```

- **Octal**: 3–4 digits, each `0–7`; value is absolute (current bits ignored).
- **Symbolic**: comma-separated `clause`s; each `clause` = `who* op perm*` where
  `who ∈ {u,g,o,a}` (default `a` when omitted), `op ∈ {+,-,=}`, `perm ∈ {r,w,x}`.
  `+` sets bits, `-` clears, `=` replaces that `who`'s bits. Applied left-to-right
  to `current_bits`.
- **Validation rule (FR-009 / SC-004)**: any malformed input → `ModeError`; the
  caller maps to `AppError::BadAttr` and changes nothing.

## New `VfsBackend` operations (`cargonaut-vfs::traits`)

| Method | Signature | LocalFs impl | Default (other backends) |
|--------|-----------|--------------|--------------------------|
| chmod | `async fn chmod(&self, path: &VfsPath, mode: u32) -> Result<(), VfsError>` | `fs::set_permissions(p, Permissions::from_mode(mode))` | `Err(Unsupported)` |
| chown | `async fn chown(&self, path: &VfsPath, uid: Option<u32>, gid: Option<u32>) -> Result<(), VfsError>` | `std::os::unix::fs::chown(p, uid, gid)` | `Err(Unsupported)` |
| symlink | `async fn symlink(&self, target: &str, link: &VfsPath) -> Result<(), VfsError>` | `std::os::unix::fs::symlink(target, link)` | `Err(Unsupported)` |
| hard_link | `async fn hard_link(&self, src: &VfsPath, link: &VfsPath) -> Result<(), VfsError>` | `std::fs::hard_link(src, link)` | `Err(Unsupported)` |

- All errors flow through the existing `map_io` → `VfsError`
  (`NotFound`/`PermissionDenied`/`Io`).
- `symlink` allows a dangling target; `hard_link` fails (surfaced) across
  filesystems or on directories.

## App orchestration (`cargonaut-core`)

`AppError` gains `BadAttr(String)` (invalid mode/owner/link input — FR-009).

| Method | Behavior |
|--------|----------|
| `chmod_selection(&mut self, spec: &str) -> Result<Vec<Event>, AppError>` | `ModeSpec::parse` (BadAttr on fail); for each `selection_or_focused` target, `apply` to its current bits, `local_fs.chmod`; collect failures; refresh; status. |
| `chown_selection(&mut self, owner: &str) -> Result<Vec<Event>, AppError>` | parse `user[:group]` (names via `nix`, or numeric); BadAttr on unknown; `local_fs.chown` each; collect failures; refresh; status. |
| `create_symlink(&mut self, name: &str) -> Result<Vec<Event>, AppError>` | link `name` in active cwd → focused entry; refuse if `name` exists/blank (BadAttr); refresh; status. |
| `create_hard_link(&mut self, name: &str) -> Result<Vec<Event>, AppError>` | as above via `hard_link`; OS rejects dir/cross-fs (reported). |

Owner string grammar: `user`, `:group`, or `user:group`; each side a name or
numeric id; empty side = leave unchanged.

## UI surface (`cargonaut-ui-tui`)

- `Command::{Chmod, Chown, CreateSymlink, CreateHardLink}` (keymap actions).
- `InputKind::{Chmod, Chown, Symlink, HardLink}` driving a prefilled
  `TextInputDialog` (chmod prefilled with current octal; chown with current
  owner; links with the target's name). chown submit → `ConfirmDialog` → apply.
- File-menu entries: Chmod / Chown / Symlink / Hardlink.
- Bindings (keymap.toml, pane mode): `C-x c`/`C-x o`/`C-x s`/`C-x l`.

## Flow

```text
C-x c / menu ─► chmod input (prefill current octal) ─► chmod_selection ─► refresh
C-x o / menu ─► chown input (prefill owner) ─► Confirm ─► chown_selection ─► refresh
C-x s / menu ─► symlink name input (prefill target name) ─► create_symlink ─► refresh
C-x l / menu ─► hardlink name input ─► create_hard_link ─► refresh
   (invalid input ─► BadAttr ─► inline error, no change; Esc ─► close, no change)
```

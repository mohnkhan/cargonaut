# Contract: File Attribute Operations Seam

**Feature**: 043-file-attributes | **Date**: 2026-06-17

The interfaces this feature exposes and the invariants tests pin down.

## 1. Mode parsing (`cargonaut-vfs::mode`)

```rust
pub enum ModeSpec { Octal(u32), Symbolic(Vec<SymClause>) }
pub enum ModeError { Empty, BadOctal, BadSymbolic }
impl ModeSpec {
    pub fn parse(input: &str) -> Result<ModeSpec, ModeError>;
    pub fn apply(&self, current_bits: u32) -> u32;
}
```

**Truth table (tests assert exactly this):**

| input | current | result |
|-------|---------|--------|
| `"644"` | any | `0o644` |
| `"0755"` | any | `0o755` |
| `"u+x"` | `0o644` | `0o744` |
| `"go-w"` | `0o666` | `0o644` |
| `"a=r"` | `0o751` | `0o444` |
| `"u+x,g+x"` | `0o644` | `0o754` |
| `""` | — | `Err(Empty)` |
| `"999"` / `"8"` | — | `Err(BadOctal)` |
| `"u?x"` / `"xyz"` | — | `Err(BadSymbolic)` |

- Octal ignores `current`; symbolic applies clauses left-to-right to `current`.

## 2. VFS operations (`cargonaut-vfs::VfsBackend` + `LocalFs`)

```rust
async fn chmod(&self, path: &VfsPath, mode: u32) -> Result<(), VfsError>;       // default: Unsupported
async fn chown(&self, path: &VfsPath, uid: Option<u32>, gid: Option<u32>) -> Result<(), VfsError>;
async fn symlink(&self, target: &str, link: &VfsPath) -> Result<(), VfsError>;
async fn hard_link(&self, src: &VfsPath, link: &VfsPath) -> Result<(), VfsError>;
```

**Contract tests (LocalFs, tempfile dirs):**
- `chmod(file, 0o600)` then `stat` ⇒ `mode.bits == 0o600` (SC-001).
- `symlink("target.txt", link)` ⇒ `link` exists, is a symlink, resolves to target;
  a dangling target is still created (SC-002).
- `hard_link(src, link)` ⇒ `link` exists and shares content/inode; `hard_link`
  of a directory or across filesystems ⇒ `Err` (not a panic) (SC-002).
- `chown(file, Some(own_uid), Some(own_gid))` (no-op to current owner) ⇒ `Ok`;
  re-stat shows the uid/gid (runnable unprivileged); chown to a foreign uid when
  unprivileged ⇒ `Err(PermissionDenied)` (SC-005/SC-006).
- A backend using the default impl ⇒ `Err(Unsupported)` for each (FR-006).
- Errors map via `map_io`: missing path ⇒ `NotFound`, EACCES ⇒ `PermissionDenied`.

## 3. App operations (`cargonaut-core`)

```rust
impl App {
    pub async fn chmod_selection(&mut self, spec: &str) -> Result<Vec<Event>, AppError>;
    pub async fn chown_selection(&mut self, owner: &str) -> Result<Vec<Event>, AppError>;
    pub async fn create_symlink(&mut self, name: &str) -> Result<Vec<Event>, AppError>;
    pub async fn create_hard_link(&mut self, name: &str) -> Result<Vec<Event>, AppError>;
}
// AppError gains: BadAttr(String)
```

**Invariants / tests (tempdirs):**
- `chmod_selection("755")` on the focused file sets `0o755`; the pane refreshes
  and the perms column reflects it (SC-001).
- chmod with **multiple tagged** files applies to all in one call (SC-003).
- `chmod_selection("nope")` ⇒ `Err(BadAttr)`, **no file changed** (SC-004).
- A multi-file batch where one target is unwritable ⇒ `Ok` with a status naming
  the failed item; the others are changed (no rollback) (SC-005/FR-010).
- `create_symlink("ln")` ⇒ `ln` appears in the listing pointing at the focused
  entry; `create_symlink` with an existing name ⇒ `Err`/status, nothing
  overwritten (SC-002).
- `chown_selection("baduser")` ⇒ `Err(BadAttr)` (unknown name), no change.
- All target lists come from `selection_or_focused` (the `..` row is never a
  target).

## 4. UI wiring (`cargonaut-ui-tui`)

- `Command::{Chmod, Chown, CreateSymlink, CreateHardLink}` exist; `keymap.toml`
  binds `C-x c`/`C-x o`/`C-x s`/`C-x l` (pane mode) to them (parse + lookup test).
- `dispatch_ui_command(Command::Chmod, …)` opens a `TextInputDialog`
  (`InputKind::Chmod`) prefilled with the focused entry's current octal mode and
  sets `Mode::Dialog`; analogous for the others (chown prefilled with owner,
  links prefilled with the target name).
- On chmod/symlink/hardlink submit, the loop calls the matching `App` method and
  applies its events; on **chown** submit, the loop opens a `ConfirmDialog`, and
  applies only on confirm (FR-007).
- Esc on any attribute dialog closes it with no change (FR-012).
- The File menu lists Chmod / Chown / Symlink / Hardlink → the same commands.

## 5. Help text

The F1 help overlay mentions the attribute keys (e.g. "C-x c chmod"). **Test:**
help string contains "C-x c" (and "chmod").

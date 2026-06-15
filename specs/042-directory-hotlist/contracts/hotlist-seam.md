# Contract: Directory Hotlist Seam

**Feature**: 042-directory-hotlist | **Date**: 2026-06-15

Interfaces this feature exposes and the invariants tests pin down.

## 1. Persistence (`cargonaut-config`)

```rust
pub struct Bookmark { pub name: String, pub path: String, pub group: Option<String> }
#[derive(Default)]
pub struct Hotlist { pub bookmarks: Vec<Bookmark> }

pub fn default_hotlist_path() -> std::path::PathBuf;        // XDG_STATE_HOME aware
impl Hotlist {
    pub fn load(path: &Path) -> Hotlist;                    // never errors: bad/absent ⇒ default()
    pub fn save(&self, path: &Path) -> std::io::Result<()>; // creates parent dirs; whole-file TOML
    pub fn add(&mut self, b: Bookmark);
    pub fn remove(&mut self, index: usize);                 // out-of-range ⇒ no-op
    pub fn grouped(&self) -> Vec<(Option<&str>, Vec<(usize, &Bookmark)>)>;
}
```

**Contract tests (config crate, explicit tempfile paths):**
- Round-trip: `save` then `load` yields an equal `Hotlist` incl. `group` (SC-002/SC-005).
- Absent file: `load` of a non-existent path ⇒ empty `Hotlist` (FR-007).
- Malformed file: `load` of garbage bytes ⇒ empty `Hotlist`, no panic (FR-013).
- `save` creates missing parent directories.
- `default_hotlist_path` honors `$XDG_STATE_HOME`, else `~/.local/state/cargonaut/hotlist.toml`.
- `grouped` buckets by group with ungrouped in a default section, preserving original indices (SC-007).

## 2. App operations (`cargonaut-core`)

```rust
impl App {
    pub fn bookmarks(&self) -> &[Bookmark];
    pub fn add_bookmark(&mut self, name: &str, group: Option<&str>) -> Result<Vec<Event>, AppError>;
    pub fn remove_bookmark(&mut self, index: usize) -> Result<Vec<Event>, AppError>;
    pub async fn jump_to_bookmark(&mut self, index: usize) -> Result<Vec<Event>, AppError>;
}
```

**Invariants / tests (core crate, tempdir + injected `hotlist_path`):**
- `add_bookmark("x", Some("g"))` uses the **active pane's cwd** as `path`,
  appends to `bookmarks()`, and the on-disk file now contains it (SC-001/SC-002).
- `add_bookmark("", _)` ⇒ `AppError`, list + file unchanged (FR-011).
- `remove_bookmark(i)` drops entry `i`, persists, and it's gone on reload (SC-005).
- `jump_to_bookmark(i)` to a valid dir navigates the active pane there (SC-001/SC-003).
- `jump_to_bookmark(i)` to a missing dir ⇒ status/`AppError`, **panes unchanged**,
  bookmark retained in `bookmarks()` (FR-008/SC-004).
- Out-of-range index on remove/jump ⇒ error/no-op, no panic.

## 3. Popup widget + wiring (`cargonaut-ui-tui`)

```rust
pub enum HotlistAction { Select(usize), Add, Remove(usize), Close }
pub struct HotlistDialog { /* rows + selection; holds no core types */ }
impl HotlistDialog {
    pub fn new(rows: Vec<HotlistRow>) -> Self;
    pub fn handle_key(&mut self, key) -> Option<HotlistAction>;
    pub fn render(&self, area, buf, theme);
}
```

**Invariants / tests:**
- `dispatch_ui_command(Command::BookmarksMenu, …)` opens `ActiveDialog::Hotlist`
  populated from `app.bookmarks()` and sets `Mode::Dialog` (replaces the old
  "not yet available" status path).
- Empty hotlist ⇒ popup still opens with a clear empty-state row (FR-010/SC-006).
- Navigation keys move selection; **select** key returns `Select(i)`; **add** key
  returns `Add`; **remove** key returns `Remove(i)`; Esc returns `Close` (FR-009/FR-012).
- On `Add`, the loop opens a `TextInputDialog` (group/name); on submit it calls
  `app.add_bookmark(name, group)` and reopens the hotlist refreshed.
- `TestBackend` render shows grouped entries (a group header + its bookmarks) (SC-007).
- Row index → bookmark is mapped against a freshly-read `app.bookmarks()` snapshot
  (rebuilt on open and after each mutation) so indices never drift.

## 4. Help text (`cargonaut-ui-tui`)

The F1 help overlay MUST mention `Ctrl-b` (open hotlist) and the in-popup
add/remove keys. **Test**: help string contains "Ctrl-b" / "bookmark".

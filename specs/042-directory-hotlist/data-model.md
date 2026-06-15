# Data Model: Directory Hotlist / Bookmarks

**Feature**: 042-directory-hotlist | **Date**: 2026-06-15

## Entities

### `Bookmark` (`cargonaut-config`)

| Field | Type | Meaning | Validation |
|-------|------|---------|------------|
| `name` | `String` | User-visible label | Non-empty (blank rejected, FR-011) |
| `path` | `String` | Target directory (path/URI string, same form `quick_cd` accepts) | Non-empty; existence checked at *jump* time, not add time (FR-008) |
| `group` | `Option<String>` | Optional group/category label | `None` ⇒ ungrouped/default section (FR-014) |

- `Serialize`/`Deserialize` (serde) for TOML. `Clone + Debug + PartialEq`.
- Two bookmarks may share a `path` under different `name`s (allowed). Default
  duplicate-name policy: **coexist** (per spec Assumptions).

### `Hotlist` (`cargonaut-config`)

| Field | Type | Meaning |
|-------|------|---------|
| `bookmarks` | `Vec<Bookmark>` | Ordered collection (insertion order) |

- `#[derive(Default)]` ⇒ empty list (the absent-file and malformed-file result).
- TOML shape:
  ```toml
  [[bookmark]]
  name = "myproj"
  path = "file:///home/u/work/myproj"
  group = "work"          # omitted when None
  ```
- Methods (all pure except `load`/`save` which touch disk):
  - `Hotlist::load(path) -> Hotlist` — read+parse; **any** IO/parse failure ⇒
    `Hotlist::default()` (empty) + recoverable notice (FR-007/FR-013).
  - `Hotlist::save(&self, path) -> io::Result<()>` — create parent dirs, write
    TOML (whole-file rewrite; last-write-wins).
  - `add(&mut self, Bookmark)` — push (validates non-empty name).
  - `remove(&mut self, index)` — remove by index.
  - `grouped(&self) -> Vec<(Option<&str>, Vec<(usize, &Bookmark)>)>` — display
    projection: bookmarks bucketed by group (ungrouped last/default), each
    carrying its original index for the popup's index→entity mapping (SC-007).

### `default_hotlist_path() -> PathBuf` (`cargonaut-config`)

- `$XDG_STATE_HOME/cargonaut/hotlist.toml` if set, else
  `$HOME/.local/state/cargonaut/hotlist.toml`, else
  `.local/state/cargonaut/hotlist.toml` (mirrors `default_config_path()`).

## `App` additions (`cargonaut-core`)

| Field | Type | Meaning |
|-------|------|---------|
| `hotlist` | `Hotlist` | In-memory hotlist, loaded at `App::new` |
| `hotlist_path` | `PathBuf` | Where to persist (resolved at construction; injectable in tests) |

Methods (UI-agnostic; return `Vec<Event>` like the rest of `App`):
- `bookmarks(&self) -> &[Bookmark]` — read-only snapshot for the popup.
- `add_bookmark(&mut self, name, group) -> Result<Vec<Event>, AppError>` —
  build a `Bookmark` from the **active pane's cwd**, push, `save`, emit a status.
  Blank name ⇒ `AppError` (FR-011), nothing saved.
- `remove_bookmark(&mut self, index) -> Result<Vec<Event>, AppError>` — remove,
  `save`, emit status. Out-of-range ⇒ no-op/error.
- `jump_to_bookmark(&mut self, index) -> Result<Vec<Event>, AppError>` —
  `quick_cd(bookmarks[index].path)`; a resolution error surfaces as a status and
  leaves panes + hotlist unchanged (FR-008). Out-of-range ⇒ error.

## UI projection (`cargonaut-ui-tui`)

- `HotlistRow` (display string per entry, incl. group header rows) +
  `HotlistAction { Select(usize) | Add | Remove(usize) | Close }` — the widget
  holds no core types; the event loop maps the selected index → `app.bookmarks()`
  on a fresh snapshot (rebuilt on open and after each mutation), matching the
  Feature 039 anti-drift discipline.

## State transitions

```text
launch ──load(hotlist_path)──► [in-memory hotlist] ──(empty if absent/malformed)

Ctrl-b ─► popup(open) ──Select(i)──► quick_cd(path) ─► pane navigates (or status on bad path)
                       ──Add───────► name prompt ─► add_bookmark ─► save ─► reopen popup
                       ──Remove(i)─► remove_bookmark ─► save ─► popup refreshed
                       ──Close/Esc─► popup closes, no change
```

# Research: Directory Hotlist / Bookmarks

**Feature**: 042-directory-hotlist | **Date**: 2026-06-15

All Technical Context items resolved (no NEEDS CLARIFICATION after
`/speckit-clarify`). Decisions and existing-code findings that shaped the plan.

## R-001: The keymap slot already exists — no binding work

- **Decision**: Reuse the existing `C-b` → `bookmarks-menu` binding
  (`design/contracts/keymap.toml`) and the existing `Command::BookmarksMenu`
  variant (`crates/cargonaut-ui-tui/src/keymap.rs:115`). No keymap change.
- **Rationale**: `BookmarksMenu` is currently unhandled in `dispatch_ui_command`,
  so `C-b` falls through to the "… not yet available" status (FR-011 of
  Feature 031). This feature just wires the dispatch arm to open the popup.
- **Alternatives considered**: Adding a new binding — unnecessary; the
  placeholder was reserved for exactly this.

## R-002: Persist as a TOML state file under `~/.local/state/cargonaut/`

- **Decision**: A dedicated `hotlist.toml` under `$XDG_STATE_HOME/cargonaut/`
  (fallback `~/.local/state/cargonaut/`), resolved by a new
  `default_hotlist_path()` in `cargonaut-config` that mirrors the existing
  `default_config_path()` XDG/tilde logic. TOML via the `serde`+`toml` already
  used for `config.toml`.
- **Rationale**: The hotlist is machine-written, user-mutated *state*, not
  hand-edited *config*. The project already separates these: command history's
  `persist_path` defaults to `~/.local/state/cargonaut/history`
  (`cargonaut-config` `HistoryConfig`). Keeping the hotlist out of `config.toml`
  avoids clobbering the user's comments on every save and follows the XDG state
  convention already chosen for history. TOML keeps it human-diffable.
- **Alternatives considered**: (a) embed in `config.toml` — rejected (mixes
  state with hand-edited config, comment-loss risk); (b) JSON/custom format —
  rejected (TOML is already a dependency and is diff-friendly).

## R-003: Persistence lives in `cargonaut-config`, not core/ui

- **Decision**: `Bookmark`/`Hotlist` types + `Hotlist::load`/`save` +
  `default_hotlist_path()` live in `cargonaut-config`. `App` (core) holds a
  `Hotlist` value; the popup (ui-tui) renders `&[Bookmark]`.
- **Rationale**: `cargonaut-config` already owns TOML file IO and path
  resolution. Placing the types there makes the SC-002/SC-005 persistence gates
  *pure config-crate unit tests with explicit tempfile paths* — deterministic
  and race-free (no shared env, no real `~/.local/state`). `cargonaut-config` is
  a dependency of both core and ui-tui, so the types are reachable everywhere.
- **Alternatives considered**: types in core — would force persistence tests
  through `App` construction and a global path; rejected for testability.

## R-004: Jump reuses `App::quick_cd` — FR-008 for free

- **Decision**: `App::jump_to_bookmark(index)` resolves the bookmark's path and
  calls the existing `App::quick_cd(path)` (`crates/cargonaut-core/src/lib.rs:1306`).
- **Rationale**: `quick_cd` already resolves a path string, navigates the active
  pane via `navigate_to`, records directory history, and returns an error for a
  missing/invalid path without mutating pane state. So FR-008 (missing target →
  graceful, panes unchanged, bookmark retained) and history-recording come with
  no new navigation code — the dispatch layer just surfaces the error as a status
  and keeps the bookmark.
- **Alternatives considered**: a bespoke navigation path — rejected; would
  duplicate `quick_cd`'s resolution/validation and risk divergent behavior.

## R-005: Popup modeled on `TasksPanelDialog`; add chains `TextInputDialog`

- **Decision**: New `HotlistDialog` (in `dialog.rs`) is a modal list widget
  holding pre-formatted display rows + a `HotlistAction` return
  (`Select(i) | Add | Remove(i) | Close`), modeled on `TasksPanelDialog`
  (`JobRow`/`TasksAction`). The **add** flow returns `Add`; the event loop then
  opens the existing `TextInputDialog` to capture a `group/name`, creates the
  bookmark from the active pane's cwd, saves, and reopens the hotlist.
- **Rationale**: `TasksPanelDialog` is the established shared pattern for a modal
  list with per-row actions over a freshly-read snapshot (Constitution III). The
  widget holds zero core types; the loop maps row index → bookmark against a
  fresh `app.bookmarks()` snapshot (same discipline Feature 039 used to avoid
  index drift). Reusing `TextInputDialog` for the name avoids a new input widget.
- **Alternatives considered**: a separate global add key (rejected in clarify —
  in-popup actions, one binding); a custom two-field input widget for name+group
  (rejected — over-engineered; `group/name` parsing on one line suffices).

## R-006: Grouping via a `group/name` add convention + grouped display

- **Decision**: A bookmark carries an optional `group: Option<String>`. The add
  prompt accepts `group/name` — if the text contains `/`, the part before the
  first `/` is the group and the remainder the name; otherwise the whole text is
  the name with no group. The popup lists bookmarks grouped (ungrouped under a
  default section).
- **Rationale**: Keeps a single `TextInputDialog` while supporting grouping
  (clarified in-scope). The `/` convention is familiar (path-like) and a no-`/`
  entry degrades to the flat case. Display grouping is a pure ordering/section
  function over `&[Bookmark]`, unit-testable without rendering.
- **Alternatives considered**: nested groups / a group-management UI — out of
  scope for M effort; flat single-level groups satisfy SC-007.

## R-007: Inject `hotlist_path` for race-free `App`-level tests

- **Decision**: `App` stores `hotlist: Hotlist` + `hotlist_path: PathBuf`.
  `App::new` resolves `default_hotlist_path()` and loads (best-effort). Core
  unit tests set `app.hotlist_path` to a tempfile (private field, in-crate
  tests) before exercising add/remove, asserting both in-memory and on-disk
  state.
- **Rationale**: Avoids an env-var path override (global state → flaky under
  parallel tests). The pure load/save lives in config (tested with explicit
  paths); App tests inject the path directly.
- **Alternatives considered**: `CARGONAUT_HOTLIST_PATH` env override (like the
  Feature 037 throttle var) — viable but global; reserved as a fallback only,
  not the test seam.

## R-008: Malformed file degrades to empty (FR-013)

- **Decision**: `Hotlist::load` returns `Ok(Hotlist::default())` (empty) plus a
  recoverable signal on parse/IO error rather than propagating a hard error;
  `App::new` logs a non-fatal notice and continues.
- **Rationale**: A corrupt state file must never block launch (FR-013). Mirrors
  `Config::load().unwrap_or_default()` used in `main.rs`.
- **Alternatives considered**: hard-fail on parse error — rejected; state files
  should degrade, not brick the app.

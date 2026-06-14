# Contract: Quick-CD Core ↔ TUI Seam

**Feature**: 038-quick-cd-popup | **Date**: 2026-06-15

Defines the boundary between `cargonaut-core` (logic) and `cargonaut-ui-tui`
(presentation/keys). Signatures are the intended shape; exact names may shift
slightly in implementation as long as the behavioral contract holds.

## Core API (`cargonaut-core::App`)

### `complete_cd`

```rust
/// Compute directory completion candidates for the active pane.
///
/// `partial` is the raw text in the quick-cd buffer. The final path
/// segment is treated as the prefix to complete; earlier segments name
/// the directory to list (resolved relative to the active pane's cwd,
/// per the path-resolution rules).
///
/// Returns full path strings (URI form, ready to place in the buffer),
/// ordered: recent-visited matches first (most-recent first), then
/// filesystem children in backend sort order, de-duplicated. Only
/// directories are returned. An empty Vec means "nothing to complete".
///
/// Does not mutate App state. A backend listing error on the resolved
/// directory yields an empty candidate set (not an Err) — a partial path
/// pointing nowhere simply has no completions.
pub async fn complete_cd(&self, partial: &str) -> Vec<String>;
```

**Guarantees**
- C1: Every returned string resolves to a directory (FR-008, SC-003).
- C2: No duplicates (by string equality).
- C3: Recent-visited matches precede filesystem-only matches (FR-008).
- C4: Order is deterministic for a given (cwd, history, filesystem) state
  (FR-007 stable cycle).
- C5: Files and non-existent entries never appear (SC-003).

### `quick_cd`

```rust
/// Accept a quick-cd path for the active pane.
///
/// Resolves `path_text` to a VfsPath relative to the active pane's cwd
/// (absolute / URI / relative per the resolution rules), then navigates
/// the active pane to it via the normal navigation path. On success the
/// previous cwd is recorded in dir history exactly as any navigation
/// would. Returns the resulting events (e.g. PaneUpdated).
///
/// Errors (returned, NOT applied) when the path does not resolve to a
/// reachable directory: non-existent, not a directory, or permission
/// denied. On error the App state is unchanged (no navigation, no
/// history mutation) — the caller keeps the prompt open and shows the
/// error (FR-006).
///
/// Empty / whitespace-only `path_text` is a no-op returning no events
/// and no error (US3 #3).
pub async fn quick_cd(&mut self, path_text: &str) -> Result<Vec<Event>, AppError>;
```

**Guarantees**
- Q1: On `Ok`, the active pane's cwd is the resolved directory; the inactive
  pane is unchanged (FR-013).
- Q2: On `Ok`, the previous cwd is appended to `dir_history_back` bounded by
  `directory_depth` (FR-005) — inherited from `navigate_to`.
- Q3: On `Err`, `app.pane(active).cwd` is byte-for-byte unchanged and history is
  unchanged (FR-006, SC-004's accept-path counterpart).
- Q4: Relative input resolves against active cwd; absolute/URI input is used as
  given (FR-012).

## Widget API (`cargonaut-ui-tui::dialog::PathInputDialog`)

```rust
/// Build a quick-cd prompt prefilled with `initial` text, cursor at end.
pub fn new(title: impl Into<String>, prompt: impl Into<String>, initial: impl Into<String>) -> Self;

/// Current buffer text (for tests + accept).
pub fn value(&self) -> &str;

/// Drive the dialog with a key. See PathInputAction.
pub fn handle_key(&mut self, code: KeyCode) -> PathInputAction;

/// Install freshly-fetched candidates (response to RequestCompletions).
/// Applies the first candidate to the buffer and marks the cache fresh
/// for the *new* buffer value. Empty input sets the "(no matches)" note
/// and leaves the buffer unchanged.
pub fn apply_completions(&mut self, candidates: Vec<String>);

/// Show an inline error and keep the prompt open (failed accept).
pub fn set_error(&mut self, msg: impl Into<String>);

/// Render (modal; clears its rect; uses theme.dialog_style()).
pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme);
```

**Behavioral contract**
- W1: `new(..)` ⇒ `value() == initial`, cursor at end (FR-014).
- W2: `Char(c)`/`Backspace` edit the buffer, clear any error, and invalidate the
  completion cache; return `Edited`.
- W3: `Tab`
  - cache stale (buffer ≠ `completion_for`) ⇒ returns
    `RequestCompletions { text: value().to_owned() }`.
  - cache fresh & non-empty ⇒ advance `cycle_idx` (wrap), set buffer to that
    candidate, return `Consumed` (FR-007).
- W4: `apply_completions(c)`: if `c` non-empty ⇒ buffer = `c[0]`, cache fresh,
  `cycle_idx = 0`; if empty ⇒ note "(no matches)", buffer unchanged (FR-009).
- W5: `Enter` ⇒ `Submit(value)`. (Empty handled by caller as no-op.)
- W6: `Esc` ⇒ `Cancel` (FR-010).
- W7: `set_error` keeps the widget alive; next edit clears the error (FR-006).

## Event-loop wiring (`cargonaut-ui-tui::lib`)

- **Open**: `dispatch_ui_command(Command::QuickCdPopup, ..)` ⇒
  `active_dialog = Some(QuickCd { widget: PathInputDialog::new("cd", "Path:",
  app.active_pane_state().cwd.display()) })`, `mode = Mode::Dialog`. (No longer
  routes to a core status stub.)
- **Keys** (in the `ActiveDialog::QuickCd` branch of `handle_key`):
  - `RequestCompletions { text }` ⇒ `let c = app.complete_cd(&text).await;
    widget.apply_completions(c);`
  - `Submit(text)` ⇒ if `text.trim().is_empty()` no-op; else
    `match app.quick_cd(&text).await { Ok(evs) => { apply events; close;
    mode=Pane } Err(e) => widget.set_error(e.to_string()) /* stay open */ }`.
  - `Cancel` ⇒ close; `mode = Pane`.
  - `Edited` / `Consumed` ⇒ nothing further.
- **Render**: `ActiveDialog::QuickCd { widget } => widget.render(area, buf, theme)`
  in the dialog-render match (sibling of the existing `Input` arm).

## Keymap (unchanged)

`design/contracts/keymap.toml` already binds `mode="pane", key="M-c",
action="quick-cd-popup"`. No edit required; the action now resolves to a working
dialog instead of a placeholder.

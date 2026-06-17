# Data Model: User Menu (F2) + Scrollable Help (F1)

**Feature**: 047-user-menu-help
**Date**: 2026-06-18

---

## Entities

### 1. `HelpSection` (static, compiled-in)

Represents one named section of the F1 help overlay.

```
HelpSection {
  title: &'static str,           // e.g., "Navigation"
  rows:  &'static [HelpRow],     // ordered list of key→description pairs
}

HelpRow {
  key:  &'static str,            // e.g., "↑ / k"
  desc: &'static str,            // e.g., "Move cursor up"
}
```

- Stored as a `&'static [HelpSection<'static>]` constant — zero runtime allocation.
- Rendered in order; section titles are visually distinct from key rows.
- The CI test `help_covers_all_keymap_bindings` iterates this data to assert SC-002 coverage.

### 2. `HelpOverlay` (UI state)

The runtime state for the F1 scrollable overlay.

```
HelpOverlay {
  scroll_offset: u16,    // current top visible line (0 = top)
}
```

- Replaces the `help_open: bool` field in `UiState`.
- `help_open` is now `Option<HelpOverlay>` in `UiState`; `None` = closed.
- `scroll_offset` is reset to 0 on every open.
- Total renderable lines is computed from the `HELP_SECTIONS` constant at render time.

**Invariants**:
- `scroll_offset` never exceeds `total_lines.saturating_sub(visible_lines)`.
- Any key that is not a navigation key or dismiss key is swallowed (FR-006).

### 3. `MenuItem` (parsed from `menu.toml`)

One entry in the user's F2 action menu.

```
MenuItem {
  label:   String,          // display label (required)
  command: String,          // shell command template (required)
  only_if: Option<String>,  // shell condition expression (optional)
  key:     Option<char>,    // single-char shortcut (optional)
}
```

- Deserialized via `serde` from the user's `menu.toml`.
- `command` may contain `{path}` as a placeholder for the highlighted entry's absolute path.
- `only_if` is evaluated at menu-open time; items whose condition exits non-zero are hidden.
- `key` must be a single printable character; duplicates within one config produce a parse warning but the first match wins.

### 4. `UserMenuConfig` (parsed from `menu.toml`)

The top-level structure of the TOML file.

```
UserMenuConfig {
  actions: Vec<MenuItem>,   // ordered list of menu items
}
```

- The TOML file is loaded fresh on every F2 press (no caching).
- A missing or empty file produces an empty `actions` vec (no error).
- A parse error produces a `MenuLoadError::Parse(String)` which is displayed in the overlay body.

### 5. `UserMenuDialog` (UI widget)

The runtime state for the F2 user menu overlay.

```
UserMenuDialog {
  items:  Vec<MenuItem>,      // visible items (only_if-passing)
  state:  ListState,          // ratatui list selection state
  error:  Option<String>,     // non-None if menu.toml had a parse error
}
```

- `items` is the post-condition-filter list: hidden items are excluded before this struct is built.
- When `error` is `Some`, the overlay body shows the error message instead of the item list; a single "Close (Esc)" hint is shown.
- Navigation: Up/Down moves `state`; Enter returns the focused item; Esc returns `None`.

**State Transitions**:
```
F2 pressed
  └─> load menu.toml
        ├─> ParseError   → UserMenuDialog { items: [], error: Some("...") }
        ├─> Empty/missing → UserMenuDialog { items: [], error: None }  (placeholder row)
        └─> Ok(config)   → evaluate only_if for each item
                             └─> UserMenuDialog { items: [passing items], error: None }
```

### 6. `MenuExecution` (async task result)

The result of running a user action command, fed back to the status bar.

```
MenuExecution {
  exit_code: i32,
  stderr_line: Option<String>,   // first line of stderr, if any
}
```

- Produced by a `tokio::task::spawn_blocking` call in `dispatch_ui_command`.
- On completion: if `exit_code == 0`, status bar shows "Done." for 2 seconds; otherwise shows `"[exit N] <stderr_line>"`.
- The TUI is not blocked during execution (FR-015).

---

## Data Flow

```
User presses F2
  │
  ▼
dispatch_ui_command(ShowUserMenu)
  ├─ guard: active_dialog.is_none() (FR-021)
  ├─ call menu_config_path() → PathBuf
  ├─ load_user_menu(path) → Result<UserMenuConfig, MenuLoadError>
  ├─ evaluate only_if conditions (tokio::task::spawn_blocking × N, timeout 200 ms)
  ├─ build UserMenuDialog { items: [visible], error, state }
  └─ *active_dialog = Some(ActiveDialog::UserMenu { widget })

User presses Enter on an item
  │
  ▼
handle_key → item.command + active_path
  ├─ substitute {path} with shell_words::quote(path)
  ├─ tokenize with shell_words::split()
  ├─ if no shell metacharacters → Command::new(prog).args(args)
  │   else → Command::new("sh").arg("-c").arg(quoted_cmd)
  └─ tokio::task::spawn_blocking(move || cmd.status() + stderr)
       └─ on completion: *status = "Done." or "[exit N] <stderr>"
```

---

## Validation Rules

| Field | Rule |
|-------|------|
| `MenuItem.label` | Required; non-empty string; displayed as-is (truncated if >60 chars) |
| `MenuItem.command` | Required; non-empty string; `{path}` is the only recognized placeholder |
| `MenuItem.only_if` | Optional; treated as a shell expression; empty string = always visible |
| `MenuItem.key` | Optional; exactly one printable ASCII character; first duplicate wins |
| `HelpOverlay.scroll_offset` | Clamped to `[0, max(0, total_lines - visible_lines)]` |

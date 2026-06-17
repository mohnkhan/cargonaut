# Contract: Skin File Format

**Version**: 1.0 | **Feature**: 046-external-theme-skins

## Overview

A skin file is a TOML file at `$XDG_CONFIG_HOME/cargonaut/themes/<name>.toml`. It maps zero or more theme element names to color values. Omitted fields inherit from the `commander-dark` built-in default.

## Location

```
$XDG_CONFIG_HOME/cargonaut/themes/<name>.toml
                 ↑ defaults to ~/.config if unset
```

## Key-Value Schema

```toml
# Each key is a theme element name (snake_case).
# Each value is a color in one of the three formats below.

panel_bg    = "Blue"           # named 16-color
panel_fg    = "Gray"
dir_fg      = "White"
exec_fg     = 46               # 256-color index (0–255)
symlink_fg  = "LightCyan"
cursor_bg   = "#ff79c6"        # RGB hex (#RRGGBB)
cursor_fg   = "#282a36"
# ... any subset of the 25 element names; all are optional
```

## Color Value Formats

| Format | TOML type | Example | Notes |
|--------|-----------|---------|-------|
| Named 16-color | string | `"Blue"` | Case-insensitive |
| Reset (terminal default) | string | `"Reset"` or `"reset"` | Case-insensitive |
| 256-color index | integer | `196` | 0–255 |
| RGB hex | string | `"#ff8800"` | Exactly `#RRGGBB` |

### Named 16-color values

| Name | Name | Name |
|------|------|------|
| `Reset` | `Black` | `Red` |
| `Green` | `Yellow` | `Blue` |
| `Magenta` | `Cyan` | `Gray` |
| `DarkGray` | `LightRed` | `LightGreen` |
| `LightYellow` | `LightBlue` | `LightMagenta` |
| `LightCyan` | `White` | |

## Element Names (25 fields)

| Element | Description |
|---------|-------------|
| `panel_bg` | Listing panel background |
| `panel_fg` | Regular-file text color |
| `dir_fg` | Directory entry text color |
| `exec_fg` | Executable file text color |
| `symlink_fg` | Symlink entry text color |
| `hidden_fg` | Hidden (dot-file) entry text color |
| `cursor_bg` | Highlighted cursor row background |
| `cursor_fg` | Highlighted cursor row text color |
| `marked_fg` | Tagged (marked) entry text color |
| `border_focused` | Focused panel border color |
| `border_unfocused` | Unfocused panel border color |
| `menu_bg` | Menu bar background |
| `menu_fg` | Menu bar text color |
| `menu_sel_bg` | Selected menu item background |
| `menu_sel_fg` | Selected menu item text color |
| `fkey_num_bg` | Function-key number chip background |
| `fkey_num_fg` | Function-key number chip text color |
| `fkey_label_bg` | Function-key label background |
| `fkey_label_fg` | Function-key label text color |
| `status_bg` | Status bar background |
| `status_fg` | Status bar text color |
| `dialog_bg` | Dialog background |
| `dialog_fg` | Dialog text color |
| `dialog_sel_bg` | Selected dialog element background |
| `dialog_sel_fg` | Selected dialog element text color |

## Error Behavior

| Error condition | App behavior |
|-----------------|-------------|
| File not found | Fall back to `commander-dark`; show one-line status |
| Not valid TOML | Fall back; show parse error in status |
| Unknown field name | Fall back; show field name in status |
| Invalid color value | Fall back; show field name + bad value in status |
| File is a directory | Treated as not-found; fall back |
| Permission denied | Fall back; show OS error in status |

## Example Skin File (Dracula-inspired)

```toml
# ~/.config/cargonaut/themes/dracula.toml
panel_bg      = "#282a36"
panel_fg      = "#f8f8f2"
dir_fg        = "#8be9fd"
exec_fg       = "#50fa7b"
symlink_fg    = "#ff79c6"
hidden_fg     = "#6272a4"
cursor_bg     = "#ff79c6"
cursor_fg     = "#282a36"
marked_fg     = "#f1fa8c"
border_focused   = "#ff79c6"
border_unfocused = "#6272a4"
menu_bg       = "#44475a"
menu_fg       = "#f8f8f2"
menu_sel_bg   = "#6272a4"
menu_sel_fg   = "#f8f8f2"
fkey_num_bg   = "#44475a"
fkey_num_fg   = "#ff79c6"
fkey_label_bg = "#282a36"
fkey_label_fg = "#6272a4"
status_bg     = "#44475a"
status_fg     = "#f8f8f2"
dialog_bg     = "#44475a"
dialog_fg     = "#f8f8f2"
dialog_sel_bg = "#6272a4"
dialog_sel_fg = "#f8f8f2"
```

## Referencing a Skin

In `~/.config/cargonaut/config.toml`:

```toml
[ui]
theme = "dracula"
```

Or on the command line:

```sh
cargonaut --theme dracula ~/src ~/dst
```

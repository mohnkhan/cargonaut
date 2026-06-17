# Data Model: External / User-Authored Theme (Skin) Files

## Entities

### ColorSpec (enum)

Represents a color value as it appears in a TOML skin file. Three variants support all three input formats (FR-003):

| Variant | TOML example | Ratatui output |
|---------|-------------|----------------|
| `Named(String)` | `"Blue"`, `"#ff8800"`, `"reset"` | `Color::Blue`, `Color::Rgb(255,136,0)`, `Color::Reset` |
| `Indexed(u8)` | `196` | `Color::Indexed(196)` |

**Validation rules**:
- `Named` starting with `#`: must be exactly `#RRGGBB` (7 chars, valid hex pairs) → `Color::Rgb`
- `Named` not starting with `#`: case-insensitive match against the 17 ratatui named colors → error if unknown
- `Indexed`: always valid (0–255, guaranteed by `u8`)

**Deserialization**: `#[serde(untagged)]` — `toml` dispatches TOML integers to `Indexed` and TOML strings to `Named` automatically.

---

### SkinFile (struct)

The deserialized TOML skin file. One `Option<ColorSpec>` field per theme element. `None` means "inherit from default".

**Constraints**:
- `#[serde(deny_unknown_fields)]` — any unrecognized TOML key is a deserialization error (FR-006, FR-008)
- `#[serde(default)]` on each field — absent TOML keys deserialize to `None` (FR-004)

**Fields** (25; `name` is not a TOML key):

| Field | Theme element |
|-------|--------------|
| `panel_bg` | Panel (listing) background |
| `panel_fg` | Regular-file foreground |
| `dir_fg` | Directory entries |
| `exec_fg` | Executable files |
| `symlink_fg` | Symlink entries |
| `hidden_fg` | Hidden (dotfile) entries |
| `cursor_bg` | Cursor row background |
| `cursor_fg` | Cursor row foreground |
| `marked_fg` | Tagged / marked entries |
| `border_focused` | Focused panel border |
| `border_unfocused` | Unfocused panel border |
| `menu_bg` | Menu bar background |
| `menu_fg` | Menu bar foreground |
| `menu_sel_bg` | Selected menu entry background |
| `menu_sel_fg` | Selected menu entry foreground |
| `fkey_num_bg` | F-key number chip background |
| `fkey_num_fg` | F-key number chip foreground |
| `fkey_label_bg` | F-key label background |
| `fkey_label_fg` | F-key label foreground |
| `status_bg` | Status bar background |
| `status_fg` | Status bar foreground |
| `dialog_bg` | Dialog background |
| `dialog_fg` | Dialog foreground |
| `dialog_sel_bg` | Selected dialog element background |
| `dialog_sel_fg` | Selected dialog element foreground |

---

### Theme (existing struct — modified)

`name` field changes from `&'static str` to `String`. All other fields unchanged.

| Field | Type | Notes |
|-------|------|-------|
| `name` | `String` | Theme name (built-in name or skin file stem) |
| `panel_bg` … `dialog_sel_fg` | `ratatui::style::Color` | 25 color fields, unchanged |

**Derives changed**: Remove `Copy`. Keep `Clone`, `PartialEq`, `Eq`, `Debug`.

---

### ThemeDir (path, not a struct)

`$XDG_CONFIG_HOME/cargonaut/themes/` or `~/.config/cargonaut/themes/` (FR-001).

Skin file path: `<ThemeDir>/<name>.toml` where `<name>` is the value of `ui.theme`.

---

## Resolution Flow

```
ui.theme = "dracula"
    │
    ├─ Theme::builtin("dracula") → None
    │
    ├─ default_theme_dir() → ~/.config/cargonaut/themes/
    │
    ├─ fs::read_to_string("~/.config/cargonaut/themes/dracula.toml")
    │      ├─ Err(not found) → (commander-dark, Some("Unknown theme "dracula" — using commander-dark"))
    │      └─ Ok(toml_str)
    │
    ├─ toml::from_str::<SkinFile>(&toml_str)
    │      ├─ Err(parse/unknown-field) → (commander-dark, Some("Skin "dracula": <error>"))
    │      └─ Ok(skin)
    │
    ├─ for each field in skin: parse_color_spec(cs)
    │      ├─ Err(bad color) → (commander-dark, Some("Skin "dracula": field panel_bg: Unknown color "Bleu""))
    │      └─ Ok(color)
    │
    └─ (Theme { name: "dracula".into(), panel_bg: <resolved>, … }, None)
```

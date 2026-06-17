# Research: External / User-Authored Theme (Skin) Files

## R-001: Where to place the skin loading code

**Decision**: All skin loading logic lives in `crates/cargonaut-ui-tui/src/theme.rs`.

**Rationale**: The `Theme` struct and its resolution logic already live here. The `cargonaut-ui-tui` crate already has `toml`, `serde`, and `ratatui` as dependencies — no new dep is required. Placing loading in `cargonaut-config` would require adding `ratatui` as a dep to the config crate (wrong direction) or splitting the code across crates for minimal benefit.

**Alternatives considered**:
- `cargonaut-config` crate: Would require adding `ratatui` dep to config (wrong dependency direction — config should not depend on a TUI library).
- A new `cargonaut-themes` crate: Overkill for ~250 lines; adds build-graph complexity for no gain.

---

## R-002: `Theme.name` field type change

**Decision**: Change `name: &'static str` to `name: String`. Remove `#[derive(Copy)]`. Keep `Clone`, `PartialEq`, `Eq`, `Debug`.

**Rationale**: Skin file names are dynamic (user-chosen), so they cannot be `&'static str`. `String` is the simplest owned type. `Copy` is removed because `String: !Copy`, but this has no practical impact — `Theme` is resolved once at startup and passed by reference throughout the event loop.

**Impact on existing code**:
- `const fn commander_dark()` and `const fn monochrome()` become `fn` (not `const`). No callers require `const`.
- Tests that compare `Theme` still work via `PartialEq`/`Eq`.
- `lib.rs` line 209: `theme.name` remains a valid `String` deref.
- `DEFAULT_THEME_NAME: &str` is unchanged (it's separate from the struct field).

**Alternatives considered**:
- `Cow<'static, str>`: Preserves `Copy`-like ergonomics via `Deref`, but adds `std::borrow::Cow` import noise and is less idiomatic for a once-resolved config value.
- `Arc<str>`: Thread-safe shared ownership, but Theme is never shared across threads; unnecessary.

---

## R-003: TOML color deserialization strategy

**Decision**: Use a `ColorSpec` enum with `#[serde(untagged)]`:

```
ColorSpec::Named(String)   — "Blue", "#ff8800", "reset"
ColorSpec::Indexed(u8)     — 0..=255 (TOML integer)
```

The `SkinFile` struct has one `Option<ColorSpec>` field per theme element. Missing fields (`None`) inherit from the built-in default. Unknown fields reject via `#[serde(deny_unknown_fields)]`.

**Rationale**: TOML natively distinguishes strings from integers, so `serde(untagged)` cleanly dispatches `panel_bg = "Blue"` to `Named` and `exec_fg = 196` to `Indexed`. No custom deserializer needed for the integer case.

**Named color parsing**: Case-insensitive match against ratatui's named colors. Recognized names: Reset, Black, Red, Green, Yellow, Blue, Magenta, Cyan, Gray, DarkGray, LightRed, LightGreen, LightYellow, LightBlue, LightMagenta, LightCyan, White.

**RGB hex parsing**: If the string starts with `#` and is 7 characters long, parse as `#RRGGBB` → `Color::Rgb(r, g, b)`. Invalid hex digits → error.

**Alternatives considered**:
- A single `String` field with custom logic: Cannot distinguish `"196"` (string) from `196` (integer) in TOML without extra ceremony.
- A TOML inline table `{ r=255, g=136, b=0 }`: More verbose for users; named colors become cumbersome.

---

## R-004: Partial skin support (missing fields inherit default)

**Decision**: `SkinFile` has all fields as `Option<ColorSpec>`. `Theme::from_skin` fills `None` fields from `Theme::commander_dark()` (the built-in default).

**Rationale**: FR-004 requires partial skins. Using `Option<ColorSpec>` per field is the idiomatic Rust/serde approach — `#[serde(default)]` makes absent TOML keys deserialize to `None`.

**Field name mapping**: The `SkinFile` struct mirrors the 30 public `Theme` fields verbatim, so the TOML key names are self-documenting and stable.

---

## R-005: Error handling and `Theme::resolve` signature

**Decision**: Change `Theme::resolve(name: &str) -> Theme` to return `(Theme, Option<String>)`:
- `(theme, None)` — built-in or skin loaded successfully
- `(default_theme, Some(msg))` — error occurred; `msg` is the one-line human-readable status

**Rationale**: The caller (`lib.rs` line 207-209) already handles the "unknown theme" status via:
```rust
let mut status: String = if Theme::builtin(&theme_name).is_none() {
    format!("Unknown theme {theme_name:?} — using {}", theme.name)
```
With the new signature, this becomes:
```rust
let (theme, theme_err) = Theme::resolve(&theme_name);
let mut status: String = theme_err.unwrap_or_default();
```
This is a clean, backward-compatible refactor. The error string surfaces the exact failure reason (not-found, parse error, invalid color, unknown field) so FR-007 is satisfied automatically.

**Alternatives considered**:
- `Result<Theme, (Theme, String)>`: More idiomatic but harder to unwrap at the call site; the existing status string handling is already a `mut String`, so `Option<String>` fits better.
- Keep `-> Theme` and add a `last_resolve_error()` static: Global mutable state — bad for testing.

---

## R-006: XDG path resolution (no new dep)

**Decision**: Inline the XDG logic in `theme.rs`:

```rust
fn default_theme_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("cargonaut/themes")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/cargonaut/themes")
    } else {
        PathBuf::from(".config/cargonaut/themes")
    }
}
```

**Rationale**: This is 5 lines and identical in shape to `default_config_path()` in `cargonaut-config`. Duplicating 5 lines avoids adding `cargonaut-config` as a real dep of `cargonaut-ui-tui` (currently it's only a dev-dep). No new Cargo.toml changes = simpler PR.

**Test approach**: Tests for `default_theme_dir` pass `XDG_CONFIG_HOME` / `HOME` overrides via environment variables using `tempfile` dirs, consistent with the existing config crate test pattern.

---

## R-007: Built-in precedence over skin files

**Decision**: `Theme::resolve` checks `Theme::builtin(name)` FIRST. If a built-in matches, the skin file is never read — even if a file named `commander-dark.toml` exists.

**Rationale**: FR-005 requires this. It prevents users from accidentally (or maliciously) replacing built-in themes. The check is case-insensitive (matching the existing `builtin()` behavior).

---

## R-008: TOML field naming convention

**Decision**: Skin TOML field names match the Rust struct field names verbatim (snake_case). Example:

```toml
# ~/.config/cargonaut/themes/dracula.toml
panel_bg    = "#282a36"
panel_fg    = "#f8f8f2"
dir_fg      = "#8be9fd"
cursor_bg   = "#ff79c6"
cursor_fg   = "#282a36"
```

**Rationale**: Consistent with Rust convention and the existing Config TOML schema in `cargonaut-config`. Users editing the file can look up field meanings in the documentation or source.

**Fields list** (30 total from `Theme` struct):
`panel_bg`, `panel_fg`, `dir_fg`, `exec_fg`, `symlink_fg`, `hidden_fg`, `cursor_bg`, `cursor_fg`, `marked_fg`, `border_focused`, `border_unfocused`, `menu_bg`, `menu_fg`, `menu_sel_bg`, `menu_sel_fg`, `fkey_num_bg`, `fkey_num_fg`, `fkey_label_bg`, `fkey_label_fg`, `status_bg`, `status_fg`, `dialog_bg`, `dialog_fg`, `dialog_sel_bg`, `dialog_sel_fg`

(25 fields — the `name` field is NOT a TOML key; it's populated from the theme name argument.)

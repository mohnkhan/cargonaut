// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Typed color theme for the TUI (Feature 031/046).
//!
//! Constitution §III requires theme variables to be *typed* — no
//! hardcoded ANSI escapes in feature code. Every themable element maps to
//! a [`ratatui::style::Color`]. Themes are resolved from a name
//! ([`Theme::resolve`]); an unknown name falls back to the built-in
//! default ([`Theme::default`]) so a bad `--theme`/config value never
//! crashes the app.
//!
//! Built-ins:
//! - [`Theme::commander_dark`] — the default; the signature blue-panel,
//!   bright-directory, cyan-selection look of the reference manager.
//! - [`Theme::monochrome`] — a 16-color-safe fallback for limited terminals.
//!
//! User skins (Feature 046): TOML files at
//! `$XDG_CONFIG_HOME/cargonaut/themes/<name>.toml` are loaded when the
//! configured theme name is not a built-in. Load errors fall back to
//! `commander-dark` with a one-line status message (FR-006).

use cargonaut_vfs::{FileMode, VfsKind};
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;
use std::path::{Path, PathBuf};

/// The name resolved when no theme / an unknown theme is requested.
pub const DEFAULT_THEME_NAME: &str = "commander-dark";

/// A color value as written in a TOML skin file.
///
/// Supports three formats (FR-003):
/// - Named 16-color: `"Blue"`, `"LightGreen"`, `"Reset"` (case-insensitive)
/// - RGB hex string: `"#RRGGBB"`
/// - 256-color index: integer `0`–`255`
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum ColorSpec {
    /// Named 16-color string or `#RRGGBB` hex string.
    Named(String),
    /// 256-color palette index (0–255).
    Indexed(u8),
}

/// The deserialized form of a TOML skin file.
///
/// Every field is `Option<ColorSpec>`: absent fields (`None`) inherit
/// from [`Theme::commander_dark`] (FR-004). Unknown TOML keys are
/// rejected by `deny_unknown_fields`, producing a descriptive error
/// (FR-006, FR-008).
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct SkinFile {
    /// Panel (listing) background.
    pub panel_bg: Option<ColorSpec>,
    /// Regular-file foreground.
    pub panel_fg: Option<ColorSpec>,
    /// Directory entries.
    pub dir_fg: Option<ColorSpec>,
    /// Executable files.
    pub exec_fg: Option<ColorSpec>,
    /// Symlink entries.
    pub symlink_fg: Option<ColorSpec>,
    /// Hidden (dotfile) entries.
    pub hidden_fg: Option<ColorSpec>,
    /// Cursor row background.
    pub cursor_bg: Option<ColorSpec>,
    /// Cursor row foreground.
    pub cursor_fg: Option<ColorSpec>,
    /// Tagged / marked entries.
    pub marked_fg: Option<ColorSpec>,
    /// Focused panel border.
    pub border_focused: Option<ColorSpec>,
    /// Unfocused panel border.
    pub border_unfocused: Option<ColorSpec>,
    /// Menu bar background.
    pub menu_bg: Option<ColorSpec>,
    /// Menu bar foreground.
    pub menu_fg: Option<ColorSpec>,
    /// Selected menu entry background.
    pub menu_sel_bg: Option<ColorSpec>,
    /// Selected menu entry foreground.
    pub menu_sel_fg: Option<ColorSpec>,
    /// F-key number chip background.
    pub fkey_num_bg: Option<ColorSpec>,
    /// F-key number chip foreground.
    pub fkey_num_fg: Option<ColorSpec>,
    /// F-key label background.
    pub fkey_label_bg: Option<ColorSpec>,
    /// F-key label foreground.
    pub fkey_label_fg: Option<ColorSpec>,
    /// Status bar background.
    pub status_bg: Option<ColorSpec>,
    /// Status bar foreground.
    pub status_fg: Option<ColorSpec>,
    /// Dialog background.
    pub dialog_bg: Option<ColorSpec>,
    /// Dialog foreground.
    pub dialog_fg: Option<ColorSpec>,
    /// Selected dialog element background.
    pub dialog_sel_bg: Option<ColorSpec>,
    /// Selected dialog element foreground.
    pub dialog_sel_fg: Option<ColorSpec>,
}

/// Returns the directory where user skin files are stored (FR-001).
///
/// Resolution order: `$XDG_CONFIG_HOME/cargonaut/themes/` →
/// `$HOME/.config/cargonaut/themes/` → `.config/cargonaut/themes/`.
pub fn default_theme_dir() -> PathBuf {
    if let Ok(xdg) = std::env::var("XDG_CONFIG_HOME") {
        PathBuf::from(xdg).join("cargonaut/themes")
    } else if let Ok(home) = std::env::var("HOME") {
        PathBuf::from(home).join(".config/cargonaut/themes")
    } else {
        PathBuf::from(".config/cargonaut/themes")
    }
}

/// A fully-specified color palette for every themable interface element.
///
/// All fields are concrete [`Color`]s — no element falls back to the
/// terminal default unintentionally (FR-002).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Theme {
    /// The name this theme resolved from.
    pub name: String,

    /// Panel (listing) background.
    pub panel_bg: Color,
    /// Regular-file foreground.
    pub panel_fg: Color,
    /// Directory entries.
    pub dir_fg: Color,
    /// Executable files (any execute bit set).
    pub exec_fg: Color,
    /// Symlink entries.
    pub symlink_fg: Color,
    /// Hidden (dotfile) entries.
    pub hidden_fg: Color,

    /// Cursor (highlight) row background.
    pub cursor_bg: Color,
    /// Cursor (highlight) row foreground.
    pub cursor_fg: Color,
    /// Tagged / marked entries.
    pub marked_fg: Color,

    /// Focused panel border.
    pub border_focused: Color,
    /// Unfocused panel border.
    pub border_unfocused: Color,

    /// Menu bar background.
    pub menu_bg: Color,
    /// Menu bar foreground.
    pub menu_fg: Color,
    /// Selected menu entry background.
    pub menu_sel_bg: Color,
    /// Selected menu entry foreground.
    pub menu_sel_fg: Color,

    /// Function-key number chip background.
    pub fkey_num_bg: Color,
    /// Function-key number chip foreground.
    pub fkey_num_fg: Color,
    /// Function-key label background.
    pub fkey_label_bg: Color,
    /// Function-key label foreground.
    pub fkey_label_fg: Color,

    /// Status bar background.
    pub status_bg: Color,
    /// Status bar foreground.
    pub status_fg: Color,

    /// Dialog background.
    pub dialog_bg: Color,
    /// Dialog foreground.
    pub dialog_fg: Color,
    /// Selected dialog element background.
    pub dialog_sel_bg: Color,
    /// Selected dialog element foreground.
    pub dialog_sel_fg: Color,
}

impl Default for Theme {
    fn default() -> Self {
        Self::commander_dark()
    }
}

impl Theme {
    /// Resolve a theme by name. Returns `(theme, error_message)`.
    ///
    /// Resolution order: built-in → skin file in [`default_theme_dir`] →
    /// commander-dark fallback with a non-`None` error string (FR-006).
    pub fn resolve(name: &str) -> (Theme, Option<String>) {
        Theme::resolve_from(name, &default_theme_dir())
    }

    /// Like [`Theme::resolve`] but uses `dir` as the skin search directory.
    /// Useful in tests to avoid env-var manipulation.
    pub fn resolve_from(name: &str, dir: &Path) -> (Theme, Option<String>) {
        if let Some(t) = Theme::builtin(name) {
            return (t, None);
        }
        match load_skin(name, dir) {
            Ok(t) => (t, None),
            Err(e) => (Theme::commander_dark(), Some(e)),
        }
    }

    /// Build a `Theme` from a parsed [`SkinFile`], inheriting missing fields
    /// from [`Theme::commander_dark`].
    fn from_skin(name: &str, skin: SkinFile) -> Result<Theme, String> {
        let mut t = Theme::commander_dark();
        t.name = name.to_owned();
        macro_rules! apply {
            ($field:ident) => {
                if let Some(cs) = skin.$field {
                    t.$field = parse_color_spec(&cs)
                        .map_err(|e| format!("field {}: {e}", stringify!($field)))?;
                }
            };
        }
        apply!(panel_bg);
        apply!(panel_fg);
        apply!(dir_fg);
        apply!(exec_fg);
        apply!(symlink_fg);
        apply!(hidden_fg);
        apply!(cursor_bg);
        apply!(cursor_fg);
        apply!(marked_fg);
        apply!(border_focused);
        apply!(border_unfocused);
        apply!(menu_bg);
        apply!(menu_fg);
        apply!(menu_sel_bg);
        apply!(menu_sel_fg);
        apply!(fkey_num_bg);
        apply!(fkey_num_fg);
        apply!(fkey_label_bg);
        apply!(fkey_label_fg);
        apply!(status_bg);
        apply!(status_fg);
        apply!(dialog_bg);
        apply!(dialog_fg);
        apply!(dialog_sel_bg);
        apply!(dialog_sel_fg);
        Ok(t)
    }

    /// Look up a built-in theme by name, or `None` if unknown.
    pub fn builtin(name: &str) -> Option<Theme> {
        let norm = name.trim().to_ascii_lowercase().replace('_', "-");
        match norm.as_str() {
            "commander-dark" | "commander" | "default" => Some(Theme::commander_dark()),
            "monochrome" | "mono" => Some(Theme::monochrome()),
            _ => None,
        }
    }

    /// The names of all built-in themes (for help/listing).
    pub fn builtin_names() -> &'static [&'static str] {
        &["commander-dark", "monochrome"]
    }

    /// The default theme: the reference manager's signature look — a blue
    /// panel background, bright-white directories, green executables, a
    /// cyan selection bar, yellow tags. Named colors only, so it renders
    /// correctly on a 16-color terminal (FR-004, FR-007).
    pub fn commander_dark() -> Theme {
        Theme {
            name: "commander-dark".to_owned(),
            panel_bg: Color::Blue,
            panel_fg: Color::Gray,
            dir_fg: Color::White,
            exec_fg: Color::LightGreen,
            symlink_fg: Color::LightCyan,
            hidden_fg: Color::DarkGray,
            cursor_bg: Color::Cyan,
            cursor_fg: Color::Black,
            marked_fg: Color::Yellow,
            border_focused: Color::LightCyan,
            border_unfocused: Color::Gray,
            menu_bg: Color::Cyan,
            menu_fg: Color::Black,
            menu_sel_bg: Color::Blue,
            menu_sel_fg: Color::White,
            fkey_num_bg: Color::Cyan,
            fkey_num_fg: Color::Black,
            fkey_label_bg: Color::Black,
            fkey_label_fg: Color::Gray,
            status_bg: Color::Cyan,
            status_fg: Color::Black,
            dialog_bg: Color::Gray,
            dialog_fg: Color::Black,
            dialog_sel_bg: Color::Blue,
            dialog_sel_fg: Color::White,
        }
    }

    /// A 16-color-safe fallback that stays legible on minimal terminals
    /// (FR-007). Uses only the base palette + bold; selection via a
    /// light-gray bar.
    pub fn monochrome() -> Theme {
        Theme {
            name: "monochrome".to_owned(),
            panel_bg: Color::Reset,
            panel_fg: Color::Reset,
            dir_fg: Color::White,
            exec_fg: Color::Green,
            symlink_fg: Color::Cyan,
            hidden_fg: Color::DarkGray,
            cursor_bg: Color::White,
            cursor_fg: Color::Black,
            marked_fg: Color::Yellow,
            border_focused: Color::White,
            border_unfocused: Color::Gray,
            menu_bg: Color::Gray,
            menu_fg: Color::Black,
            menu_sel_bg: Color::White,
            menu_sel_fg: Color::Black,
            fkey_num_bg: Color::Gray,
            fkey_num_fg: Color::Black,
            fkey_label_bg: Color::Reset,
            fkey_label_fg: Color::Gray,
            status_bg: Color::Gray,
            status_fg: Color::Black,
            dialog_bg: Color::Gray,
            dialog_fg: Color::Black,
            dialog_sel_bg: Color::White,
            dialog_sel_fg: Color::Black,
        }
    }

    /// Foreground color for a directory entry, keyed on its kind / mode /
    /// hidden flag (FR-003). Directories and executables additionally get
    /// a BOLD modifier via [`Theme::entry_style`].
    pub fn entry_fg(&self, kind: &VfsKind, mode: Option<&FileMode>, hidden: bool) -> Color {
        if hidden {
            return self.hidden_fg;
        }
        match kind {
            VfsKind::Dir => self.dir_fg,
            VfsKind::Symlink { .. } => self.symlink_fg,
            _ => {
                if is_executable(mode) {
                    self.exec_fg
                } else {
                    self.panel_fg
                }
            }
        }
    }

    /// The base (non-cursor) [`Style`] for a listing row: the type color
    /// on the panel background, bold for directories/executables, and the
    /// `marked_fg` color override when the row is tagged (FR-003).
    pub fn entry_style(
        &self,
        kind: &VfsKind,
        mode: Option<&FileMode>,
        hidden: bool,
        marked: bool,
    ) -> Style {
        let fg = if marked {
            self.marked_fg
        } else {
            self.entry_fg(kind, mode, hidden)
        };
        let mut style = Style::default().fg(fg).bg(self.panel_bg);
        if matches!(kind, VfsKind::Dir) || is_executable(mode) || marked {
            style = style.add_modifier(Modifier::BOLD);
        }
        style
    }

    /// The cursor (highlight) row [`Style`].
    pub fn cursor_style(&self) -> Style {
        Style::default().fg(self.cursor_fg).bg(self.cursor_bg)
    }

    /// The status-bar [`Style`].
    pub fn status_style(&self) -> Style {
        Style::default().fg(self.status_fg).bg(self.status_bg)
    }

    /// The dialog body [`Style`].
    pub fn dialog_style(&self) -> Style {
        Style::default().fg(self.dialog_fg).bg(self.dialog_bg)
    }

    /// Border [`Style`] for a panel, by focus.
    pub fn border_style(&self, focused: bool) -> Style {
        let c = if focused {
            self.border_focused
        } else {
            self.border_unfocused
        };
        let mut s = Style::default().fg(c).bg(self.panel_bg);
        if focused {
            s = s.add_modifier(Modifier::BOLD);
        }
        s
    }
}

/// True if the mode has any execute bit set (`0o111`).
fn is_executable(mode: Option<&FileMode>) -> bool {
    mode.map(|m| m.bits & 0o111 != 0).unwrap_or(false)
}

/// Convert a [`ColorSpec`] from a skin file into a ratatui [`Color`] (FR-003).
///
/// - `Indexed(u8)` → `Color::Indexed`
/// - `Named` starting with `#` → `Color::Rgb` (exactly `#RRGGBB`)
/// - `Named` otherwise → case-insensitive match against the 17 named colors
fn parse_color_spec(cs: &ColorSpec) -> Result<Color, String> {
    match cs {
        ColorSpec::Indexed(i) => Ok(Color::Indexed(*i)),
        ColorSpec::Named(s) => {
            if let Some(hex) = s.strip_prefix('#') {
                if hex.len() != 6 {
                    return Err(format!("invalid hex color {s:?}: expected #RRGGBB"));
                }
                let r = u8::from_str_radix(&hex[0..2], 16)
                    .map_err(|_| format!("invalid hex color {s:?}"))?;
                let g = u8::from_str_radix(&hex[2..4], 16)
                    .map_err(|_| format!("invalid hex color {s:?}"))?;
                let b = u8::from_str_radix(&hex[4..6], 16)
                    .map_err(|_| format!("invalid hex color {s:?}"))?;
                Ok(Color::Rgb(r, g, b))
            } else {
                match s.to_ascii_lowercase().as_str() {
                    "reset" => Ok(Color::Reset),
                    "black" => Ok(Color::Black),
                    "red" => Ok(Color::Red),
                    "green" => Ok(Color::Green),
                    "yellow" => Ok(Color::Yellow),
                    "blue" => Ok(Color::Blue),
                    "magenta" => Ok(Color::Magenta),
                    "cyan" => Ok(Color::Cyan),
                    "gray" | "grey" => Ok(Color::Gray),
                    "darkgray" | "darkgrey" | "dark_gray" | "dark_grey" => Ok(Color::DarkGray),
                    "lightred" | "light_red" => Ok(Color::LightRed),
                    "lightgreen" | "light_green" => Ok(Color::LightGreen),
                    "lightyellow" | "light_yellow" => Ok(Color::LightYellow),
                    "lightblue" | "light_blue" => Ok(Color::LightBlue),
                    "lightmagenta" | "light_magenta" => Ok(Color::LightMagenta),
                    "lightcyan" | "light_cyan" => Ok(Color::LightCyan),
                    "white" => Ok(Color::White),
                    _ => Err(format!("unknown color {s:?}")),
                }
            }
        }
    }
}

/// Load and parse a skin TOML file from `dir/<name>.toml` (FR-001, FR-005).
///
/// Returns an `Err` with a human-readable status message on any failure
/// (file not found, invalid TOML, unknown field, bad color value).
pub fn load_skin(name: &str, dir: &Path) -> Result<Theme, String> {
    let path = dir.join(format!("{name}.toml"));
    let content = std::fs::read_to_string(&path)
        .map_err(|_e| format!("Unknown theme {name:?} — using commander-dark"))?;
    let skin: SkinFile = toml::from_str(&content)
        .map_err(|e| format!("Skin {name:?}: {e}"))?;
    Theme::from_skin(name, skin).map_err(|e| format!("Skin {name:?}: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn mode(bits: u32) -> FileMode {
        FileMode {
            bits,
            uid: None,
            gid: None,
        }
    }

    /// Write a skin file into `<dir>/cargonaut/themes/<name>.toml` and
    /// return the themes directory path.
    fn write_skin(dir: &TempDir, name: &str, content: &str) -> std::path::PathBuf {
        let themes = dir.path().join("cargonaut/themes");
        fs::create_dir_all(&themes).unwrap();
        fs::write(themes.join(format!("{name}.toml")), content).unwrap();
        themes
    }

    // ---------------------------------------------------------------------------
    // T005 (red): skin_full_palette_loads — calls load_skin which doesn't exist
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_full_palette_loads() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(
            &dir,
            "dracula",
            r##"
panel_bg  = "#282a36"
panel_fg  = "#f8f8f2"
dir_fg    = "#8be9fd"
exec_fg   = "#50fa7b"
symlink_fg = "#ff79c6"
hidden_fg  = "#6272a4"
cursor_bg  = "#ff79c6"
cursor_fg  = "#282a36"
marked_fg  = "#f1fa8c"
border_focused   = "#ff79c6"
border_unfocused = "#6272a4"
menu_bg    = "#44475a"
menu_fg    = "#f8f8f2"
menu_sel_bg = "#6272a4"
menu_sel_fg = "#f8f8f2"
fkey_num_bg  = "#44475a"
fkey_num_fg  = "#ff79c6"
fkey_label_bg = "#282a36"
fkey_label_fg = "#6272a4"
status_bg  = "#44475a"
status_fg  = "#f8f8f2"
dialog_bg  = "#44475a"
dialog_fg  = "#f8f8f2"
dialog_sel_bg = "#6272a4"
dialog_sel_fg = "#f8f8f2"
"##,
        );
        let result = load_skin("dracula", &themes_dir);
        let theme = result.expect("full dracula skin must load without error");
        assert_eq!(theme.panel_bg, Color::Rgb(40, 42, 54));
        assert_eq!(theme.name, "dracula");
    }

    // ---------------------------------------------------------------------------
    // T006 (red): skin_missing_file_falls_back — resolve new sig not yet present
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_missing_file_falls_back() {
        let dir = TempDir::new().unwrap();
        let themes_dir = dir.path().join("cargonaut/themes");
        fs::create_dir_all(&themes_dir).unwrap();
        let result = load_skin("no-such-skin", &themes_dir);
        let err = result.expect_err("missing skin file must return Err");
        assert!(
            err.contains("no-such-skin"),
            "error must name the missing skin; got: {err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // T007 (red): parse_color_spec — three format variants
    // ---------------------------------------------------------------------------
    #[test]
    fn parse_color_spec_named() {
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("Blue".into())).unwrap(),
            Color::Blue
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("reset".into())).unwrap(),
            Color::Reset
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("LIGHTGREEN".into())).unwrap(),
            Color::LightGreen
        );
    }

    #[test]
    fn parse_color_spec_indexed() {
        assert_eq!(
            parse_color_spec(&ColorSpec::Indexed(196)).unwrap(),
            Color::Indexed(196)
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Indexed(0)).unwrap(),
            Color::Indexed(0)
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Indexed(255)).unwrap(),
            Color::Indexed(255)
        );
    }

    #[test]
    fn parse_color_spec_rgb_hex() {
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("#ff8800".into())).unwrap(),
            Color::Rgb(255, 136, 0)
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("#282a36".into())).unwrap(),
            Color::Rgb(40, 42, 54)
        );
        assert_eq!(
            parse_color_spec(&ColorSpec::Named("#000000".into())).unwrap(),
            Color::Rgb(0, 0, 0)
        );
    }

    // ---------------------------------------------------------------------------
    // T029 (red): skin_resolve_via_theme_name — FR-008 full resolve chain
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_resolve_via_theme_name() {
        let dir = TempDir::new().unwrap();
        write_skin(&dir, "test-skin", "panel_bg = \"Red\"\n");
        // Full resolve chain: not a builtin → load from dir → return (theme, None)
        let (theme, err) = Theme::resolve_from("test-skin", &dir.path().join("cargonaut/themes"));
        assert!(err.is_none(), "valid skin must return no error; got: {err:?}");
        assert_eq!(theme.panel_bg, Color::Red);
        assert_eq!(theme.name, "test-skin");
    }

    // ---------------------------------------------------------------------------
    // T014 (US2): skin_partial_inherits_defaults — only specified fields change
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_partial_inherits_defaults() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(&dir, "green-cursor", "cursor_bg = \"Green\"\ncursor_fg = \"Black\"\n");
        let theme = load_skin("green-cursor", &themes_dir).expect("partial skin must load");
        // Overridden fields
        assert_eq!(theme.cursor_bg, Color::Green);
        assert_eq!(theme.cursor_fg, Color::Black);
        // Unspecified fields fall back to commander-dark defaults
        let def = Theme::commander_dark();
        assert_eq!(theme.panel_bg, def.panel_bg);
        assert_eq!(theme.dir_fg, def.dir_fg);
        assert_eq!(theme.status_bg, def.status_bg);
        assert_eq!(theme.name, "green-cursor");
    }

    // ---------------------------------------------------------------------------
    // T015 (US2): skin_empty_equals_default_colors — all fields inherit default
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_empty_equals_default_colors() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(&dir, "empty-skin", "");
        let theme = load_skin("empty-skin", &themes_dir).expect("empty skin must load");
        let def = Theme::commander_dark();
        // All color fields match commander-dark; only name differs
        assert_eq!(theme.panel_bg, def.panel_bg);
        assert_eq!(theme.cursor_bg, def.cursor_bg);
        assert_eq!(theme.dir_fg, def.dir_fg);
        assert_eq!(theme.name, "empty-skin");
    }

    // ---------------------------------------------------------------------------
    // T017 (US3): invalid color name falls back gracefully
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_invalid_color_returns_err() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(&dir, "broken", "panel_bg = \"Bleu\"\n");
        let err = load_skin("broken", &themes_dir).expect_err("invalid color must return Err");
        assert!(
            err.contains("Bleu"),
            "error must name the bad value; got: {err:?}"
        );
        assert!(
            err.contains("broken"),
            "error must name the skin; got: {err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // T018 (US3): unknown field name rejected by deny_unknown_fields
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_unknown_field_returns_err() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(&dir, "wrong-key", "frobnicate = \"Blue\"\n");
        let err = load_skin("wrong-key", &themes_dir).expect_err("unknown field must return Err");
        assert!(
            err.contains("wrong-key"),
            "error must name the skin; got: {err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // T019 (US3): invalid TOML syntax returns descriptive error
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_bad_toml_returns_err() {
        let dir = TempDir::new().unwrap();
        let themes_dir = write_skin(&dir, "bad-toml", "panel_bg = \n");
        let err = load_skin("bad-toml", &themes_dir).expect_err("bad TOML must return Err");
        assert!(
            err.contains("bad-toml"),
            "error must name the skin; got: {err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // T020 (US3): directory-as-skin-path falls back (treated as not-found)
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_directory_path_returns_err() {
        let dir = TempDir::new().unwrap();
        // Create a directory where the .toml file would be
        let themes_dir = dir.path().join("cargonaut/themes");
        fs::create_dir_all(&themes_dir).unwrap();
        fs::create_dir_all(themes_dir.join("dir-skin.toml")).unwrap();
        let err = load_skin("dir-skin", &themes_dir).expect_err("directory must return Err");
        assert!(
            err.contains("dir-skin"),
            "error must name the skin; got: {err:?}"
        );
    }

    // ---------------------------------------------------------------------------
    // T030 (US3): XDG_CONFIG_HOME env override changes search directory
    // ---------------------------------------------------------------------------
    #[test]
    fn skin_xdg_env_override() {
        let dir = TempDir::new().unwrap();
        write_skin(&dir, "xdg-test", "exec_fg = 46\n");
        // Test resolve_from directly (avoids touching global env)
        let (theme, err) =
            Theme::resolve_from("xdg-test", &dir.path().join("cargonaut/themes"));
        assert!(err.is_none(), "valid skin via XDG path must have no error; got: {err:?}");
        assert_eq!(theme.exec_fg, Color::Indexed(46));
        assert_eq!(theme.name, "xdg-test");
    }

    // T-THEME-2: unknown name falls back to default, never panics (FR-006).
    #[test]
    fn resolve_unknown_falls_back_to_default() {
        let (t, err) = Theme::resolve("does-not-exist");
        assert_eq!(t, Theme::default());
        assert_eq!(t.name, DEFAULT_THEME_NAME);
        assert!(err.is_some(), "unknown name must produce an error message");
    }

    #[test]
    fn resolve_known_names_and_aliases() {
        assert_eq!(Theme::resolve("commander-dark").0.name, "commander-dark");
        assert_eq!(Theme::resolve("Commander_Dark").0.name, "commander-dark");
        assert_eq!(Theme::resolve("monochrome").0.name, "monochrome");
        assert_eq!(Theme::resolve("mono").0.name, "monochrome");
        assert!(Theme::builtin("nope").is_none());
    }

    // T-THEME-3: directory / executable / symlink / regular / hidden are
    // each a distinct color in the default theme (FR-003, SC-002).
    #[test]
    fn entry_types_are_visually_distinct() {
        let t = Theme::commander_dark();
        let dir = t.entry_fg(&VfsKind::Dir, None, false);
        let exec = t.entry_fg(&VfsKind::File, Some(&mode(0o755)), false);
        let sym_target = Box::new(cargonaut_vfs::VfsPath::parse("file:///x").unwrap());
        let sym = t.entry_fg(&VfsKind::Symlink { target: sym_target }, None, false);
        let file = t.entry_fg(&VfsKind::File, Some(&mode(0o644)), false);
        let hidden = t.entry_fg(&VfsKind::File, Some(&mode(0o644)), true);
        let all = [dir, exec, sym, file, hidden];
        for i in 0..all.len() {
            for j in (i + 1)..all.len() {
                assert_ne!(all[i], all[j], "colors {i} and {j} must differ");
            }
        }
    }

    // T-THEME-4: cursor row, marked row, and normal row are mutually
    // distinct (SC-002).
    #[test]
    fn cursor_marked_normal_are_distinct() {
        let t = Theme::commander_dark();
        let normal = t.entry_style(&VfsKind::File, Some(&mode(0o644)), false, false);
        let marked = t.entry_style(&VfsKind::File, Some(&mode(0o644)), false, true);
        let cursor = t.cursor_style();
        assert_ne!(normal, marked);
        assert_ne!(normal.fg, cursor.fg);
        assert_ne!(marked.fg, cursor.bg.map(|_| cursor.fg).unwrap_or(cursor.fg));
        assert_ne!(normal.bg, cursor.bg);
    }

    // T-THEME-1: every themable element is a concrete color (no Reset
    // leaks in the *default* theme that would render as terminal default
    // for the signature elements).
    #[test]
    fn default_theme_signature_elements_are_concrete() {
        let t = Theme::commander_dark();
        assert_eq!(t.panel_bg, Color::Blue);
        assert_ne!(t.dir_fg, Color::Reset);
        assert_ne!(t.cursor_bg, Color::Reset);
        assert_ne!(t.status_bg, Color::Reset);
    }

    // FR-007: the default theme uses only named (16-color) colors so it
    // degrades cleanly on limited terminals.
    #[test]
    fn default_theme_uses_only_named_colors() {
        let t = Theme::commander_dark();
        let colors = [
            t.panel_bg,
            t.panel_fg,
            t.dir_fg,
            t.exec_fg,
            t.symlink_fg,
            t.hidden_fg,
            t.cursor_bg,
            t.cursor_fg,
            t.marked_fg,
            t.border_focused,
            t.status_bg,
        ];
        for c in colors {
            assert!(
                !matches!(c, Color::Rgb(_, _, _) | Color::Indexed(_)),
                "default theme must use named colors for 16-color safety, got {c:?}"
            );
        }
    }

    #[test]
    fn executable_detection() {
        assert!(is_executable(Some(&mode(0o755))));
        assert!(is_executable(Some(&mode(0o744))));
        assert!(!is_executable(Some(&mode(0o644))));
        assert!(!is_executable(None));
    }
}

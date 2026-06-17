# Feature Specification: External / User-Authored Theme (Skin) Files

**Feature Branch**: `046-external-theme-skins`

**Created**: 2026-06-17

**Status**: Draft

**Input**: User description: "External / user-authored theme (skin) files — issue #49. Allow users to drop in custom TOML color-palette files (skins) that the app loads at startup alongside the two built-in themes (commander-dark, monochrome). A skin file lives at `~/.config/cargonaut/themes/<name>.toml` (XDG_CONFIG_HOME honored), is referenced by name in `ui.theme` in the main config, and maps each of the ~30 themeable element names to a color value (named 16-color, 256-color index, or RGB hex). Loading errors (bad path, bad color, unknown field) fall back to the built-in default and log a one-line status, never crashing. This closes issue #49 and unblocks user theming without requiring binary changes."

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Create and apply a custom color palette (Priority: P1)

A developer or power user wants a different color scheme — perhaps a light-background variant, a high-contrast mode, or a personal aesthetic — and does not want to wait for it to be added as a built-in. They create a TOML file at `~/.config/cargonaut/themes/my-theme.toml`, set `ui.theme = "my-theme"` in their config file (or pass `--theme my-theme` on the command line), and the app loads their palette on next launch.

**Why this priority**: This is the entire value proposition of the feature. Without P1 working, there is no external theming at all.

**Independent Test**: Create a skin file with a single distinctive color (e.g., red panel background), launch the app referencing it, and confirm the panel renders in that color.

**Acceptance Scenarios**:

1. **Given** a well-formed skin file at `~/.config/cargonaut/themes/dracula.toml` that overrides all color fields, **When** the app launches with `ui.theme = "dracula"`, **Then** the TUI renders with the skin's palette and no fallback status appears.
2. **Given** `ui.theme = "dracula"` and no skin file for "dracula" exists, **When** the app launches, **Then** the app starts with the built-in default palette and displays a one-line status ("Unknown theme "dracula" — using commander-dark" or equivalent) — it does not crash or block startup.
3. **Given** `--theme dracula` is passed on the command line and a matching skin file exists, **When** the app launches, **Then** the CLI override takes precedence over any config-file value.

---

### User Story 2 — Partial skin: override only some colors (Priority: P2)

A user wants to tweak just one or two colors (e.g., change the cursor highlight to green) without specifying every element. They create a skin file with only the fields they want to override; all unspecified fields inherit from the built-in default.

**Why this priority**: Requiring a complete 30-field file to change one color is a high barrier. Partial overrides lower the effort to near-zero for minor tweaks.

**Independent Test**: Create a skin file with a single `cursor_bg` field and confirm only the cursor color changes while all other elements render as the default.

**Acceptance Scenarios**:

1. **Given** a skin file that specifies only `cursor_bg = "#00ff00"`, **When** the app loads it, **Then** the cursor bar is rendered green and every other element uses the built-in default colors.
2. **Given** an empty skin file (no fields), **When** the app loads it, **Then** the app renders with the full built-in default palette and no error is shown (empty skin is valid, not an error).

---

### User Story 3 — Resilient loading: corrupt or invalid skin degrades gracefully (Priority: P2)

A user has a skin file with a typo in a color value (e.g., `panel_bg = "Bleu"`) or an unrecognized field name (`frobnicate = "Red"`). The app must not crash; it falls back to the built-in default and shows a one-line status identifying the problem.

**Why this priority**: Silent crashes or unclear errors when tweaking config are a reliability regression. Graceful degradation is a hard constitutional requirement (every user-facing operation must not crash on bad input).

**Independent Test**: Provide a skin file with an invalid color string; confirm the app starts with the default palette and shows a human-readable status.

**Acceptance Scenarios**:

1. **Given** a skin file containing `panel_bg = "Bleu"` (unknown color name), **When** the app loads it, **Then** the app falls back to the default theme and displays a one-line status identifying the file and the problematic field.
2. **Given** a skin file containing `frobnicate = "Blue"` (unknown field name), **When** the app loads it, **Then** the app falls back to the default and displays a one-line status identifying the unknown field.
3. **Given** a skin file that is not valid TOML (e.g., missing closing quote), **When** the app loads it, **Then** the app falls back to the default and displays a one-line parse-error status.

---

### User Story 4 — Three color formats in a single skin file (Priority: P1)

A user writing a skin file uses a mix of color formats: a named 16-color constant for portability (`"Blue"`), a 256-color palette index for precision (`196`), and an RGB hex string for truecolor richness (`"#ff8800"`). All three formats must work in the same file.

**Why this priority**: Without all three formats, the skin file format is artificially limiting. Named colors are for 16-color terminal compatibility; indexed/RGB are for richer displays. A skin author must be able to mix them freely.

**Independent Test**: Create a skin file with one field per format type, launch the app, and confirm each element renders in the correct color.

**Acceptance Scenarios**:

1. **Given** `panel_bg = "Blue"` (named), **When** loaded, **Then** the panel background is the terminal's named Blue.
2. **Given** `exec_fg = 196` (256-color index), **When** loaded, **Then** executable entries render in palette color 196.
3. **Given** `cursor_bg = "#ff8800"` (RGB hex), **When** loaded, **Then** the cursor bar renders in the specified RGB orange.

---

### Edge Cases

- **File not found**: `ui.theme` names a skin that does not exist as a built-in or a file — falls back to default, one-line status.
- **Directory instead of file**: `~/.config/cargonaut/themes/my-theme.toml` is a directory — treated as not-found; falls back to default.
- **Permission denied**: skin file exists but is not readable — falls back to default, one-line status with the OS error.
- **Empty file**: a zero-byte or blank skin file — treated as a valid empty partial skin, renders with all defaults.
- **All three color formats, including `"reset"` or `"none"`**: explicit color reset is valid for fields where inheriting the terminal default is intentional (e.g., `monochrome` theme uses `Color::Reset` for the panel background).
- **XDG_CONFIG_HOME override**: when `XDG_CONFIG_HOME` is set, skin files are searched in `$XDG_CONFIG_HOME/cargonaut/themes/` rather than `~/.config/cargonaut/themes/`.
- **Built-in name collision**: if a skin file is named `commander-dark.toml` or `monochrome.toml`, the built-in takes precedence and the file is ignored.
- **CLI `--theme` with a skin name**: the CLI flag works for skin names as well as built-in names.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The app MUST search for skin files in `<XDG_CONFIG_HOME>/cargonaut/themes/` (defaulting to `~/.config/cargonaut/themes/` when `XDG_CONFIG_HOME` is unset) when the configured theme name is not a built-in.
- **FR-002**: A skin file MUST be a TOML file named `<theme-name>.toml` whose keys are a subset of the ~30 theme element names; the value for each key MUST be a color specification in one of the three supported formats.
- **FR-003**: Supported color formats MUST include: (a) named 16-color identifiers (e.g., `"Blue"`, `"LightGreen"`, `"Reset"`), (b) 256-color palette index as an integer 0–255, and (c) RGB hex string `"#RRGGBB"`.
- **FR-004**: A skin file MAY omit any subset of element fields; omitted fields MUST inherit from the built-in default theme, not be left undefined.
- **FR-005**: When the configured theme name matches a built-in name (case-insensitively), the built-in MUST be used regardless of whether a skin file with that name exists.
- **FR-006**: Any load error (file not found, unreadable, TOML parse error, unknown field, invalid color value) MUST result in fallback to the built-in default theme AND a one-line human-readable status message at startup — the app MUST NOT crash or block startup.
- **FR-007**: The status message for a load error MUST identify at minimum: the skin name or file path, and a short description of the error (e.g., "Unknown color "Bleu" in field panel_bg").
- **FR-008**: The `--theme <name>` CLI flag MUST work for skin file names as well as built-in names, with the same fallback behavior on error.
- **FR-009**: Skin loading MUST NOT require a binary rebuild or code change — the full workflow is: create file, set config, restart app.
- **FR-010**: The feature MUST NOT introduce live-reload (re-reading the skin file while the app is running); restart is the intended update mechanism.

### Key Entities

- **SkinFile**: A TOML file at `<theme-dir>/<name>.toml`. Contains zero or more color-field mappings. Each mapping key is a theme element name; each value is a color in one of the three supported formats.
- **ThemeName**: A string value from `ui.theme` config or `--theme` CLI flag. Resolved first against built-in names, then as a skin file name.
- **ThemeDir**: The platform-appropriate directory for skin files: `$XDG_CONFIG_HOME/cargonaut/themes/` or `~/.config/cargonaut/themes/`.
- **ColorSpec**: A color value in one of: named string (`"Blue"`), integer index (0–255), or RGB hex string (`"#RRGGBB"`).

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can create a skin file with five color overrides, set `ui.theme`, and see their custom palette applied on next launch — without modifying any source code or recompiling.
- **SC-002**: A skin file with an invalid color value causes the app to start within the same startup time budget as a normal launch; the user sees a one-line status, the default palette, and can use the app normally.
- **SC-003**: All three color formats (named, 256-index, RGB hex) work in a single skin file without any format taking precedence or conflicting with the others.
- **SC-004**: An empty skin file (zero fields) is a valid no-op: the app renders identically to having no skin file at all.
- **SC-005**: Skin file resolution (find → parse → validate → apply) adds less than 5 ms to cold-start time, keeping total startup within the 150 ms SC-004 budget from the constitution.

## Assumptions

- No live-reload within a session; the user must restart the app to see skin changes take effect. This is the same behavior as the existing `--theme` CLI flag and config `ui.theme` field.
- The built-in default theme for fallback is `commander-dark`; the exact fallback name is already encoded as `DEFAULT_THEME_NAME` in the codebase.
- Skin file names are case-sensitive on case-sensitive filesystems (Linux); the theme name in config is matched to the filename literally.
- The set of valid theme element field names is the ~30 public fields of the existing `Theme` struct, documented in the skin file format specification that ships with this feature.
- No UI for listing or browsing available skins is in scope; that is a future improvement.
- No skin file validation tool or linter is in scope; the error message on load is the feedback mechanism.
- The `--theme` flag and `ui.theme` config value use the same resolution order (built-in first, then skin file); no separate `--skin` flag is introduced.

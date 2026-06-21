# Contract — CLI version output & in-app About view

## CLI `--version` (FR-011)

- Short (`-V` / bare `--version`): unchanged — `cargonaut <version>`.
- Long (`--version` long form via clap `long_version`, also shown in `--help`
  context): includes copyright + license, e.g.:

```text
cargonaut 0.1.0
© 2024–2026 Mohiuddin Khan Inamdar
License: MIT OR Apache-2.0
<repository URL>
```

- Implemented with clap derive: `#[command(version, long_version = LONG_VERSION)]`
  where `LONG_VERSION` is a `concat!` const built from `AboutInfo` fields.
- Exit code 0; no TUI launched; safe to run in scripts.

## In-app About — F1 Help section (FR-012)

- The existing `HELP_SECTIONS` entry titled `"About"` is enriched so its rows
  render `about_lines()`: name+version, author, copyright, license, repository.
- Reachable exactly as today: F1 opens the help overlay; the About section is one
  of its sections (SC-005: within two keystrokes of the main view).
- `help_covers_all_keymap_bindings` and existing help tests must still pass.

## In-app About — dedicated dialog (FR-012)

- New `Command::ShowAbout` → `ActiveDialog::About`, a centered modal rendering
  `about_lines()`.
- Reachable from the application menu bar (new "About" entry mapping to
  `Command::ShowAbout`). If a direct keybinding is also added, it MUST be defined
  in `design/contracts/keymap.toml` first (Constitution §III).
- Dismissed by `Esc` (and `Enter`); restores `Mode::Pane` and clears
  `active_dialog` (same lifecycle as other modals).

## Consistency guarantee

- All three surfaces (CLI long version, help section, About dialog) derive from
  the single `AboutInfo` / `about_lines()` source, so version/author/copyright/
  license can never drift between them (one unit test covers the strings — SC-005).

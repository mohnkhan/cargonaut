# Implementation Plan: External / User-Authored Theme (Skin) Files

**Branch**: `046-external-theme-skins` | **Date**: 2026-06-17 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/046-external-theme-skins/spec.md`

## Summary

Allow users to create TOML skin files (`~/.config/cargonaut/themes/<name>.toml`) mapping the ~30 theme element names to color values. The app resolves these at startup when `ui.theme` names an unknown built-in; on any error, it falls back to `commander-dark` with a one-line status. All changes are confined to `cargonaut-ui-tui/src/theme.rs` — no binary changes, no new crate deps, no new config fields.

## Technical Context

**Language/Version**: Rust 1.78+ (workspace-pinned)

**Primary Dependencies**: `toml` (already in `cargonaut-ui-tui`), `serde` (already in `cargonaut-ui-tui`), `ratatui::style::Color` (already in `cargonaut-ui-tui`)

**Storage**: TOML files in `$XDG_CONFIG_HOME/cargonaut/themes/` (read-only at load time)

**Testing**: `cargo test --workspace` (unit tests only; no PTY, no integration binary)

**Target Platform**: Linux terminal (all existing platforms supported by cargonaut-ui-tui)

**Project Type**: Library crate extension (cargonaut-ui-tui theme module)

**Performance Goals**: Skin file resolution adds <5 ms to cold startup (SC-005); overall startup remains within the 150 ms constitutional budget (SC-004 Phase 1)

**Constraints**: No new Cargo.toml dep changes; no changes to `cargonaut-config`, `cargonaut-core`, or the binary; `name: &'static str` → `name: String` (remove `Copy`, keep `Clone` + `PartialEq` + `Eq`)

**Scale/Scope**: ~250 lines of new code in `theme.rs`, ~60 lines of new tests

## Constitution Check

| Principle | Status | Notes |
|-----------|--------|-------|
| §I Code Quality — clippy -D warnings | PASS | No unsafe, no new warnings expected |
| §I Code Quality — missing_docs | PASS | All new public items will carry doc-comments |
| §II TDD — red before green | PASS | Tests written first in failing state |
| §III UX — typed theme variables | PASS | Skin files resolve to concrete `Color` values; no hardcoded ANSI |
| §IV Performance — ≤8 MiB binary | PASS | Adds ~0 KiB to stripped release binary |
| §IV Performance — ≤150 ms startup | PASS | Skin load is a single TOML parse + fs read; <5 ms |
| §V SSD Preservation | PASS | No build artifact changes; CI exempt |

No constitution violations.

## Project Structure

### Documentation (this feature)

```text
specs/046-external-theme-skins/
├── plan.md              ← this file
├── research.md          ← Phase 0 output
├── data-model.md        ← Phase 1 output
├── quickstart.md        ← Phase 1 output
├── contracts/           ← Phase 1 output
└── tasks.md             ← /speckit-tasks output
```

### Source Code Changes

```text
crates/cargonaut-ui-tui/src/theme.rs        ← primary change (extend existing file)
  - Theme struct: name: &'static str → String, remove Copy derive
  - Theme::commander_dark(), monochrome(): const fn → fn
  - Theme::resolve(): returns (Theme, Option<String>) instead of Theme
  - Theme::builtin(): updated for String name
  - New: ColorSpec enum (Named/Indexed)
  - New: parse_color_spec(cs) -> Result<Color, String>
  - New: SkinFile struct (#[serde(deny_unknown_fields)])
  - New: default_theme_dir() -> PathBuf (XDG logic, 4 lines)
  - New: load_skin(name, dir) -> Result<Theme, String>
  - New: Theme::from_skin(name, skin) -> Theme
  - New: 20+ unit tests

crates/cargonaut-ui-tui/src/lib.rs          ← small tweak
  - Line 207-209: update resolve call-site for new (Theme, Option<String>) return

No other files need changes.
```

## Complexity Tracking

No constitution violations — complexity tracking section is N/A.

## Technical Decisions (from research.md)

See [research.md](./research.md) for full rationale. Summary:

| Decision | Choice | Key Reason |
|----------|--------|-----------|
| Crate location for skin loading | `cargonaut-ui-tui/src/theme.rs` | No new deps; theme type lives here; toml+serde already present |
| `Theme.name` field type | `String` (was `&'static str`) | Skin names are dynamic; owned String is simplest |
| `Copy` derive | Removed | Required by `String` field; callers use `&theme` or clone already |
| Unknown TOML fields | Strict (`deny_unknown_fields`) | Spec says unknown field = error + fallback (FR-006, FR-008) |
| Partial skin files | Supported | Missing fields inherit from built-in default (FR-004) |
| `Theme::resolve` signature | Returns `(Theme, Option<String>)` | Error message needed at call-site without a separate lookup |
| Color spec format | `ColorSpec` enum (untagged serde) | Handles both string (`"Blue"`, `"#ff8800"`) and integer (`196`) |
| XDG path logic | Inline in theme.rs (4 lines) | Avoids adding cargonaut-config dep to ui-tui for 4 lines |

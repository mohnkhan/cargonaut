# Implementation Plan: User Menu (F2) + Scrollable Hypertext Help (F1)

**Branch**: `047-user-menu-help` | **Date**: 2026-06-18 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/047-user-menu-help/spec.md`

## Summary

Feature 031 shipped F1 (help) and F2 (user menu) as labeled stubs on the function-key bar; both produce a "not yet available" message. This feature replaces both stubs with live implementations:

1. **F1 — Scrollable help overlay**: a multi-section, compiled-in keybinding reference rendered as a full-screen scrollable modal overlay in `cargonaut-ui-tui`. Content covers all live bindings from `design/contracts/keymap.toml`, organized into named sections, scrollable with arrow/Page keys, dismissed with Esc/F1.

2. **F2 — User action menu**: a modal menu overlay whose items are loaded from `$XDG_CONFIG_HOME/cargonaut/menu.toml` on each F2 press. Items have a label, a shell command (with optional `{path}` placeholder shell-quoted at runtime), an optional `only_if` condition, and an optional single-char shortcut. Actions run async via Tokio so the TUI stays responsive.

All changes live in `crates/cargonaut-ui-tui` and `cargonaut-config`. No new crates are required. A new dependency `shell-words` (or `shell-quote`) is added to `cargonaut-ui-tui` for safe command tokenization.

## Technical Context

**Language/Version**: Rust 1.76 (MSRV per workspace)

**Primary Dependencies**:
- `ratatui 0.27` — TUI rendering (existing)
- `crossterm 0.28` — keyboard/mouse input (existing)
- `tokio 1.40` — async runtime for action execution (existing)
- `toml 0.8` / `serde` — `menu.toml` parsing (existing in workspace)
- `shell-words 1.x` — NEW: safe shell tokenization for commands with shell operators; falls back to `Command::new(prog).args(...)` when no operators detected

**Storage**: `$XDG_CONFIG_HOME/cargonaut/menu.toml` (user config file, read on F2 open, not cached)

**Testing**: `cargo test --workspace` (existing); new unit tests in `cargonaut-ui-tui`; new CI assertions that F1 content covers all keymap bindings

**Target Platform**: Linux (primary), macOS (secondary) — same as existing

**Project Type**: TUI desktop application (binary)

**Performance Goals**: F1 overlay opens in <100 ms (SC-001); F2 menu open + action launch in <500 ms (SC-003); `only_if` condition eval ≤200 ms with a timeout

**Constraints**:
- NFR-001: ≤8 MiB stripped release binary — compiled-in help text adds <10 KiB, safe
- NFR-002: ≤16 ms keypress→first-paint — overlay render is a Paragraph widget, ~0.5 ms
- NFR-003 / SC-003: ≤64 MiB RSS — no heap-heavy data structures introduced
- Constitution §Dev-Workflow: every destructive action needs confirmation — N/A here (no destructive actions; actions are user-defined)
- Constitution macro-safety: `{path}` substitution MUST use `shell-words` or `Command::new(...).arg(...)`, never raw string interpolation

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| §I Code Quality — clippy -D warnings | PASS | No new `unsafe`; all code must be clippy-clean |
| §II Test-First — red commit before green | PASS | TDD applies: write failing tests for each FR before implementation |
| §III UX Consistency — shared dialog! macro / widgets | PASS | New overlays reuse existing `dialog_style()`, `centered_rect()`, `ListState` patterns; no ad-hoc layouts |
| §III Keymap source-of-truth | PASS | F2 key already defined in `design/contracts/keymap.toml` as `show-user-menu`; F1 as `show-help` |
| §III Theme variables typed | PASS | Overlays use `theme.dialog_style()` — no hardcoded ANSI |
| §IV Performance — keypress ≤16 ms | PASS | Modal open is synchronous Paragraph render; action runs in Tokio task |
| §IV Binary size NFR-001 ≤8 MiB | PASS | Help text ≈8 KiB; `shell-words` adds ≈20 KiB stripped |
| §V SSD preservation | N/A | CI exempt; dev host uses `make tmpfs-setup` |
| §Dev-Workflow macro-safety | PASS | `shell-words::split()` + `Command::new(prog).args(args)` when safe; `sh -c` only when shell operators detected; `{path}` always quoted |

**No violations.** No Complexity Tracking entry required.

## Project Structure

### Documentation (this feature)

```text
specs/047-user-menu-help/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── menu-toml-schema.md  # menu.toml format contract
└── tasks.md             # Phase 2 output (/speckit-tasks)
```

### Source Code

```text
crates/cargonaut-ui-tui/src/
├── lib.rs               # MODIFY: replace help_open bool + HELP_BODY const with HelpOverlay widget;
│                        #         add UserMenu/ActiveDialog::UserMenu; wire dispatch
├── dialog.rs            # ADD: HelpOverlay struct + impl; UserMenuDialog struct + impl
└── (no new files)

crates/cargonaut-config/src/
└── lib.rs               # ADD: menu_config_path(), parse_menu_toml(), MenuItem, UserMenuConfig types

examples/
└── menu.toml            # NEW: commented example user menu config

design/contracts/
└── keymap.toml          # NO CHANGE (F1/F2 already defined)
```

## Complexity Tracking

No constitution violations requiring justification.

---

## Phase 0 Research Findings

See `research.md` for full details. Key decisions:

1. **Shell command execution strategy**: use `shell-words::split()` to tokenize the command string; if it produces exactly one token (no shell operators), use `Command::new(prog).arg(path_arg)`. If it produces multiple tokens, use `Command::new(tokens[0]).args(&tokens[1..])` with `{path}` replaced by the quoted path. Only if the command string contains shell metacharacters (`|`, `;`, `&&`, `||`, `$`, `` ` ``) fall back to `sh -c "cmd_with_quoted_path"`. The `shell-words::quote()` function produces the shell-safe quoted path token.

2. **Help content structure**: a `HelpSection` is a named group of `(key_label, description)` pairs. The compiled-in content is a `&'static [HelpSection<'static>]`. The overlay maintains a `scroll_offset: u16` and renders a window of lines. This replaces the current flat `HELP_BODY: &str` constant and the unit tests that assert on it.

3. **`only_if` condition evaluation**: use `std::process::Command::new("sh").arg("-c").arg(expr).status()` with a `.wait_with_output()` limited by a 200 ms wall-clock timeout via a Tokio `timeout()` wrapper at menu-open time (conditions are evaluated eagerly, not lazily). Timed-out conditions are treated as hidden (exit ≠ 0).

4. **menu.toml reload**: load fresh on every F2 press — no caching. The file is small (user-defined); stale-cache surprises are worse than a ~1 ms TOML parse.

5. **`shell-words` crate**: version `1.1` — MIT licensed, no transitive deps, <300 lines. Suitable for adding to `cargonaut-ui-tui`'s `[dependencies]`.

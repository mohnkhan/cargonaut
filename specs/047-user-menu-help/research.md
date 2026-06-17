# Research: User Menu (F2) + Scrollable Help (F1)

**Feature**: 047-user-menu-help
**Date**: 2026-06-18

---

## R-001: Shell command execution strategy for user-defined actions

**Question**: How do we safely execute a user-provided command string with a path substitution, honoring the constitution's macro-safety rule (no raw string interpolation)?

**Decision**: Tiered approach using `shell-words` crate:
1. Parse the command string with `shell-words::split()`.
2. Substitute `{path}` with `shell-words::quote(path).into_owned()` in the token list.
3. If the command string contains none of `|`, `;`, `&&`, `||`, `$`, `` ` ``, `>`, `<` → use `Command::new(&tokens[0]).args(&tokens[1..])` (no shell involved).
4. If shell metacharacters are detected → rebuild a quoted command line and run `sh -c "<quoted_cmd>"`.

**Rationale**: Constitution §Dev-Workflow says "prefer `Command::new(prog).arg(arg)` over `sh -c` to bypass the shell entirely." The tiered approach satisfies this preference while still supporting power-user commands with pipes and redirections.

**Alternatives considered**:
- Always use `sh -c`: simpler but wider attack surface; constitution explicitly discourages it.
- Always use `Command::new` split on whitespace: breaks paths with spaces and commands like `gzip -c | base64`.
- Forbid shell operators entirely: too restrictive for the intended "scriptable action menu" use case.

---

## R-002: `shell-words` vs `shell-quote` crates

**Decision**: Use `shell-words 1.1`.

**Rationale**:
- `shell-words` provides both `split()` (tokenize a shell command string) and `quote()` (safely escape a string for use in a shell command). One crate covers both needs.
- `shell-quote` (a different crate) only provides quoting, not splitting.
- `shell-words` is MIT-licensed, has no transitive dependencies, ~400 lines, actively maintained (last release 2023). Binary size impact: ≈20 KiB stripped.
- The workspace already uses `toml 0.8` and `serde` for TOML parsing — no additional parser needed.

**Alternatives considered**:
- `shlex`: similar scope but `shell-words` is better maintained and more widely used in the Rust ecosystem.
- Hand-rolled quoting: error-prone; explicitly called out as "forbidden" in the constitution.

---

## R-003: Help overlay scrolling model

**Question**: Should the help overlay use ratatui's `Paragraph` with built-in scroll, or a custom line-based scroll?

**Decision**: Use ratatui `Paragraph::new(text).scroll((offset, 0))` with a manually maintained `scroll_offset: u16`.

**Rationale**:
- `Paragraph::scroll((row, col))` is the idiomatic ratatui approach for scrollable text content.
- The existing `draw_help()` already uses `Paragraph`; extending it is minimal change.
- A `scroll_offset` field in a new `HelpOverlay` struct replaces the current `help_open: bool` field in `UiState`.
- `Home` key resets to 0; `End` key scrolls to `total_lines - visible_lines`.

**Alternatives considered**:
- `ratatui::widgets::List`: suited for interactive item selection, not read-only scrollable text.
- Custom line buffer: unnecessary complexity when `Paragraph::scroll` already handles this.

---

## R-004: Help content structure — static vs dynamic

**Question**: Should the help content be a flat `&'static str` (current) or a structured `&'static [HelpSection]`?

**Decision**: Replace `HELP_BODY: &str` with a `&'static [HelpSection<'static>]` where `HelpSection` has a `title: &'static str` and `rows: &'static [(&'static str, &'static str)]` (key, description).

**Rationale**:
- Structured sections enable rendering section headers distinctly (bold/colored) from key rows.
- The existing unit tests already assert on specific key mentions in `HELP_BODY`; these become assertions on the structured data (more precise).
- A CI test can iterate the structured content and assert that every action in `keymap.toml` appears in at least one row — this is SC-002.
- The struct is entirely `'static`, zero runtime allocation.

**Alternatives considered**:
- Keep `HELP_BODY: &str` and just add more content: can't programmatically verify completeness; hard to add visual section separators.
- Read from a file at runtime: breaks offline/embedded use; contradicts FR-008.

---

## R-005: menu.toml config path resolution

**Question**: How is `~/.config/cargonaut/menu.toml` resolved?

**Decision**: Use the same XDG pattern already established in `cargonaut-config::default_config_path()`:
```
$XDG_CONFIG_HOME/cargonaut/menu.toml
  └── fallback: $HOME/.config/cargonaut/menu.toml
```
A new `menu_config_path() -> PathBuf` function in `cargonaut-config` provides this. The TUI calls it at F2-open time.

**Rationale**:
- Mirrors the existing pattern for `config.toml` and `themes/` directory (Feature 046).
- No new dependency needed (`std::env::var` is sufficient; the `dirs` crate is not in the workspace and adding it for one path would be over-engineering).

---

## R-006: `only_if` condition timeout and async strategy

**Question**: How do we evaluate `only_if` shell conditions without blocking the TUI event loop?

**Decision**: Evaluate all conditions synchronously at F2-open time using `std::process::Command` with a dedicated thread-spawn timeout pattern:
- Spawn each `only_if` condition as a blocking `std::process::Command` in a `tokio::task::spawn_blocking` call.
- Apply `tokio::time::timeout(Duration::from_millis(200), ...)` around each spawn.
- Conditions that time out are treated as hidden (false).
- Since conditions are evaluated at F2-open (before the menu renders), the user experiences a brief (<200 ms) delay if conditions are slow — acceptable for a user-initiated action.

**Rationale**:
- The TUI event loop is async (Tokio); blocking `std::process::Command::status()` on it would stall rendering. `spawn_blocking` moves the syscall off the async executor thread.
- 200 ms timeout is tight enough to feel responsive and loose enough for a `git status --short` to return.

**Alternatives considered**:
- Pre-evaluate conditions in a background task between F2 presses: overly complex; creates stale-state risk.
- No timeout: a hanging `only_if` (e.g., waiting on a network mount) would freeze the UI.

---

## R-007: F2 menu interaction while another dialog is open (FR-021)

**Decision**: Guard F2 dispatch in `dispatch_ui_command` by checking `active_dialog.is_none()`. If any dialog is already open, the `ShowUserMenu` command is swallowed (returns immediately with no state change).

**Rationale**: Already the pattern for other commands that must not stack modals (e.g., F12 tasks panel is ignored if a confirm dialog is open). Consistent behavior.

---

## R-008: Binary size impact

**Measurements (estimated)**:
- Help content as structured static data: ~6 KiB text + ~2 KiB padding = ~8 KiB added to `.rodata`
- `shell-words 1.1` stripped: ~18 KiB
- TOML parser for `menu.toml`: already in the binary (feature 046 added `toml` to `cargonaut-ui-tui`)
- Total estimated increase: **~26 KiB** — well within the 32 KiB budget stated in SC-007

Current binary size baseline: within NFR-001's 8 MiB limit (verified by CI `check-binary-size.sh`).

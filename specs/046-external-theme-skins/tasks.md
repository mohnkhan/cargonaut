# Tasks: External / User-Authored Theme (Skin) Files

**Input**: Design documents from `specs/046-external-theme-skins/`

**Prerequisites**: plan.md ✓, spec.md ✓, data-model.md ✓, contracts/skin-file-format.md ✓, research.md ✓, quickstart.md ✓

**TDD**: Constitution §II is NON-NEGOTIABLE. Every functional requirement test MUST be committed in a **failing state** before the implementation that makes it pass. Git history MUST show `(red)` commit before `(green)` commit per task.

**Primary change file**: `crates/cargonaut-ui-tui/src/theme.rs`
**Secondary change file**: `crates/cargonaut-ui-tui/src/lib.rs` (one call-site update)

---

## Phase 1: Setup

_No project-level setup required — no new Cargo dependencies, no new crates, no CI config changes. All deps (`toml`, `serde`, `ratatui`) are already present in `cargonaut-ui-tui`._

**Checkpoint**: Branch `046-external-theme-skins` is checked out ✓

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Establish the core types and helper functions that every user story implementation depends on. Must complete before any story work.

**⚠️ CRITICAL**: Phases 3–5 cannot begin until this phase is complete.

- [ ] T001 Change `Theme.name` field: `&'static str` → `String`; remove `Copy` from `#[derive(...)]`; change `pub const fn commander_dark()` and `pub const fn monochrome()` to `pub fn` (drop `const`) in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T002 Add `ColorSpec` enum with `Named(String)` and `Indexed(u8)` variants, `#[derive(Debug, Clone, Deserialize)]`, and `#[serde(untagged)]` attribute to `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T003 Add `SkinFile` struct with all 25 `Option<ColorSpec>` fields (`panel_bg`…`dialog_sel_fg`), `#[derive(Debug, Default, Deserialize)]`, `#[serde(deny_unknown_fields)]`, and `#[serde(default)]` on each field to `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T004 Add `fn default_theme_dir() -> PathBuf` with inline XDG logic (`XDG_CONFIG_HOME` → `HOME/.config` → `.config` fallback) to `crates/cargonaut-ui-tui/src/theme.rs`

**Checkpoint**: `cargo check -p cargonaut-ui-tui` passes; existing tests still compile (they may fail on changed `Theme::resolve` return type — that is resolved in Phase 3)

---

## Phase 3: User Story 1 — Create and apply a custom color palette (P1) 🎯 MVP

**Goal**: A user can create a skin file, set `ui.theme = "<name>"`, and see their palette on next launch. Built-in names take precedence over skin files.

**Independent Test**: Create a temp-dir skin with red `panel_bg`, call `Theme::resolve("my-skin")` with `XDG_CONFIG_HOME` pointing at the temp dir, and confirm the returned theme has `panel_bg = Color::Red`.

**Also covers User Story 4 (P1) — three color formats**: color format parsing (`parse_color_spec`) is the foundational shared mechanism for US1 through US4, so its TDD cycle lives here.

### TDD: Red commits (write failing tests first)

- [ ] T005 [US1] Write failing unit test `skin_full_palette_loads`: call `Theme::resolve("dracula")` with temp `XDG_CONFIG_HOME` containing a full `dracula.toml` (all 25 fields), assert `(theme, None)` returned and `theme.panel_bg == Color::Rgb(40,42,54)` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T006 [US1] Write failing unit test `skin_missing_file_falls_back`: call `Theme::resolve("no-such-skin")` with temp `XDG_CONFIG_HOME` (no matching file), assert returned theme name is `"commander-dark"` and error string contains `"no-such-skin"` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T007 [US4] Write failing unit tests for `parse_color_spec` (three format variants): `parse_color_spec(Named("Blue")) == Color::Blue`, `parse_color_spec(Indexed(196)) == Color::Indexed(196)`, `parse_color_spec(Named("#ff8800")) == Color::Rgb(255,136,0)` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T029 [US1] Write failing unit test `skin_resolve_via_theme_name`: call `Theme::resolve` with a skin-file name (not a builtin) using temp `XDG_CONFIG_HOME`, verify the returned `(theme, None)` tuple; this tests the same code path that `lib.rs` exercises when `--theme <name>` is passed (FR-008) in `crates/cargonaut-ui-tui/src/theme.rs`

### TDD: Green commits (implement to make tests pass)

- [ ] T008 [US4] Implement `fn parse_color_spec(cs: &ColorSpec) -> Result<Color, String>`: dispatch `Indexed(u8)` → `Color::Indexed`; `Named` starting with `#` → parse hex `#RRGGBB` → `Color::Rgb`; `Named` otherwise → case-insensitive match to 17 ratatui named colors → error if unknown in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T009 [US1] Implement `fn Theme::from_skin(name: &str, skin: SkinFile) -> Theme`: start from `Theme::commander_dark()`, override each non-`None` field by calling `parse_color_spec` (propagate error on invalid color); set `name = name.to_owned()` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T010 [US1] Implement `fn load_skin(name: &str, dir: &Path) -> Result<Theme, String>`: read `dir/<name>.toml`, parse as `SkinFile`, call `Theme::from_skin`; on any `io::Error` or parse error, return `Err(formatted message)` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T011 [US1] Update `Theme::resolve(name: &str) -> (Theme, Option<String>)`: check `Theme::builtin(name)` first (return `(theme, None)` on hit); otherwise call `load_skin(name, &default_theme_dir())`; on `Ok(theme)` return `(theme, None)`; on `Err(msg)` return `(Theme::commander_dark(), Some(msg))` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T012 [US1] Update `Theme::builtin(name: &str) -> Option<Theme>` to return `Option<Theme>` with `String` name field (not `&'static str`) in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T013 [US1] Update `lib.rs` call-site: replace `let theme = Theme::resolve(&theme_name)` + builtin-check status block with `let (theme, theme_err) = Theme::resolve(&theme_name); let mut status: String = theme_err.unwrap_or_default();` in `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: `cargo test -p cargonaut-ui-tui` — T005, T006, T007 pass (green). `cargo build --workspace` succeeds.

---

## Phase 4: User Story 2 — Partial skin: override only some colors (P2)

**Goal**: A skin file with only `cursor_bg` specified inherits all other colors from `commander-dark`. An empty skin file is valid (renders full default palette, no error).

**Independent Test**: Call `Theme::resolve` with a one-field skin; confirm only that field differs from `commander_dark()`.

### TDD: Red commits

- [ ] T014 [US2] Write failing unit test `skin_partial_inherits_defaults`: skin file with only `cursor_bg = "Green"`; assert `theme.cursor_bg == Color::Green` AND `theme.panel_bg == Theme::commander_dark().panel_bg` (all other fields equal default) in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T015 [US2] Write failing unit test `skin_empty_equals_default`: zero-field skin file; assert returned theme equals `Theme::commander_dark()` and error is `None` in `crates/cargonaut-ui-tui/src/theme.rs`

### TDD: Green commits

_(T009 `Theme::from_skin` already fills `None` fields from `commander_dark()`. If T014/T015 are failing, the bug is in `from_skin` — fix it here.)_

- [ ] T016 [US2] Verify T014 and T015 pass with `cargo test -p cargonaut-ui-tui skin_partial skin_empty`; if failing, patch `Theme::from_skin` field-by-field fallback logic in `crates/cargonaut-ui-tui/src/theme.rs`

**Checkpoint**: `cargo test -p cargonaut-ui-tui` — T014, T015 pass (green).

---

## Phase 5: User Story 3 — Resilient loading: corrupt or invalid skin degrades gracefully (P2)

**Goal**: Every load-time error (invalid color, unknown field, bad TOML, missing file, permission denied) falls back to `commander-dark` and surfaces a human-readable one-line status. App never crashes.

**Independent Test**: Provide a skin file with `panel_bg = "Bleu"` → app returns `(commander-dark, Some(msg))` where `msg` contains `"Bleu"` and `"panel_bg"`.

### TDD: Red commits

- [ ] T017 [US3] [P] Write failing unit test `skin_invalid_color_falls_back`: skin with `panel_bg = "Bleu"` (unknown name); assert `(commander-dark, Some(msg))` and `msg` contains both `"panel_bg"` and `"Bleu"` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T018 [US3] [P] Write failing unit test `skin_unknown_field_falls_back`: skin with `frobnicate = "Blue"` (unknown TOML key); assert `(commander-dark, Some(msg))` and `msg` contains `"frobnicate"` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T019 [US3] [P] Write failing unit test `skin_bad_toml_falls_back`: skin file content is `panel_bg = "Blue` (unterminated string literal); assert `(commander-dark, Some(msg))` and `msg` is non-empty in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T020 [US3] Write failing unit test `default_theme_dir_xdg_override`: set `XDG_CONFIG_HOME=/tmp/xdg_test`; assert `default_theme_dir()` returns `PathBuf::from("/tmp/xdg_test/cargonaut/themes")` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T030 [US3] Write failing unit tests for `io::Error` edge cases: (a) skin path is a directory (not a file) → `(commander-dark, Some(msg))`; (b) skin file exists but is unreadable (`chmod 000` in tempdir on Linux) → `(commander-dark, Some(msg))`; both assert `msg` is non-empty in `crates/cargonaut-ui-tui/src/theme.rs`

### TDD: Green commits

_(The error propagation through `load_skin` and `Theme::resolve` was established in Phase 3. If T017–T019 fail, the error message format needs refinement. T020 tests `default_theme_dir` directly.)_

- [ ] T021 [US3] Fix error message formatting in `load_skin` / `Theme::from_skin` so all error messages identify field name and bad value where applicable; ensure `parse_color_spec` error includes the field name context in `crates/cargonaut-ui-tui/src/theme.rs`

**Checkpoint**: `cargo test -p cargonaut-ui-tui` — T017–T020 pass (green). `cargo test --workspace` all green.

---

## Phase 6: Polish & Cross-Cutting Concerns

**Purpose**: CI gate, missing-docs compliance, documentation updates.

- [ ] T022 Run `cargo clippy --workspace --all-targets -- -D warnings` and fix any new warnings introduced by Feature 046 code in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T023 Add `///` doc-comments to all new public items in `theme.rs` (`ColorSpec`, `SkinFile`, `parse_color_spec`, `load_skin`, `default_theme_dir`, `Theme::from_skin`) to satisfy `#![warn(missing_docs)]` in `crates/cargonaut-ui-tui/src/theme.rs`
- [ ] T028 Write a `harness = false` bench `skin_resolve_latency` (following the `keypress_latency.rs` pattern) that calls `Theme::resolve` 1 000 times with an existing skin file in a temp dir, asserts mean iteration time <5 ms, and exits non-zero on failure (SC-005 CI gate) in `crates/cargonaut-ui-tui/benches/theme_resolve.rs`; add `[[bench]] name = "theme_resolve" harness = false test = false` entry to `crates/cargonaut-ui-tui/Cargo.toml`
- [ ] T024 Run `make ci-local` and confirm all tests pass (expected: 341+ unit + 12 integration, all green)
- [ ] T025 [P] Update `README.md`: increment test count in badge + "At a Glance" table; add Feature 046 one-line entry in the Feature History section
- [ ] T026 [P] Append Feature 046 section to `Learnings.md` (≥3 bullets: what was hard, root causes, non-obvious decisions)
- [ ] T027 [P] Add Feature 046 entry at top of `CHANGELOG.md`

**Checkpoint**: `make ci-local` green; all three doc files updated.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 2 (Foundational)**: No dependencies — start immediately
- **Phase 3 (US1+US4)**: Requires Phase 2 complete — BLOCKS Phases 4 and 5
- **Phase 4 (US2)**: Requires Phase 3 complete
- **Phase 5 (US3)**: Requires Phase 3 complete; can run in parallel with Phase 4
- **Phase 6 (Polish)**: Requires Phases 4 and 5 complete

### Within-Phase Task Dependencies

- T001 → T002 → T003 → T004 (all in theme.rs; sequential)
- T005, T006, T007 can be written in parallel (all unit test functions in theme.rs, no data dependency between them; serialize for single agent)
- T008 → T009 → T010 → T011 → T012 → T013 (implementation chain)
- T014, T015 can be written in parallel (independent unit tests)
- T017, T018, T019 can be written in parallel (independent unit tests); T020 is also parallel
- T025, T026, T027 are parallel (different files)

### TDD Sequencing (Constitution §II)

Per constitutional requirement, the git history MUST show:

```
T005 (red): unit test skin_full_palette_loads — FAILING
T006 (red): unit test skin_missing_file_falls_back — FAILING
T007 (red): unit tests parse_color_spec (3 formats) — FAILING
T008 (green): impl parse_color_spec — T007 passes
T009 (green): impl Theme::from_skin — T005 progresses
T010 (green): impl load_skin — T005 progresses
T011 (green): impl Theme::resolve (new sig) — T005, T006 pass
T012 (green): fix Theme::builtin String name — workspace compiles
T013 (green): update lib.rs call-site — full build passes
T014 (red): unit test skin_partial_inherits_defaults — FAILING or PASSING (from_skin already handles)
T015 (red): unit test skin_empty_equals_default — FAILING or PASSING
T016 (green): patch from_skin if needed — T014, T015 pass
T017 (red): unit test skin_invalid_color_falls_back — FAILING
T018 (red): unit test skin_unknown_field_falls_back — FAILING
T019 (red): unit test skin_bad_toml_falls_back — FAILING
T020 (red): unit test default_theme_dir_xdg_override — FAILING
T021 (green): fix error message formatting — T017–T020 pass
```

---

## Parallel Example: Phase 3

```bash
# Write all three red-commit test stubs (serialize for single agent, all theme.rs):
Task: T005 — skin_full_palette_loads test stub
Task: T006 — skin_missing_file_falls_back test stub
Task: T007 — parse_color_spec three-format test stubs
# Then implement the green chain: T008 → T009 → T010 → T011 → T012 → T013
```

---

## Implementation Strategy

### MVP First (US1 + US4 Only)

1. Complete Phase 2: Foundational
2. Complete Phase 3: US1 + US4 TDD cycle (T005–T013)
3. **STOP and VALIDATE**: `cargo test -p cargonaut-ui-tui` green; smoke-test with a real skin file
4. Complete Phases 4, 5: US2, US3 TDD cycles
5. Complete Phase 6: Polish + docs

### Incremental Delivery

- After Phase 3: Full skin loading works (most users need this)
- After Phase 4: Partial/empty skins work (lowers authoring barrier)
- After Phase 5: Errors degrade gracefully (reliability complete)

---

## Notes

- All code changes are confined to two files: `theme.rs` and `lib.rs` (one call-site)
- No new Cargo deps — `toml`, `serde`, `ratatui` are already in `cargonaut-ui-tui/Cargo.toml`
- `default_theme_dir` uses environment variables; tests that call it MUST set `XDG_CONFIG_HOME` in a temp dir to avoid side effects from the developer's real `~/.config`
- `#[serde(deny_unknown_fields)]` on `SkinFile` is the mechanism for US3 AC2 (unknown field error); it fires at TOML deserialization, not in custom code
- `Theme::commander_dark()` is the fallback; it is also the default fill-source in `Theme::from_skin`
- Built-in precedence (FR-005) is enforced by the `Theme::builtin()` check in `Theme::resolve` running before any file I/O

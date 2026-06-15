# Implementation Plan: Directory Hotlist / Bookmarks

**Branch**: `042-directory-hotlist` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/042-directory-hotlist/spec.md`

## Summary

Implement a persisted directory hotlist (issue #42, Feature 031 §Out-of-Scope
follow-up). `Ctrl-b` (already bound to the placeholder `Command::BookmarksMenu`)
opens a modal popup listing saved bookmarks organized by group; selecting one
navigates the active pane to its directory. Inside the popup, a key adds the
active pane's current directory as a new bookmark (prompting for a `group/name`)
and another key removes the highlighted entry. The hotlist persists to a
dedicated TOML state file under `~/.local/state/cargonaut/` (honoring
`$XDG_STATE_HOME`), separate from `config.toml`.

The work splits cleanly across the existing crate boundaries:
- **`cargonaut-config`** owns the `Bookmark`/`Hotlist` data types, their TOML
  (de)serialization, `default_hotlist_path()` (XDG-aware, mirrors
  `default_config_path()`), and `Hotlist::load`/`save`. This is the
  deterministic persistence gate (SC-002/SC-005), unit-tested with tempfiles.
- **`cargonaut-core`** holds the loaded `Hotlist` on `App`, loads it at
  construction (best-effort; malformed → empty + notice, FR-013), and exposes
  UI-agnostic `bookmarks()`, `add_bookmark`, `remove_bookmark`, and
  `jump_to_bookmark` (the last reuses `App::quick_cd`, so FR-008 invalid-path
  rejection + history recording come for free).
- **`cargonaut-ui-tui`** adds a `HotlistDialog` widget (modeled on
  `TasksPanelDialog`) + an `ActiveDialog::Hotlist` variant, wires
  `Command::BookmarksMenu` in `dispatch_ui_command` to open it, and handles the
  in-popup select/add/remove keys (add chains into the existing
  `TextInputDialog` for the name).

## Technical Context

**Language/Version**: Rust (workspace edition; same toolchain as the rest of
Cargonaut).

**Primary Dependencies**: `serde` + `toml` (already used by `cargonaut-config`
for `config.toml`) for hotlist (de)serialization; `ratatui`/`crossterm` for the
popup; existing `cargonaut-core` (`App`, `quick_cd`, `navigate_to`,
`active_pane_state`); existing `cargonaut-ui-tui::dialog` widgets. **No new
crates.**

**Storage**: A single TOML state file (the hotlist) under
`~/.local/state/cargonaut/hotlist.toml` (or `$XDG_STATE_HOME/cargonaut/...`).
Read once at `App` construction; rewritten on each add/remove.

**Testing**: `cargo test --workspace`. Persistence (load/save/round-trip/
malformed) tested in `cargonaut-config` with explicit tempfile paths (SC-002/
SC-005 gates — no env globals, race-free). `App` bookmark methods tested in
`cargonaut-core` (`#[tokio::test]`, tempdir fixtures, injecting `hotlist_path`).
Popup widget + wiring tested in `cargonaut-ui-tui` (`TestBackend` render +
synthetic key sequences), mirroring the `TasksPanelDialog` tests.

**Target Platform**: Linux terminal (TUI).

**Project Type**: Single Rust workspace (multi-crate). Touches config (data +
IO), core (state + ops), and ui-tui (popup) — one concern per crate.

**Performance Goals**: Honor NFR-002 (≤16 ms keypress→first-paint). Hotlist IO
is one small file read at startup and one small write per mutation — never on
the render path. Popup render is a bounded list (`TestBackend`-tested) with no
per-frame IO.

**Constraints**: Honor NFR-001 (≤8 MiB binary) — no new deps. `unsafe`-free.
`#![warn(missing_docs)]` on new public items. Keymap binding already lives in
`design/contracts/keymap.toml` (`C-b` → `bookmarks-menu`); no new binding.

**Scale/Scope**: New `Bookmark`/`Hotlist` types + IO in config; `App` field +
~4 methods in core; 1 dialog widget + 1 `ActiveDialog` variant + dispatch/key
wiring in ui-tui; plus tests. Personal-scale data (tens to low-hundreds of
bookmarks); whole file rewritten on save (no incremental store needed).

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality**: No `unsafe`. New public items (`Bookmark`, `Hotlist`,
  `App::bookmarks`/`add_bookmark`/`remove_bookmark`/`jump_to_bookmark`,
  `HotlistDialog`, `HotlistAction`) carry doc comments. clippy `-D warnings` +
  `cargo fmt --check` enforced by CI. ✅ (no waivers)
- **II. Test-First**: Every FR/SC gets a failing test first (red) then green,
  with `(red)`→`(green)` commit history. Persistence SCs (SC-002/SC-005) gated
  by config-crate round-trip tests; jump/add/remove (SC-001/003/005) by core
  tests; grouping (SC-007) + empty-state (SC-006) + missing-target (SC-004) by
  core/ui tests. ✅
- **III. UX Consistency**: The popup reuses the shared `dialog.rs` widget family
  (modeled on `TasksPanelDialog`), not an ad-hoc layout; the name prompt reuses
  `TextInputDialog`. Binding is already in `keymap.toml` (single source of
  truth). Typed theme colors only. Help overlay updated for `Ctrl-b`. ✅
- **IV. Performance (NFR-001/002)**: No new deps. File IO off the render path
  (startup + on mutation only). No measurable frame-budget impact. ✅
- **V. SSD Preservation**: Dev build/test via `make` wrappers; no `cargo clean`/
  `rm -rf target`. ✅

**Result**: PASS — no violations, Complexity Tracking empty.

## Project Structure

### Documentation (this feature)

```text
specs/042-directory-hotlist/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/           # Phase 1 output (hotlist + dialog seam)
└── tasks.md             # Phase 2 output (/speckit-tasks — NOT created here)
```

### Source Code (repository root)

```text
crates/cargonaut-config/src/
└── lib.rs        # + Bookmark/Hotlist types, default_hotlist_path(),
                  #   Hotlist::load/save/add/remove + tests

crates/cargonaut-core/src/
└── lib.rs        # + App.hotlist + App.hotlist_path, load at App::new,
                  #   bookmarks()/add_bookmark()/remove_bookmark()/
                  #   jump_to_bookmark() (reuses quick_cd) + tests

crates/cargonaut-ui-tui/src/
├── dialog.rs     # + HotlistDialog widget + HotlistAction + tests
└── lib.rs        # + ActiveDialog::Hotlist, dispatch BookmarksMenu → open,
                  #   in-popup select/add/remove key handling, help text + tests

design/contracts/keymap.toml   # (unchanged — C-b → bookmarks-menu already present)
```

**Structure Decision**: Single workspace, three-crate split by concern
(config = data+IO, core = state+ops, ui-tui = popup). This matches the
established seam used by Features 038/039 (a core method behind a `dialog.rs`
widget dispatched through `dispatch_ui_command`), and puts persistence in the
crate that already owns TOML IO so the SC-002/SC-005 gates are pure and
race-free.

## Complexity Tracking

> No constitution violations — section intentionally empty.

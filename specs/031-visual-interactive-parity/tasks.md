---
description: "Task list — Visual & Interactive Parity Layer (031)"
---

# Tasks: Visual & Interactive Parity Layer

**Input**: Design documents from `specs/031-visual-interactive-parity/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: REQUIRED — constitution §II (Test-First, NON-NEGOTIABLE). Every FR lands as a failing test (red commit `Tnnn (red): …`) before the implementation that makes it pass (green commit `Tnnn (green): …`).

**Organization**: grouped by user story (US1–US5, priority order). Each story is an independently testable increment.

## Path Conventions

Rust workspace. Crates: `crates/cargonaut-core`, `crates/cargonaut-ui-tui`, `crates/cargonaut-config`, `crates/cargonaut-bin`, `crates/cargonaut-vfs`. Keymap contract: `design/contracts/keymap.toml`. Build/test via `make build` / `make test` (tmpfs-guarded, §V).

---

## Phase 1: Setup (Shared Infrastructure)

- [x] T001 Confirm workspace builds clean before changes: `make build && make test` green; record baseline test count + binary size in the PR scratch notes.
- [ ] T002 [P] Add `globset` dependency to `crates/cargonaut-core/Cargo.toml` (and re-export path if needed) for pattern selection (US5); verify `cargo tree` shows it and binary-size headroom remains within NFR-001.
- [x] T003 [P] Add the `theme` and `chrome` module declarations as empty stubs in `crates/cargonaut-ui-tui/src/lib.rs` (`pub mod theme; pub mod chrome;`) with `#![warn(missing_docs)]`-compliant module docs so later phases compile incrementally.

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: structural changes every later story builds on. MUST complete before US1–US5.

- [x] T004 Flip `UiConfig::mouse` default to `true` in `crates/cargonaut-config/src/lib.rs` (US3 default-on); update the doc comment and `config.schema.json` default; add/adjust a config default unit test (red→green).
- [x] T005 Fix the dead CLI flags in `crates/cargonaut-bin/src/main.rs`: merge `cli.theme` → `config.ui.theme` and `cli.mc_keys` → `config.ui.mc_keys` before `App::new`; add a `--no-mouse` flag that sets `config.ui.mouse=false`. (red: test that `--theme X`/`--no-mouse` change the effective config; green: implement.)
- [x] T006 Introduce `FrameLayout { left, right, status, menu, fkeys, ministatus_left, ministatus_right }` in `crates/cargonaut-ui-tui/src/lib.rs` and make `draw_frame` RETURN it (lift the rects currently local at the old draw_frame body); store the latest in `run_loop`. (red: unit test asserting `draw_frame` yields the expected rects for a known terminal size via `TestBackend`; green: implement.) Blocks all mouse hit-testing.
- [ ] T007 Add the new `Command` variants to `crates/cargonaut-core/src/lib.rs` `Command` enum: `CursorTo(usize)`, `Mkdir(String)`, `SelectByPattern(String)`, `UnselectByPattern(String)`, `CycleSortKey`, `ToggleSortReverse`, `CycleListingMode`, `RecursiveDirSize`, `ToggleQuickView`, `ViewExternal`, `EditExternal`, `OpenMenuBar`, `ShowHelp` (compile-only stubs returning a `Status("not yet implemented")` where behavior lands later). Keeps the crate compiling for parallel work.

**Checkpoint**: workspace compiles; config + flags + layout-lift + command surface in place.

---

## Phase 3: User Story 1 — Themed visual appearance (Priority: P1) 🎯 MVP

**Goal**: the app renders with a real color palette; entry types, cursor, and marked rows are visually distinct; theme selectable by name. (FR-001..007; SC-001/002/008)

**Independent test**: `cargo run -- ~/ /tmp` shows blue panels + colored entries; `--theme monochrome` changes palette; `--theme bogus` falls back without crashing.

### Tests (write first — red)

- [x] T008 [P] [US1] Theme resolution tests in `crates/cargonaut-ui-tui/src/theme.rs` (`#[cfg(test)]`): T-THEME-1 every element returns a concrete `Color`; T-THEME-2 `resolve("nope")` → default + notice; per contracts/themes.md.
- [x] T009 [P] [US1] Per-entry style test: directory/executable/symlink/regular/hidden produce distinct styles (T-THEME-3) and cursor vs marked vs normal are distinct (T-THEME-4), via a helper that maps `(VfsKind, mode, hidden, selected, cursor)` → `Style`.
- [x] T010 [P] [US1] `TestBackend` render test in `pane.rs` asserting a directory row uses `theme.dir_fg` and the cursor row uses `theme.cursor_bg` under `commander-dark`.

### Implementation (green)

- [x] T011 [US1] Implement the `Theme` struct + fields in `crates/cargonaut-ui-tui/src/theme.rs` per data-model.md.
- [x] T012 [US1] Implement `Theme::builtin` / `Theme::resolve` with built-ins `commander-dark` (default) and `monochrome` per contracts/themes.md; unknown → default + status notice (FR-006).
- [x] T013 [US1] Thread `&Theme` through `draw_frame` → `draw_pane` in `crates/cargonaut-ui-tui/src/lib.rs`; build the `Theme` once in `run_loop` from `app.config().ui.theme`.
- [x] T014 [US1] Apply per-entry colors in `PaneView::render` (`crates/cargonaut-ui-tui/src/pane.rs`): style each `ListItem` by `entry.meta.kind`/mode/hidden; set highlight (cursor) style to `theme.cursor_*`; marked rows to `theme.marked_*` (replaces the bare `Modifier::REVERSED` at the old pane.rs:185/191).
- [x] T015 [US1] Apply theme to borders + status line in `draw_pane`/status render (focused vs unfocused border color; `theme.status_*` instead of `Modifier::REVERSED`).
- [x] T016 [US1] Apply theme to existing dialogs (`crates/cargonaut-ui-tui/src/dialog.rs` Confirm/Resume render) — `&Theme` param, `theme.dialog_*`.
- [x] T017 [US1] Low-color degrade check (FR-007): ensure `commander-dark` uses named/indexed colors that render on a 16-color `TestBackend`; add a render test at reduced color depth.

**Checkpoint**: US1 independently demoable — colored UI; theme switching works.

---

## Phase 4: User Story 2 — Screen chrome: function-key bar + menu bar (Priority: P1)

**Goal**: top menu bar, bottom F-key bar, per-pane mini-status; bar/menu invoke real commands; deferred actions say "not yet available". (FR-008..012; SC-001/004/005)

**Independent test**: launch → both bars visible; F9 opens a menu; selecting an item runs its command; mini-status updates on cursor move; narrow terminal abbreviates without panic.

### Tests (red)

- [x] T018 [P] [US2] `FunctionKeyBar` render + label test in `crates/cargonaut-ui-tui/src/chrome.rs` (`TestBackend`): the 10 canonical labels render; per-button sub-rects are exposed for hit-testing.
- [x] T019 [P] [US2] `MenuBar` test: titles render; opening a menu yields an item list; selecting an item resolves to the expected `Command`.
- [x] T020 [P] [US2] Mini-status test in `pane.rs`/`chrome.rs`: highlighted entry produces a status line containing name/size/mtime/perms.
- [x] T021 [P] [US2] Narrow-terminal degrade test: rendering chrome at width 20 truncates labels and does not panic (FR-012).

### Implementation (green)

- [x] T022 [US2] Implement `FunctionKeyBar` (labels F1..F10 per contracts/commands-delta.md; `available` flag) and `MenuBar` (+ dropdown overlay) widgets in `chrome.rs`, themed via `&Theme`, exposing their `Rect`s.
- [x] T023 [US2] Implement per-pane `MiniStatus` line (name/size/mtime/perms via `config.ui.date_format`).
- [x] T024 [US2] Extend the main layout in `draw_frame` to `[menubar(1), panes(min), ministatus(1)×2, status(1), fkeybar(1)]`; populate the menu/fkey/ministatus fields of `FrameLayout` (T006); degrade constraints on small terminals.
- [x] T025 [US2] Wire menu/F-key activation through the keyboard path: add `OpenMenuBar`/`ShowHelp` handling and menu-item → `Command` dispatch in `run_loop`; deferred commands emit a "not yet available" status (FR-011) — never a silent no-op.
- [x] T026 [US2] Add/confirm bindings in `design/contracts/keymap.toml` for F9 menu, F1 help (single source of truth, §III); ensure `ui_command_to_core` maps `OpenMenuBar`/`ShowHelp`.

**Checkpoint**: US2 demoable — recognizable OFM chrome; actions discoverable and labeled.

---

## Phase 5: User Story 3 — Mouse support (Priority: P1)

**Goal**: click to focus/move cursor, double-click to descend, wheel to scroll, click bar/menu; default on; disable preserves native selection. (FR-013..018; SC-003/004)

**Independent test**: with mouse on, click rows, double-click a dir, scroll, click an F-key button + menu title; `--no-mouse` → keyboard-only + native selection.

### Tests (red)

- [x] T027 [P] [US3] Hit-test unit tests per contracts/mouse-interaction.md: T-MOUSE-2 click in right panel sets active=Right + correct index; T-MOUSE-6 click outside regions is a no-op; row→index math with scroll offset.
- [x] T028 [P] [US3] Double-click rule test (T-MOUSE-3): two left-downs same row within window → `Descend`; different rows → two cursor moves, no descend.
- [x] T029 [P] [US3] `CursorTo` core dispatch test in `crates/cargonaut-core` (tokio+TempDir+LocalFs): `CursorTo(n)` clamps to visible len and survives a subsequent `sync_from`.
- [x] T030 [P] [US3] Capture-disabled test (T-MOUSE-1): with `mouse=false`, a synthesized mouse event changes no state.

### Implementation (green)

- [x] T031 [US3] Implement `Command::CursorTo(usize)` dispatch in `crates/cargonaut-core/src/lib.rs` (clamp to visible subset; set authoritative pane cursor).
- [x] T032 [US3] Enable/disable `EnableMouseCapture`/`DisableMouseCapture` in `run()` gated on `app.config().ui.mouse` (default on); symmetric best-effort teardown.
- [x] T033 [US3] Implement `handle_mouse(MouseEvent, &FrameLayout, …)` in `run_loop`: left-click → focus + `CursorTo`; double-click (track `(col,row,Instant)`, ~400 ms) → `Descend`; scroll → `CursorUp/Down`; replace the catch-all that currently drops mouse events.
- [x] T034 [US3] Hit-test the chrome: clicking an F-key button (e.g. #7) or a menu title dispatches its `Command`/opens the menu (uses T022 sub-rects). (T-MOUSE-5)
- [ ] T035 [US3] Add the runtime mouse-toggle key + document the hold-modifier bypass (FR-013) in keymap.toml + help text.

**Checkpoint**: US3 demoable — full mouse-driven navigation; the headline defect is visibly fixed.

---

## Phase 6: User Story 4 — Richer panel listing (Priority: P2)

**Goal**: mtime/perms columns, `..` entry, sort cycle+reverse, brief/full/quick-view modes, recursive dir-size. (FR-019..023; SC-006)

**Independent test**: listing shows mtime/perms + working `..`; C-s cycles sort; M-t cycles modes incl. quick-view preview; C-Space computes dir size without freezing.

### Tests (red)

- [ ] T036 [P] [US4] Column-format unit tests in `pane.rs`: name/size/mtime(`date_format`)/perms(mode bits) for a known `VfsMetadata`.
- [ ] T037 [P] [US4] Parent-entry tests in `crates/cargonaut-core`: `..` synthesized as row 0 except at root; activating it ascends (FR-020).
- [ ] T038 [P] [US4] Sort tests: `CycleSortKey` rotates among Name/Ext/Size/Mtime; `ToggleSortReverse` inverts; listing reorders (FR-021).
- [ ] T039 [P] [US4] Listing-mode tests: `CycleListingMode` brief→full→quick-view; quick-view bounding (≤256 KiB/≤1000 lines; binary/oversized → placeholder kind) (FR-022).
- [ ] T040 [P] [US4] Recursive dir-size test: async walk returns total; UI loop not blocked (simulate via a large temp tree) (FR-023).

### Implementation (green)

- [ ] T041 [US4] Promote the per-pane sort from hardcoded `NameAsc` (old core lib.rs:270) to a mutable field; implement `CycleSortKey`/`ToggleSortReverse` applying `cargonaut-vfs::Sort`; surface active order in mini-status.
- [ ] T042 [US4] Synthesize the `..` parent entry in the listing model + handle its activation (ascend) in core dispatch; suppress at filesystem root.
- [ ] T043 [US4] Render mtime + perms columns in `PaneView::render` for the "full" mode; implement "brief" (name-only, multi-column) layout; `CycleListingMode` switches them.
- [ ] T044 [US4] Implement quick-view: `ToggleQuickView`/mode that makes the passive panel show a bounded `QuickView` preview of the active pane's highlighted file; read off the frame path; placeholder for non-text/binary/oversized.
- [ ] T045 [US4] Implement `RecursiveDirSize`: spawn an async VFS walk; update the highlighted dir's displayed size when done without blocking the loop.

**Checkpoint**: US4 demoable — informative, navigable panels.

---

## Phase 7: User Story 5 — Operation parity (Priority: P2)

**Goal**: mkdir, pattern select/unselect, live transfer progress dialog, F3/F4 external pager/editor. (FR-024..027, FR-030/031; SC-005/007)

**Independent test**: F7 creates a dir; `+`/`-` tag by glob; copy shows live progress + ETA + cancel; F3 opens `$PAGER`, F4 opens `$EDITOR` and returns cleanly.

### Tests (red)

- [ ] T046 [P] [US5] Mkdir round-trip test (`crates/cargonaut-core`, tokio+TempDir+LocalFs): `Mkdir(name)` creates the dir and refreshes; invalid name/permission error reported, no panic (FR-024).
- [ ] T047 [P] [US5] Pattern-select tests: `SelectByPattern("*.rs")` tags matches via `globset`; `UnselectByPattern` untags; zero-match reports zero (FR-025).
- [ ] T048 [P] [US5] `ProgressView` projection test: from a synthesized `Running{bytes_done,total,throughput,eta}` event the view exposes current item/progress/throughput/ETA (FR-026).
- [ ] T049 [P] [US5] Progress dialog render test (`dialog.rs`, `TestBackend`) + cancel routes to the existing `CancellationToken` (FR-027).

### Implementation (green)

- [ ] T050 [US5] Implement `MkdirPromptDialog` (reuse shared dialog widgets, §III) + `Command::Mkdir(name)` dispatch (VFS create dir + refresh).
- [ ] T051 [US5] Implement `PatternPromptDialog` + `SelectByPattern`/`UnselectByPattern` dispatch over the visible listing using `globset`.
- [ ] T052 [US5] Implement `ProgressView` projection in core from existing transfer `Running` events; add `TransferProgressDialog` in `dialog.rs` (current item, per-op + overall progress, throughput, ETA); show while ≥1 transfer running; dismiss on complete/cancel + refresh target panel (replaces the status-bar count placeholder).
- [ ] T053 [US5] Implement `ViewExternal`/`EditExternal` in `run_loop`: suspend alt-screen + raw mode, run `Command::new(pager_or_editor).arg(path)` (`$PAGER`→less/more, `$EDITOR`→vi/nano; no `sh -c`, §Dev-Workflow), restore terminal, refresh panel (FR-030/031).
- [ ] T054 [US5] Confirm keymap.toml bindings for `+`/`-`/F7/C-s/M-t/C-Space/F3/F4 and that `ui_command_to_core` maps each (contracts/commands-delta.md, T-CMD-1/2).

**Checkpoint**: US5 demoable — daily operations complete with live feedback.

---

## Phase 8: Polish & Cross-Cutting Concerns

- [ ] T055 [P] Deferred-command audit: every keymap command not wired this feature emits a clear "not yet available" status (SC-005, FR-011); add a test enumerating them.
- [ ] T056 Performance gates: run `cargo bench` keypress-latency (NFR-002 ≤16 ms) and `scripts/check-binary-size.sh` (NFR-001 ≤8 MiB); record results; investigate if theming/chrome regressed either >10% (§IV).
- [ ] T057 [P] Full regression: `make ci-local` green (clippy -D warnings, `cargo test --workspace`, release build, docs-gate, binary-size); confirm no existing keybinding changed behavior (FR-028, SC-010).
- [ ] T058 [P] Open GitHub issues + add ROADMAP.md rows for every "Out of Scope" item (internal viewer/editor, find-file, hotlist, compare-dirs, panelize, subshell, tabs, chmod/chown, sym/hardlink, bulk-rename, undo, external skins, user menu, VFS archive/remote) per CLAUDE.md deferral policy (FR-029, SC-009).
- [ ] T059 Docs (MANDATORY per CLAUDE.md): update `README.md` At-a-Glance (test count, feature count) + Feature History (Feature 031 entry); append a `Learnings.md` section (≥3 bullets: theme threading, FrameLayout-lift for hit-testing, quick-view/dir-size off-frame-path, external-tool terminal suspend).
- [ ] T060 Run `quickstart.md` manual validation for all five user stories; capture a screenshot/asciinema for the PR.

---

## Dependencies & Execution Order

- **Setup (P1: T001–T003)** → **Foundational (T004–T007)** → user stories.
- **US1 (T008–T017)**: depends on Foundational (theme threading uses the render path; no dependency on US2–US5). MVP.
- **US2 (T018–T026)**: depends on Foundational T006 (FrameLayout regions) and US1's `&Theme` (for colored chrome — can render with default theme if US1 not yet merged, but sequence US1 first).
- **US3 (T027–T035)**: depends on Foundational T006 (layout rects) and US2 (chrome click targets for T034); core `CursorTo` (T031) independent.
- **US4 (T036–T045)**: depends on Foundational + US1 (rendering); independent of US2/US3.
- **US5 (T046–T054)**: depends on Foundational + US1 (dialog theming); independent of US2/US3/US4.
- **Polish (T055–T060)**: after all targeted stories.

## Parallel Opportunities

- Setup: T002, T003 in parallel.
- All `[P]` test tasks within a story author in parallel (distinct test fns/files): e.g. T008–T010, T018–T021, T027–T030, T036–T040, T046–T049.
- Across stories after Foundational: US4 and US5 implementation can proceed in parallel with US2/US3 (different files: pane/core listing vs chrome/mouse), provided US1's theme threading (T013) has landed.

## Implementation Strategy

- **MVP = US1** (themed appearance) — the single biggest fix for the "looks off" complaint; ship-ready on its own.
- Then **US2 + US3** together (chrome + mouse) — they share the FrameLayout and deliver the "missing mouse + missing chrome" complaints.
- Then **US4** and **US5** (richer panels + operation parity) — can be parallelized.
- Each story merges only with its tests green and the perf/size gates intact; deferrals tracked before PR (T058).

# Phase 0 Research: Visual & Interactive Parity Layer

All decisions below are grounded in the existing codebase (verified file:line references from the gap analysis) and the four clarifications recorded in spec.md.

## R1. Color model & theme application (US1)

- **Decision**: Introduce a typed `Theme` struct in a new `cargonaut-ui-tui/src/theme.rs`, built from `ratatui::style::Color`. Resolve `config.ui.theme` (a name) → built-in `Theme` at startup. Thread `&Theme` into `draw_frame` → `draw_pane`/`PaneView::render`/chrome/dialog render. Color each list row by `entry.meta.kind` + executable bit.
- **Rationale**: Constitution §III mandates *typed* theme variables and forbids hardcoded ANSI. `ratatui::Color` already supports `Indexed(u8)` (16/256) and `Rgb` (truecolor) — no new crate. Today `Color` is never used anywhere; the app only uses `Modifier::REVERSED/BOLD` (lib.rs:341/356-360, pane.rs:191/185) — that is the entire cause of "looks off".
- **Alternatives considered**: (a) external skin-file loader now — **deferred** per clarification (scope: file format + parser + errors). (b) raw ANSI strings — rejected (violates §III). (c) a `Style`-per-element map keyed by string — rejected in favor of a typed struct for compile-time safety.
- **Degrade rule**: themes declare colors as named/indexed/rgb; on low-color terminals rely on crossterm/terminal downsampling and prefer the 16-color named palette for the default theme so it is legible everywhere (FR-007).

## R2. Default theme identity (US1)

- **Decision**: Ship at least two built-ins: `commander-dark` (default — blue panel background, bright-white/cyan directories, green executables, cyan/black selection bar, yellow marked) evoking the reference look, and `monochrome` (safe fallback for 8-color/limited terminals). Change `UiConfig::theme` default from the inert `"solarized-dark"` to `commander-dark`.
- **Rationale**: Directly fixes the reported "looks off"; gives an unmistakable first-launch identity (SC-001/SC-002). Keeping a monochrome built-in guarantees FR-007 legibility.
- **Alternatives**: keep `solarized-dark` name — rejected (it currently maps to nothing and is not the signature look).

## R3. Mouse capture & hit-testing (US3)

- **Decision**: Enable `crossterm::event::EnableMouseCapture` in `run()` **by default** (gated so config/flag can disable); handle `CtEvent::Mouse(MouseEvent)` in a new `handle_mouse()`. Lift the layout rects currently local to `draw_frame` (lib.rs:331-335) into a `FrameLayout` returned to the loop and stored. Hit-test `(column,row)` via `Rect::contains` against panes / status / menu bar / fkey bar. Add `AppCommand::CursorTo(usize)` in core so a clicked row survives the per-frame `PaneView::sync_from` re-sync.
- **Rationale**: crossterm delivers no mouse events until capture is enabled; events are currently dropped at lib.rs:146. Rects must be loop-owned because the loop, not the renderer, processes events. The `CursorTo` command keeps `App` the single source of truth for the cursor (consistent with the existing `sync_from` discipline).
- **Default-on bypass**: provide a runtime toggle key + document the terminal's hold-modifier (commonly Shift) bypass so users can still do native text selection (FR-013, clarified). `DisableMouseCapture` in teardown, symmetric and best-effort.
- **Double-click**: track `(col,row,Instant)` of the last left-down; a second left-down on the same row within ~400 ms = double-click → `Descend`. (`Instant::now()` is fine in app code; the workflow-script `Date.now()` ban does not apply to the Rust binary.)
- **Alternatives**: SGR vs normal mouse protocol — crossterm handles this; no manual escape parsing.

## R4. Screen chrome (US2)

- **Decision**: New `cargonaut-ui-tui/src/chrome.rs` with three widgets: `MenuBar` (top row, menu titles + a dropdown overlay), `FunctionKeyBar` (bottom row, `N Label` buttons), and a per-pane `MiniStatus` line. Each carries the on-screen `Rect`s it occupies (folded into `FrameLayout`) so US3 can hit-test them. F-key/menu items dispatch the *same* `Command`s as the keymap.
- **Rationale**: Constitution §III — keymap is the single source of truth; chrome is a second invocation path, not a new vocabulary. The bars are also the click targets US3 needs, hence US2+US3 are sequenced together.
- **Labels**: F1 Help, F2 Menu, F3 View, F4 Edit, F5 Copy, F6 RenMov, F7 Mkdir, F8 Delete, F9 PullDn, F10 Quit (reference-canonical). Deferred actions show the label and report "not yet available" (FR-011).
- **Layout impact**: the main vertical layout grows from `[panes, status]` to `[menubar(1), panes(min), ministatus(1)×panes, status(1), fkeybar(1)]`. Constraints must degrade on small terminals (FR-012).

## R5. Listing columns, `..`, sort, modes (US4)

- **Decision**: `PaneView::render` formats columns name/size/mtime/perms from the existing `VfsMetadata` (size, mtime, mode bits, kind, is_hidden — types.rs:184-189). Synthesize a `..` parent entry as row 0 (except at root). `App` holds the active `Sort` (existing enum types.rs:200-213; today hardcoded `NameAsc` at core lib.rs:270) and applies it; `CycleSortKey` rotates it + reverse toggle. Listing mode: reuse existing `ListingMode` (config) mapping Brief→Brief, Full→Standard/Long; add **quick-view** as a panel *view kind* (passive panel shows a bounded preview), not a column layout.
- **Rationale**: All metadata already exists — this is presentation + dispatch. Sort enum already exists. Quick-view is modeled separately because it transforms the *other* panel rather than re-columning the active one.
- **Quick-view bounding**: read ≤256 KiB / ≤1000 lines of the highlighted file as UTF-8 (lossy); non-text/binary/oversized → placeholder. Read off the frame path to protect NFR-002.
- **Recursive dir-size**: spawn an async walk (VFS `list` recursion) on demand; update the entry's displayed size when done; never block the loop (FR-023).
- **Alternatives**: a full pluggable column system (UserListingConfig already hints at it) — kept minimal here (name/size/mtime/perms); user/plugin columns remain future.

## R6. Operations: mkdir, pattern select, progress dialog, F3/F4 (US5)

- **Decision**:
  - **Mkdir**: `MkdirPromptDialog` (reuse dialog widgets) → `AppCommand::Mkdir(name)` → VFS create dir → refresh listing (FR-024).
  - **Pattern select**: `PatternPromptDialog` → `AppCommand::SelectByPattern(glob)` / `UnselectByPattern(glob)` using `globset` over the visible listing; zero-match reports a status (FR-025).
  - **Progress dialog**: `TransferProgressDialog` renders a `ProgressView` projected from the engine's existing `Running{bytes_done,total,throughput,eta}` events (engine already emits these per job.rs). Cancel routes to the existing `CancellationToken` path (FR-026/027). Replaces the status-bar-count placeholder.
  - **F3/F4**: suspend the alternate screen + raw mode, run `Command::new(pager_or_editor).arg(path)`, restore terminal, refresh panel (FR-030/031). Resolve `$PAGER`/`$EDITOR` with `less`/`more` and `vi`/`nano` fallbacks.
- **Rationale**: progress data already exists and is merely unshown; mkdir/pattern are thin dispatch wiring; external view/edit reuses the existing terminal teardown/setup discipline (the loop already always restores the terminal on exit) and avoids building the deferred internal viewer/editor.
- **Safety**: external spawn uses `Command::new().arg()` (no shell), per constitution macro-safety rule.

## R7. Config & CLI flag wiring

- **Decision**: Flip `UiConfig::mouse` default to `true` (clarified). In `cargonaut-bin/src/main.rs`, merge `cli.theme` into `config.ui.theme` and `cli.mc_keys` into `config.ui.mc_keys` before `App::new` (currently parsed then dropped, main.rs:27-35). `--theme` finally takes effect (FR-005).
- **Rationale**: fixes the dead-flag bug the gap analysis identified; makes mouse-on observable by default.
- **Backward-compat**: existing config files that set `mouse=false` keep working; only the *default* changes.

## R8. Testing strategy (constitution §II)

- **Decision**: Unit tests for pure logic (theme name→palette, hit-test math, column formatting, sort rotation, glob match, quick-view truncation, progress projection, parent-entry synthesis). `TestBackend` snapshot-style tests for pane/chrome/dialog rendering with a known theme. Integration test for mkdir round-trip via `LocalFs`+`TempDir`. Keep the existing keypress-latency + binary-size gates green.
- **Rationale**: matches the existing test patterns (pane.rs/dialog.rs already use `TestBackend`; core uses tokio+TempDir+LocalFs). Each FR lands red→green per §II.

## Open questions

None remaining. All NEEDS CLARIFICATION resolved; product decisions fixed by the spec's Clarifications session.

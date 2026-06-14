# Tasks: Panel Filter Prompt Dialog

**Input**: Design documents from `specs/033-panel-filter-prompt/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/filter-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR gets a
red→green pair; git history MUST show `(red)` before `(green)`. SC-005 (set/clear/invalid
coverage) is the gating requirement.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files, no ordering dependency)
- **[Story]**: US1 / US2 / US3, or SETUP / FOUND / POLISH
- File paths are exact.

## Conventions

- Core crate: `crates/cargonaut-core/src/lib.rs`
- Pane view: `crates/cargonaut-ui-tui/src/pane.rs`
- Shared dialog widget: `crates/cargonaut-ui-tui/src/dialog.rs` (reused, **not** modified)
- Event loop / dialog wiring: `crates/cargonaut-ui-tui/src/lib.rs`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands the
  implementation. `[P]` items within a phase may be authored together.

---

## Phase 1: Setup

- [ ] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean baseline builds
  + tests (`make build && make test`). Record the current stripped binary size
  (`scripts/check-binary-size.sh`) as the pre-`globset` baseline for T022.
- [ ] T002 [SETUP] Add `globset = "0.4"` to `[workspace.dependencies]` in root `Cargo.toml`
  and reference it (`globset = { workspace = true }`) in `crates/cargonaut-core/Cargo.toml`.
  Run `make build` to confirm it resolves; confirm `aho-corasick`/`regex-automata` are not
  newly duplicated in `Cargo.lock` (research R-001).

---

## Phase 2: Foundational (Blocking — filter type + state migration)

**Purpose**: Replace the `Option<String>` substring filter with a compiled
`Option<PaneFilter>` across core and the pane view. Every user story depends on this.

**⚠️ No user-story phase can start until this is complete.**

- [ ] T003 [FOUND] (red) In `crates/cargonaut-core/src/lib.rs` tests, add failing unit tests
  for `PaneFilter`: (a) `compile("*.rs")` then `is_match("lib.rs")` true / `is_match("a.md")`
  false (glob path, FR-003a); (b) `compile("rs")` (no metachars) matches `"lib.rs"` and
  `"parser.md"` via auto-`*rs*` (FR-003a); (c) case-insensitive — `compile("*.RS")` matches
  `"lib.rs"` (FR-003b); (d) `compile("[")` returns `Err(AppError::BadFilter(_))` (FR-006);
  (e) `pattern()` returns the original text for prefill (FR-002).
- [ ] T004 [FOUND] (green) Implement `pub struct PaneFilter { pattern, matcher }`
  (`#[derive(Debug, Clone)]`) + `compile` (trim, auto-wrap metachar-free patterns as
  `*pattern*`, `GlobBuilder::case_insensitive(true)`, map `globset::Error` →
  `AppError::BadFilter`), `is_match`, `pattern`. Add `AppError::BadFilter(String)` with
  `#[error("bad filter: {0}")]`. Make T003 pass. `#![warn(missing_docs)]` clean.
- [ ] T005 [FOUND] Migrate `PaneState.filter` to `Option<PaneFilter>` in
  `crates/cargonaut-core/src/lib.rs`; update `visible_indices` to use `pf.is_match(name)`;
  update existing core tests that plant a filter (`toggle_panel_filter_clears_existing_filter`,
  and any `filter: Some(..)`/`filter = Some("..".into())`) to construct via `PaneFilter::compile`.
  `make test` green.
- [ ] T006 [FOUND] Migrate `PaneView.filter` to `Option<PaneFilter>` in
  `crates/cargonaut-ui-tui/src/pane.rs`; keep `sync_from` cloning the option;
  update `visible_indices` to `pf.is_match(name)`; update the pane tests
  (`substring_filter_constrains_visibility`, `cursor_down_with_filter_only_walks_visible`)
  to build the filter via `PaneFilter::compile`. Refresh the stale module/field doc
  comments (the "T1.26 swaps this for a real glob" notes). `make test` green.
- [ ] T007 [FOUND] In `crates/cargonaut-ui-tui/src/lib.rs` add
  `ActiveDialog::FilterPrompt { widget: PathInputDialog }` and its render arm
  `ActiveDialog::FilterPrompt { widget } => widget.render(darea, f.buffer_mut(), theme)`.
  (Open + key handling land per-story below; until then the branch may close on Esc.)

**Checkpoint**: Workspace compiles; all pre-existing tests green under the new filter type.
No user-visible behavior change yet (filter still only settable from tests).

---

## Phase 3: User Story 1 — Set a filter on the focused pane (P1) 🎯 MVP

**Goal**: `Alt-!` → prompt (prefilled) → type a pattern → Enter narrows the focused pane;
cursor resets; other pane untouched; filter persists across navigation
(FR-001, FR-002, FR-003, FR-003a/b/c, FR-004, FR-009, FR-010).

**Independent Test**: `set_filter` with a valid pattern narrows the active pane's
`visible_indices`, resets cursor, leaves the other pane unchanged, and survives a cd.

- [ ] T008 [US1] (red) In `crates/cargonaut-core/src/lib.rs` tests, add failing tests for
  `App::set_filter`: (a) `set_filter("*.rs")` ⇒ active pane `visible_indices` only `.rs`
  entries (FR-003), cursor == 0 (FR-004), emits `PaneUpdated(active)` + `Status`; (b) bare
  word narrows by substring (FR-003a); (c) the inactive pane's `visible_indices` is
  unchanged (FR-009); (d) after `set_filter` + a `Descend`/`Ascend` (or `navigate_to`), the
  filter is still applied to the new listing (FR-003c).
- [ ] T009 [US1] (green) Implement `pub fn set_filter(&mut self, pattern: &str) ->
  Result<Vec<Event>, AppError>` (synchronous, research R-005): on non-empty valid pattern,
  set `active_pane_mut().filter = Some(PaneFilter::compile(..)?)`, cursor = 0, return
  `[PaneUpdated(active), Status("Filter: <pattern>")]`. (Empty/clear branch lands in T014;
  for now an empty string may early-return without setting.) Make T008 pass.
- [ ] T010 [US1] In `crates/cargonaut-ui-tui/src/lib.rs`, intercept
  `Command::TogglePanelFilter` in the command-routing path to open
  `ActiveDialog::FilterPrompt` prefilled with the focused pane's current pattern
  (`filter.as_ref().map(|f| f.pattern())`), set `mode = Mode::Dialog`
  (contract filter-seam "Open"). (FR-001, FR-002, FR-010)
- [ ] T011 [US1] In the same file, wire the `ActiveDialog::FilterPrompt` key branch:
  `Submit(text)` → `app.set_filter(&text)`; on `Ok` apply events + close + `Mode::Pane`.
  (Error + Cancel paths complete in US3.)
- [ ] T012 [US1] Manual smoke per quickstart steps 1–3 and 7 (build, `Alt-!`, `*.rs`, Enter;
  re-open shows prefill; filter survives descend/ascend).

**Checkpoint**: MVP — a user can open the prompt and apply a glob/substring filter to the
focused pane.

---

## Phase 4: User Story 2 — Clear the filter via the prompt (P1)

**Goal**: empty submit clears the focused pane's filter and restores the full listing,
preserving prior clear-on-empty behavior (FR-005).

**Independent Test**: `set_filter("")` / whitespace clears an active filter and is a safe
no-op when none is set; empty submit through the prompt closes it and restores the listing.

- [ ] T013 [US2] (red) In `crates/cargonaut-core/src/lib.rs` tests, add/repurpose
  `toggle_panel_filter_clears_existing_filter` into `set_filter` clear tests: (a) with an
  active filter, `set_filter("")` ⇒ `filter == None`, full listing visible again, cursor 0,
  `Status` reports cleared (FR-005); (b) `set_filter("   ")` behaves identically
  (whitespace-only); (c) clearing when already `None` is a no-op `Ok`.
- [ ] T014 [US2] (green) Add the empty/whitespace branch to `App::set_filter` (clear +
  cursor 0 + cleared `Status`). Make `Command::TogglePanelFilter`'s **core** dispatch a
  no-op with an explanatory comment (the TUI intercepts it now — research R-007); update any
  remaining test that dispatched the command for its clear side effect. Make T013 pass.
- [ ] T015 [US2] Confirm the TUI `FilterPrompt` `Submit("")` path closes the prompt and
  clears (no special-casing needed — `set_filter("")` returns `Ok` with the clear `Status`).
  Manual smoke per quickstart step 4.

**Checkpoint**: set and clear both work end to end through the single prompt entry point.

---

## Phase 5: User Story 3 — Recover from an invalid pattern (P2)

**Goal**: an uncompilable pattern keeps the prompt open with an inline error and leaves
pane state unchanged; editing clears the error; cancel reverts cleanly
(FR-006, FR-007, FR-008).

**Independent Test**: `set_filter("[")` errors without mutating state; the dialog shows the
error and clears it on edit; Esc leaves the filter exactly as before.

- [ ] T016 [US3] (red) In `crates/cargonaut-core/src/lib.rs` tests, add a failing test that
  `set_filter("[")` (unterminated class) returns `Err(AppError::BadFilter(_))` and the
  active pane's `filter` + `visible_indices` are byte-for-byte unchanged (FR-006, SC-003).
- [ ] T017 [US3] (green) Ensure `compile`'s `globset::Error` → `AppError::BadFilter` mapping
  makes T016 pass and that `set_filter` is atomic (compile before assigning). (Largely from
  T004; close any gap.)
- [ ] T018 [US3] In `crates/cargonaut-ui-tui/src/lib.rs`, complete the `FilterPrompt` key
  branch: `Submit` `Err(e)` → `widget.set_error(e.to_string())`, prompt stays open
  (FR-006); `Cancel` → close + `Mode::Pane`, pane filter untouched (FR-008); ignore
  `RequestCompletions` (no glob completion). Editing-clears-error (FR-007) is provided by
  the shared widget (verify the existing `dialog.rs` test
  `path_input_set_error_renders_and_clears_on_edit` still covers it; add a thin assertion if
  not).
- [ ] T019 [US3] Add an injected-input end-to-end test (mirror Feature 038's injected-input
  gate) driving the event loop: open prompt → invalid pattern → assert prompt still open +
  error shown + pane unchanged; then edit to a valid pattern → Enter → pane filtered; then
  open → clear → restored; and an open → Esc → unchanged case (SC-005). If no reusable
  injected-input harness exists for dialogs, place this as a `cargonaut-ui-tui` integration
  test using the same `TestBackend`/key-injection approach as the QuickCd E2E.
- [ ] T020 [US3] Manual smoke per quickstart steps 5–6 (invalid pattern error + clear; Esc
  reverts).

**Checkpoint**: all three user stories complete; set / clear / invalid / cancel covered by
automated tests (SC-005).

---

## Phase 6: Polish & Docs

- [ ] T021 [POLISH] Run `make ci-local` (fmt + clippy `-D warnings` + test + release build +
  gates) and fix any findings. Run `cargo fmt`.
- [ ] T022 [POLISH] Run `scripts/check-binary-size.sh`; confirm the stripped release binary
  is still ≤8 MiB (NFR-001) with `globset` added; note the delta vs the T001 baseline.
- [ ] T023 [POLISH] Update `README.md` (At-a-Glance metrics: test count, feature count,
  binary size) + a Feature History one-liner for #33; append a `Learnings.md` section
  (≥3 bullets: the `Option<String>`→`Option<PaneFilter>` migration, the auto-substring
  decision, the binary-size-from-already-present-regex-machinery finding). Mandatory per
  CLAUDE.md docs gate.
- [ ] T024 [POLISH] Update `ROADMAP.md`: mark the #33 row resolved (filter prompt shipped),
  referencing this feature. Close GitHub issue #33 from the merged PR.

---

## Dependencies & ordering

```
Setup (T001 → T002)
  └─▶ Foundational (T003→T004→T005→T006→T007)   [BLOCKING for all stories]
        ├─▶ US1 (T008→T009→T010→T011→T012)        [MVP]
        ├─▶ US2 (T013→T014→T015)                  [needs T009 set_filter scaffold]
        └─▶ US3 (T016→T017→T018→T019→T020)        [needs T011 key branch]
              └─▶ Polish (T021→T022→T023→T024)
```

- US1 is the MVP and must land first. US2 extends `set_filter` (T009) with the clear branch.
  US3 extends the TUI key branch (T011) with error/cancel handling. T019 (E2E) depends on
  US1+US2+US3 wiring being present.
- `[P]`: the red test-authoring tasks T008/T013/T016 touch the same test module so are not
  parallel with each other, but core (lib.rs) vs pane (pane.rs) foundational edits
  (T005 vs T006) are independent files and may be authored in parallel after T004.

## Requirement → task coverage

| FR / SC   | Tasks |
|-----------|-------|
| FR-001    | T010, T011 |
| FR-002    | T003(e), T004, T010 |
| FR-003    | T008(a), T009 |
| FR-003a   | T003(a,b), T004, T008(a,b) |
| FR-003b   | T003(c), T004 |
| FR-003c   | T008(d), T009 |
| FR-004    | T008, T009, T013 |
| FR-005    | T013, T014, T015 |
| FR-006    | T003(d), T016, T017, T018 |
| FR-007    | T018 |
| FR-008    | T018, T020 |
| FR-009    | T008(c), T009 |
| FR-010    | T007, T010, T018 (reuse `PathInputDialog`, no new widget) |
| SC-001    | T008, T012 |
| SC-002    | T013, T015 |
| SC-003    | T016, T017, T019 |
| SC-004    | T008(c) |
| SC-005    | T019 |

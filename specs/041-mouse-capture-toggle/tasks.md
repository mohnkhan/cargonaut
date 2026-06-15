# Tasks: In-Session Mouse Capture Toggle

**Input**: Design documents from `specs/041-mouse-capture-toggle/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md,
contracts/mouse-toggle-seam.md

**Tests**: REQUIRED. Constitution §II (Test-First, NON-NEGOTIABLE) — every FR gets
a red→green pair; git history MUST show `(red)` before `(green)`. The pure
`plan_mouse_toggle` truth table is the gating decision test (SC-001/004).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: may run in parallel (different files / disjoint regions, no ordering dependency)
- **[Story]**: US1 / US2 / US3, or SETUP/FOUND/POLISH
- File paths are exact.

## Conventions

- Keymap contract: `design/contracts/keymap.toml`
- Command enum + keymap tests: `crates/cargonaut-ui-tui/src/keymap.rs`
- Event loop / dispatch / help / pure toggle fn: `crates/cargonaut-ui-tui/src/lib.rs`
- Chrome (menu bar + indicator): `crates/cargonaut-ui-tui/src/chrome.rs`
- Build/test via `make build` / `make test` (tmpfs-guarded, Constitution §V).
- Each `(red)` commit lands the failing test; the paired `(green)` commit lands
  the implementation. `[P]` tests within a phase may be authored together.
- **No new crates / no `Cargo.toml` changes** (plan.md Technical Context).

---

## Phase 1: Setup

- [X] T001 [SETUP] Confirm tmpfs is active (`make tmpfs-status`) and a clean
  baseline builds + tests (`make build && make test`). Verify no `Cargo.toml`
  changes are needed (no new dependencies).

---

## Phase 2: Foundational (Blocking — command variant + keymap binding)

**Purpose**: Register the new action so every user story can dispatch it. Until
the binding + variant exist, no toggle behavior can be wired or tested.

**⚠️ No user-story phase can start until this is complete.**

- [X] T002 [FOUND] (red) In `crates/cargonaut-ui-tui/src/keymap.rs`, add a failing
  test asserting `Keymap::load(DEFAULT_KEYMAP_TOML)` succeeds and
  `lookup(Mode::Global, M-m)` resolves to `Command::ToggleMouseCapture`
  (contract §1/§2). Add a non-collision assertion: no other binding resolves
  `M-m`. (Will not compile until the variant exists — that is the red state.)
- [X] T003 [FOUND] (green) Add the `ToggleMouseCapture` variant to the `Command`
  enum in `crates/cargonaut-ui-tui/src/keymap.rs` with a doc comment
  (`/// Toggle runtime mouse capture on/off (FR-001).`). Add the binding block to
  `design/contracts/keymap.toml` (`mode = "global"`, `key = "M-m"`,
  `action = "toggle-mouse-capture"  # FR-001 (#38)`). Make T002 pass.

**Checkpoint**: `M-m` parses and resolves to the new command; nothing handles it yet.

---

## Phase 3: User Story 1 — Suspend/resume mouse capture mid-session (Priority: P1) 🎯 MVP

**Goal**: `M-m` flips mouse capture; suspended releases the mouse to the terminal,
resuming re-captures it — without restart.

**Independent Test**: Launch with mouse on, press `M-m` → terminal-native
selection works; press `M-m` again → in-app clicks work again.

- [ ] T004 [P] [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add failing
  unit tests for the pure decision fn `plan_mouse_toggle(supported, currently)`
  covering the full truth table (contract §3): `(false,false)→Disabled`,
  `(false,true)→Disabled`, `(true,false)→EnabledNow`, `(true,true)→SuspendedNow`.
- [ ] T005 [US1] (green) Implement `MouseToggleOutcome` enum +
  `plan_mouse_toggle(supported: bool, currently: bool) -> MouseToggleOutcome`
  (pure, no I/O) in `crates/cargonaut-ui-tui/src/lib.rs` with doc comments. Make
  T004 pass.
- [ ] T006 [US1] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a failing
  test that drives `dispatch_ui_command(Command::ToggleMouseCapture, …)` against
  a `UiState` (mouse-supported session) and asserts: starting `mouse_enabled=true`
  → after dispatch `mouse_enabled=false` and `status` contains "suspended";
  dispatch again → `mouse_enabled=true` and `status` contains "on". (Reuse the
  existing `fresh_ui` test helper; config with `ui.mouse=true`.)
- [ ] T007 [US1] (green) Add the `Command::ToggleMouseCapture` arm to
  `dispatch_ui_command` in `crates/cargonaut-ui-tui/src/lib.rs` (contract §4):
  match `plan_mouse_toggle(app.config().ui.mouse, ui.mouse_enabled)`; on
  `EnabledNow` `execute!(stdout(), EnableMouseCapture)?` + set flag + status; on
  `SuspendedNow` `execute!(stdout(), DisableMouseCapture)?` + clear flag + status;
  `return Ok(())`. Make T006 pass.

**Checkpoint**: US1 fully functional — toggle suspends/resumes capture. **MVP done.**

---

## Phase 4: User Story 2 — See current capture state (Priority: P2)

**Goal**: State is discoverable both transiently (status line, done in US1) and
persistently (menu-bar indicator).

**Independent Test**: Toggle off → UI shows suspended; toggle on → UI shows active.

- [ ] T008 [P] [US2] (red) In `crates/cargonaut-ui-tui/src/chrome.rs`, add a
  failing test for `mouse_indicator(session_supported, captured)` returning
  `"[mouse:on]"` / `"[mouse:susp]"` / `"[mouse:off]"` (contract §5 truth table).
- [ ] T009 [US2] (green) Implement `mouse_indicator(...)` in
  `crates/cargonaut-ui-tui/src/chrome.rs` with a doc comment. Make T008 pass.
- [ ] T010 [US2] (red) In `crates/cargonaut-ui-tui/src/chrome.rs` (or `lib.rs`
  `draw_frame` tests), add a failing `TestBackend` render test: with
  `captured=true` the menu-bar row contains `[mouse:on]`; with `captured=false`
  + supported it contains `[mouse:susp]`; with `session_supported=false` it
  contains `[mouse:off]`.
- [ ] T011 [US2] (green) Render the indicator in the right gutter of the menu-bar
  row: thread `mouse_enabled` + `config.ui.mouse` into `draw_frame`
  (`crates/cargonaut-ui-tui/src/lib.rs`) and draw `mouse_indicator(...)` right-
  aligned in the menu `Rect` using typed `theme.menu_fg/bg` (dim when `off`).
  Make T010 pass. No new layout row (Constitution III/IV).

**Checkpoint**: capture state visible at a glance, independent of US1's transient message.

---

## Phase 5: User Story 3 — Disabled-session no-op (Priority: P3)

**Goal**: When mouse support is off for the session, `M-m` explains rather than
silently doing nothing.

**Independent Test**: Launch `--no-mouse`, press `M-m` → status explains disabled;
no capture; indicator stays `[mouse:off]`.

- [ ] T012 [US3] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a failing
  test: dispatch `Command::ToggleMouseCapture` with a config where
  `ui.mouse=false` (and `mouse_enabled=false`); assert `mouse_enabled` stays
  `false` and `status` contains "disabled for this session".
- [ ] T013 [US3] (green) Confirm the `Disabled` outcome path in the
  `dispatch_ui_command` arm sets the explanatory status and performs **no**
  `execute!` and **no** flag change (already routed via `plan_mouse_toggle` from
  T007 — wire/verify the status text). Make T012 pass.

**Checkpoint**: all three user stories independently testable and green.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T014 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  failing test asserting the help-overlay text contains both `M-m` and `Shift`
  (FR-010 / SC-006).
- [ ] T015 [POLISH] (green) Update the help-overlay mouse line in
  `crates/cargonaut-ui-tui/src/lib.rs` (the existing `Mouse: …` text, ~lib.rs:1256)
  to document the `M-m` toggle and the Shift-drag native-selection bypass. Make
  T014 pass.
- [ ] T016 [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  regression test for FR-007: simulate a suspended state (`mouse_enabled=false`)
  and assert the value passed to `run_external` is `false` (i.e. external
  suspend/restore preserves the *current* toggle, not the launch value). If a
  direct test is impractical, assert via a small helper that reads
  `ui.mouse_enabled` at the call site; document the manual quickstart step 5.
- [ ] T017 [POLISH] (green) Verify/adjust the `run_external(_, _, ui.mouse_enabled)`
  call site so FR-007 holds; make T016 pass (likely no code change — confirm and
  lock with the test).
- [ ] T0XA [P] [POLISH] (red) In `crates/cargonaut-ui-tui/src/lib.rs`, add a
  failing test for FR-008 / SC-005 (clean exit releases capture). Factor the
  teardown into a small testable helper (e.g. `fn teardown_terminal(out)` that
  unconditionally issues `DisableMouseCapture` + `disable_raw_mode` +
  `LeaveAlternateScreen`) and assert — regardless of the last `mouse_enabled`
  value — that `DisableMouseCapture` is always emitted (assert via a `Vec<u8>`
  writer capturing the control bytes, or by asserting the helper is called on
  both Ok and Err loop-exit paths). This is the constitution §II CI gate for
  SC-005.
- [ ] T0XB [POLISH] (green) Extract the teardown block in `run()`
  (`crates/cargonaut-ui-tui/src/lib.rs:72-76`) into the helper from T0XA and call
  it on every exit path; make T0XA pass. Behavior unchanged (teardown is already
  unconditional) — this only adds the testable seam + gate.
- [ ] T0XC [POLISH] FR-011 (graceful degradation): confirm the
  `Command::ToggleMouseCapture` arm tolerates terminals that ignore mouse
  capture. Terminals without mouse support silently accept/ignore the control
  sequence (no error), matching the existing `run()` startup behavior. Document
  this in a code comment on the dispatch arm; if any environment surfaces an
  `execute!` error, downgrade that call to best-effort (`let _ = execute!(…)`)
  so a toggle never crashes the loop. No new test unless an error path is
  reproducible.
- [ ] T018 [POLISH] Run `make ci-local` (fmt, clippy `-D warnings`, test, release
  build, docs-gate). Run `cargo run -p cargonaut-bin` and walk the quickstart.md
  manual steps 1–8. Fix any clippy/fmt issues.
- [ ] T019 [P] [POLISH] Docs (Constitution / CLAUDE.md MANDATORY): update
  `README.md` ("At a Glance" metrics — test count, feature count, binary size;
  + Feature History one-liner for Feature 041) and append a Feature 041 section
  to `Learnings.md` (≥3 bullets: the single-flag reuse, the pure-function
  testable seam, FR-007 already-satisfied finding).
- [ ] T020 [POLISH] Close issue #38: confirm FR-013 toggle delivered; reference
  the merged PR. (No new deferral/ROADMAP row needed — this feature *resolves* a
  deferral rather than creating one.)

---

## Dependencies & Execution Order

- **Setup (T001)** → **Foundational (T002–T003)** → user stories.
- **US1 (T004–T007)**: depends only on Foundational. **MVP.**
- **US2 (T008–T011)**: depends on Foundational; `mouse_indicator` (T008/T009) is
  independent of US1, but the live indicator render (T011) is most meaningful
  after US1 flips the flag. Can be authored in parallel with US1.
- **US3 (T012–T013)**: depends on US1's dispatch arm (T007) since the `Disabled`
  branch lives in the same `match`.
- **Polish (T014–T020)**: after US1–US3. T019 (docs) gates the PR (docs-gate).

## Parallel Opportunities

- T004 (US1 pure-fn test) ∥ T008 (US2 indicator test) — different files/regions.
- T014 (help test) ∥ T016 (FR-007 regression test) ∥ T0XA (teardown gate) ∥ T019
  (docs) in Polish — different files/regions.

## Independent Test Criteria

- **US1**: `M-m` suspends then resumes capture in a running session (two presses).
- **US2**: indicator reads `[mouse:on]`/`[mouse:susp]`/`[mouse:off]` correctly.
- **US3**: `--no-mouse` + `M-m` → explanatory status, never captures.

## Suggested MVP Scope

**Phase 1 + Phase 2 + Phase 3 (US1)** — a working `M-m` toggle with transient
status feedback. US2 (persistent indicator) and US3 (disabled-session message)
are incremental enhancements.

## Format Validation

All tasks use `- [ ] TNNN [P?] [Story] description + exact path`. Setup/Foundational/
Polish carry SETUP/FOUND/POLISH; user-story tasks carry US1/US2/US3.

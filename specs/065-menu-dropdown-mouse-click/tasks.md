# Tasks: Click-on-dropdown-item support for the pull-down menu bar

**Feature**: 065-menu-dropdown-mouse-click | **Branch**: `065-menu-dropdown-mouse-click`
**Spec**: [spec.md](./spec.md) | **Plan**: [plan.md](./plan.md)
**Contracts**: [contracts/menubar-mouse.md](./contracts/menubar-mouse.md)

## Conventions

- **TDD is NON-NEGOTIABLE** (Constitution §II): every behavior task ships as a `(red)` commit
  with a failing test, then a `(green)` commit that makes it pass. Commit subjects use
  `T0NN (red): …` / `T0NN (green): …`.
- `[P]` = parallelizable (different file, no dependency on an incomplete task).
- All builds/tests via `make` wrappers (tmpfs guard, Constitution §V). Never `cargo clean`.
- Two source files in play:
  - `crates/cargonaut-ui-tui/src/chrome.rs` — `MenuBar` methods + unit tests
  - `crates/cargonaut-ui-tui/src/lib.rs` — `FrameLayout` field, `handle_mouse` wiring +
    integration tests

> Incorporates `/speckit-analyze` remediations: U1 (`FrameLayout.full` plumbing → T003),
> I1 (`in_dropdown` predicate → T009/T010), C1 (no-motion degradation test → T023), A1
> (FR-002 wording, fixed in spec.md).

---

## Phase 1: Setup

- [ ] T001 Confirm working branch `065-menu-dropdown-mouse-click` is checked out and `make tmpfs-status` shows `target/` linked to tmpfs; run `make tmpfs-setup` if not.
- [ ] T002 Baseline green: run `cargo test -p cargonaut-ui-tui` to confirm a clean starting point before adding failing tests.

---

## Phase 2: Foundational (BLOCKING — shared by US1/US2/US3)

Pure `MenuBar` methods + the `FrameLayout.full` plumbing underpin every user story.

- [ ] T003 Add a `full: Rect` field to `FrameLayout` in `crates/cargonaut-ui-tui/src/lib.rs`; set it from `let area = f.size();` in `draw_frame` (the return literal at ~lib.rs:3112) and add `full: …` to the two test-only `FrameLayout { … }` literals (~lib.rs:3396, ~lib.rs:5708). No behavior change. (finding U1 — buffer area for hit-testing)
- [ ] T004 (red) Add unit test `menu_bar_dropdown_rect_matches_render` in `crates/cargonaut-ui-tui/src/chrome.rs` asserting the dropdown rect (x/y/width/height incl. short-buffer clamping) equals the rectangle `render` draws for an open menu. (FR-002)
- [ ] T005 (green) Add private `fn dropdown_rect(&self, area: Rect, buf: Rect) -> Option<Rect>` to `MenuBar` in `crates/cargonaut-ui-tui/src/chrome.rs` and refactor `render` to consume it (single source of geometry). Make T004 pass. (FR-002)
- [ ] T006 (red) Add unit test `menu_bar_item_hit_test` in `crates/cargonaut-ui-tui/src/chrome.rs`: first row → `Some(0)`, last visible row → `Some(last)`, border row → `None`, point outside dropdown → `None`, closed menu → `None`. (FR-002, FR-003)
- [ ] T007 (red) Add unit test `menu_bar_item_hit_test_clamped` in `crates/cargonaut-ui-tui/src/chrome.rs`: with a short buffer that clips trailing items, a click on a clipped row returns `None`. (Edge case: short terminal)
- [ ] T008 (green) Add `pub fn item_at(&self, area: Rect, buf: Rect, x: u16, y: u16) -> Option<usize>` to `MenuBar` in `crates/cargonaut-ui-tui/src/chrome.rs` (uses `dropdown_rect`; interior rows only; never returns clipped/out-of-range index). Make T006 + T007 pass. Add `///` doc. (FR-002, FR-003)
- [ ] T009 (red) Add unit test `menu_bar_in_dropdown` in `crates/cargonaut-ui-tui/src/chrome.rs`: point on border → `true`, point on an item → `true`, point just outside → `false`, closed menu → `false`. (FR-003 vs FR-004 boundary; finding I1)
- [ ] T010 (green) Add `pub fn in_dropdown(&self, area: Rect, buf: Rect, x: u16, y: u16) -> bool` to `MenuBar` in `crates/cargonaut-ui-tui/src/chrome.rs` (true iff inside `dropdown_rect`, border included). Make T009 pass. Add `///` doc. (FR-003, FR-004; finding I1)
- [ ] T011 (red) Add unit test `menu_bar_select_sets_item` in `crates/cargonaut-ui-tui/src/chrome.rs`: `select(i)` then `selected_command()` returns item `i`; out-of-range index clamps to last item; `select` on a closed menu is a no-op (no panic). (FR-001, FR-007)
- [ ] T012 (green) Add `pub fn select(&mut self, idx: usize)` to `MenuBar` in `crates/cargonaut-ui-tui/src/chrome.rs` (clamp to open menu range; no-op when closed). Make T011 pass. Add `///` doc. (FR-001, FR-007)

**Checkpoint**: `cargo test -p cargonaut-ui-tui` green; `MenuBar` exposes `item_at`,
`in_dropdown`, `select`; `render` uses `dropdown_rect`; `FrameLayout` carries `full`.

---

## Phase 3: User Story 1 — Click a dropdown item to run it (P1) 🎯 MVP

**Goal**: Left-clicking an item in an open dropdown invokes its command and closes the menu.

**Independent test**: Open the File menu via title click, click "Mkdir" → Mkdir dispatched,
menu closed.

- [ ] T013 (red) [US1] Add integration test `t_menu_mouse_click_item_dispatches` in the `crates/cargonaut-ui-tui/src/lib.rs` test module: with the File menu open, a `Down(Left)` on the "Mkdir" item row dispatches `Command::Mkdir` and leaves the menu closed. (FR-001, FR-012)
- [ ] T014 (red) [US1] Add integration test `t_menu_mouse_click_first_and_last_item` in `crates/cargonaut-ui-tui/src/lib.rs`: clicking the first item row invokes the first command and the last visible row invokes the last command (no off-by-one against the border). (FR-002)
- [ ] T015 (red) [US1] Add integration test `t_menu_mouse_click_border_noop` in `crates/cargonaut-ui-tui/src/lib.rs`: a `Down(Left)` on the dropdown border (inside the frame, not an item) dispatches nothing and the menu stays open. (FR-003)
- [ ] T016 (green) [US1] In `handle_mouse` `Down(MouseButton::Left)` (in `crates/cargonaut-ui-tui/src/lib.rs`), after the existing `title_at` branch add, in order: (a) if `menu.is_open()` and `item_at(ui.layout.menu, ui.layout.full, x, y) == Some(i)` → `select(i)`, take `selected_command()`, `close()`, `dispatch_ui_command(...)`, `return`; (b) else if `menu.is_open()` and `in_dropdown(ui.layout.menu, ui.layout.full, x, y)` → `return` (no-op). Make T013–T015 pass. (FR-001, FR-002, FR-003, FR-012)

**Checkpoint**: US1 tests green → menus are mouse-operable end to end. **This is the MVP.**

---

## Phase 4: User Story 2 — Close or switch menus with the mouse (P2)

**Goal**: Click a different title to switch; click the open title to close; click outside to
close AND pass the click through to the panel.

**Independent test**: Open File, click a right-panel row → menu closes and the right pane
focuses with the cursor on that row.

- [ ] T017 (red) [US2] Add integration test `t_menu_mouse_outside_closes_and_passes_through` in `crates/cargonaut-ui-tui/src/lib.rs`: with a menu open, a `Down(Left)` on a file-panel row closes the menu AND focuses that pane + moves the cursor to the clicked row. (FR-004)
- [ ] T018 (red) [US2] Add integration test `t_menu_mouse_switch_and_toggle` in `crates/cargonaut-ui-tui/src/lib.rs`: clicking a different title switches the open menu; clicking the open menu's own title closes it. (FR-005, FR-006)
- [ ] T019 (green) [US2] In `handle_mouse` (in `crates/cargonaut-ui-tui/src/lib.rs`), when a menu is open and the click is neither a title, an item, nor `in_dropdown` → `menu.close()` and **fall through** (do not `return`) to the existing panel-hit logic. Confirm the existing `title_at` branch already covers switch/toggle. Make T017 + T018 pass. (FR-004, FR-005, FR-006)

**Checkpoint**: US2 tests green → menus dismiss/switch by mouse with pass-through.

---

## Phase 5: User Story 3 — Hover to highlight the item under the pointer (P3)

**Goal**: Pointer movement over an item row highlights it; no dispatch; degrades gracefully.

**Independent test**: Open a menu, move pointer over item 2 (no click) → item 2 highlighted.

- [ ] T020 (red) [US3] Add integration test `t_menu_mouse_hover_moves_highlight` in `crates/cargonaut-ui-tui/src/lib.rs`: a `Moved` event over item 2's row sets the selection to item 2 and dispatches nothing; a subsequent `Down(Left)` on it invokes item 2. (FR-007)
- [ ] T021 (red) [US3] Add integration test `t_menu_mouse_hover_border_no_change` in `crates/cargonaut-ui-tui/src/lib.rs`: a `Moved` event over the border / outside item rows leaves the highlight unchanged and dispatches nothing. (FR-008)
- [ ] T022 (green) [US3] Add a `MouseEventKind::Moved` arm to `handle_mouse` in `crates/cargonaut-ui-tui/src/lib.rs`: if `menu.is_open()` and `item_at(ui.layout.menu, ui.layout.full, x, y) == Some(i)` → `menu.select(i)` (O(1), no allocation, no dispatch). Make T020 + T021 pass. (FR-007, FR-008)

**Checkpoint**: All three stories green → full feature behavior present.

---

## Phase 6: Polish & Cross-Cutting

- [ ] T023 [P] [US3] Add integration test `t_menu_mouse_click_without_hover` in `crates/cargonaut-ui-tui/src/lib.rs`: with no prior `Moved` event, a `Down(Left)` on an item still selects-and-invokes it — proving clicks are independent of hover (graceful degradation on terminals that send no motion events). (FR-010; finding C1)
- [ ] T024 Add/confirm a regression test that with mouse suspended/disabled the new branches are inert and keyboard menu nav (F9/arrows/Enter/Esc) is unchanged — reuse the existing disabled-mouse harness in `crates/cargonaut-ui-tui/src/lib.rs`. (FR-009, FR-011)
- [ ] T025 Run `make ci-local` (fmt → clippy `-D warnings` → test → release build → docs/size) and fix any findings; ensure new `pub fn`s have `///` docs (`#![warn(missing_docs)]`).
- [ ] T026 [P] Update `README.md`: bump the "At a Glance" metrics (test count, feature count) and add a "Feature History" one-line entry for 065 menu dropdown mouse clicks. (MANDATORY docs gate)
- [ ] T027 [P] Update `Learnings.md`: append a section for feature 065 with ≥3 bullets (geometry single-source `dropdown_rect`; hover via `Moved` + graceful degradation; close-and-pass-through; `FrameLayout.full` plumbing; `in_dropdown` predicate). (MANDATORY docs gate)
- [ ] T028 Manual validation per [quickstart.md](./quickstart.md) steps 1–10 in a real terminal (`make build` then run the binary). Confirm SC-001…SC-005.

---

## Dependencies & Execution Order

- **Setup (T001–T002)** → no deps.
- **Foundational (T003–T012)** → depends on Setup. **Blocks all user stories.**
  - T003 (FrameLayout.full) is independent of the chrome.rs tasks but required before any
    `handle_mouse` task (T016, T019, T022) compiles.
  - T005 needs T004; T008 needs T005 + (T006,T007); T010 needs T005 (+ T009); T012 needs T011.
- **US1 (T013–T016)** → depends on Foundational. Delivers MVP.
- **US2 (T017–T019)** → depends on Foundational; sequence after US1 (same `handle_mouse` fn).
- **US3 (T020–T022)** → depends on Foundational; sequence after US2 (same `handle_mouse` fn).
- **Polish (T023–T028)** → depends on the targeted stories; T023 needs US1+US3 wired.

### Story independence

- US1 alone = a usable, shippable MVP (menus fully mouse-operable for invocation).
- US2 adds dismissal/switching ergonomics.
- US3 adds hover polish.

### Parallel opportunities

- `[P]` tasks: T023 (lib.rs test, after wiring) is independent of T026/T027; T026 (`README.md`)
  and T027 (`Learnings.md`) are genuinely parallel — different files.
- All other lib.rs edits are sequential (same function); chrome.rs unit tasks land sequentially
  (same file).

## Implementation Strategy

1. **MVP first**: Phases 1 → 2 → 3 (US1). Demo: open a menu, click an item, it runs.
2. **Incremental**: layer US2 then US3; each ends green and independently demoable.
3. **Close out**: Polish — degradation + disabled-mouse tests, `make ci-local`, mandatory
   README/Learnings, manual quickstart.

## Task Summary

- **Total tasks**: 28
- **Setup**: 2 (T001–T002)
- **Foundational**: 10 (T003–T012)
- **US1 (P1, MVP)**: 4 (T013–T016)
- **US2 (P2)**: 3 (T017–T019)
- **US3 (P3)**: 3 (T020–T022)
- **Polish**: 6 (T023–T028)
- **Parallel `[P]`**: T023, T026, T027
- **MVP scope**: Phases 1–3 (through T016)

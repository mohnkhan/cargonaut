# Tasks: Subshell Scrollback Rendering (Feature 055)

**Input**: Design documents from `specs/055-subshell-scrollback/`

**Branch**: `055-subshell-scrollback` | **Closes**: #79

**Constitution §II — TDD required**: Every FR has a red commit (failing test) before the green commit (implementation). Red/green commit pairs are noted on each task pair.

**Organization**: 2 user stories + foundational accessor. Stories can be implemented sequentially; US2 depends on US1 infrastructure.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: User story this task belongs to (US1, US2)

---

## Phase 1: Setup (No Changes Needed)

**Purpose**: Branch already created; no new crates, configs, or files needed. All changes land in existing files. Skip to Phase 2.

---

## Phase 2: Foundational — `screen_mut` Accessor

**Purpose**: Add `SubshellState::screen_mut()` to `subshell.rs`. Required before US1 render wiring in `lib.rs`. No behaviour change — purely an accessor.

**⚠️ CRITICAL**: Must complete before Phase 3 US1 lib.rs tasks (T003–T005).

- [ ] T001 Add compile-time smoke test `let _: fn(&mut SubshellState) -> &mut vt100::Screen = SubshellState::screen_mut;` inside `_assert_subshell_state_fields` in `crates/cargonaut-ui-tui/src/subshell.rs` mod tests — **red commit: `T001 (red): screen_mut contract assertion`** (fails to compile; method does not exist yet)
- [ ] T001b Add `screen_mut(&mut self) -> &mut vt100::Screen` method to `SubshellState` in `crates/cargonaut-ui-tui/src/subshell.rs` (alongside existing `screen(&self)` at line 207; delegates to `self.parser.screen_mut()`) — **green commit: `T001b (green): screen_mut accessor`**

**Checkpoint**: `cargo check -p cargonaut-ui-tui` passes. No test changes yet.

---

## Phase 3: User Story 1 — Scroll Up Through Subshell History (Priority: P1) 🎯 MVP

**Goal**: Scrolling the mouse wheel inside the subshell panel makes earlier output visible; scrolling back down returns to the live view.

**Independent Test**: Open subshell → run `seq 1 100` → scroll mouse wheel UP → earlier line numbers appear. Scroll DOWN → returns to live prompt.

### Tests for User Story 1 (TDD — red commit before implementation)

> **Write these tests FIRST, ensure they FAIL (todo!() panics) before implementing**

- [ ] T002 [US1] Add failing test stub `render_vt100_screen_scrollback_offset_changes_content` (body: `todo!()`) to `crates/cargonaut-ui-tui/src/subshell.rs` mod tests — **red commit: `T002 (red): scrollback render test stub`**
- [ ] T003 [US1] Add failing test stub `render_vt100_screen_hides_cursor_in_scrollback` (body: `todo!()`) to `crates/cargonaut-ui-tui/src/subshell.rs` mod tests — **red commit: `T003 (red): cursor-hide scrollback test stub`**

### Implementation for User Story 1

- [ ] T004 [US1] Fix inverted scroll direction in `crates/cargonaut-ui-tui/src/lib.rs` `handle_mouse_event`: swap `ScrollDown` → `saturating_sub(1)` and `ScrollUp` → `saturating_add(1)` (lines ~1899, 1914) — **green for FR-004**
- [ ] T005 [US1] Apply `s.screen_mut().set_scrollback(s.scroll_offset as usize)` before `let subshell_screen = ...` in `crates/cargonaut-ui-tui/src/lib.rs` run loop (after `poll_output`, before line 417) — **green for FR-001**
- [ ] T006 [US1] Add `s.screen_mut().set_scrollback(0)` after `term.draw(...)?` in `crates/cargonaut-ui-tui/src/lib.rs` run loop (after line 449) — **green for FR-002**
- [ ] T007 [US1] Skip cursor rendering in `render_vt100_screen` when `screen.scrollback() > 0` in `crates/cargonaut-ui-tui/src/subshell.rs` (wrap the cursor block at line ~394 with `if screen.scrollback() == 0 { ... }`) — **green for FR-004 cursor correctness**
- [ ] T008 [US1] Implement `render_vt100_screen_scrollback_offset_changes_content` test body in `crates/cargonaut-ui-tui/src/subshell.rs`: create `vt100::Parser::new(5, 10, 20)`, feed 25 lines, render at `set_scrollback(0)` and `set_scrollback(5)`, assert buffers differ — **green commit: `T008 (green): scrollback render changes content`**
- [ ] T009 [US1] Implement `render_vt100_screen_hides_cursor_in_scrollback` test body in `crates/cargonaut-ui-tui/src/subshell.rs`: create parser, feed content, verify that after `set_scrollback(n > 0)` the rendered buffer's cursor cell (live cursor position) does NOT carry `Modifier::REVERSED` — **green commit: `T009 (green): cursor hidden in scrollback`**
- [ ] T015 [US1] Add `scroll_lock_preserved_on_new_pty_output` test in `crates/cargonaut-ui-tui/src/subshell.rs` mod tests: create `SubshellState` (or simulate `scroll_offset` directly), set `scroll_offset = 5`, call `poll_output` on a parser with pending data, assert `scroll_offset` is still 5 (US1 AC4: scroll-lock preserved when new output arrives) — **green commit: `T015 (green): scroll-lock preserved on PTY output`**

**Checkpoint**: `cargo test -p cargonaut-ui-tui` passes. Manual test: `make run`, open subshell, `seq 1 100`, scroll up → older lines appear. Scroll down → live view returns. Cursor not rendered during scrollback.

---

## Phase 4: User Story 2 — Scrollback at Maximum History Boundary (Priority: P2)

**Goal**: Scrolling past the end of the scrollback buffer does not panic or show garbled output; the view stabilises at the oldest available line.

**Independent Test**: Open subshell, scroll up past 200 rows of history without crashing. No garbled terminal cells.

### Tests for User Story 2 (TDD — red commit before verification)

> **Write these tests FIRST with todo!() — then implement**

- [ ] T010 [US2] Add failing test stub `scrollback_clamps_at_buffer_limit` (body: `todo!()`) to `crates/cargonaut-ui-tui/src/subshell.rs` mod tests — **red commit: `T010 (red): scrollback boundary clamp test stub`**

### Implementation for User Story 2

- [ ] T011 [US2] Implement `scrollback_clamps_at_buffer_limit` test body in `crates/cargonaut-ui-tui/src/subshell.rs`: create `vt100::Parser::new(5, 10, 20)` (20-row scrollback), feed 200 lines, call `screen_mut().set_scrollback(999)` (exceeds buffer), call `render_vt100_screen`, assert no panic and buffer is non-empty — **green commit: `T011 (green): scrollback boundary clamped by vt100`**
- [ ] T017 [US2] Fix `SubshellState::resize` in `crates/cargonaut-ui-tui/src/subshell.rs`: add `self.scroll_offset = 0;` after `self.parser = vt100::Parser::new(rows, cols, 200);` so a stale offset does not reference a fresh parser's empty scrollback — **green commit: `T017 (green): reset scroll_offset on resize`**

**Note**: No lib.rs code changes needed for US2 — vt100 `set_scrollback` already clamps to `min(rows, scrollback.len())` (R-006 in research.md). T017 is a correctness fix for the resize edge case (spec Edge Cases line 47).

**Checkpoint**: `cargo test -p cargonaut-ui-tui scrollback` passes (all 3 scrollback tests). Manual: scroll up 500× without crash.

---

## Phase 5: Polish & Documentation

**Purpose**: Update mandatory docs (CLAUDE.md requires README.md + Learnings.md on every feature merge).

- [ ] T012 [P] Update `README.md`: increment test count in "At a Glance" table; add one-line entry in "Feature History" for Feature 055
- [ ] T013 [P] Update `Learnings.md`: append Feature 055 section (minimum 3 bullets: scroll-direction inversion bug, set_scrollback-before-extraction pattern, cursor hide during scrollback, resize scroll_offset reset)
- [ ] T016 [P] Verify frame-rate regression gate: run `make ci-local` confirming keypress-latency bench still passes after lib.rs render-path changes (covers FR-005 and SC-001/SC-002 via existing NFR-002 bench)

> **PR note**: Reference "Closes #79" in the PR description body. Not a code task.

**Checkpoint**: `make ci-local` passes all steps (clippy, test, build, check-pr-body, docs-gate).

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 2 (Foundational)**: No dependencies — start immediately
- **Phase 3 (US1)**: Depends on Phase 2 (T001) — needs `screen_mut()`
- **Phase 4 (US2)**: Depends on Phase 2 (T001b) only — T010–T011 and T017 need only `screen_mut()`; can begin after T001b even if US1 tests are incomplete
- **Phase 5 (Polish)**: Depends on Phase 3+4 being code-complete

### Within User Story 1

```
T002 (red stub) ──► T004 (scroll direction fix)
T003 (red stub) ──► T005 (set_scrollback before draw) ──► T008 (green T002)
                ──► T006 (reset after draw)            ──► T009 (green T003)
                ──► T007 (cursor hide)
```

T004, T005, T006, T007 can all be committed as separate green commits in any order after T002/T003 stubs exist.

### Within User Story 2

```
T010 (red stub) ──► T011 (green boundary test)
```

### Parallel Opportunities

- T002 and T003 (both red stubs in same file, same mod tests — write sequentially)
- T004, T005, T006, T007 are in different functions/locations and can be applied in any order
- T008 and T009 (both green test completions) can be done in parallel after T004–T007
- T012 and T013 (docs in different files) can be done in parallel

---

## Parallel Example: User Story 1

```bash
# After T002/T003 red stubs are committed:

# Apply all lib.rs changes (T004, T005, T006) — same file, sequential:
# T004: fix scroll direction (~line 1899, 1914)
# T005: set_scrollback before extraction (~line 415)
# T006: set_scrollback(0) reset after draw (~line 449)

# Apply subshell.rs cursor fix (T007) — different function, parallel with T005/T006:
# T007: guard cursor block with `if screen.scrollback() == 0`

# Then complete the green tests (T008, T009):
# T008: implement scrollback-changes-content test body
# T009: implement cursor-hide test body
```

---

## Implementation Strategy

### MVP (User Story 1 Only — 5 focused tasks)

1. T001: Add `screen_mut()` accessor
2. T002–T003: Red test stubs
3. T004–T007: Four targeted code changes (scroll direction fix, set_scrollback before/after, cursor hide)
4. T008–T009: Green tests
5. **STOP and VALIDATE**: `cargo test -p cargonaut-ui-tui` passes; manual scroll works

### Incremental Delivery

1. MVP (US1) → validates core rendering is wired ← most impactful
2. US2 boundary tests → adds confidence without code change
3. Polish → docs gate passes → PR ready to merge

---

## Notes

- All `lib.rs` changes are in the existing `run_loop` function — no new functions needed
- The scroll direction fix (T004) is a single-line swap per event arm, not a behaviour change to `scroll_offset` storage
- vt100 `set_scrollback` is O(1) — no frame-rate concern (Constitution §IV met)
- `screen.scrollback()` in `render_vt100_screen` (T007) reads `scrollback_offset` which is always correct during the draw (set before, reset after)
- `unsafe` is not needed anywhere in this feature (Constitution §I met)

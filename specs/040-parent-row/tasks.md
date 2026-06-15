---
description: "Task list for Feature 040 — Panel `..` parent entry as first row"
---

# Tasks: Panel `..` Parent Entry as First Row

**Input**: Design documents from `specs/040-parent-row/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/

**Tests**: REQUIRED. Constitution §II (NON-NEGOTIABLE) mandates TDD — failing
test before implementation, red commit before green. Because this feature
*reinterprets* the existing cursor model, the foundational red step both adds new
tests AND updates the ~20 existing index-coupled tests to the virtual-row model;
they fail until the green model lands.

**Organization**: US1 = ascend via the `..` row (MVP); US2 = `..` never
selectable/operable; US3 = presence rules (filter/hidden/root).

## Format: `[ID] [P?] [Story] Description`

- **[P]**: different files / no dependency on an incomplete task
- **[Story]**: US1 / US2 / US3 (setup, foundational, polish carry no label)

## Path Conventions

- Core: `crates/cargonaut-core/src/lib.rs`
- TUI: `crates/cargonaut-ui-tui/src/pane.rs`, `crates/cargonaut-ui-tui/src/lib.rs`

---

## Phase 1: Setup

- [ ] T001 Confirm tmpfs target active (`make tmpfs-status`) and baseline green (`make test`) before changes — Constitution §V.
- [ ] T002 Confirm no keymap change is needed: the `..` row reuses the existing `Descend`/Enter activation and the mouse double-click→`Descend` path; `design/contracts/keymap.toml` stays unchanged. Record in the PR.

---

## Phase 2: Foundational (Blocking Prerequisites) — the virtual-row cursor model

**Purpose**: Reinterpret `PaneState.cursor` as a virtual-row index so every story
builds on one model. **⚠️ Blocks US1, US2, US3.**

- [ ] T003 In `crates/cargonaut-core/src/lib.rs` (`#[cfg(test)]`): write failing unit tests for the model — `has_parent`/`parent_offset`/`row_count`/`on_parent_row`/`focused_row`; `focused_entry_index()` returns `None` on the `..` row and `Some(real)` otherwise; `CursorDown`/`CursorTo` clamp to `row_count()-1`; fresh non-root listing focuses the first real entry (`cursor == parent_offset()`, FR-014) and an empty non-root dir focuses `..`. ALSO update the existing index-coupled core tests (cursor/position assertions) to the virtual-row expectations. (RED)
- [ ] T004 In `crates/cargonaut-core/src/lib.rs`: add `FocusedRow` enum and `PaneState` helpers `has_parent()`/`parent_offset()`/`row_count()`/`on_parent_row()`/`focused_row()` + a private `default_cursor()`; update `focused_entry_index()` to apply `parent_offset`; change `CursorDown`/`CursorUp`/`CursorTo` to clamp against `row_count()`; change every cursor-reset site (`navigate_to`, `relist_active`, `refresh_active_pane`, `toggle_hidden`, `set_filter`, mkdir relist — see research R-004) from `cursor = 0` to `cursor = default_cursor()`. Make T003 pass. (GREEN) — doc comments on new public items.

**Checkpoint**: cursor addresses `[.. ] ++ visible entries`; selection still real-indexed; no `..` rendered yet, `Descend` not yet parent-aware.

---

## Phase 3: User Story 1 — Ascend via the `..` row (Priority: P1) 🎯 MVP

**Goal**: A `..` row renders as the first row of a non-root pane; activating it
(Enter when focused, or double-click) ascends to the parent.

**Independent Test**: In a non-root dir, assert `..` is row 0; cursor up onto it,
`Descend` → cwd is the parent; double-click row 0 → cwd is the parent.

- [ ] T005 [US1] In `crates/cargonaut-core/src/lib.rs`: failing tests `descend_on_parent_row_ascends` (cursor on `..` + `Descend` → cwd == parent) and `cursor_up_from_first_entry_lands_on_parent_then_clamps`. (RED)
- [ ] T006 [US1] In `crates/cargonaut-core/src/lib.rs`: change the `Descend` dispatch arm to `if active pane on_parent_row() → ascend_to_parent() else descend_into_focused()`. Make T005 pass. (GREEN)
- [ ] T007 [P] [US1] In `crates/cargonaut-ui-tui/src/pane.rs`: failing `TestBackend` tests — a non-root listing renders `..` as the first row; `sync_from` maps the virtual `state.cursor` to the `ListState` selection; no `..` at a root. Update existing `pane.rs` tests asserting positions to the virtual model. (RED)
- [ ] T008 [US1] In `crates/cargonaut-ui-tui/src/pane.rs`: add `has_parent()`/`row_count()`; in `sync_from` clamp the selection to `row_count()-1`; in `render` prepend a themed `..` `ListItem` when `has_parent()`; offset `focused_entry_index()`. Make T007 pass. (GREEN)
- [ ] T009 [P] [US1] In `crates/cargonaut-ui-tui/src/lib.rs`: failing mouse test — double-clicking the `..` row ascends (the click sets `CursorTo(virtual idx)`, the double-click dispatches `Descend`). Update existing mouse hit-test assertions for virtual indices. (RED)
- [ ] T010 [US1] In `crates/cargonaut-ui-tui/src/lib.rs`: adjust `handle_mouse` as needed so a click maps to the virtual-row index and a double-click on the `..` row ascends via the parent-aware `Descend` (likely no logic change beyond the core clamp; confirm/repair). Make T009 pass. (GREEN)

**Checkpoint**: MVP — the `..` row is visible and ascends by key and mouse.

---

## Phase 4: User Story 2 — `..` is never selectable or operable (Priority: P2)

**Goal**: The parent row can't be tagged and never enters a copy/move/delete set.

**Independent Test**: Toggle selection on `..` → nothing selected; invert /
select-by-pattern (incl. a `..`-matching pattern) → `..` excluded; copy/move/
delete set never contains `..`.

- [ ] T011 [US2] In `crates/cargonaut-core/src/lib.rs`: failing/locking tests — `selection_toggle_on_parent_row_is_noop`; `selection_invert_excludes_parent_row`; `select_by_pattern_never_matches_parent_row` (even pattern `..`); `selection_or_focused` on the parent row with no tags yields an empty set (copy operates on nothing). These guarantees fall out of the foundational model (real-indexed selection + `focused_entry_index() == None` on `..`); if any fails, add the minimal guard. (RED→GREEN)

**Checkpoint**: US1 + US2 — visible, ascends, and provably safe to never operate on `..`.

---

## Phase 5: User Story 3 — Presence rules: filter/hidden/root (Priority: P3)

**Goal**: `..` is always present in a non-root dir regardless of filter/hidden,
and never present at a root.

**Independent Test**: Non-root dir with a zero-matching filter still shows `..`;
toggling hidden files keeps `..`; at a root no `..` and ascent is a no-op.

- [ ] T012 [US3] In `crates/cargonaut-core/src/lib.rs`: failing/locking tests — `parent_row_present_with_zero_match_filter`; `parent_row_unaffected_by_toggle_hidden`; `no_parent_row_at_root` + ascent no-op at root. (RED→GREEN — presence keys off `has_parent()`, independent of filter/hidden.)
- [ ] T013 [P] [US3] In `crates/cargonaut-ui-tui/src/pane.rs`: failing `TestBackend` test — `render` still shows `..` first when an active filter matches zero entries, and shows none at a root. Make it pass. (RED→GREEN)

**Checkpoint**: All three stories independently verified.

---

## Phase 6: Polish & Cross-Cutting Concerns

- [ ] T014 Run `make ci-local` (clippy `-D warnings` → `cargo test --workspace` → release → docs-gate); fix any clippy/fmt/`missing_docs` findings.
- [ ] T015 [P] Run the quickstart smoke scenarios (`specs/040-parent-row/quickstart.md`); confirm SC-001…SC-006; capture in the PR body.
- [ ] T016 [P] Update `README.md`: "At a Glance" metrics (test count, feature count) + a Feature History entry for Feature 040. (docs-gate)
- [ ] T017 [P] Append a Feature 040 section to `Learnings.md` (≥3 bullets): virtual-row cursor vs render-only synthesis; why selection/op no-ops fall out for free; the ~20-test index-shift blast radius and how the offset cancels for real-index assertions. (docs-gate)
- [ ] T018 [P] Update `CHANGELOG.md`; move #37 to Resolved in `ROADMAP.md`; close issue #37 referencing the PR once merged.

---

## Dependencies & Execution Order

- **Setup (P1)** → **Foundational (P2, T003–T004)** blocks everything.
- **US1 (P3)** depends on Foundational; its core task (T006) before the TUI tasks
  (T007–T010). T007 and T009 (different files) may be written in parallel.
- **US2 (P4)** and **US3 (P5)** depend on Foundational; largely characterization
  tests of the model and can run in parallel with US1's TUI work.
- **Polish (P6)** after the stories; docs tasks T016–T018 are independent `[P]`.

### Within each story

- `(red)` test task precedes its `(green)` implementation task.
- Core model/behavior before its TUI rendering/mouse wiring.

---

## Implementation Strategy

### MVP first (US1)

1. Setup → Foundational → US1.
2. **STOP and VALIDATE**: a `..` row renders and ascends by Enter and double-click.

### Incremental delivery

1. Foundation + US1 → visible, clickable `..` (MVP).
2. + US2 → provably non-operable.
3. + US3 → presence rules locked.
4. Polish: gates + mandatory docs + close #37.

## Notes

- `[P]` = different files, no dependency on an incomplete task.
- Build/test only via `make` targets (Constitution §V); never `cargo clean` /
  `rm -rf target`.
- Commit per red and per green; no `Co-Authored-By: Claude` trailer (CLAUDE.md).
  Feature-branch PR MUST modify both `README.md` and `Learnings.md` (docs-gate).
- The selection set MUST keep referring to the same real entries (FR-010) — verify
  no off-by-one slips into `selected` when the `..` row is present.

---
description: "Task list for Feature 048: F2 User-Menu Mouse-Click Support"
---

# Tasks: F2 User-Menu Mouse-Click Support (Feature 048)

**Input**: Design documents from `specs/048-f2-mouse-click/`

**Branch**: `048-f2-mouse-click`

**Key finding from research**: No production routing code needs to change. The fkey-bar
already maps F2 → `Command::ShowUserMenu` (chrome.rs:229), and `handle_mouse` already
routes all fkey-bar clicks through `dispatch_ui_command` (lib.rs:1246). This feature
is **test-only**: add a `mouse_with_dlg()` helper that exposes the resulting
`Option<ActiveDialog>` state, and write a strong integration test asserting
`ActiveDialog::UserMenu` is set on a left-click of the F2 button.

**TDD requirement (constitution §II)**: Each task with a test MUST produce a failing
red commit before the green commit. Task IDs marked `(red)` / `(green)` indicate
the two-commit pair.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependencies)
- **[Story]**: Which user story this task belongs to

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Confirm toolchain and build hygiene before adding tests.

- [ ] T001 Confirm `make tmpfs-status` shows active symlink; run `make test` green as baseline in `crates/cargonaut-ui-tui/`

---

## Phase 2: Foundational (Blocking Prerequisites)

**Purpose**: Add the `mouse_with_dlg()` test helper that all new tests depend on.
This is the enabling infrastructure — without it, the T-MOUSE-5b test cannot be written.

- [ ] T002 (red) Add `async fn mouse_with_dlg(…) -> (String, Option<ActiveDialog>)` helper in `crates/cargonaut-ui-tui/src/lib.rs` `#[cfg(test)]` block, mirroring `mouse()` but returning `dlg` alongside `status`; add stub test `f2_mouse_click_opens_user_menu` with `assert!(false, "T002 red: stub")` so the test compiles but fails — commit as red

**Checkpoint**: `cargo test -p cargonaut-ui-tui f2_mouse_click_opens_user_menu` fails with assertion error (red state confirmed).

---

## Phase 3: User Story 1 — Mouse Click on F2 Opens User Menu (Priority: P1) 🎯 MVP

**Goal**: A left-click on the on-screen F2 button sets `ActiveDialog::UserMenu`, proving
the mouse path is equivalent to the keyboard F2 path.

**Independent Test**: `cargo test -p cargonaut-ui-tui f2_mouse_click_opens_user_menu` passes.

### Implementation for User Story 1

- [ ] T003 (green) [US1] Replace the stub body in `f2_mouse_click_opens_user_menu` (`crates/cargonaut-ui-tui/src/lib.rs`) with the real assertion: call `mouse_with_dlg(left_click(15, 23), …)`, then `assert!(matches!(dlg, Some(ActiveDialog::UserMenu { .. })), …)` — commit as green; verify test passes
- [ ] T004 [US1] Strengthen existing T-MOUSE-5 test `click_fkey_button_dispatches_command` in `crates/cargonaut-ui-tui/src/lib.rs` to also use `mouse_with_dlg` and replace the weak negative string check with `assert!(matches!(dlg, Some(ActiveDialog::UserMenu { .. })))` — one commit
- [ ] T005 [US1] Run `cargo clippy -p cargonaut-ui-tui -- -D warnings` and `cargo fmt --check`; fix any warnings in `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: `cargo test -p cargonaut-ui-tui` is fully green; T-MOUSE-5 and T-MOUSE-5b both assert `ActiveDialog::UserMenu`.

---

## Phase 4: Polish & Cross-Cutting Concerns

**Purpose**: CI gate, documentation, and roadmap cleanup required before PR merge.

- [ ] T006 Run `make ci-local` and confirm all five pipeline steps pass (clippy → test → build → check-pr-body → docs-gate) in repo root
- [ ] T007 Update `README.md`: bump test count in "At a Glance" table and add Feature 048 entry to "Feature History" section
- [ ] T008 Update `Learnings.md`: append Feature 048 section with ≥3 bullets covering what was hard, root causes, and non-obvious decisions
- [ ] T009 Update `ROADMAP.md`: remove (or move to a `## Closed` section) the Tier 3 row for issue [#70](https://github.com/mohnkhan/cargonaut/issues/70) now that this feature closes it

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 baseline green — provides `mouse_with_dlg()` helper that Phase 3 requires
- **Phase 3 (US1)**: Depends on Phase 2 (T002 must be complete before T003)
- **Phase 4 (Polish)**: Depends on Phase 3 completion

### Within Phase 3

- T003 (green) must follow T002 (red) — the red/green TDD pair
- T004 can follow T003 (strengthens an adjacent test, same file)
- T005 (lint/fmt check) follows T003 and T004

### Parallel Opportunities

- T005 (clippy/fmt) and T006 (ci-local) partially overlap but T006 is the superset; run T005 first to catch quick issues, then T006 for full pipeline.
- T007 and T008 (README + Learnings) can be written in parallel (different sections of different files).

---

## Implementation Strategy

### MVP (single commit pair)

1. Complete T001 (baseline green)
2. Complete T002 (red commit — helper + stub test)
3. Complete T003 (green commit — real assertion)
4. **STOP and VALIDATE**: `cargo test -p cargonaut-ui-tui f2_mouse_click_opens_user_menu` passes
5. Continue with T004–T009 for full CI + docs gate compliance

### Full delivery order

```
T001 → T002 (red) → T003 (green) → T004 → T005 → T006 → T007 + T008 → T009
```

---

## Notes

- [P] tasks = different files, no dependencies between them
- The routing in production code is already correct per research.md R-001 — do NOT modify `handle_mouse`, `dispatch_ui_command`, or `chrome.rs` button mappings
- `mouse_with_dlg()` is a test-only helper (inside `#[cfg(test)]`) — it never appears in the release binary
- Click coordinates `(x=15, y=23)` with `fkeys = Rect { x: 0, y: 23, width: 100, height: 1 }` place the click in the F2 slot (x in [10, 20)) — same as existing T-MOUSE-5
- No `menu.toml` is needed in the test environment: `ShowUserMenu` with no valid actions still sets `ActiveDialog::UserMenu` (new_error variant) per research.md R-005
- All file modifications are confined to `crates/cargonaut-ui-tui/src/lib.rs` plus the two mandatory doc files and ROADMAP.md

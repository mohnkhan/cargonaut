---

description: "Task list for Feature 059 — cargonaut-core god-file split"
---

# Tasks: cargonaut-core God-File Split

**Input**: Design documents from `specs/059-cargonaut-core-split/`

**Prerequisites**: plan.md, spec.md, research.md, data-model.md, contracts/, quickstart.md (all present)

**Tests**: This is a **move-only refactor** (FR-005). No new behavior ⇒ no new red-first tests (Constitution §II justified deviation, plan.md §Complexity Tracking). The existing ~140-test suite is the regression guard and travels **with** the code it exercises (R-004). The added contract gate is the rustdoc-JSON surface diff (contracts/).

**Organization**: Tasks are grouped by the three (all-P1) user stories. Because this is a refactor, US1 is the substantive work (the split); US2 (API stability) and US3 (behavior/gates) are verification stories that gate on US1 — they are *checked continuously* during US1 and *proven* in their own phases. This sequencing is intentional and stated in Dependencies.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no dependency on an incomplete task)
- Module extractions are **not** `[P]`: each edits `crates/cargonaut-core/src/lib.rs` (add `mod` + `pub use`, remove moved code), so they serialize.

## Per-extraction definition of done (applies to every T in Phase 3)

Each extraction task is complete only when, after the move:
1. `cargo fmt` applied;
2. `make test` (workspace) green — **same test count** as the recorded baseline;
3. `cargo clippy --workspace --all-targets -- -D warnings` clean;
4. the moved public items are re-exported from `lib.rs` so the surface is unchanged;
5. committed as a single reviewable move (`feat(059): extract <module> [no-docs]` — production+test move, no behavior change).

Never run `cargo clean` / `rm -rf target` (Constitution §V); use `make` wrappers.

---

## Phase 1: Setup (baseline & guards)

**Purpose**: Lock the ground truth the whole refactor is measured against.

- [ ] T001 Confirm SSD/tmpfs guard: run `make tmpfs-status` and verify `target → /tmp/cargonaut/<hash>/target` is an active symlink (Constitution §V); abort and run `make tmpfs-setup` if not.
- [ ] T002 Record the pre-refactor **green baseline**: run `make test` and capture the total executed-test count (e.g. `cargo test --workspace 2>&1 | grep 'test result:'`) into a scratch note; this number must hold at every later step (FR-006/SC-004).
- [ ] T003 [P] Verify the committed API baseline is current: regenerate `target/doc/cargonaut_core.json` via `cargo +nightly rustdoc -p cargonaut-core -- -Z unstable-options --output-format json`, run `specs/059-cargonaut-core-split/contracts/extract-public-api.py` on it, and confirm the output `diff`s empty against `contracts/public-api-baseline.txt` (179 lines). If non-empty, the baseline is stale — refresh it before any move.

---

## Phase 2: Foundational (BLOCKING prerequisite)

**Purpose**: Extract shared test fixtures so per-module tests can relocate without losing their helpers. This MUST precede any Phase-3 test relocation.

**⚠️ CRITICAL**: No module's tests can move until `test_support` exists.

- [ ] T004 Create `crates/cargonaut-core/src/test_support.rs` as `#[cfg(test)] pub(crate) mod test_support` (declared in `lib.rs` as `#[cfg(test)] mod test_support;`). Move the shared test helpers out of the monolithic `mod tests` — `make_app`, `app_with_three`, `mode_of`, `entry_index`, `submit_one_copy`, and any other fixture used by ≥2 test groups — making each `pub(crate)`. Update the still-monolithic `mod tests` to `use crate::test_support::*`. Run `make test` (same count) + clippy; commit.

**Checkpoint**: shared fixtures available crate-wide under `#[cfg(test)]`; surface unaffected (cfg-gated).

---

## Phase 3: User Story 1 — Maintainer navigates to a responsibility area quickly (P1) 🎯 MVP

**Goal**: `lib.rs` becomes a thin module root; implementation lives in ≥4 (target ~13) cohesive submodules, each with its co-located tests.

**Independent Test**: Open any one responsibility (e.g. compare) and find its code + tests in a single aptly-named file, not in `lib.rs`; `lib.rs` shows declarations/re-exports + only the `App`/`SideState` structs.

> Order respects R-008: leaf value types first, then the `App` core, then the `impl App` method modules. Each task moves **production code + that module's `#[cfg(test)] mod tests`** together (R-004 privacy rule). Each obeys the per-extraction DoD above.

### Leaf value types (no `App` dependency)

- [ ] T005 [US1] Extract `error` module: move `AppError`, `UndoEntry` (+ derives) to `crates/cargonaut-core/src/error.rs`; add `mod error;` + `pub use error::{AppError, UndoEntry};` in `lib.rs`; move their tests (if any) into `error.rs`. DoD.
- [ ] T006 [US1] Extract `command` module: move `Command`, `Event`, `DialogKind` to `crates/cargonaut-core/src/command.rs`; re-export all three; relocate any shape tests. DoD.
- [ ] T007 [US1] Extract `jobs` module: move `JobStatus`, `JobView`, `ProgressView`, `ResumeOfferView` and the projection fns `transfer_state_snapshot` (pub), `job_status_from`, `resume_offer_view`, `crc32_partial` to `crates/cargonaut-core/src/jobs.rs`; re-export the public items (`JobStatus`, `JobView`, `ProgressView`, `ResumeOfferView`, `transfer_state_snapshot`); move the crc32 + job-view tests into `jobs.rs`. DoD.
- [ ] T008 [US1] Extract `pane` module: move `PaneId`, `PaneFilter`, `PaneState`, `FocusedRow`, `TabBarEntry`, `ViewMode`, `SplitOrient` (+ all their inherent impls incl. `PaneState`'s 7 methods) and helpers `pane_idx`, `glob_match` (pub) to `crates/cargonaut-core/src/pane.rs`; re-export every public type + `glob_match`; mark `pane_idx` `pub(crate)` if referenced by other modules; move pane-filter / parent-row / glob_match tests into `pane.rs`. DoD. (Note: `App`/`SideState` stay in `lib.rs` — do not move them.)

### Core router

- [ ] T009 [US1] Extract `app` module: move `impl App` constructor `new`, all accessors (`registry`, `view_mode`, `active_progress`, `split_orient`, `config`, `active_pane`, `pane`, `active_pane_state`, `status`, `active_pane_mut`, `pane_mut`) and the `dispatch` router to `crates/cargonaut-core/src/app.rs` as `impl App { … }`; add `mod app;`; methods need no re-export (inherent methods are reachable via the type). Move the constructor/dispatch/accessor tests into `app.rs`. DoD.

### `impl App` method modules (each is one `impl App` block)

- [ ] T010 [US1] Extract `nav` module to `crates/cargonaut-core/src/nav.rs`: navigation + cwd/listing + filter methods (`relist_active`, `navigate_into`, `refresh_active_pane`, `descend_into_focused`, `sync_other_panel_path`, `show_focused_in_other_panel`, `ascend_to_parent`, `navigate_to`, `resolve_cd_target`, `quick_cd`, `complete_cd`, `selection_or_focused`, `set_filter`) + helpers `parse_path`, `next_sort_key`, `sort_label` (make `pub(crate)` only if shared). Move nav/quick-cd/complete-cd/filter/sync tests. DoD.
- [ ] T011 [US1] Extract `history` module to `crates/cargonaut-core/src/history.rs`: `history_prev_dir`, `history_next_dir`; move history tests. DoD.
- [ ] T012 [US1] Extract `fsops` module to `crates/cargonaut-core/src/fsops.rs`: `mkdir`, `select_by_pattern`, `recursive_dir_size`; move mkdir/select/dir-size tests. DoD.
- [ ] T013 [US1] Extract `attrs` module to `crates/cargonaut-core/src/attrs.rs`: `chmod_selection`, `chown_selection`, `collect_subtree`, `collect_subtree_capped`, `chmod_recursive`, `chown_recursive`, `attr_roots`, `create_symlink`, `create_hard_link`, `link_source` + `RECURSE_NODE_CAP`, `recursive_status`, `attr_status`; move the large attrs/recursive/links + `collect_subtree` test block (these tests call private methods → must be co-located). DoD.
- [ ] T014 [US1] Extract `compare` module to `crates/cargonaut-core/src/compare.rs`: `compare_directories`; move compare-directories tests. DoD.
- [ ] T015 [US1] Extract `rename` module to `crates/cargonaut-core/src/rename.rs`: `undo_last_operation`, `apply_bulk_rename` + free fn `validate_rename_proposals` (pub, re-export); move bulk-rename + undo + validate tests. DoD.
- [ ] T016 [US1] Extract `hotlist` module to `crates/cargonaut-core/src/hotlist.rs`: `bookmarks`, `add_bookmark`, `remove_bookmark`, `jump_to_bookmark`, `persist_hotlist`; move bookmark tests. DoD.
- [ ] T017 [US1] Extract `tabs` module to `crates/cargonaut-core/src/tabs.rs`: `tab_new`, `tab_close`, `tab_next`, `tab_prev`, `tab_bar_view`; move the Feature-053 tab + cross-pane + isolation + tab-bar tests. DoD.
- [ ] T018 [US1] Extract `transfers` module to `crates/cargonaut-core/src/transfers.rs`: `transfer_ids`, `transfer`, `job_views`, `cancel_transfer`, `pause_transfer`, `resume_paused`, `confirm_copy`, `transfer_opts`, `scan_resume_offers`, `pending_resume_views`, `resume_offer`, `start_over_offer`, `skip_offer`, `request_copy_confirmation`, `request_move_confirmation`, `request_delete_confirmation`; move transfer/job-view/pause/cancel/resume tests. DoD.

- [ ] T019 [US1] Final `lib.rs` tidy: confirm `lib.rs` contains only crate docs, `use`, `mod` declarations, the full `pub use` re-export surface, and the `App` + `SideState` struct definitions; remove the now-empty `#[cfg(test)] mod tests`; `cargo fmt`. Verify `wc -l lib.rs ≤ ~230`, `grep -c '    fn ' lib.rs == 0`, `grep -c '#[cfg(test)]' lib.rs == 0` (FR-001/FR-010/SC-001). Also verify the **per-submodule size ceiling** (SC-006): `wc -l crates/cargonaut-core/src/*.rs | sort -n | tail` — no production submodule should be a new god-file (split an over-large module further if needed). Commit.

**Checkpoint (US1)**: ≥4 (≈13) cohesive submodules + `test_support`; `lib.rs` thin; full suite green throughout.

---

## Phase 4: User Story 2 — Downstream crates compile unchanged (P1)

**Goal**: Prove the public API is byte-for-byte stable and no consumer needed edits.

**Independent Test**: Surface diff empty; workspace + benches build with zero downstream `src/` changes.

- [ ] T020 [US2] Regenerate the public surface (`cargo +nightly rustdoc … --output-format json` → `extract-public-api.py`) and `diff` against `contracts/public-api-baseline.txt`; **must be empty** (FR-003/SC-003). If not, reconcile by fixing re-exports (never by editing the baseline).
- [ ] T021 [P] [US2] Assert zero downstream edits: `git diff --stat origin/main -- crates/cargonaut-ui-tui/src crates/cargonaut-transfer/src crates/cargonaut-bin/src` is empty (FR-004).
- [ ] T022 [P] [US2] Build the API consumers: `make build` (full workspace) and `cargo build -p cargonaut-core --benches` both succeed unchanged (benches are an API consumer the constitution's perf gates depend on — SC-004).

**Checkpoint (US2)**: public API provably identical; all consumers compile untouched.

---

## Phase 5: User Story 3 — Behavior & quality gates provably unchanged (P1)

**Goal**: Demonstrate no behavior change and green gates.

**Independent Test**: Full suite passes with the same test count; clippy/doc/fmt gates clean.

- [ ] T023 [US3] `cargo test --workspace` passes; executed-test count equals the T002 baseline (no test dropped/disabled) (FR-006/SC-004).
- [ ] T024 [P] [US3] `cargo clippy --workspace --all-targets -- -D warnings` clean (FR-009/SC-005).
- [ ] T025 [P] [US3] `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc -p cargonaut-core --no-deps` clean (no new doc warnings; `#![warn(missing_docs)]` satisfied) (FR-008/SC-005).
- [ ] T026 [P] [US3] `cargo fmt --check` clean.

**Checkpoint (US3)**: behavior preserved; all per-PR quality gates green.

---

## Phase 6: Polish, docs & ship

**Purpose**: Mandatory docs updates, deferral paper-trail closeout, and merge gate.

- [ ] T027 [P] Update `README.md`: "At a Glance" metrics (test count unchanged; feature count +1) and add a Feature 059 one-line entry to Feature History (CLAUDE.md docs mandate).
- [ ] T028 [P] Append a Feature 059 section to `Learnings.md` (≥3 bullets): the descendant-module privacy insight (why `App`/`SideState` stay at root → zero widening), the test-co-location requirement driven by private-method visibility, and the rustdoc-JSON surface-diff gate (jq absent → Python) (CLAUDE.md docs mandate).
- [ ] T029 Reconcile the deferral paper-trail: this feature *implements* issue #86, so the ROADMAP row for #86 is resolved — update `ROADMAP.md` to mark it done/remove per house style, and ensure the PR body closes #86 (CLAUDE.md Deferrals rule).
- [ ] T030 Run the full merge gate `make ci-local` (clippy → test → release build → check-pr-body → docs-gate); must be green.
- [ ] T031 Run `specs/059-cargonaut-core-split/quickstart.md` end-to-end (the one-shot block) and confirm "ALL GREEN".
- [ ] T032 Open the PR targeting `main` (branch `059-cargonaut-core-split`), body closing #86, no AI-attribution trailers; ensure README + Learnings are in the diff so docs-gate passes.

---

## Dependencies & Execution Order

### Phase order

- **Phase 1 (Setup)** → **Phase 2 (Foundational: `test_support`)** → **Phase 3 (US1 split)** → **Phase 4 (US2 verify)** → **Phase 5 (US3 verify)** → **Phase 6 (ship)**.
- US2 and US3 are *verification* stories: they are meaningful only **after** US1's moves and therefore depend on Phase 3 completion. (For a move-only refactor the three P1 stories are facets of one change, not independent feature slices — US2/US3 invariants are also re-checked after every Phase-3 task via the per-extraction DoD.)

### Within Phase 3 (the split)

- T004 (`test_support`) blocks all of T005–T018 (their tests need shared fixtures).
- T005–T018 **serialize** — each edits `lib.rs`. Recommended order = listed order (leaf types → core → method modules) so the crate compiles at each step. T009 (`app`) should land after the leaf types it references in signatures are re-exported, and before/after the method modules interchangeably (all are `impl App`).
- T019 (lib.rs tidy) is last in Phase 3.

### Parallel opportunities

- T003 is `[P]` within Setup.
- Within Phase 4: T021, T022 are `[P]` (read-only / separate build invocations) after T020.
- Within Phase 5: T024, T025, T026 are `[P]` (independent gates) alongside T023.
- Within Phase 6: T027, T028 are `[P]` (different files); T029–T032 serialize toward the PR.
- The module extractions (T005–T018) are **deliberately not parallel** (shared `lib.rs`).

---

## Implementation Strategy

### MVP = US1 (the split)

1. Phase 1 Setup → Phase 2 `test_support` → Phase 3 extractions T005→T019.
2. **STOP and VALIDATE**: `lib.rs` thin, suite green, ≥4 modules. This alone delivers the navigability value.

### Then prove the invariants

3. Phase 4 (US2) — API surface diff empty + consumers compile untouched.
4. Phase 5 (US3) — full suite same-count + clippy/doc/fmt gates.
5. Phase 6 — README/Learnings/ROADMAP, `make ci-local`, quickstart, PR closing #86.

### Notes

- Commit after every extraction (small, reviewable, reversible — R-008 / issue #86 §Suggested approach).
- If any extraction turns a private helper into a cross-module call, prefer `pub(crate)` (never `pub`) — the narrowest legal widening (FR-007); the surface diff stays empty because `pub(crate)` is invisible to it.
- Target ~13 modules but the floor is 4 (FR-002); merging two tiny modules is acceptable if it improves cohesion.

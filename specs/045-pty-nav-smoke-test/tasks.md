# Tasks: PTY Binary-Level Navigation Smoke Test

**Input**: Design documents from `specs/045-pty-nav-smoke-test/`

**Prerequisites**: plan.md ✅ spec.md ✅ research.md ✅ data-model.md ✅ contracts/ ✅

**TDD**: Per Constitution §II, each US phase commits the failing test first, then
the green implementation. The red→green commit pair is the unit of progress.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (independent files, no in-flight dependencies)
- **[Story]**: User story the task belongs to (US1, US2, US3)
- File paths are repo-root-relative

---

## Phase 1: Setup — Shared PTY Helper Module

**Purpose**: Extract the common helpers from `resume_sigkill.rs` into a shared
module and add the new `delta_contains` helper and key constants. CI must stay
green throughout this phase.

- [ ] T001 Create `crates/cargonaut-bin/tests/common/mod.rs` and move `spawn`, `output_contains`, `wait_until`, `sigkill`, and the `PtyHandle` type alias from `crates/cargonaut-bin/tests/resume_sigkill.rs` into it; add `pub use` exports; add `pub fn enabled() -> bool { std::env::var("CARGONAUT_PTY_TESTS").map(|v| v == "1").unwrap_or(false) }`; annotate with `#[allow(dead_code)]` to silence unused-in-this-binary warnings from Cargo
- [ ] T002 Add `delta_contains(sink: &Arc<Mutex<Vec<u8>>>, prev_len: usize, needle: &str) -> bool` helper to `crates/cargonaut-bin/tests/common/mod.rs`; add key-sequence constants `KEY_DOWN`, `KEY_UP`, `KEY_ENTER`, `KEY_BACKSPACE`, `KEY_F10` as `pub const &[u8]`; verify `enabled()` (from T001) is exported from the module
- [ ] T003 Update `crates/cargonaut-bin/tests/resume_sigkill.rs` to import helpers via `#[path = "common/mod.rs"] mod common; use common::*;`; remove the duplicated local definitions; run `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test resume_sigkill` and confirm it passes

**Checkpoint**: `resume_sigkill_smoke` still passes; `common/mod.rs` compiles cleanly.

---

## Phase 2: Foundational — Red Test Stubs (TDD Gate)

**Purpose**: Replace the `#[ignore]`d stub with three failing test bodies. This is
the Constitution §II "red commit" — tests must compile and fail (not panic with a
missing-body error).

**⚠️ CRITICAL**: These tasks must be committed and verified as FAILING before the
Phase 3–5 implementations begin.

- [ ] T004 Replace the entire body of `crates/cargonaut-bin/tests/local_navigation.rs` with three `#[test]` functions — `nav_cursor_arrow_keys`, `nav_descend_enter`, `nav_ascend_backspace` — each gated by `if !common::enabled() { return; }` and ending with `assert!(false, "not yet implemented: <name>")`. Add `#[cfg(unix)]` at file top and `#[path = "common/mod.rs"] mod common; use common::*;` import. Run `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation` and confirm all three fail.

**Checkpoint**: Three test failures visible; zero `#[ignore]`d tests in the crate.

---

## Phase 3: User Story 1 — Cursor Arrow Keys (Priority: P1) 🎯 MVP

**Goal**: `nav_cursor_arrow_keys` passes — arrow keys advance and retreat the cursor,
proven at the binary level via the mini-status line observable.

**Independent Test**: `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_cursor_arrow_keys` passes.

- [ ] T005 [US1] Implement `nav_cursor_arrow_keys` in `crates/cargonaut-bin/tests/local_navigation.rs`:
  1. Create `TempDirFixture`: `left = tempfile::tempdir()`, create `left/aaa`, `left/bbb`, `left/ccc` as directories; `right = tempfile::tempdir()`
  2. `spawn(exe, left.path(), right.path())` — use `env!("CARGO_BIN_EXE_cargonaut")` for `exe`
  3. `assert!(wait_until(Duration::from_secs(5), || output_contains(&sink, "Quit")), "TUI did not start")`
  4. Loop: `prev = sink.lock().unwrap().len()` → write `KEY_DOWN` → flush → `assert!(wait_until(5s, || delta_contains(&sink, prev, "aaa")), "cursor did not reach aaa")`
  5. Repeat step 4 for `bbb` (second Down)
  6. `prev = ...len()` → write `KEY_UP` → flush → `assert!(wait_until(5s, || delta_contains(&sink, prev, "bbb")), "cursor did not retreat to bbb")`
  7. Quit: write `KEY_F10`, flush; `let pid = child.process_id().unwrap(); assert!(wait_until(5s, || child.try_wait().map(|s| s.is_some()).unwrap_or(false)), "binary did not exit"); let _ = child.wait();` — call `sigkill(pid)` if deadline hit

**Checkpoint**: `nav_cursor_arrow_keys` green; other two still fail.

---

## Phase 4: User Story 2 — Descend on Enter (Priority: P1)

**Goal**: `nav_descend_enter` passes — pressing Enter while the cursor is on a
subdirectory changes the pane's CWD to that subdirectory, observable in the pane title.

**Independent Test**: `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_descend_enter` passes.

- [ ] T006 [US2] Implement `nav_descend_enter` in `crates/cargonaut-bin/tests/local_navigation.rs`:
  1. Same fixture as T005 (`aaa/`, `bbb/`, `ccc/` inside `left`). Note: fixture uses only directories — Enter on a file is implicitly not exercised (US2 AC2); the fixture design makes this a non-issue since all navigable entries are directories.
  2. Spawn, wait for `"Quit"` signal
  3. Navigate to `bbb` using the polling pattern from T005: `prev1 = len()` → `KEY_DOWN` → `wait_until(5s, || delta_contains(&sink, prev1, "aaa"))` (land on aaa), then `prev2 = len()` → `KEY_DOWN` → `wait_until(5s, || delta_contains(&sink, prev2, "bbb"))` (land on bbb). Do NOT use a fixed sleep — poll each step per FR-006.
  4. `prev = sink.lock().unwrap().len()`
  5. Write `KEY_ENTER`, flush
  6. `assert!(wait_until(5s, || delta_contains(&sink, prev, "bbb")), "pane did not descend into bbb")`
  7. Quit cleanly (same pattern as T005 step 7)

**Checkpoint**: `nav_cursor_arrow_keys` and `nav_descend_enter` both green; `nav_ascend_backspace` still fails.

---

## Phase 5: User Story 3 — Ascend on Backspace (Priority: P2)

**Goal**: `nav_ascend_backspace` passes — pressing Backspace after descending returns
the pane to the parent directory, observable in the pane title.

**Independent Test**: `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_ascend_backspace` passes.

- [ ] T007 [US3] Implement `nav_ascend_backspace` in `crates/cargonaut-bin/tests/local_navigation.rs`:
  1. Same fixture; record `left_name = left.path().file_name().unwrap().to_string_lossy().into_owned()`
  2. Spawn, wait for `"Quit"` signal
  3. Navigate to `aaa` using polling: `prev_nav = len()` → `KEY_DOWN` → `wait_until(5s, || delta_contains(&sink, prev_nav, "aaa"))`. Then `prev_desc = len()` → `KEY_ENTER` → `wait_until(5s, || delta_contains(&sink, prev_desc, "aaa"))` to confirm descent. Do NOT use fixed sleep between steps per FR-006.
  4. `prev = sink.lock().unwrap().len()`
  5. Write `KEY_BACKSPACE`, flush
  6. `assert!(wait_until(5s, || delta_contains(&sink, prev, &left_name)), "pane did not ascend back to {left_name}")`
  7. Quit cleanly

**Checkpoint**: All three navigation tests pass. `cargo test -- --ignored` reports zero ignored tests in `cargonaut-bin`.

---

## Phase 6: Polish & Documentation

**Purpose**: Mandatory CLAUDE.md documentation gates and final CI validation.

- [ ] T008 Update `README.md`: increment the test count in the "At a Glance" metrics table and add a one-line entry for Feature 045 in the "Feature History" section
- [ ] T009 Update `Learnings.md`: append a Feature 045 section with ≥3 bullets covering: (a) the delta-buffer assertion pattern and why cumulative-buffer scanning gives false positives, (b) the `tests/common/mod.rs` helper-sharing pattern and the `mod.rs` naming requirement that prevents Cargo from treating it as a test binary root, (c) the mini-status line as the observable cursor-position signal (why the pane title alone isn't sufficient for cursor-movement assertions)
- [ ] T010 Run `CARGONAUT_PTY_TESTS=1 make ci-local` and confirm all gates pass: clippy, tests, build, check-pr-body, docs-gate

**Checkpoint**: CI green locally. Feature ready for PR.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately
- **Phase 2 (Foundational)**: Depends on Phase 1 completion (needs `common::enabled()`)
- **Phase 3 (US1)**: Depends on Phase 2 — failing test exists before implementation
- **Phase 4 (US2)**: Depends on Phase 2 — can start after Phase 2 (parallel with Phase 3 if separate sessions, but same file so sequential in practice)
- **Phase 5 (US3)**: Depends on Phase 2 — same sequential constraint
- **Phase 6 (Polish)**: Depends on Phases 3–5 all passing

### Task Dependencies (within phases)

```
T001 → T002 → T003   (helper module built incrementally, then resume test updated)
T003 → T004          (common module must compile before failing stubs reference it)
T004 → T005          (red commit before green)
T004 → T006          (red commit before green)
T004 → T007          (red commit before green)
T005, T006, T007 → T008, T009, T010
```

### Parallel Opportunities

T001–T003 are sequential (same file). T005, T006, T007 are sequential (same file).
No meaningful parallelism within this feature — all changes land in two files
(`common/mod.rs` and `local_navigation.rs`).

---

## Parallel Example (for reference)

```bash
# Phase 1 runs sequentially (same file):
# T001 → T002 → T003

# Phase 3–5 are independently testable but share local_navigation.rs:
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_cursor_arrow_keys
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_descend_enter
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_ascend_backspace

# Final smoke (all at once):
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test resume_sigkill
```

---

## Implementation Strategy

### MVP Scope (US1 only)

1. Complete Phase 1 (Setup)
2. Complete Phase 2 (red stubs)
3. Complete Phase 3 (US1 — `nav_cursor_arrow_keys` green)
4. **Validate**: `CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test local_navigation nav_cursor_arrow_keys` passes

### Full Delivery

Complete all phases in order. Total estimated effort: 0.5 ew (as per issue #30).

---

## Notes

- The `tests/common/mod.rs` pattern requires the file to be named `mod.rs` inside a `common/` subdirectory — Cargo treats only top-level `tests/*.rs` files as test binary roots, ignoring `mod.rs` files in subdirectories.
- The mini-status line observable (R-002) is key to making arrow-key assertions reliable without ANSI parsing; see `research.md` for the full rationale.
- All three test functions share the same fixture structure — the `TempDirFixture` helper may be extracted as a small inline function within `local_navigation.rs` to reduce duplication.

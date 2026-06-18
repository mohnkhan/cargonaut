# Tasks: Compare Directories + Diff Tagged Files (Feature 049)

**Input**: Design documents from `specs/049-compare-dirs/`

**Prerequisites**: plan.md ✓ spec.md ✓ research.md ✓ data-model.md ✓ contracts/ ✓ quickstart.md ✓

**TDD**: Constitution §II mandates red → green per task. Tests are committed in failing state before the implementation that makes them pass. Per-task git history MUST show the red commit preceding the green commit.

**Organization**: Tasks grouped by user story for independent implementation and testing.

## Format: `[ID] [P?] [Story?] Description`

- **[P]**: Can run in parallel (different files, no shared dependencies)
- **[Story]**: Which user story (US1, US2) from spec.md

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Dependency wiring and stub scaffolding that unblocks all story work.

- [ ] T001 Move `crc32fast` from `[dev-dependencies]` to `[dependencies]` in `crates/cargonaut-core/Cargo.toml` (required for production `compare_directories()`)
- [ ] T002 [P] Add `DiffConfig { pub tool: Option<String> }` + `pub diff: DiffConfig` to `Config` in `crates/cargonaut-config/src/lib.rs`; include TOML round-trip tests (default → None; `[diff] tool = "vimdiff"` → Some; unknown keys rejected)
- [ ] T003 [P] Add `Command::CompareDirectories` stub variant to `cargonaut-core::Command` enum and stub dispatch arm (`CompareDirectories => Ok(vec![])`) in `crates/cargonaut-core/src/lib.rs` (unblocks US1 test compilation)

**Checkpoint**: Dependencies wired, stubs in place — US1/US2 test writing can begin.

---

## Phase 2: Foundational (Blocking Prerequisite)

**Purpose**: Extend the TUI suspend/restore infrastructure before any diff wiring. Must not regress F3/F4.

**⚠️ CRITICAL**: Both user story phases depend on this change to `PendingExternal`.

- [ ] T004 Extend `PendingExternal` from `{ program: String, path: String }` to `{ program: String, args: Vec<String> }` in `crates/cargonaut-ui-tui/src/lib.rs`; update `run_external()` (`.arg(&ext.path)` → `.args(&ext.args)`); update `queue_external()` F3/F4 call site (`path: local` → `args: vec![local]`); run `cargo test -p cargonaut-ui-tui` to confirm F3/F4 tests still pass

**Checkpoint**: `PendingExternal` refactor complete; F3/F4 regression-free — US1 TUI wiring and US2 both unblocked.

---

## Phase 3: User Story 1 — Compare Two Panels (Priority: P1) 🎯 MVP

**Goal**: Pressing `C-x d` with two local-filesystem panels open compares visible entries by name/size/CRC32 and additively marks all differing entries using the existing tag system.

**Independent Test**: Open two tempdirs containing identical, size-differing, content-differing, and panel-only files; press `C-x d`; verify only differing files are tagged and previously-tagged files are unaffected. See quickstart.md Scenario 1–3.

### Tests for User Story 1 (TDD — red commits required) ⚠️

> **Write these tests first and commit them in failing state before writing implementation**

- [ ] T005 [US1] Write failing unit tests for `crc32_partial()` in `crates/cargonaut-core/src/lib.rs`: same-content files → same hash; different-content files → different hash; file >4 MiB → uses head-only read path; unreadable path → `None`; empty file → consistent hash (red)
- [ ] T006 [P] [US1] Write failing unit tests for `App::compare_directories()` via `app.dispatch(Command::CompareDirectories)` in `crates/cargonaut-core/src/lib.rs`: left-only entry → left pane tagged only; right-only entry → right pane tagged only; size-differ → both tagged; hash-differ (same size, different content) → both tagged; identical entries → neither tagged; same-path both panels → Status message, no tags added; non-local pane (sftp://) → Status error; additive — existing selection not cleared; >1,000 visible entries → first returned event is `Status("Comparing…")` (FR-009 progress indicator) (red)

### Implementation for User Story 1

- [ ] T007 [US1] Implement `crc32_partial(path: &std::path::Path, size: u64) -> Option<u32>` private fn in `crates/cargonaut-core/src/lib.rs`: if `size <= 4_194_304` read full file via `std::fs::read()`; else open file and `read(&mut buf[..524_288])` (head 512 KiB only); hash with `crc32fast::hash()`; return `None` on any I/O error (green for T005)
- [ ] T008 [US1] Implement full `App::compare_directories(&mut self) -> Result<Vec<Event>, AppError>` and replace stub dispatch arm in `crates/cargonaut-core/src/lib.rs`: (1) check both panes are `file://` scheme — return Status error otherwise; (2) if both cwds equal return Status("Both panels point to the same directory — compare would mark nothing"); (3) if total visible entries across both panes >1,000 push `Status("Comparing…")` as the first event (FR-009); (4) build `HashMap<&str, (usize, u64, &VfsKind)>` for each pane's `visible_indices()`; (5) classify each entry per data-model.md compare table; (6) additively insert differing indices into `pane.selected` via `BTreeSet::insert`; (7) return `[Status("Comparing…" if applicable), PaneUpdated(Left), PaneUpdated(Right), Status("N entries differ")]` (green for T006; depends on T007)
- [ ] T009 [US1] Wire `keymap::Command::CompareDirectories` match arm in `handle_key()` in `crates/cargonaut-ui-tui/src/lib.rs`: call `app.dispatch(cargonaut_core::Command::CompareDirectories).await?` and process returned `PaneUpdated` + `Status` events using the existing event-handling pattern
- [ ] T010 [US1] Write failing SC-001 criterion bench in `crates/cargonaut-core/benches/compare_dirs.rs` using the startup bench as pattern: create two `TempDir`s each with 1,000 files (500 identical, 250 size-differing, 250 content-differing); measure `app.dispatch(Command::CompareDirectories).await` wall-clock time; assert p95 ≤ 2,000 ms; register `[[bench]] name = "compare_dirs" harness = false` in `crates/cargonaut-core/Cargo.toml` (red)
- [ ] T011 [US1] Run `cargo bench -p cargonaut-core --bench compare_dirs` and confirm SC-001 passes ≤2 s; if timing is tight, verify partial-read path is triggered for large files (green for T010; depends on T008)

**Checkpoint**: `C-x d` fully functional. US1 independently testable per quickstart.md Scenarios 1–3.

---

## Phase 4: User Story 2 — Diff Two Tagged Files (Priority: P2)

**Goal**: Pressing `C-x C-d` with exactly two files tagged (one per panel) suspends the TUI, hands the terminal to the configured diff tool with both paths as final args, and resumes cleanly on exit.

**Independent Test**: Tag one file per panel; press `C-x C-d` with `[diff] tool = "diff -u"` configured; observe TUI suspends and `diff` output appears; on exit, TUI repaints. Error paths per quickstart.md Scenario 5 all show correct status messages.

### Tests for User Story 2 (TDD — red commits required) ⚠️

> **Write these tests first and commit them in failing state before writing implementation**

- [ ] T012 [US2] Write failing unit/integration tests for `queue_diff()` in `crates/cargonaut-ui-tui/src/lib.rs`: exactly 2 files tagged (one per pane) + tool configured → `pending_external` set with correct `program` + `args` where `args[-2]` is the left-pane file path and `args[-1]` is the right-pane file path; 1 file tagged → `pending_external` stays None, status = "Diff requires exactly 2 tagged files"; 3 files tagged → same error; tool = None → status contains "No diff tool configured"; tool = "" (empty) → status contains "Diff tool string is empty"; 2 dirs tagged (0 files) → status = "Diff requires exactly 2 tagged files (0 tagged)" (red)

### Implementation for User Story 2

- [ ] T013 [US2] Implement `queue_diff(app: &App, ui: &mut UiState, status: &mut String, diff_tool: Option<&str>)` in `crates/cargonaut-ui-tui/src/lib.rs`: collect tagged file paths from `[PaneId::Left, PaneId::Right]` (files and symlinks only, skip dirs); validate `tagged.len() == 2`; split `tool_str` via `shell_words::split()`; build `PendingExternal { program: argv[0], args: argv[1..] + tagged }` (green for T012)
- [ ] T014 [US2] Wire `keymap::Command::DiffTwoTaggedFiles` match arm in `handle_key()` in `crates/cargonaut-ui-tui/src/lib.rs`: call `queue_diff(app, ui, &mut status, app.config().diff.tool.as_deref())`
- [ ] T015 [US2] Write and pass TUI test for diff invocation in `crates/cargonaut-ui-tui/src/lib.rs` confirming that after `queue_diff()` with a valid 2-file setup, `ui.pending_external` contains a `PendingExternal` with `program` matching the first token of the tool string and the two file paths as the last two elements of `args` (green; validates T013 + T014 together)

**Checkpoint**: `C-x C-d` fully functional. US1 + US2 both independently testable.

---

## Phase 5: Polish & Cross-Cutting Concerns

**Purpose**: Schema, docs, and issue close-out.

- [ ] T016 [P] Regenerate (or manually update) `design/contracts/config.schema.json` to add `[diff]` section with `tool` property (`type: ["string", "null"]`, `default: null`, description matching contracts/config-diff.md)
- [ ] T017 [P] Update `README.md`: increment test count in "At a Glance" metrics table; add one-line entry to "Feature History" for Feature 049
- [ ] T018 [P] Append Feature 049 section to `Learnings.md` (minimum 3 bullets: what was hard, root causes, non-obvious decisions — e.g., CRC32 vs SHA-2 choice, additive tagging semantics, PendingExternal.args refactor)
- [ ] T019 [P] Update `ROADMAP.md`: remove or move to `## Closed` the #43 row
- [ ] T020 Close GitHub issue #43 with a comment referencing the merged PR

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately; T001/T002/T003 are all parallelizable
- **Phase 2 (Foundational)**: No dependency on Phase 1 tasks — can run in parallel with Phase 1
- **Phase 3 (US1)**: Requires T001 (crc32fast dep) and T003 (Command::CompareDirectories stub) before T005/T006 test compilation; T004 before T009 TUI wiring
- **Phase 4 (US2)**: Requires T002 (DiffConfig) and T004 (PendingExternal.args) before T012/T013/T014
- **Phase 5 (Polish)**: Requires all Phase 3 + Phase 4 tasks complete

### User Story Dependencies

- **US1 (P1)**: Can start after T001 + T003 (Phase 1); TUI wiring (T009) also requires T004
- **US2 (P2)**: Can start after T002 + T004; independent of US1 core logic

### Within User Story 1

```
T001 (crc32fast dep)
T003 (Command stub)
T004 (PendingExternal.args)    ← required for T009 only
    ↓
T005 (red: crc32_partial tests)    T006 [P] (red: compare_directories tests)
    ↓                                  ↓
T007 (green: crc32_partial impl)       |
    ↓                                  ↓
T008 (green: compare_directories impl) ←
    ↓                  ↓
T009 (TUI wire)    T010 (red: bench)
                       ↓
                   T011 (green: bench passes)
```

### Within User Story 2

```
T002 (DiffConfig)
T004 (PendingExternal.args)
    ↓
T012 (red: queue_diff tests)
    ↓
T013 (green: queue_diff impl)
    ↓
T014 (wire handle_key)
    ↓
T015 (integration test — green)
```

---

## Parallel Opportunities

```bash
# Phase 1 — all three parallelizable:
T001: crates/cargonaut-core/Cargo.toml
T002: crates/cargonaut-config/src/lib.rs
T003: crates/cargonaut-core/src/lib.rs   ← same file as T008 but different section

# Phase 3 red phase — both test files, same module but additive:
T005: tests for crc32_partial (new fn, no conflict)
T006: tests for compare_directories (new fn, no conflict)

# Phase 5 — all four docs tasks are independent files:
T016: design/contracts/config.schema.json
T017: README.md
T018: Learnings.md
T019: ROADMAP.md
```

---

## Implementation Strategy

### MVP First (User Story 1 Only)

1. Phase 1 (T001–T003) + Phase 2 (T004)
2. Phase 3 (T005–T011) — US1 complete
3. **STOP and VALIDATE**: Run quickstart.md Scenarios 1–3 manually
4. Ship if US1 alone is sufficient; add US2 in the same PR or defer

### Full Feature (Both Stories)

1. Phase 1 + Phase 2 → foundation ready
2. US1 (T005–T011) → test independently
3. US2 (T012–T015) → test independently
4. Polish (T016–T020)
5. Open PR; CI must pass all gates including SC-001 bench

---

## Notes

- `[P]` tasks = different files or additive sections; no shared-state conflicts
- `[US1]`/`[US2]` maps each task to its user story for traceability
- Red commits (failing tests) MUST precede green commits (implementations) per Constitution §II
- Partial-read strategy for large files: head 512 KiB only — see research.md R-002
- `crc32fast` is in workspace deps but was dev-only in `cargonaut-core` — T001 promotes it
- `PendingExternal.args` refactor (T004) is backward-compatible: F3/F4 becomes `args: vec![path]`
- SC-001 bench goes in `crates/cargonaut-core/benches/` following the `startup.rs` pattern

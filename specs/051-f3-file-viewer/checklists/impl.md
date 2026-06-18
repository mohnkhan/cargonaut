# Implementation Readiness Checklist: Internal File Viewer F3

**Purpose**: Validate that all requirements are specified with sufficient clarity, completeness, and consistency for implementation to proceed without guesswork. This checklist tests the *requirements themselves*, not the implementation.
**Created**: 2026-06-19
**Feature**: [spec.md](../spec.md) · [plan.md](../plan.md) · [tasks.md](../tasks.md) · [data-model.md](../data-model.md) · [contracts/viewer-keymap.md](../contracts/viewer-keymap.md)
**Depth**: Standard (PR-review audience)
**Focus areas**: Architecture contracts · TDD scenario coverage · Performance measurability · Edge case specification · Constitution compliance

---

## Requirement Completeness

- [ ] CHK001 Are all `FileViewerAction` variants (`Close`, `Swallow`, `NeedsData`) specified with their expected callers and preconditions documented in data-model.md? [Completeness, data-model.md §FileViewerAction]
- [ ] CHK002 Are the exact public methods required on `FileViewerDialog` to satisfy T015's chord-dispatch arm (`open_goto_prompt`, `goto_end`, `toggle_wrap`, `close`, `toggle_mode`, `advance_search`, `append_lines`, `set_status`) all listed in data-model.md or tasks.md? [Completeness, Gap]
- [ ] CHK003 Is the `SeqLookup` API that T015 calls (`lookup_sequence`, `Partial`, `Found`, `NoMatch`) documented in spec, plan, or keymap.rs inline docs — or is it only implied from the existing codebase? [Completeness, Assumption]
- [ ] CHK004 Are requirements for what happens to `chord_buf` on viewer close specified (clear vs. leave intact)? [Completeness, T015]
- [ ] CHK005 Is it specified whether `handle_key` returns `Ok(false)` or `Ok(true)` when a `SeqLookup::Partial` is pending inside the FileViewer arm? [Completeness, T015]
- [ ] CHK006 Are requirements for the `set_status` method (max length, truncation, thread-safety) defined anywhere? [Completeness, Gap]
- [ ] CHK007 Is the `word_wrap` rendering behavior for lines shorter than viewport width specified (no-op vs. explicit left-align)? [Completeness, Spec §FR-010]
- [ ] CHK008 Are requirements for the hex mode `End` / `G` key specified for files whose total byte count is not a multiple of 16 (partial last row)? [Completeness, Edge Case]

---

## Requirement Clarity

- [ ] CHK009 Is "ANSI stripping happens at load time" (research.md R-001) unambiguously specified as applying to both `Loaded` and `Streaming` variants, not just the initial window? [Clarity, research.md §R-001, Gap]
- [ ] CHK010 Is the phrase "retain the symlink's display name for the title bar" (T013, M2 fix) clear enough to implement: does `display name` mean `Path::file_name()` of the original argument, not the canonicalized path? [Clarity, T013]
- [ ] CHK011 Is the `(empty file)` message in T009 defined as appearing in the *content area* or the *status bar*? The task says "content area" but the keymap contract's status line format doesn't list this case. [Clarity, T009, contracts/viewer-keymap.md]
- [ ] CHK012 Is "streaming forward-scroll — approaching window end" quantified with a specific lookahead threshold (e.g., within N lines of window edge) to determine when `NeedsData` fires? [Clarity, T042]
- [ ] CHK013 Is the `buffer_end_line` field referenced in T045 (`partial_search_annotation`) defined in data-model.md? The `ViewBuffer::Streaming` struct does not show this field. [Clarity, Ambiguity, data-model.md §ViewBuffer]
- [ ] CHK014 Is the hex-mode title bar `[hex]` vs. text-mode `[text]` suffix consistently specified in both the keymap contract and spec (FR-006/011), with no ambiguity around what label a binary file that the user has force-toggled to text mode shows? [Clarity, contracts/viewer-keymap.md §Title bar]
- [ ] CHK015 Is the status line `wrap: on/off` field's exact position within the status string format defined? The keymap contract shows `Line <N>/<TOTAL>  [<filename>]  [wrap: on/off]` — are the bracket delimiters mandatory? [Clarity, contracts/viewer-keymap.md §Status line]

---

## Requirement Consistency

- [ ] CHK016 Does the `SearchState` struct in data-model.md include a `direction` field and a separate `last_match_col` field — but T028's "all visible matches" requirement implies scanning all line positions, making `last_match_col` insufficient as the sole highlight anchor? Are the data model fields consistent with FR-018? [Consistency, data-model.md §SearchState, Spec §FR-018]
- [ ] CHK017 Does T030 wire `Command::PreviewSearchNext/Prev` (`n`/`N`) via the raw keymap path established in T015, or does it add a separate `widget.advance_search` call independently? Is there a consistency risk between T030's approach and the T015 chord-dispatch arm? [Consistency, T030, T015]
- [ ] CHK018 Are the `Streaming` scroll methods (`scroll_down`, `page_down`) in T042 consistent with the `Loaded` variants from T010, or do they have divergent return signatures (`FileViewerAction` vs. `()`)? [Consistency, T010, T042]
- [ ] CHK019 Is the goto prompt format (`Go to line: _` vs. `Go to offset: _`) used consistently between T034 (state machine) and T035 (rendering) and the keymap contract? [Consistency, T034, T035, contracts/viewer-keymap.md §Goto prompt]
- [ ] CHK020 Does task T023 (`Command::ToggleHexView` in lib.rs) align with T015's chord-dispatch approach — i.e., `ToggleHexView` should arrive via `SeqLookup::Found`, not as a raw key? [Consistency, T023, T015, C2 fix]

---

## Acceptance Criteria Quality

- [ ] CHK021 Is SC-001 ("p50 ≤ 150 ms for ≤ 1 MiB file") measurable under the T048 bench setup? Does "p50" mean criterion's mean or median, and is the assert written against the right criterion `Measurement` field? [Measurability, Spec §SC-001, T048]
- [ ] CHK022 Is SC-002 ("≤ 16 ms keypress→repaint") measurable via T049 as written — does the bench isolate the render round-trip from the event loop latency or measure both combined? [Measurability, Spec §SC-002, T049]
- [ ] CHK023 Is SC-003 ("≤ 64 MiB RSS with 1 GiB file open") measurable via T050 using the `jemalloc` or `std` allocator on Linux — is the RSS measurement method specified (e.g., `/proc/self/status VmRSS` vs. `jemalloc_ctl`)? [Measurability, Spec §SC-003, T050]
- [ ] CHK024 Is SC-005 ("≥ 30 new tests") verifiable from the final task list? Can the count of new `#[test]` functions be derived from tasks T004/T007/T008/T017/T018/T024/T025/T031/T032/T037-T039, and does that sum reach 30? [Measurability, Spec §SC-005]
- [ ] CHK025 Is the acceptance criterion for "streaming opens quickly" (Quickstart Scenario 5, T040-T044) defined with a measurable threshold — or is it only "within 150 ms visible" (SC-001 applies to ≤ 1 MiB, not the 15 MiB streaming scenario)? [Measurability, Spec §SC-001, Gap]

---

## Edge Case and Scenario Coverage

- [ ] CHK026 Are requirements defined for what happens when a file's ANSI-stripped content is empty (e.g., a file containing only ANSI escape codes)? [Edge Case, Gap]
- [ ] CHK027 Are requirements defined for a very narrow terminal (e.g., ≤ 20 columns) where the hex row format (`00000000  HH HH …  |ASCII16|` = 73 chars) exceeds the viewport width? [Edge Case, Spec Edge Cases]
- [ ] CHK028 Are requirements for search wrap-around behavior specified — does searching past the last match wrap to the beginning, or does it stop with "Pattern not found"? [Clarity, Coverage, Spec §FR-017]
- [ ] CHK029 Is the behavior of `G` (goto end) in text mode on a streaming file specified — does it require reading the entire file to find the last line, or does it jump to `total_lines_hint` which may be approximate? [Edge Case, Ambiguity, data-model.md §ViewBuffer, T044]
- [ ] CHK030 Are requirements defined for what happens when the file grows while the viewer is open (e.g., a log file being written to)? [Coverage, Gap]
- [ ] CHK031 Is the behavior of `Backspace` in goto/search prompts on an already-empty buffer specified (no-op vs. dismiss prompt)? [Edge Case, Gap]
- [ ] CHK032 Is `Enter` with an empty goto buffer specified as a no-op or as navigating to line 1? [Clarity, T034, contracts/viewer-keymap.md §Goto prompt]
- [ ] CHK033 Are requirements defined for files with no trailing newline — does the last line display correctly and does the line count include it? [Edge Case, Gap]

---

## Architecture Requirements Quality

- [ ] CHK034 Is the requirement that `open_file_viewer` be `async` (using `spawn_blocking`) documented in plan.md as a hard constraint, or is a synchronous alternative acceptable? [Completeness, plan.md §R-002, Assumption]
- [ ] CHK035 Is the `FileViewerDialog` field `status: String` documented as the *only* mechanism for communicating viewer status to the render layer, or are other state fields (e.g., `search`, `prompt`) also consulted directly in `render`? [Clarity, data-model.md §FileViewerDialog]
- [ ] CHK036 Is the `WINDOW_MAX_LINES = 2000` cap specified as a hard limit (drop from front on push) or a soft limit (grow until OOM)? [Clarity, data-model.md §Constants]
- [ ] CHK037 Is the requirement that all file I/O happens in `spawn_blocking` (not blocking the async executor) documented as a MUST (not a SHOULD) in plan.md or constitution? [Completeness, plan.md §R-002]
- [ ] CHK038 Is the requirement that `FileViewerDialog` contains no async fields (tokio handles, channels) specified anywhere — i.e., that it is a pure synchronous data structure owned by the sync dialog model? [Completeness, Assumption]

---

## Non-Functional Requirements

- [ ] CHK039 Is binary size impact of `strip-ansi-escapes = "0.2"` documented (research.md R-007 says ~15-30 KB) and is the SC-004 headroom (current ~2.72 MiB + 30 KB + viewer code < 8 MiB) explicitly validated? [Measurability, Spec §SC-004, research.md §R-007]
- [ ] CHK040 Are performance requirements specified for hex mode seek operations — is there an expectation that `File::seek + read_exact` completes within the SC-002 16 ms keypress budget? [Completeness, Spec §SC-002, research.md §R-005]
- [ ] CHK041 Are there accessibility requirements defined for the viewer's key bindings (e.g., screen reader compatibility, colorblind-safe highlight contrast for `Style::reversed()`)? [Coverage, Gap]

---

## Constitution Compliance

- [ ] CHK042 Are all new public items in `dialog.rs` listed in tasks.md with an explicit doc-comment requirement, satisfying the `#![warn(missing_docs)]` mandate (Constitution §I)? [Completeness, Constitution §I, H4 fix]
- [ ] CHK043 Is the TDD red→green requirement (Constitution §II) enforced for every FR-tagged task — are there any FR-### tasks in tasks.md that skip a red commit? [Coverage, Constitution §II]
- [ ] CHK044 Is the keymap.toml-first ordering (Constitution §III) documented as a task-level constraint in tasks.md, not just as a phase-level note? [Clarity, Constitution §III, M1 fix]
- [ ] CHK045 Are any `unsafe` blocks anticipated in the viewer implementation, and if so, is the required `// SAFETY:` comment specified as a task requirement? [Completeness, Constitution §I]
- [ ] CHK046 Is there a task that runs `make check-tmpfs` or equivalent to confirm tmpfs is active before the benchmark tasks (T048–T050) execute? [Coverage, Constitution §V]

---

## Dependencies and Assumptions

- [ ] CHK047 Is the assumption that `memchr` is already a transitive dependency (research.md R-003) validated against the current `Cargo.lock` — or could a `crossterm` version bump remove it? [Assumption, research.md §R-003]
- [ ] CHK048 Is the assumption that `VfsKind::File` and `VfsKind::Symlink` are both valid variants in the current core crate documented and verified against the existing codebase? [Assumption, T046]
- [ ] CHK049 Is the assumption that `ExternalTool::Pager` (the call being replaced in T014) is not used by any other dispatch path documented, to confirm a clean replacement without regressions? [Assumption, T014, H1 fix]
- [ ] CHK050 Is the dependency between the `tokio::task::spawn_blocking` API and the Tokio version pinned in `Cargo.toml` (`tokio = "1.40"`) documented — specifically that `spawn_blocking` is stable in 1.40? [Assumption, Dependency]

---

## Notes

- Items marked **[Gap]** indicate the requirement is absent and must be added or confirmed as intentionally out of scope before implementation begins.
- Items marked **[Ambiguity]** indicate the requirement exists but needs clarification to avoid multiple valid interpretations.
- Items marked **[Assumption]** indicate an implicit assumption that should be verified against the current codebase before the relevant task is started.
- CHK013 (`buffer_end_line` field) is a **blocker for T045** — if the field is absent from `ViewBuffer::Streaming`, T045 cannot implement the streaming annotation without adding it.
- CHK016 (`last_match_col` vs. all-matches highlighting) is a **blocker for T028** — the data model may need a `matches: Vec<(usize, usize)>` per-line field instead of a single `last_match_col`.

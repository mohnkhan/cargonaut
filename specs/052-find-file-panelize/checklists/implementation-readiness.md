# Implementation Readiness Checklist: Find-File and Panelize

**Purpose**: Pre-implementation quality gate — validate that all spec artifacts (spec.md, plan.md, tasks.md, contracts/, data-model.md) are complete, clear, consistent, and measurable before coding begins. Items are "unit tests for the requirements" — they ask whether requirements are *written correctly*, not whether the system *works*.
**Created**: 2026-06-19
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)
**Audience**: Implementer + PR reviewer
**Depth**: Thorough (pre-implementation gate)

---

## TDD / Test Coverage Specification Quality

- [x] CHK001 — Are all 19 FRs (FR-001–FR-019) traceable to at least one red→green task pair in tasks.md, so no FR is implemented without a prior failing test? [Completeness, Spec §FR-001–§FR-019]
- [x] CHK002 — Are the Constitution §II red→green commit conventions ("`(red)` before `(green)` in git history") documented in tasks.md conventions so the implementer knows exactly what the pattern requires? [Clarity, tasks.md §Conventions]
- [x] CHK003 — Is the runtime `rg --version` skip guard (T012) specified using `std::process::Command` at test start (not an invalid `#[cfg_attr(not(rg_available), ignore)]` compile-time predicate, which Rust does not support)? [Correctness, tasks.md T012]
- [x] CHK004 — Are all four TestBackend render scenarios (InputFocused, Walking, ResultsFocused with 2 results, and long-path truncation with `…`) specified in T010, so the render contract is fully verified by the red test? [Completeness, tasks.md T010]
- [x] CHK005 — Is the test-only `start_walk_with_delay` helper (T016) specified to isolate `thread::sleep` injection to the test code path only, with an explicit statement that production `start_walk` must have no sleep? [Clarity, tasks.md T016]
- [x] CHK006 — Does T033 specify the exact tarpaulin invocation (`--package cargonaut-ui-tui --lcov`) and the 80% threshold as a hard gate (not advisory), enforcing Constitution §II before T034 (issue close)? [Measurability, tasks.md T033]

---

## Async Concurrency Specification Quality

- [x] CHK007 — Is the abort timing bound (≤300 ms, checked per-directory-read cycle) specified in contracts §7 with a quantified upper bound, not a vague "cancel promptly"? [Measurability, contracts §7, Spec §SC-006]
- [x] CHK008 — Is the mpsc channel drain strategy (loop `try_recv()` until `Empty`, not a single `recv()` call) specified in contracts §4 to prevent single-item-per-tick partial draining? [Clarity, contracts §4]
- [x] CHK009 — Is the "incremental" streaming definition quantified in FR-005 as "batched per 100ms UI tick" (not "frame-by-frame"), so an implementer cannot satisfy it with a non-streaming bulk load? [Clarity, Spec §FR-005]
- [x] CHK010 — Are the Done-event phase transitions (Walking + Done(0 results) → NoResults; Walking + Done(≥1 results) → ResultsFocused) captured in contracts §3b truth table, preventing the bug where 0-result walks incorrectly transition to ResultsFocused? [Completeness, contracts §3b]
- [x] CHK011 — Is the abort atomicity specified: `widget.cancel()` must be called *before* `active_dialog = None` (not after), so the walk task cannot send further results after the dialog is closed? [Clarity, tasks.md T017]

---

## External Process Integration Quality

- [x] CHK012 — Is the ripgrep invocation command fully specified in tasks.md T013, including the mandatory flags (`--files-with-matches`, `--no-messages`) and argument ordering, so the implementer cannot accidentally invoke rg in a different mode? [Completeness, tasks.md T013]
- [x] CHK013 — Is `tokio::process::Command` (not `std::process::Command`) mandated for Content-mode walk, with an explicit reason (`kill_on_drop` for async-native cancellation) documented in tasks.md T013? [Clarity, tasks.md T013]
- [x] CHK014 — Is rg non-zero exit handling specified as: send `Done { truncated: false }` with accumulated results (never panic, never hang the event loop), per T030/T030B? [Coverage, Spec §FR-012, tasks.md T030]
- [x] CHK015 — Is `plan_content_available` specified as a pure function (runtime binary check via `rg --version`) with a clear return contract (`true` = rg found, `false` = unavailable), so Content-mode gating is testable without I/O faking? [Clarity, tasks.md T004/T005]

---

## Contract Completeness

- [x] CHK016 — Does contracts §5 (Panelize) enumerate all seven panel operations (cursor movement, tag, copy, move, delete, F3 view, F4 edit) consistent with FR-009, so no operation is silently left untested? [Completeness, contracts §5, Spec §FR-009]
- [x] CHK017 — Does contracts §8 (Help text) include the task cross-reference (→ T020 red, T021 green) so the help-text requirement is tied to a specific red→green pair? [Traceability, contracts §8]
- [x] CHK018 — Do contracts §3b (Enter) and §3c (Esc) truth tables cover all four phases (InputFocused, Walking, ResultsFocused, NoResults) with no phase missing from either table? [Completeness, contracts §3b/§3c]
- [x] CHK019 — Does contracts §6 (find_label lifecycle) specify that find_label is cleared by `navigate_to(real_dir)` but NOT by Esc-cancel, so partial-navigation clearing is distinguishable from cancel? [Clarity, contracts §6]
- [x] CHK020 — Is the keymap single-source-of-truth requirement (contract §1: `design/contracts/keymap.toml` first, then parsed at startup) traceable to FR-013, so implementers cannot hardcode `M-?` in feature code? [Traceability, contracts §1, Spec §FR-013]

---

## Data Model Completeness

- [x] CHK021 — Are all `FindFileDialog` struct fields (results, walk_rx, abort_flag, phase, mode, input, scroll_offset, truncated, notice, content_available) named in data-model.md with their types, so the implementer has a complete struct blueprint? [Completeness, data-model.md]
- [x] CHK022 — Are the `FindEvent` enum variants (`Found(PathBuf)`, `Done { truncated: bool }`) fully specified in data-model.md, distinguishing per-file events from the terminal signal? [Completeness, data-model.md]
- [x] CHK023 — Is it explicitly stated in data-model.md that `SyntheticListing` is NOT a new type — it reuses the existing `DirListing` struct — preventing the introduction of an unnecessary parallel type? [Clarity, data-model.md]

---

## Error and Edge Case Coverage

- [x] CHK024 — Is FR-018 (unreadable root → error notice, no walk started; unreadable subdir → silently skipped) specified with distinguishable behavior for the two cases, so implementers do not conflate root failure with subdir failure? [Clarity, Spec §FR-018]
- [x] CHK025 — Is the empty-input edge case (empty string → substitute `"**"` before passing to globset, preventing a panic on empty-glob construction) captured in both FR-003 and the spec edge cases section? [Completeness, Spec §FR-003, §Edge Cases]
- [x] CHK026 — Is the max_results truncation path (walk stops at `config.search.max_results`, sends `Done { truncated: true }`, header shows "N matches (truncated)") specified in contracts §4 and verifiable by T024? [Completeness, contracts §4, tasks.md T024]
- [x] CHK027 — Is the "re-open dialog after cancel" scenario (US3 scenario 2: new dialog opens with no stale results or state from the prior walk) explicitly specified in the user stories? [Coverage, Spec §US3]

---

## Performance and Constitution Requirements Quality

- [x] CHK028 — Is SC-001 (≤5 s name search on 10k-file tmpfs tree) tied to a specific hardware context ("tmpfs-backed, warm cache, same hardware as CI benchmarks") so the criterion is not environment-ambiguous? [Measurability, Spec §SC-001]
- [x] CHK029 — Is SC-002 (≤16ms frame budget during walk) explicitly tied to the existing `benches/keypress-latency.rs` bench as the CI gate, with a statement that no new bench is required? [Measurability, Spec §SC-002]
- [x] CHK030 — Are FR-015 (`#![warn(missing_docs)]` on all new public items) and FR-016 (no `unsafe`) verifiable via the existing CI clippy step (`-D warnings`), not requiring new tooling? [Measurability, Spec §FR-015/§FR-016]
- [x] CHK031 — Is the binary-size constraint (SC-007: no regression beyond 50 KiB stripped, total ≤8 MiB) tied to `scripts/check-binary-size.sh` in T031, so it is a CI-enforceable gate and not a post-merge observation? [Measurability, Spec §SC-007, tasks.md T031]

---

## Discoverability and UX Specification Quality

- [x] CHK032 — Is FR-019 (F1 help overlay) specified with the exact section placement ("Navigation or Search section") and a verifiable test criterion (help string contains both `M-?` and `Find`), so the requirement is testable? [Clarity, Spec §FR-019, contracts §8]
- [x] CHK033 — Is FR-010 (status bar `[Find: pattern]`) specified to clarify it *replaces* the directory path segment (not appends to it), so implementers know the exact rendering location and the passive pane is unaffected? [Clarity, Spec §FR-010]
- [x] CHK034 — Are FR-002 (Tab toggles Name/Content) and FR-012 (Tab disabled when rg absent) specified consistently — i.e., Tab in Name mode with rg absent shows notice and stays in Name, never silently succeeds or crashes? [Consistency, Spec §FR-002/§FR-012]

---

## Implementation Sequencing Quality

- [x] CHK035 — Does tasks.md §Dependencies specify that T001 (tmpfs check + baseline build) must complete before any task that requires compilation, preventing SSD violations from a cold-start? [Completeness, tasks.md §Dependencies]
- [x] CHK036 — Does tasks.md §Dependencies specify that T002–T003 (Foundational: Command variant + M-? binding) must complete before all three user-story phases, so no story phase begins without a dispatchable action? [Completeness, tasks.md §Dependencies]
- [x] CHK037 — Is the Polish phase ordering specified (T031 CI gate → T032 docs → T033 tarpaulin coverage → T034 issue close), so docs and coverage cannot be skipped before the PR merges? [Completeness, tasks.md §Dependencies]

---

## Notes

Total: 37 items (CHK001–CHK037).
Focus areas: TDD specification quality, async concurrency completeness, external process integration, contract seam coverage, data model completeness, error/edge case handling, constitution traceability, discoverability, sequencing.

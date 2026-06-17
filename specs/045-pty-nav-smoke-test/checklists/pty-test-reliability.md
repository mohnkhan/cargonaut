# PTY Test Reliability Checklist: PTY Binary-Level Navigation Smoke Test

**Purpose**: Validate that the requirements for the PTY navigation tests are precise,
complete, consistent, and free of ambiguity that would lead to a flaky or unmaintainable
test suite. Items assess the *requirements themselves* — not whether the implementation
passes.

**Created**: 2026-06-17
**Feature**: [spec.md](../spec.md) | [research.md](../research.md) | [contracts/test-functions.md](../contracts/test-functions.md)
**Audience**: PR reviewer
**Depth**: Standard

---

## Observable Signal Specification

- [ ] CHK001 Is the startup-ready signal ("Quit" string in the function-key bar) precisely identified so it cannot produce a false positive from directory names or file contents? [Clarity, Spec §Assumptions]
- [ ] CHK002 Is the mini-status line format documented clearly enough that an implementer can identify exactly which bytes constitute the "focused entry name" signal, without ANSI parsing? [Clarity, research.md R-002]
- [ ] CHK003 Are the observable signals for "CWD changed" (pane title update) vs "cursor moved" (mini-status update) distinctly specified, so they cannot be confused during assertion? [Consistency, research.md R-002]
- [ ] CHK004 Is it specified which pane (left vs. right) all assertions target, in case the binary renders both panes simultaneously? [Clarity, Spec §Assumptions]
- [ ] CHK005 Are the expected entry names (`aaa`, `bbb`, `ccc`) documented as being unique enough to not collide with any other observable TUI string (e.g., a translated label, a theme name, or a status message)? [Clarity, research.md R-004]
- [ ] CHK006 Is it specified that the mini-status line is absent (empty) when the cursor rests on the `..` parent row, so the startup state before any arrow key is predictable? [Completeness, research.md R-002]

---

## Timing & Deadline Requirements

- [ ] CHK007 Is the 5-second per-assertion deadline quantified and consistent across all three test functions, or are different deadlines permitted per scenario? [Clarity, Spec §SC-002, FR-006]
- [ ] CHK008 Is a maximum overall test timeout (30 s per plan.md) cross-referenced in the spec so that per-assertion deadlines and total runtime are reconcilable? [Consistency, plan.md §Technical Context]
- [ ] CHK009 Is the polling interval (50 ms per research.md / resume_sigkill pattern) documented as a requirement, or only implied? An undocumented interval is a hidden tuning knob. [Completeness, research.md R-001]
- [ ] CHK010 Are cleanup deadline requirements (how long to wait for F10 exit before killing) specified consistently across all three test functions? [Consistency, Spec §FR-007, contracts/test-functions.md]
- [ ] CHK011 Is the requirement that startup polling uses the same `wait_until` helper (not a fixed sleep) stated explicitly in FR-006, or only implied by the clarification text? [Clarity, Spec §FR-006]

---

## TDD / Constitution Compliance Requirements

- [ ] CHK012 Is the "red commit before green commit" ordering requirement stated as a hard gate (blocking) rather than an advisory? An advisory TDD requirement is effectively unenforceable in CI. [Clarity, tasks.md §Phase 2, constitution §II]
- [ ] CHK013 Is the observable failure condition for the red commit defined — i.e., is it specified that the red tests must fail *for the right reason* (assertion failed) rather than compile-error or panic? [Completeness, tasks.md T004]
- [ ] CHK014 Is it specified what the green commit must not break — i.e., that `resume_sigkill_smoke` must continue passing after the helper refactor (T001–T003)? [Completeness, tasks.md §Phase 1 Checkpoint]
- [ ] CHK015 Are the naming conventions for the three test functions (`nav_cursor_arrow_keys`, `nav_descend_enter`, `nav_ascend_backspace`) documented in both the spec and the contracts, and are they consistent? [Consistency, Spec §FR-008, contracts/test-functions.md]

---

## Test Isolation & Environment Requirements

- [ ] CHK016 Is it specified that each test function creates its own independent `TempDirFixture` (not shared state between tests), so tests can run in any order without interfering? [Completeness, Spec §FR-002, data-model.md §TempDirFixture]
- [ ] CHK017 Are the requirements for zombie-process prevention (F10 quit + deadline kill) stated for each test function, not just as a global principle? [Completeness, Spec §FR-007, contracts/test-functions.md]
- [ ] CHK018 Is it documented that the test must set `TERM=xterm-256color` when spawning the binary, and that omitting this env var could cause crossterm to render differently? [Completeness, data-model.md §PtyHandle]
- [ ] CHK019 Is the `#[cfg(unix)]` scope requirement stated at the file level (not per-function), so it cannot be applied inconsistently? [Clarity, Spec §FR-009]
- [ ] CHK020 Is the requirement for the `tests/common/mod.rs` naming (vs. `tests/common.rs`) documented — specifically that `mod.rs` prevents Cargo from treating it as a standalone test binary root? [Completeness, tasks.md §Notes, plan.md §Structure Decision]

---

## Edge Case Coverage

- [ ] CHK021 Is the "Backspace at the pane root" edge case (no navigable parent) covered with a specified outcome — i.e., is "pane remains at current directory without crashing" sufficient, or must a specific observable signal be defined? [Clarity, Spec §US3 AC2]
- [ ] CHK022 Is the "Enter on a regular file" edge case (US2 AC2) either explicitly tested or explicitly excluded with justification? The current note in tasks.md acknowledges it but does not add an observable requirement. [Coverage Gap, tasks.md T006 step 1, Spec §US2 AC2]
- [ ] CHK023 Is the "Binary not found" edge case specified with a concrete failure message requirement, or is it open-ended? [Clarity, Spec §Edge Cases]
- [ ] CHK024 Is the "Empty directory" edge case (cursor on `..`, arrow keys do not panic) testable from the spec — i.e., is "does not panic" the full requirement, or must the test also assert the cursor remains on `..`? [Clarity, Spec §Edge Cases]
- [ ] CHK025 Is the "Non-TTY environment" skip requirement specified in terms of the `#[cfg(unix)]` guard alone, or are there additional runtime conditions (e.g., PTY device unavailable) that require a runtime skip rather than a compile-time guard? [Completeness, Spec §Edge Cases]

---

## SC-002 Flakiness & Reliability Requirements

- [ ] CHK026 Is SC-002's "does not flake on three consecutive CI runs" criterion measurable without manual intervention, or is it inherently a post-implementation observation? If the latter, does the spec acknowledge this limitation? [Measurability, Spec §SC-002]
- [ ] CHK027 Are requirements defined for what the test should do when the PTY output sink produces no new bytes within the deadline — is the failure message required to be human-actionable (e.g., print what was received)? [Completeness, Spec §FR-006]
- [ ] CHK028 Is there a requirement that the test prints a diagnostic (e.g., the last N bytes of PTY output) on assertion failure, so CI logs are self-diagnosing without re-running locally? [Gap, Spec §FR-006]

---

## Requirement Consistency Across Artifacts

- [ ] CHK029 Are the key sequence byte values (`\x1b[B`, `\x1b[A`, `\r`, `\x7f`, `\x1b[21~`) consistent between spec.md Assumptions, data-model.md Key Sequences, and contracts/test-functions.md? [Consistency]
- [ ] CHK030 Is the delta-buffer assertion strategy (taking `prev_len` before each action) consistent across all three test functions as described in contracts/test-functions.md, and does it match the tasks.md implementation steps? [Consistency, contracts §nav_cursor_arrow_keys, tasks.md T005–T007]
- [ ] CHK031 Does the SC-004 requirement ("zero ignored tests in `cargonaut-bin`") align with T010's validation command (`cargo test -- --ignored`)? Specifically, is `--ignored` guaranteed to surface tests in the `cargonaut-bin` integration test suite and not be scoped to unit tests only? [Clarity, Spec §SC-004, tasks.md T010]

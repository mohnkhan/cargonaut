# Test Requirements Quality Checklist: F2 User-Menu Mouse-Click Support

**Purpose**: Validate the quality, completeness, and measurability of the test design and behavioral parity requirements before implementation. This is a requirements review ("unit tests for English"), not a verification of system behavior.
**Created**: 2026-06-18
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)
**Audience**: PR reviewer
**Depth**: Standard

---

## Requirement Completeness

- [x] CHK001 - Are requirements defined for what happens when F2 is left-clicked while another dialog is already open (e.g., TasksPanel)? The guard exists in production code (Feature 047), but spec.md's FR section does not explicitly state "clicking F2 while a dialog is active is a no-op." [Completeness, Gap, Spec §FR-001] → **Fixed**: FR-001 updated to explicitly state "If another dialog is already active, the click MUST be ignored (no-op), identical to the keyboard F2 guard behavior."
- [x] CHK002 - Are the fkey-bar button bounds (x/y extent of the F2 button hit area) specified precisely enough to drive test coordinate selection? The spec says "at the F2 button position" — is this defined with enough precision that a future test author could reproduce the coordinates independently? [Completeness, Spec §FR-004] → **Pass**: plan.md §Implementation approach specifies: 100-wide fkey-bar, 10 buttons, button index 1 → x in [10, 20); coordinates (15, 23) documented. Sufficient for reproducibility.
- [x] CHK003 - Is the "no menu.toml" case specified as an explicit acceptance scenario rather than inferred from the Assumptions section? (Currently documented only in Edge Cases and Assumptions, not as a numbered acceptance scenario.) [Completeness, Spec §Edge Cases] → **Pass**: Edge Cases is the correct location for this; it is not a primary user journey. The acceptance scenarios cover the positive (dialog opens) and negative (Esc closes) flows. Edge Case coverage is adequate.
- [x] CHK004 - Are requirements for the negative case (clicking a non-F2 fkey-bar button does NOT open UserMenu) traceable to a task? The spec defines acceptance scenario 3, but tasks.md has no task for this — it relies on pre-existing test coverage. Is this reliance documented explicitly? [Completeness, Gap] → **Pass**: tasks.md Notes section explicitly states "No production routing code changes are needed" and references existing T-MOUSE-* tests for negative-case coverage. The reliance is documented.

---

## Requirement Clarity

- [x] CHK005 - Is "same outcome as pressing the F2 keyboard key" in FR-002 defined precisely enough to be testable? "Same outcome" could mean: same `ActiveDialog` variant set, same dialog rendering, or same user experience. Is the measurable interpretation pinned to `ActiveDialog::UserMenu { .. }` matching? [Clarity, Spec §FR-002] → **Fixed**: FR-002 updated to define "Same outcome" as `ActiveDialog::UserMenu { .. }` variant being set, explicitly excluding sub-field parity from scope.
- [x] CHK006 - Is FR-003 ("IF the routing does NOT already dispatch ShowUserMenu…") a conditional requirement or a diagnostic check? A conditional FR that may or may not apply based on research findings is unusual in a spec — is its scope (verification step vs. production fix requirement) clear? [Clarity, Spec §FR-003] → **Pass**: The conditional phrasing is correct — FR-003 is a safety-net requirement that handles the case where the routing is broken. Research confirmed it isn't, but the spec must be valid regardless of research findings. Conditional FRs are appropriate here.
- [x] CHK007 - Is "left-click" in FR-001 and the acceptance scenarios precisely defined (left mouse button only, any point within F2 button bounds)? Could a right-click or middle-click at the same coordinates ambiguously satisfy the requirement? [Clarity, Spec §FR-001] → **Pass**: "Left-click" in TUI context maps unambiguously to `MouseButton::Left` (crossterm's enum). The term is a standard domain term; no additional disambiguation needed. Acceptance scenario uses "left-clicks" explicitly.
- [x] CHK008 - Does the spec define what "zero behavioral divergence" in SC-002 means at the implementation level — same dialog type set, or also same dialog internal state (widget content, entry_path)? [Clarity, Spec §SC-002] → **Fixed**: SC-002 rewritten to say "both result in `ActiveDialog::UserMenu { .. }` being set — measurable by asserting the same enum variant," removing the vague "zero behavioral divergence" phrase.

---

## Acceptance Criteria Quality

- [x] CHK009 - Is SC-001 ("integration test passes green without special env flags") objectively measurable by a CI system, or does it depend on the test runner configuration assumed by plan.md? [Measurability, Spec §SC-001] → **Pass**: `cargo test --workspace` with no env flags is a CI-runnable, deterministic gate. The CI pipeline from CLAUDE.md already runs this step. Objectively measurable.
- [x] CHK010 - Can SC-002 ("zero behavioral divergence") be objectively verified by comparing `ActiveDialog` state alone, or does it also require comparing rendered output? Is the measurable boundary stated? [Measurability, Spec §SC-002] → **Fixed**: SC-002 now explicitly states the measurable boundary is the enum variant assertion, not rendered output.
- [x] CHK011 - Is SC-003 ("no existing tests regress") measurable with a specific gate (e.g., `make ci-local` green) or is it open-ended? If `make ci-local` is the gate, should it be stated explicitly in SC-003? [Measurability, Spec §SC-003] → **Fixed**: SC-003 updated to say "verified by `make ci-local` completing all five pipeline steps green."

---

## Scenario Coverage

- [x] CHK012 - Are requirements defined for the state of `entry_path` in the resulting `ActiveDialog::UserMenu` when the F2 click happens with the cursor on the `..` parent row? The Feature 047 handler resolves this to the pane cwd, but is the test required to assert this sub-field or only the dialog variant? [Coverage, Spec §US1] → **Pass**: FR-002 (updated) explicitly excludes sub-field parity from scope. The test asserts only the variant. The entry_path resolution behavior is Feature 047's responsibility, not this feature's.
- [x] CHK013 - Is the behavior for "mouse disabled" (`--no-mouse` or config `ui.mouse = false`) specified? The spec does not mention what happens when mouse is globally disabled and the user somehow triggers a left-click event — is this explicitly excluded or unintentionally omitted? [Coverage, Gap] → **Pass**: Out of Scope + the `if !ui.mouse_enabled { return Ok(()); }` guard at handle_mouse:1227 short-circuits before fkey routing. The test explicitly uses `fresh_ui(..., true)` (mouse enabled). Omission is intentional; explicitly noted in plan.md Constraints.
- [x] CHK014 - Does the spec address what the test's cleanup state must be (e.g., that `active_dialog` is reset to `None` after test teardown) to prevent test pollution between test functions? [Coverage, Edge Case] → **Pass**: Each `#[tokio::test]` creates its own `let mut dlg: Option<ActiveDialog> = None` local variable via `mouse_with_dlg()`. No shared state; cleanup is automatic via Rust's drop semantics. No spec requirement needed.

---

## Dependencies & Assumptions

- [x] CHK015 - Is the assumption "fkey-bar renders F-key buttons at predictable positions based on terminal width" validated and traceable to source code? plan.md states this as an assumption; is it documented in `chrome.rs` or test infrastructure so future test authors can verify the coordinate calculation? [Assumption, Spec §Assumptions] → **Pass**: `chrome.rs` unit test at line 584–587 already exercises `command_at` with specific coordinates. The calculation (`width / 10 * slot_index`) is implicit in that test and in the existing T-MOUSE-5 test. Sufficient traceability.
- [x] CHK016 - Is the dependency on `ActiveDialog::UserMenu` being visible within the test module (i.e., the variant is accessible in `#[cfg(test)]`) explicitly stated? The `mouse_with_dlg()` helper must return `Option<ActiveDialog>` which requires the enum to be in scope — is this a documented assumption or already guaranteed by the module structure? [Dependency, plan.md §Implementation approach] → **Pass**: `#[cfg(test)]` block is inside `crates/cargonaut-ui-tui/src/lib.rs`, the same file that defines `ActiveDialog`. Enum visibility is guaranteed by Rust's module system. No documentation gap.

---

## TDD Compliance (Constitution §II)

- [x] CHK017 - Is the red commit's failure mode precisely specified? plan.md says "commit as red" using `assert!(false, "T002 red: stub")` — is this specific enough for a reviewer to confirm TDD was followed from git history alone (i.e., the commit message convention clearly distinguishes red from green)? [Completeness, plan.md §Phase 2] → **Pass**: tasks.md uses "(red)" and "(green)" labels on task IDs (T002 red, T003 green). Constitution §II requires "T-ID (red):" / "T-ID (green):" commit message format — consistent with all prior features. Sufficient for git history review.
- [x] CHK018 - Does the tasks.md TDD pair (T002 red / T003 green) map cleanly to a single requirement (FR-004), or does the split introduce ambiguity about which commit satisfies which FR? [Consistency, tasks.md §Phase 2–3] → **Pass**: T002 (helper + stub) satisfies FR-004's "test MUST exist" at the structural level; T003 (correct assertion) satisfies FR-004's "asserts ActiveDialog::UserMenu" at the behavioral level. The split is clear and maps to FR-004 exclusively. No ambiguity.

---

## Notes

- Performance, security, and accessibility items are explicitly excluded — not applicable to this XS test-only feature.
- Spec updated pre-implementation: FR-001 (guard behavior), FR-002 ("same outcome" defined), SC-002 (measurable boundary), SC-003 (make ci-local gate added).
- All 18 items pass after spec updates. Ready for implementation.

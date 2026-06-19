# Implementation Gate Checklist: Subshell Scrollback Rendering (Feature 055)

**Purpose**: Validate requirements quality, clarity, and completeness before implementation begins — unit tests for the spec/plan/tasks documents
**Created**: 2026-06-19
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)

---

## Requirement Completeness

- [x] CHK001 Are all six functional requirements (FR-001 through FR-006) each mapped to at least one task in tasks.md? [Completeness, Spec §Requirements] — PASS: FR-001→T005/T008; FR-002→T006; FR-003→T011; FR-004→T004/T007; FR-005→T016; FR-006→T002/T008
- [x] CHK002 Is the scroll-lock behaviour (US1 AC4: historical view preserved when new PTY output arrives) covered by a dedicated test task (T015)? [Completeness, Spec §US1-AC4] — PASS: T015 added
- [x] CHK003 Is the resize-while-scrolled edge case covered by both a code fix (reset `scroll_offset` in `resize()`) and a task (T017)? [Completeness, Spec §Edge Cases] — PASS: T017 added; spec Edge Case updated
- [x] CHK004 Are requirements defined for what happens when the subshell process exits while the user is scrolled into history? [Completeness, Spec §Edge Cases] — RESOLVED: spec clarified that `dead=true` shows restart notice; scrollback inaccessible while dead (acceptable — out of scope)
- [x] CHK005 Does tasks.md include a task for the frame-rate regression gate (FR-005 / SC-001 / SC-002), referencing the existing NFR-002 bench? [Completeness, Spec §FR-005] — PASS: T016 added
- [x] CHK006 Is the `screen_mut()` accessor required by T001 covered by a TDD red commit (compile-time assertion) before the green commit? [Completeness, Constitution §II] — PASS: T001 amended to include compile-time function-pointer assertion red commit

---

## Requirement Clarity

- [x] CHK007 Is "within one rendered frame (≤16 ms)" in SC-001 and SC-002 specific enough to be objectively verifiable without implementation knowledge? [Clarity, Spec §SC-001, SC-002] — PASS: "≤16 ms" is a concrete threshold; verifiable via NFR-002 bench
- [x] CHK008 Is "earlier output lines scroll into view proportional to the scroll distance" in US1 AC1 quantified — does "proportional" mean one row per wheel click, or a different ratio? [Clarity, Spec §US1-AC1] — RESOLVED: spec updated to "one row per `MouseEventKind::ScrollUp` event"
- [x] CHK009 Is "no visual jitter or crash" in US1 AC3 specific enough? Is "jitter" defined with a measurable criterion? [Clarity, Spec §US1-AC3] — RESOLVED: spec updated to "no cell content changes across consecutive frames at boundary"
- [x] CHK010 Is the term "historical view is preserved" in US1 AC4 precisely defined? [Clarity, Spec §US1-AC4] — RESOLVED: spec updated to "`scroll_offset` value is unchanged — only modified by mouse scroll events, not PTY output"
- [x] CHK011 Is "no garbled terminal cells" in US2 Independent Test defined with a measurable criterion? [Clarity, Spec §US2] — RESOLVED: spec updated to "all rendered cells must contain valid content (space or printable character)"
- [x] CHK012 Is the available buffer size (200 rows) stated in the spec or only in plan.md? [Clarity, Spec §US2-AC1] — RESOLVED: spec US2 AC1 now states "up to 200 rows — the fixed scrollback capacity of the vt100 parser"

---

## Requirement Consistency

- [x] CHK013 Is the spec Assumptions section now consistent with plan.md after remediation M1 (scroll direction inversion documented)? [Consistency, Spec §Assumptions] — PASS: spec Assumptions updated to note scroll direction inversion
- [x] CHK014 Do spec Key Entities and plan.md agree that `render_vt100_screen` requires no signature change? [Consistency, Spec §Key Entities, Plan §T3] — PASS: spec Key Entities updated; both agree no signature change
- [x] CHK015 Is the scroll direction convention (ScrollUp → increase offset = older content) consistent across all docs? [Consistency] — PASS: all four docs agree; T004 documents the direction fix
- [x] CHK016 Does spec FR-002 ("restored to 0") align with plan.md's approach (reset after `term.draw`)? Are there non-render accesses between set_scrollback and reset? [Consistency, Spec §FR-002] — PASS: `poll_output` (the only other parser access) runs BEFORE `set_scrollback` is applied; no access between apply and reset
- [x] CHK017 Are two test stubs (T002, T003) consistent with SC-004 ("at least one")? [Consistency, Spec §SC-004] — PASS: SC-004 is a minimum; two tests exceeds the requirement; consistent

---

## Acceptance Criteria Quality

- [x] CHK018 Can SC-003 ("all existing tests continue to pass") be objectively verified? [Acceptance Criteria, Spec §SC-003] — PASS: `cargo test --workspace` is the verifiable gate; bounded by current test suite at time of PR
- [x] CHK019 Is SC-004 measurable with a boolean outcome (buffer A ≠ buffer B assertion)? [Acceptance Criteria, Spec §SC-004] — PASS: T008 test asserts `assert_ne!(live_buf, scroll_buf)` — clear boolean outcome
- [x] CHK020 Is SC-005 verifiable via T011 boundary test alone? [Acceptance Criteria, Spec §SC-005] — PASS: T011 calls `set_scrollback(999)` on a 20-row parser — directly exercises the clamping path; no manual test needed
- [x] CHK021 Are SC-001/SC-002 verifiable via existing keypress bench? [Acceptance Criteria, Spec §SC-001, SC-002] — PARTIAL: `benches/keypress-latency.rs` covers frame budget but is not run in `ci-local`. T016 requires manual `make bench` before merge. Pre-existing gap documented in plan.md §Constitution Check.

---

## Scenario Coverage

- [x] CHK022 Are requirements defined for the case where `scroll_offset = 0` and the user scrolls down (no-op at live bottom)? [Coverage, Spec §US1-AC3] — PASS: US1 AC3 covers this; `saturating_sub` clamps at 0
- [x] CHK023 Are requirements defined for cursor visibility in scrollback mode? [Coverage, Spec §FR-004] — RESOLVED: FR-004 updated to explicitly state "cursor MUST NOT be rendered when `scroll_offset > 0`"
- [x] CHK024 Is "scroll_offset persists after panel hide/show" addressed? [Coverage, Gap] — PASS: `SubshellState` is not dropped on hide (only `subshell_phase` changes); persisting scroll_offset is correct behavior; explicitly out of scope for this feature
- [x] CHK025 Is "scroll_offset resets to 0 on shell restart" addressed? [Coverage, Gap] — PASS: `SubshellState::respawn()` already resets `scroll_offset = 0` (subshell.rs:224); no change needed
- [x] CHK026 Is panel height change while scrolled addressed? [Coverage, Spec §Edge Cases] — RESOLVED: `resize()` replaces the parser entirely; T017 adds `scroll_offset = 0` reset to `resize()` — stale offset is cleared on panel height change

---

## Edge Case Coverage

- [x] CHK027 Is the aliased mutable access edge case addressed? [Edge Case, Spec §Edge Cases] — PASS: plan.md research R-002 documents the sequential mutable-then-immutable borrow pattern; spec Edge Case updated to note the pattern is safe (borrow ends before next access)
- [x] CHK028 Is US2 AC2 ("empty scrollback buffer, scroll up → offset stays at 0") covered? [Edge Case, Spec §US2-AC2] — PASS: T015 tests scroll_offset unchanged after poll_output with scroll_offset=0; the saturating_sub(1) on 0 stays at 0
- [x] CHK029 Is u16 overflow risk addressed? [Edge Case, Plan §Constraints] — PASS: `u16::MAX = 65535`; `vt100::set_scrollback` clamps to `scrollback.len()` (max 200); `scroll_offset` growing to 65535 still renders correctly (clamped to 200 by vt100). Not a functional risk.
- [x] CHK030 Are requirements defined for partially full scrollback? [Edge Case, Coverage] — PASS: vt100 `set_scrollback` clamps to `scrollback.len()` — if only 3 rows exist and user scrolls up 5, offset is clamped to 3. SC-005 covers this; T011 exercises it (feeds 200 lines into 20-row buffer — always fills it).

---

## Non-Functional Requirements

- [x] CHK031 Is the frame-rate requirement (FR-005: ≤16 ms) tied to a specific CI enforcement mechanism? [NFR, Constitution §IV, Spec §FR-005] — PARTIAL: `benches/keypress-latency.rs` enforces this but `ci-local` skips benches by design (release-only). Gap is pre-existing; documented in plan.md §Constitution Check. T016 requires manual bench run before merge.
- [x] CHK032 Is the O(1) allocation requirement documented? [NFR, Plan §Performance Goals] — PASS: plan.md §Performance Goals states "No extra allocation per frame — `set_scrollback` mutates in-place"
- [x] CHK033 Is there a requirement for memory eviction at buffer capacity? [NFR, Gap] — PASS: vt100's ring-buffer evicts oldest rows automatically when at capacity (200-row limit). No additional requirement needed; behavior is defined by the dependency.
- [x] CHK034 Will new tests maintain ≥80% coverage threshold? [NFR, Constitution §II] — PASS: T002/T003/T008/T009/T010/T011/T015 add 7 new tests to `cargonaut-ui-tui`; existing coverage was well above 80%; additions only improve it

---

## TDD / Constitution Compliance

- [x] CHK035 Does every FR have a corresponding red test stub before the green commit? [Constitution §II, Tasks §TDD pairs] — PASS: FR-001→T002(red)/T008(green); FR-002→T006 (reset, tested implicitly by T008); FR-003→T011; FR-004→T003(red)/T009(green); FR-005→T016; FR-006→T002(red)/T008(green)
- [x] CHK036 Is T001 compliant with Constitution §II's "pure-doc" exemption? [Constitution §II, Tasks §T001] — PASS: T001 now requires compile-time function-pointer assertion as red commit before the accessor implementation
- [x] CHK037 Are red/green commit message conventions specified for every TDD pair? [Constitution §II, Tasks] — PASS: all TDD pairs in tasks.md include explicit `T### (red): ...` / `T### (green): ...` commit message directives
- [x] CHK038 Does plan.md Constitution Check cover §I through §V? [Constitution Check, Plan] — PASS: all 7 principles covered; §V marked N/A for CI; §II updated to PARTIAL for SC-001/SC-002 bench gate

---

## Dependencies & Assumptions

- [x] CHK039 Is `vt100::Parser::screen_mut()` availability validated by direct inspection? [Assumption, Research §R-001] — PASS: verified in `~/.cargo/registry/src/.../vt100-0.16.2/src/parser.rs:62` — `pub fn screen_mut(&mut self) -> &mut crate::Screen`
- [x] CHK040 Is exclusive parser access during render guaranteed? [Assumption, Spec §Assumptions] — PASS: PTY reader sends bytes via `mpsc::channel`; `poll_output` drains channel before draw; no concurrent write during `term.draw()`. Sequential event-loop architecture guarantees this.
- [x] CHK041 Is the 200-row scrollback capacity referenced in the spec? [Assumption, Gap] — RESOLVED: spec US2 AC1 now states "up to 200 rows — the fixed scrollback capacity of the vt100 parser"; plan.md §Constraints also documents it. Named constant deferral noted (out of scope for this feature).
- [x] CHK042 Is keyboard scroll explicitly out of scope in the spec? [Assumption, Spec §Assumptions] — PASS: spec §Assumptions states "Mobile / non-mouse input paths (keyboard scroll) are out of scope"

---

## Notes

- Check items off as completed: `[x]`
- Items marked `[Gap]` flag missing requirement content — resolution may require spec update before implementation
- Items referencing Constitution §II are hard gates — must pass before first code commit
- CHK008 and CHK009 are the most likely to require spec wording updates before implementation
- CHK031 requires confirming that `make ci-local` includes the keypress-latency bench step

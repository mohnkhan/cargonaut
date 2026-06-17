# Implementation Gate Checklist: User Menu (F2) + Scrollable Help (F1)

**Purpose**: Pre-implementation requirements quality gate — validates that spec, plan, and tasks are internally consistent, unambiguous, and safe to implement. Tests the REQUIREMENTS, not the code.
**Created**: 2026-06-18
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)
**Analyzer findings incorporated**: C1, C2, C3, C4, G1, G2, G3 from `/speckit-analyze` report

---

## Requirement Completeness

- [ ] CHK001 Is the full set of keyboard navigation keys for the F1 overlay (Up, Down, PageUp, PageDown, Home, End, Esc, F1) explicitly listed in the requirements, or does spec §FR-003/FR-005 rely on an implied "standard scrollable overlay" contract? [Completeness, Spec §FR-003]

- [ ] CHK002 Does spec §FR-015 specify what "remains responsive" means for the TUI while an async action runs — is the user able to navigate panes, press keys, or are they limited to seeing the status bar update only? [Completeness, Spec §FR-015]

- [ ] CHK003 Is the behavior of the `{path}` placeholder when the cursor is on the `..` parent row defined in the requirements (not just in Edge Cases prose), and does it match what tasks.md T026 will implement? [Completeness, Spec Edge Cases, Gap]

- [ ] CHK004 Are the requirements complete for what happens when an action command is still running and the user presses F2 again — can a second action be launched concurrently, or must the first complete first? [Completeness, Gap]

- [ ] CHK005 Does spec §FR-016 define the display duration for the "Done." success message (spec Assumptions says 2 seconds) — is this duration requirement in the FRs themselves or only buried in Assumptions? [Completeness, Spec §FR-016, Assumption]

- [ ] CHK006 Are requirements specified for how `menu.toml` is found when both `XDG_CONFIG_HOME` and `HOME` are unset — is the fallback path (`".config/cargonaut/menu.toml"`) documented as an explicit FR or only in plan.md? [Completeness, Spec §FR-010, Gap]

---

## Requirement Clarity & Measurability

- [ ] CHK007 Is "full-screen modal overlay" (spec §FR-001) quantified — does it mean 100% of terminal columns/rows, or is a small border margin acceptable? The current spec says "covers the entire terminal area" but `centered_rect` is referenced in plan.md. [Clarity, Spec §FR-001, Conflict]

- [ ] CHK008 Is "line number indicator" (spec §FR-004) specific enough — is `[N/M]` (lines) or `[N%]` (percentage) required, or is either acceptable? An implementer choosing one form should not fail a spec review. [Clarity, Spec §FR-004]

- [ ] CHK009 Is "truncated to one line" (spec §FR-016) defined with a character limit? Without a maximum column width, "one line" depends on terminal width. [Clarity, Spec §FR-016]

- [ ] CHK010 Is "single printable character" (spec §FR-020, menu.toml `key` field) defined to exclude control characters, space, and high-Unicode code points, or does the requirement rely on TOML's `char` type to enforce this? [Clarity, Spec §FR-020]

- [ ] CHK011 Is spec §SC-003 "within 500 ms of the Enter keypress" defined as the time from keypress to process spawn, to first byte of output, or to status-bar update? These can differ by hundreds of milliseconds. [Measurability, Spec §SC-003]

- [ ] CHK012 Is "truncated with ellipsis" (spec Edge Cases — very long action label) measurable — what is the maximum label width in characters before truncation applies? [Clarity, Edge Case]

---

## Requirement Consistency (Analyzer Finding Resolution)

- [ ] CHK013 **[Analyzer C1 — RESOLVE BEFORE T001]** Does spec §FR-014 still reference `shell-quote` crate while plan.md, research.md, and tasks.md all use `shell-words`? This terminology drift must be corrected in spec.md before implementation begins. [Consistency, Spec §FR-014]

- [ ] CHK014 **[Analyzer C2 — RESOLVE]** Does spec Assumptions still claim the `dirs` crate "is already a dependency"? Research R-005 confirmed it is not in the workspace. This stale assumption should be corrected to avoid misleading implementers. [Consistency, Spec Assumptions]

- [ ] CHK015 **[Analyzer C3 — RESOLVE BEFORE T012/T031]** Do tasks T012 and T031 have a clear division of ownership for the "User Menu (F2)" section in `HELP_SECTIONS`? T012 must either create a placeholder or leave it entirely to T031 — the current text implies both tasks fully define it. [Consistency, tasks.md T012/T031]

- [ ] CHK016 **[Analyzer C4 — RESOLVE BEFORE T009/T014]** Is the `HelpOverlay::handle_key` method signature consistent between the test spec (T009) and implementation spec (T014)? T009 describes tests without a `visible_height` parameter; T014 introduces one. The test will not compile against the implementation. [Consistency, tasks.md T009/T014]

- [ ] CHK017 Are the two instances of "shell-quote" in spec.md (FR-014 and Assumptions §L148) both updated to match the plan/task decision of `shell-words 1.1`? [Consistency]

---

## Shell Safety & Security Requirements

- [ ] CHK018 Does spec §FR-014 explicitly forbid raw string interpolation of `{path}` into shell commands (e.g., `format!("cat {path}")`)? The requirement says "shell-quote crate or `Command::new`" but does not say the naive alternative is forbidden. [Clarity, Spec §FR-014, Security]

- [ ] CHK019 Is the requirement for detecting shell metacharacters (spec Edge Cases, plan.md R-001) exhaustive — are `>`, `<`, `!`, `{`, `}`, `(`, `)` also in scope, or only the listed set (`|`, `;`, `&&`, `||`, `$`, `` ` ``)? An incomplete list would allow shell injection through unlisted operators. [Completeness, Spec Edge Cases, Security]

- [ ] CHK020 Is the `only_if` condition itself subject to the same macro-safety quoting rules as `command`? Spec §FR-019 defines the behavior but does not explicitly state that `{path}` in `only_if` must also be shell-quoted (as distinguished from raw). [Completeness, Spec §FR-019]

- [ ] CHK021 Are the requirements clear on whether `command` fields in `menu.toml` are interpreted by the user's `$SHELL` or always by `/bin/sh -c`? The distinction matters for shell-specific features (arrays, `[[`, `source`). [Clarity, Gap]

- [ ] CHK022 Is there a requirement preventing a user from defining an action whose `only_if` expression itself launches long-running or harmful processes? The 200 ms timeout is stated in plan.md but not in spec §FR-019. [Coverage, Spec §FR-019, Gap]

---

## Modal UX & Edge Case Coverage

- [ ] CHK023 **[Analyzer G3 — RESOLVE BEFORE T027]** Does spec §FR-021 (or Edge Cases) define the behavior when F1 is pressed while the F2 menu is open? The current Edge Cases say "F2 menu closes first; F1 overlay is not opened simultaneously" — but this behavior has no corresponding task and contradicts how the existing dialog-swallow architecture works. [Consistency, Spec Edge Cases]

- [ ] CHK024 Is the behavior of pressing F2 while the F1 help overlay is open specified? The spec defines F1→F2 but not F2→F1. [Coverage, Gap]

- [ ] CHK025 **[Analyzer G2 — DECIDE]** Does spec Assumptions include "mouse support for the F2 menu" as in-scope, but tasks.md has zero tasks for it? Either the assumption must be removed (with a deferral issue opened per CLAUDE.md policy), or a mouse-handling task must be added to Phase 4. [Consistency, Spec Assumptions, tasks.md]

- [ ] CHK026 Are requirements defined for the F2 overlay behavior when `menu.toml` is modified on disk between two F2 presses within the same session — does the spec explicitly promise "reloaded on each open" (plan.md says so) at the requirements level? [Completeness, Spec §FR-010, Gap]

- [ ] CHK027 Is the behavior specified when multiple menu items share the same `key` shortcut character — the spec says "first duplicate wins" only in the contract doc, not in spec §FR-020 or the requirements table. [Completeness, Spec §FR-020]

- [ ] CHK028 Are requirements defined for the F1 overlay when the application has zero live keybindings (hypothetical empty keymap) — should HELP_SECTIONS still render section headers or show a dedicated empty-state message? [Edge Case, Coverage]

---

## Non-Functional Requirements Quality

- [ ] CHK029 **[Analyzer G1 — RESOLVE BEFORE T032]** Are SC-001 (F1 <100 ms) and SC-003 (F2 launch <500 ms) backed by a CI gate (bench or integration test) as required by constitution §II? The current tasks only cover these via manual quickstart scenarios (T036), not automated gates. [Measurability, Spec §SC-001/SC-003, Constitution §II]

- [ ] CHK030 Is spec §SC-007 "32 KiB above Feature 046 baseline" documented with the actual Feature 046 baseline binary size, so an implementer can measure the delta without running a separate build? [Measurability, Spec §SC-007]

- [ ] CHK031 Is spec §FR-009 "render legibly on terminals of at least 80×24" paired with a definition of "legibly" — e.g., minimum readable characters per line, or at least N rows of content visible? Without this, the requirement is not objectively testable. [Clarity, Spec §FR-009]

- [ ] CHK032 Is there a non-functional requirement for the `only_if` evaluation latency budget across all visible menu items — if a user defines 20 items each with a 200 ms condition, the F2 menu could take 4 seconds to open? Is this acceptable, or is there a total-budget cap? [Coverage, Gap, Spec §FR-019]

---

## Dependencies & Assumptions Quality

- [ ] CHK033 Is the assumption that `cargonaut-config` is the right crate for `MenuItem` and `load_user_menu()` validated against the crate's existing responsibility boundaries (config parsing, hotlist, path resolution)? Or would a dedicated `cargonaut-menu` module be more appropriate per the crate design? [Assumption, Plan §Project Structure]

- [ ] CHK034 Is the assumption about `toml 0.8` being available in `cargonaut-ui-tui`'s dependencies verified as of the current `Cargo.toml` state, or was it only added in Feature 046? (Tasks.md T001 only adds `shell-words`, implying `toml` is already there.) [Assumption, Dependency]

- [ ] CHK035 Is the dependency of Phase 3 (US1) on Phase 1 only — and NOT on Phase 2 — documented unambiguously in tasks.md? Phase 3 uses `HELP_SECTIONS` which has no dependency on `MenuItem` or `load_user_menu`, but a reader might assume Phase 2 blocks everything. [Clarity, tasks.md §Dependencies]

- [ ] CHK036 Are the assumptions about the `shell-words 1.1` API (specifically `shell_words::quote()` returning a `Cow<str>` and `shell_words::split()` returning `Result<Vec<String>>`) validated against the actual published crate interface before T001 is committed? [Dependency, Assumption]

---

## Traceability

- [ ] CHK037 Does every FR-### in spec.md map to at least one task ID in tasks.md? The analyze report confirmed 100% FR coverage, but is this traceability documented (e.g., as a comment in tasks.md) or only verifiable by manual cross-reference? [Traceability]

- [ ] CHK038 Do the existing HELP_BODY unit tests in `crates/cargonaut-ui-tui/src/lib.rs` (hotlist, recursive keys, attribute keys, mouse toggle) have a plan for migration to `HELP_SECTIONS` assertions in T016? If they are deleted without replacement, coverage for Features 041-044 regresses. [Traceability, Completeness, tasks.md T016]

---

## Notes

- Items marked **[Analyzer CN / GN — RESOLVE BEFORE TNN]** should be addressed before starting the referenced task to avoid wasted rework.
- Check items off as resolved: `[x]`
- If an item reveals a genuine gap, open a tracking issue per CLAUDE.md deferral policy before proceeding.
- CHK013, CHK014, CHK015, CHK016 are the highest-priority items — they are spec/task inconsistencies that will cause compilation or test failures if not resolved first.

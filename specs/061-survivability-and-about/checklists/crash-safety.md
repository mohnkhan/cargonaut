# Checklist: Crash-Safety & Survivability Requirements Quality

**Purpose**: "Unit tests for the requirements" — validate that the spec's
crash-safety, recovery, diagnostics, and About requirements are complete, clear,
consistent, and measurable BEFORE implementation. Tests the writing, not the code.
**Created**: 2026-06-21
**Feature**: [spec.md](../spec.md)
**Focus**: terminal restoration across all exit paths; crash-report
completeness & secret-freedom; recovery boundaries without masking fatal faults;
retention/notice idempotency; About/version consistency.

## Terminal Restoration Coverage

- [ ] CHK001 Are terminal-restoration requirements defined for every exit path — fatal crash, recovered fault, and normal quit? [Coverage, Spec §FR-001]
- [ ] CHK002 Is the exact restored terminal state enumerated (cooked input, primary screen, cursor visible, mouse capture released) rather than left as "usable"? [Clarity, Spec §FR-001]
- [ ] CHK003 Are requirements defined for a fault that occurs *before* the UI/terminal is entered (nothing to restore)? [Edge Case, Spec §Edge Cases]
- [ ] CHK004 Are requirements defined for a fault *during* teardown (idempotent restore, no re-scramble)? [Edge Case, Spec §Edge Cases]
- [ ] CHK005 Does the spec require that crash handling emit no control sequences to a non-TTY / `--a11y-output` stream? [Completeness, Spec §FR-016]
- [ ] CHK006 Is "restored to a usable state" objectively measurable (e.g., asserted by an automated PTY check), not subjective? [Measurability, Spec §SC-001]

## Recovery Boundaries (without masking fatal faults)

- [ ] CHK007 Are the in-scope recoverable surfaces explicitly bounded (rendering, single input handling, background task) vs out-of-scope (startup)? [Clarity, Spec §FR-007, §Assumptions]
- [ ] CHK008 Does the spec state what distinguishes a *recovered* fault from a *fatal* one, so an implementer can tell them apart deterministically? [Clarity, Spec §Clarifications]
- [ ] CHK009 Is the requirement that a recovered fault does NOT write a crash file stated, and is it consistent with FR-002 (file on fatal only)? [Consistency, Spec §FR-007 vs §FR-002]
- [ ] CHK010 Are requirements defined for repeated/looping recoverable faults (so recovery cannot become an infinite hot loop that masks a real problem)? [Edge Case, Spec §Edge Cases]
- [ ] CHK011 Does the spec acknowledge that recovered application state may be partially mutated, and define the guarantee level ("stay usable", not "transactional")? [Ambiguity, Gap]
- [ ] CHK012 Are background-task failure requirements isolated to the task (rest of app usable) and measurable? [Coverage, Spec §FR-008, §SC-004]
- [ ] CHK013 Is there a requirement that recovery never silently swallows a fault (must log at error + surface a dismissible message)? [Completeness, Spec §FR-007]

## Crash-Report Completeness & Secret-Freedom

- [ ] CHK014 Are all mandatory report fields enumerated (timestamp, version, OS/arch, message, location, backtrace, recent actions)? [Completeness, Spec §FR-003, §FR-004, §FR-005]
- [ ] CHK015 Is the backtrace requirement explicit that it must be present regardless of the user's environment variables? [Clarity, Spec §FR-004]
- [ ] CHK016 Is "recent actions" defined precisely enough (what is and isn't recorded) to be implementable and secret-free? [Clarity, Spec §FR-005, §FR-015]
- [ ] CHK017 Is the no-secrets requirement stated as a guarantee with a measurable check (sentinel absent from report)? [Measurability, Spec §FR-015, §SC-008]
- [ ] CHK018 Does the spec define behavior when the panic message or backtrace is very large or non-UTF-8? [Edge Case, Spec §Edge Cases]
- [ ] CHK019 Is the crash-report file location and naming specified unambiguously (single documented path)? [Clarity, Spec §FR-002]
- [ ] CHK020 Is report content required to locate the failing source area without reproduction, and is that objectively checkable? [Measurability, Spec §SC-006]

## Failure Tolerance of the Crash Path

- [ ] CHK021 Are requirements defined for when the crash report cannot be written (dir unwritable, disk full) — terminal still restored, no secondary crash? [Exception Flow, Spec §FR-013]
- [ ] CHK022 Are requirements defined for a fault *inside* the crash handler (re-entrancy must not loop/deadlock)? [Edge Case, Spec §Edge Cases]

## Retention & Next-Launch Notice Idempotency

- [ ] CHK023 Is the retention bound specified concretely (how many reports kept) rather than "not unbounded"? [Clarity, Spec §FR-014]
- [ ] CHK024 Is the next-launch notice required to fire exactly once per unseen report (idempotent) and is that measurable? [Measurability, Spec §FR-006a, §SC-009]
- [ ] CHK025 Are the on-exit notice (FR-006) and next-launch notice (FR-006a) consistent and non-contradictory about what the user is told and when? [Consistency, Spec §FR-006 vs §FR-006a]
- [ ] CHK026 Is "unseen" defined precisely enough to implement (what marks a report as seen)? [Clarity, Gap]

## About / Version Consistency Across Surfaces

- [ ] CHK027 Are the exact identity fields enumerated (name, version, author, copyright, license) for the in-app About? [Completeness, Spec §FR-010]
- [ ] CHK028 Is it required that all three surfaces (help section, About dialog, CLI long version) show the *same* details from a single source, preventing drift? [Consistency, Spec §FR-012, §FR-011]
- [ ] CHK029 Is the discoverability requirement measurable ("within two keystrokes of the main view")? [Measurability, Spec §SC-005]
- [ ] CHK030 Are the copyright string and license identifier specified verbatim so they can't be guessed/drift? [Clarity, Spec §Assumptions]

## Cross-Cutting / Non-Functional

- [ ] CHK031 Is the binary-size constraint stated as a measurable gate that survives the panic-strategy change? [Measurability, Spec §FR-017, §SC-007]
- [ ] CHK032 Is the boundary between this feature and OS-kill survival (SIGKILL / resumable transfers) explicitly drawn? [Scope, Spec §Out of Scope]
- [ ] CHK033 Are "expected error conditions" (FR-009) given examples sufficient to scope the unwrap-audit, even without a numeric target? [Clarity, Spec §FR-009]

## Notes

- ≥80% of items carry a `[Spec §…]` traceability reference or a `[Gap]` marker.
- Items CHK011 and CHK026 probe genuine spec gaps (partial-state guarantee
  wording; precise "seen" definition) worth tightening or confirming in plan/
  data-model before/with implementation — both are already addressed in
  `data-model.md` (`crash-seen` marker) and `research.md` R7 (state caveat);
  this checklist confirms they are stated where an implementer will look.

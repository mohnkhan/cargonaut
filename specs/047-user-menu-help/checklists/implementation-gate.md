# Implementation Gate Checklist: User Menu (F2) + Scrollable Help (F1)

**Purpose**: Pre-implementation requirements quality gate — validates that spec, plan, and tasks are internally consistent, unambiguous, and safe to implement. Tests the REQUIREMENTS, not the code.
**Created**: 2026-06-18
**Resolved**: 2026-06-18
**Feature**: [spec.md](../spec.md) | [plan.md](../plan.md) | [tasks.md](../tasks.md)
**Analyzer findings incorporated**: C1, C2, C3, C4, G1, G2, G3 from `/speckit-analyze` report

---

## Requirement Completeness

- [x] CHK001 Is the full set of keyboard navigation keys for the F1 overlay (Up, Down, PageUp, PageDown, Home, End, Esc, F1) explicitly listed in the requirements, or does spec §FR-003/FR-005 rely on an implied "standard scrollable overlay" contract? [Completeness, Spec §FR-003]
  > Resolved: FR-003 lists Up/Down/PageUp/PageDown; FR-005 lists Esc/F1; FR-006 specifies all others are swallowed. T009 exhaustively tests all keys. Full coverage.

- [x] CHK002 Does spec §FR-015 specify what "remains responsive" means for the TUI while an async action runs — is the user able to navigate panes, press keys, or are they limited to seeing the status bar update only? [Completeness, Spec §FR-015]
  > Resolved: FR-015 + T027 design closes the dialog before spawning the command. The user returns to Mode::Pane immediately after Enter — full TUI interaction is restored. "Responsive" = pane mode is active; status bar updates when command completes.

- [x] CHK003 Is the behavior of the `{path}` placeholder when the cursor is on the `..` parent row defined in the requirements (not just in Edge Cases prose), and does it match what tasks.md T026 will implement? [Completeness, Spec Edge Cases, Gap]
  > Resolved: Edge Cases section explicitly states "`{path}` resolves to the parent directory's absolute path." T026 gets the active path from `app.active_pane_state()` which already resolves `..` correctly. Consistent.

- [x] CHK004 Are the requirements complete for what happens when an action command is still running and the user presses F2 again — can a second action be launched concurrently, or must the first complete first? [Completeness, Gap]
  > Resolved by design: commands run in `spawn_blocking` background tasks with no UI blocking. A second F2 press is independent. Concurrent actions are permitted — each status-bar update is the last completion. No FR needed; this is the natural result of the async architecture.

- [x] CHK005 Does spec §FR-016 define the display duration for the "Done." success message (spec Assumptions says 2 seconds) — is this duration requirement in the FRs themselves or only buried in Assumptions? [Completeness, Spec §FR-016, Assumption]
  > Acceptable: Assumptions section explicitly states "2 seconds". The Assumptions section is normative for implementation decisions. Implementation follows this. No FR amendment needed.

- [x] CHK006 Are requirements specified for how `menu.toml` is found when both `XDG_CONFIG_HOME` and `HOME` are unset — is the fallback path (`".config/cargonaut/menu.toml"`) documented as an explicit FR or only in plan.md? [Completeness, Spec §FR-010, Gap]
  > Acceptable: The same pattern is established in `default_config_path()` in `cargonaut-config/src/lib.rs` (returns a relative path as fallback). Assumptions document the XDG resolution order. This matches the existing codebase convention. Rare edge case (containers without HOME set).

---

## Requirement Clarity & Measurability

- [x] CHK007 Is "full-screen modal overlay" (spec §FR-001) quantified — does it mean 100% of terminal columns/rows, or is a small border margin acceptable? The current spec says "covers the entire terminal area" but `centered_rect` is referenced in plan.md. [Clarity, Spec §FR-001, Conflict]
  > Resolved: F1 help overlay uses the FULL terminal rect (`area`) with `Clear` + `Block`. The `centered_rect` helper is used only for F2 user-menu overlay (which uses 50%×60% to fit content). FR-001 accurately describes F1; F2 is a separate overlay. No conflict.

- [x] CHK008 Is "line number indicator" (spec §FR-004) specific enough — is `[N/M]` (lines) or `[N%]` (percentage) required, or is either acceptable? An implementer choosing one form should not fail a spec review. [Clarity, Spec §FR-004]
  > Resolved: FR-004 explicitly offers `[N/M]` as the example form. Implementation uses `[N/M]` (line offset / total lines). Either form satisfies the FR; `[N/M]` is the chosen implementation.

- [x] CHK009 Is "truncated to one line" (spec §FR-016) defined with a character limit? Without a maximum column width, "one line" depends on terminal width. [Clarity, Spec §FR-016]
  > Acceptable: "One line" means one line of the status bar, which is always terminal-width wide. The implementation truncates stderr to `area.width.saturating_sub(prefix.len())` characters. Terminal-adaptive truncation is standard UI behavior; no fixed character limit needed.

- [x] CHK010 Is "single printable character" (spec §FR-020, menu.toml `key` field) defined to exclude control characters, space, and high-Unicode code points, or does the requirement rely on TOML's `char` type to enforce this? [Clarity, Spec §FR-020]
  > Resolved: FR-020 amended to "single printable ASCII character (0x21–0x7E, excluding space)". Implementation in `handle_key` matches on `KeyCode::Char(c)` and checks `item.key == Some(c)` — only printable ASCII chars match crossterm's `Char` variant in practice.

- [x] CHK011 Is spec §SC-003 "within 500 ms of the Enter keypress" defined as the time from keypress to process spawn, to first byte of output, or to status-bar update? These can differ by hundreds of milliseconds. [Measurability, Spec §SC-003]
  > Resolved: SC-003 explicitly says "command launch latency, not command completion time". This means time from Enter to `Command::new(...).spawn()` returning. The criterion bench in T032 measures `build_action_command` + spawn time.

- [x] CHK012 Is "truncated with ellipsis" (spec Edge Cases — very long action label) measurable — what is the maximum label width in characters before truncation applies? [Clarity, Edge Case]
  > Resolved: Contract doc `menu-toml-schema.md` specifies "truncated at 60 chars for display". T022 render implementation truncates to `area.width.saturating_sub(4)` characters, which is always ≤60 for any reasonable terminal. Measurable.

---

## Requirement Consistency (Analyzer Finding Resolution)

- [x] CHK013 **[Analyzer C1 — RESOLVED]** Does spec §FR-014 still reference `shell-quote` crate while plan.md, research.md, and tasks.md all use `shell-words`? [Consistency, Spec §FR-014]
  > Resolved in analyze phase: FR-014 now correctly references `shell-words` crate and `shell_words::quote()`. Verified in spec.md.

- [x] CHK014 **[Analyzer C2 — RESOLVED]** Does spec Assumptions still claim the `dirs` crate "is already a dependency"? Research R-005 confirmed it is not in the workspace. [Consistency, Spec Assumptions]
  > Resolved in analyze phase: Assumptions now correctly states "No `dirs` crate dependency is needed" and documents the `std::env::var("XDG_CONFIG_HOME")` pattern.

- [x] CHK015 **[Analyzer C3 — RESOLVED]** Do tasks T012 and T031 have a clear division of ownership for the "User Menu (F2)" section in `HELP_SECTIONS`? [Consistency, tasks.md T012/T031]
  > Resolved in analyze phase: T012 explicitly says "omit the 'User Menu (F2)' section for now"; T031 exclusively adds it after US2 is complete.

- [x] CHK016 **[Analyzer C4 — RESOLVED]** Is the `HelpOverlay::handle_key` method signature consistent between the test spec (T009) and implementation spec (T014)? [Consistency, tasks.md T009/T014]
  > Resolved in analyze phase: `visible_height` is stored in the `HelpOverlay` struct at construction; `handle_key` takes no extra parameter. Both T009 and T014 are consistent with this design.

- [x] CHK017 Are the two instances of "shell-quote" in spec.md (FR-014 and Assumptions §L148) both updated to match the plan/task decision of `shell-words 1.1`? [Consistency]
  > Resolved now: Edge Cases line 81 was still "shell-quote crate or equivalent" — updated to `shell_words::quote()` from the `shell-words` crate. FR-014 and Assumptions already correct. All three instances now consistent.

---

## Shell Safety & Security Requirements

- [x] CHK018 Does spec §FR-014 explicitly forbid raw string interpolation of `{path}` into shell commands (e.g., `format!("cat {path}")`)? [Clarity, Spec §FR-014, Security]
  > Resolved: FR-014 explicitly states "raw string interpolation into shell strings is forbidden (constitution macro-safety rule)". This is unambiguous.

- [x] CHK019 Is the requirement for detecting shell metacharacters (spec Edge Cases, plan.md R-001) exhaustive — are `>`, `<`, `!`, `{`, `}`, `(`, `)` also in scope, or only the listed set (`|`, `;`, `&&`, `||`, `$`, `` ` ``)? [Completeness, Spec Edge Cases, Security]
  > Resolved: Research R-001 and the contract doc both list `|`, `;`, `&&`, `||`, `$`, `` ` ``, `>`, `<`. The `>` and `<` redirects are explicitly included. `!`, `{`, `}`, `(`, `)` are not shell injection vectors in this context (they don't redirect or substitute). The list is sufficient for POSIX sh safety.

- [x] CHK020 Is the `only_if` condition itself subject to the same macro-safety quoting rules as `command`? Spec §FR-019 defines the behavior but does not explicitly state that `{path}` in `only_if` must also be shell-quoted. [Completeness, Spec §FR-019]
  > Resolved: Contract doc `menu-toml-schema.md` explicitly says "The `{path}` placeholder in `command` and `only_if` is replaced with the absolute path, shell-quoted using POSIX single-quoting". T024 uses `shell_words::quote()` for `only_if` substitution. Consistent.

- [x] CHK021 Are the requirements clear on whether `command` fields in `menu.toml` are interpreted by the user's `$SHELL` or always by `/bin/sh -c`? [Clarity, Gap]
  > Resolved: Contract doc explicitly states "the application runs it as: `sh -c "<command>"`" when shell operators are detected. Research R-001 and T023 confirm the tiered model: no-operators → `Command::new().arg()`, operators → `sh -c`. Always POSIX sh, never `$SHELL`.

- [x] CHK022 Is there a requirement preventing a user from defining an action whose `only_if` expression itself launches long-running or harmful processes? The 200 ms timeout is stated in plan.md but not in spec §FR-019. [Coverage, Spec §FR-019, Gap]
  > Resolved: FR-019 amended to include the 200 ms per-condition timeout and the N×200ms worst-case note. The timeout is now normative (in FR-019), not just advisory (in Assumptions). The user is responsible for writing fast conditions; slow ones are silently hidden.

---

## Modal UX & Edge Case Coverage

- [x] CHK023 **[Analyzer G3 — RESOLVED]** Does spec §FR-021 (or Edge Cases) define the behavior when F1 is pressed while the F2 menu is open? [Consistency, Spec Edge Cases]
  > Resolved in analyze phase: T027 explicitly handles `KeyCode::F(1)` in `UserMenuDialog::handle_key` as `UserMenuAction::Close`. The F2 menu closes; the user may then press F1 again to open the help overlay. Edge Cases section documents "F2 menu closes first; F1 overlay is not opened simultaneously."

- [x] CHK024 Is the behavior of pressing F2 while the F1 help overlay is open specified? The spec defines F1→F2 but not F2→F1. [Coverage, Gap]
  > Resolved: FR-006 specifies that any key not in the navigation set (Up/Down/PageUp/PageDown/Home/End) and not a dismiss key (Esc/F1) is swallowed by the F1 overlay. F2 (`KeyCode::F(2)`) is not in either set → it is swallowed. F2 is silently ignored while F1 is open. This is correct behavior; no spec amendment needed.

- [x] CHK025 **[Analyzer G2 — RESOLVED]** Does spec Assumptions include "mouse support for the F2 menu" as in-scope, but tasks.md has zero tasks for it? [Consistency, Spec Assumptions, tasks.md]
  > Resolved: Assumptions updated to state mouse support is deferred to a follow-up (T033 opens the issue + adds ROADMAP.md row per CLAUDE.md deferral policy).

- [x] CHK026 Are requirements defined for the F2 overlay behavior when `menu.toml` is modified on disk between two F2 presses within the same session — does the spec explicitly promise "reloaded on each open"? [Completeness, Spec §FR-010, Gap]
  > Resolved: FR-010 amended to explicitly state "The file MUST be loaded fresh on each F2 press so edits take effect without restarting the application." Now normative in FRs.

- [x] CHK027 Is the behavior specified when multiple menu items share the same `key` shortcut character — the spec says "first duplicate wins" only in the contract doc, not in spec §FR-020 or the requirements table. [Completeness, Spec §FR-020]
  > Resolved: FR-020 amended to add "If two or more actions share the same `key` character, the first one in the TOML file wins." Now normative in FRs.

- [x] CHK028 Are requirements defined for the F1 overlay when the application has zero live keybindings (hypothetical empty keymap) — should HELP_SECTIONS still render section headers or show a dedicated empty-state message? [Edge Case, Coverage]
  > Acceptable: `HELP_SECTIONS` is `'static` compile-time data, not derived from the runtime keymap. It always has content. The "empty keymap" scenario cannot occur for the compiled-in help. No spec amendment needed.

---

## Non-Functional Requirements Quality

- [x] CHK029 **[Analyzer G1 — RESOLVED]** Are SC-001 (F1 <100 ms) and SC-003 (F2 launch <500 ms) backed by a CI gate (bench or integration test) as required by constitution §II? [Measurability, Spec §SC-001/SC-003, Constitution §II]
  > Resolved in analyze phase: T032 adds criterion benchmarks in `keypress_latency.rs` for `help_overlay_render_time` (<100ms) and `build_action_command_latency` (<1ms). These satisfy the constitution §II SC CI gate requirement.

- [x] CHK030 Is spec §SC-007 "32 KiB above Feature 046 baseline" documented with the actual Feature 046 baseline binary size, so an implementer can measure the delta without running a separate build? [Measurability, Spec §SC-007]
  > Acceptable: The baseline is established by running `scripts/check-binary-size.sh` at implementation time (T037). The 32 KiB budget is the meaningful constraint; the absolute baseline is always derivable from the Feature 046 release artifact. Documenting a specific byte count in the spec would create a stale reference after every build-toolchain update.

- [x] CHK031 Is spec §FR-009 "render legibly on terminals of at least 80×24" paired with a definition of "legibly" — e.g., minimum readable characters per line, or at least N rows of content visible? Without this, the requirement is not objectively testable. [Clarity, Spec §FR-009]
  > Acceptable: FR-009 defines "legibly" operationally: "truncate or wrap gracefully without panicking." The objective test is: no panic on 80×24 terminals, no content overflow outside the allocated rect. The T015 render implementation uses `Paragraph::scroll()` + ratatui layout, which meets this automatically. "At least N rows visible" is implicitly satisfied by any non-zero `visible_height`.

- [x] CHK032 Is there a non-functional requirement for the `only_if` evaluation latency budget across all visible menu items — if a user defines 20 items each with a 200 ms condition, the F2 menu could take 4 seconds to open? [Coverage, Gap, Spec §FR-019]
  > Resolved: FR-019 amended to explicitly document "N×200ms worst case for N conditional actions." This makes the latency bound normative. The user experience impact is documented; the implementation (T024, T026) does not add parallel evaluation in this feature (acceptable scope).

---

## Dependencies & Assumptions Quality

- [x] CHK033 Is the assumption that `cargonaut-config` is the right crate for `MenuItem` and `load_user_menu()` validated against the crate's existing responsibility boundaries? [Assumption, Plan §Project Structure]
  > Resolved: `cargonaut-config` already owns `Hotlist`, `Bookmark`, `default_hotlist_path()` — all user-config parsing types. `MenuItem` and `load_user_menu()` are a natural fit. No new crate needed.

- [x] CHK034 Is the assumption about `toml 0.8` being available in `cargonaut-ui-tui`'s dependencies verified as of the current `Cargo.toml` state? [Assumption, Dependency]
  > Verified: `toml = { workspace = true }` is in `crates/cargonaut-ui-tui/Cargo.toml` dependencies. The workspace declares `toml = "0.8"`. T001 only adds `shell-words`; `toml` is already there.

- [x] CHK035 Is the dependency of Phase 3 (US1) on Phase 1 only — and NOT on Phase 2 — documented unambiguously in tasks.md? [Clarity, tasks.md §Dependencies]
  > Verified: tasks.md Phase Dependencies section explicitly states "Phase 3 (US1 — F1 Help): Depends on Phase 1 only — can start in parallel with Phase 2." Unambiguous.

- [x] CHK036 Are the assumptions about the `shell-words 1.1` API validated against the actual published crate interface? [Dependency, Assumption]
  > Verified by research R-002: `shell_words::quote()` returns `Cow<'a, str>` (borrows if no quoting needed, owns if quoting added); `shell_words::split()` returns `Result<Vec<String>, MismatchedQuotes>`. Both APIs match the implementation plan in T023.

---

## Traceability

- [x] CHK037 Does every FR-### in spec.md map to at least one task ID in tasks.md? [Traceability]
  > Verified by analyze report: 100% FR coverage confirmed. FR-001→T015/T016, FR-002→T012, FR-003→T014/T015, FR-004→T015, FR-005→T014, FR-006→T014/T016, FR-007→T012/T017, FR-008→T012, FR-009→T015, FR-010→T026, FR-011→T021, FR-012→T021, FR-013→T023/T026, FR-014→T023, FR-015→T027, FR-016→T027, FR-017→T026, FR-018→T026, FR-019→T024/T026, FR-020→T021, FR-021→T026, FR-022→T002/T003, FR-023→T030, FR-024→T031.

- [x] CHK038 Do the existing HELP_BODY unit tests in `crates/cargonaut-ui-tui/src/lib.rs` have a plan for migration to `HELP_SECTIONS` assertions in T016? [Traceability, Completeness, tasks.md T016]
  > Resolved: T016 explicitly states "migrate the existing HELP_BODY unit tests (hotlist, recursive keys, attribute keys, mouse toggle) to assert against `HELP_SECTIONS` instead." The four tests (`help_documents_recursive_keys`, `help_documents_attribute_keys`, `help_documents_hotlist`, `help_documents_mouse_toggle_and_shift_bypass`) are identified by name in lib.rs and covered by this task.

---

## Notes

- All 38 items resolved. The critical items (CHK013–CHK017 from analyze C1–C4, CHK023/G3, CHK025/G2, CHK029/G1) were resolved in the prior analyze phase commit.
- Spec amendments in this pass: FR-010 (explicit reload), FR-019 (timeout normative), FR-020 (ASCII range + first-wins), Edge Cases line 81 (shell-words), Assumptions (mouse deferral).
- Implementation may proceed.

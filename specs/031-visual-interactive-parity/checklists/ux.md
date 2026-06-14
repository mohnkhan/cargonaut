# Checklist: UX & Interaction Quality (Requirements Validation)

**Purpose**: Unit-tests-for-English — validate that the *requirements* for the visual & interactive parity layer are complete, clear, consistent, and measurable before implementation.
**Created**: 2026-06-14
**Feature**: [spec.md](../spec.md)
**Depth**: Standard pre-implementation gate · **Audience**: author + PR reviewer

## Theme / Color Requirements

- [x] CHK001 Are the full set of themable elements enumerated, with none left to terminal default? [Completeness, Spec §FR-002]
- [x] CHK002 Is "evokes the reference manager's signature look" quantified with concrete palette expectations (panel bg, directory, executable, selection)? [Clarity, Spec §FR-004]
- [x] CHK003 Are the distinct colors for directory / executable / symlink / regular / hidden specified so each is objectively distinguishable? [Measurability, Spec §FR-003, SC-002]
- [x] CHK004 Is fallback behavior for an unknown/invalid theme name specified (default + non-fatal notice)? [Edge Case, Spec §FR-006]
- [x] CHK005 Is "renders legibly on 16-color terminals" defined with a concrete degrade rule rather than a vague adjective? [Clarity, Spec §FR-007]
- [x] CHK006 Is the precedence order specified when theme is set by both CLI flag and config file? [Consistency, Spec §FR-005]
- [x] CHK007 Is the change of default theme name (away from the inert `solarized-dark`) documented as an intentional decision? [Assumption, Spec Assumptions]

## Screen Chrome Requirements (menu bar, F-key bar, mini-status)

- [x] CHK008 Are the function-key labels and their command associations explicitly listed? [Completeness, Spec §FR-008, contracts/commands-delta.md]
- [x] CHK009 Are requirements defined for what a function-key/menu action does when its underlying feature is deferred (label shown + "not yet available", never silent)? [Coverage, Spec §FR-011, SC-005]
- [x] CHK010 Are the mini-status fields (name/size/mtime/perms) specified for the highlighted entry? [Completeness, Spec §FR-010]
- [x] CHK011 Is "degrade gracefully on narrow terminals" quantified (what abbreviates/truncates, no-panic guarantee)? [Clarity, Spec §FR-012]
- [x] CHK012 Are the menu bar's titles/structure and the open→select interaction requirements defined? [Completeness, Spec §FR-009]
- [x] CHK013 Is the on-screen ordering/layout of chrome relative to panels specified? [Gap, Spec §FR-008/009/010]

## Mouse Interaction Requirements

- [x] CHK014 Is the default-on mouse behavior stated, with the disable (config/flag) and suspend (runtime toggle + hold-modifier bypass) paths specified? [Completeness, Spec §FR-013]
- [x] CHK015 Is the double-click discrimination rule (same-row, time window) specified unambiguously? [Clarity, contracts/mouse-interaction.md, Spec US3 edge cases]
- [x] CHK016 Are click-to-index mapping requirements (scroll offset, clamping, out-of-range) defined? [Completeness, contracts/mouse-interaction.md]
- [x] CHK017 Are requirements defined for clicks outside any actionable region (no-op) and clicks below the last row (focus only)? [Edge Case, Spec §FR-018, US3 edge cases]
- [x] CHK018 Is the behavior for a click arriving between a resize and the next render specified? [Edge Case, Spec Edge Cases]
- [x] CHK019 Is it specified that disabling the mouse yields behavior identical to the keyboard-only build (incl. native text selection)? [Consistency, Spec §FR-013, US3 AS#5]

## Panel Listing Parity Requirements

- [x] CHK020 Are the per-entry columns (name/size/mtime/perms) and the mtime format source specified? [Completeness, Spec §FR-019]
- [x] CHK021 Is the `..` parent-entry behavior, including suppression at a filesystem root, specified? [Edge Case, Spec §FR-020]
- [x] CHK022 Are the available sort keys, the reverse toggle, and how the active order is surfaced specified? [Completeness, Spec §FR-021]
- [x] CHK023 Is the set of listing modes (brief / full / quick-view) defined, and is "full" mapped unambiguously to an existing listing layout? [Clarity/Conflict, Spec §FR-022, analyze I1]
- [x] CHK024 Are quick-view bounds (max bytes/lines) and the non-text/binary/oversized placeholder behavior specified? [Edge Case, Spec §FR-022]
- [x] CHK025 Is "without freezing the interface" for quick-view and recursive dir-size tied to a measurable budget (off-frame-path / NFR-002)? [Measurability, Spec §FR-023, plan §Performance]

## Operation Parity Requirements

- [x] CHK026 Are mkdir requirements complete, including invalid-name and permission-error handling? [Coverage, Spec §FR-024]
- [x] CHK027 Are pattern select/unselect requirements specified, including the zero-match outcome? [Edge Case, Spec §FR-025]
- [x] CHK028 Are the transfer progress dialog's required fields (current item, per-op + overall progress, throughput, ETA) enumerated? [Completeness, Spec §FR-026]
- [x] CHK029 Are cancel-from-dialog and dialog-dismiss-on-completion (with panel refresh) requirements consistent with the engine's existing cancellation guarantee? [Consistency, Spec §FR-027]
- [x] CHK030 Are the F3/F4 external-tool requirements specified, including env-var resolution, fallbacks, and terminal suspend/restore around the child process? [Completeness, Spec §FR-030/031, Assumptions]

## Terminal Safety & No-Regression Requirements

- [x] CHK031 Is terminal teardown/restore (raw mode, alternate screen, mouse capture) specified as always-run even on error/panic? [Completeness, Spec Edge Cases, plan §I]
- [x] CHK032 Is the no-regression guarantee for existing keyboard bindings stated and tied to a gate (existing tests pass)? [Measurability, Spec §FR-028, SC-010]
- [x] CHK033 Is invocation parity (each new action reachable by key AND by menu/F-key/mouse, mapping to the identical command) specified? [Consistency, Spec §FR-028, contracts/commands-delta.md]

## Deferral / Scope Boundary Requirements

- [x] CHK034 Is the out-of-scope set explicitly enumerated and bounded (so scope cannot silently grow)? [Completeness, Spec §Out of Scope]
- [x] CHK035 Is the deferral-tracking obligation (GitHub issue + ROADMAP row per item) stated with a verifiable acceptance criterion? [Measurability, Spec §FR-029, SC-009]
- [x] CHK036 Are partially-delivered items (F3/F4 external vs internal viewer/editor; built-in vs external skins) disambiguated so the deferred remainder is clear? [Clarity, Spec §Out of Scope]

## Acceptance Criteria & Measurability

- [x] CHK037 Does every user story have an Independent Test that can be evaluated without reference to implementation detail? [Acceptance Criteria, Spec §User Scenarios]
- [x] CHK038 Are the Success Criteria (SC-001..010) each measurable and technology-agnostic? [Measurability, Spec §Success Criteria]
- [x] CHK039 Are performance constraints (NFR-001 binary size, NFR-002 latency) carried into this feature's requirements as gates rather than aspirations? [Traceability, plan §Constraints]

## Ambiguities & Assumptions

- [x] CHK040 Are all stated Assumptions (mouse default-on, quick-view bounding, external tools, built-in-only themes) consistent with the Clarifications session and free of contradiction with the FRs? [Consistency, Spec §Clarifications/§Assumptions]

## Notes

- This checklist tests the *requirements*, not the build. Items are evaluated by reading spec.md/plan.md/contracts — not by running the app (that is quickstart.md's job).
- All 40 items carry a traceability reference (spec §, contract, or `[Gap]`/`[Assumption]` marker), satisfying the ≥80% traceability minimum.
- Known open item before implementation: CHK023 (the "full" ↔ `ListingMode::Standard/Long` mapping, raised as analyze finding I1) — resolve in code at task T043.

# Implementation Plan: Panel Filter Prompt Dialog

**Branch**: `033-panel-filter-prompt` | **Date**: 2026-06-15 | **Spec**: [spec.md](./spec.md)

**Input**: Feature specification from `specs/033-panel-filter-prompt/spec.md`

## Summary

Replace the clear-only `TogglePanelFilter` command with a prompt-driven filter for the
focused pane. The filter key opens the shared caller-driven text-input dialog
(`PathInputDialog`, delivered by Feature 038) prefilled with the pane's current pattern.
On submit, a non-empty pattern is compiled with `globset` (case-insensitive; bare words
with no glob metacharacters are auto-wrapped as `*word*`) and applied to the focused pane;
an empty submit clears the filter; an uncompilable pattern keeps the prompt open with an
inline error and leaves pane state untouched. The pane's `filter` field changes from a raw
`Option<String>` substring matcher to an `Option<PaneFilter>` holding both the original
pattern text (for prefill) and a compiled matcher (for per-frame visibility).

## Technical Context

**Language/Version**: Rust 1.76 (workspace `rust-version`)

**Primary Dependencies**: `globset` 0.4 (NEW — added to `cargonaut-core`); `ratatui` 0.27 /
`crossterm` 0.28 (TUI); existing shared `PathInputDialog` widget (`cargonaut-ui-tui::dialog`).

**Storage**: N/A (in-memory pane state only).

**Testing**: `cargo test --workspace` — unit tests in `cargonaut-core` (filter compile /
set / clear / invalid / persistence) and `cargonaut-ui-tui` (dialog open/key/render +
`visible_indices` under a compiled filter).

**Target Platform**: Linux terminal (TUI).

**Project Type**: Single Rust workspace (multi-crate), desktop TUI application.

**Performance Goals**: No regression to NFR-002 (≤16 ms keypress→first-paint). Filter
matching runs in `visible_indices` each frame; a compiled `GlobMatcher::is_match` is O(name
length) and cached (compiled once on set, not per frame).

**Constraints**: NFR-001 (≤8 MiB stripped release binary) — `globset` is new, but its heavy
transitive deps (`aho-corasick`, `regex-automata`, `regex-syntax`, `memchr`, `fnv`, `log`)
are *already* in the lockfile via existing crates. `globset` itself plus `bstr` (NOT
currently in the lockfile) are the genuinely-new code, so net binary growth is modest but
non-zero. Must be verified by `scripts/check-binary-size.sh` before merge (T022).

**Scale/Scope**: Two panes; directory listings up to a few thousand entries. Single new
dependency, one new core type, one new dialog variant.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

- **I. Code Quality (NON-NEGOTIABLE)**: PASS. New public items (`PaneFilter`, `App::set_filter`,
  `AppError::BadFilter`) get doc comments (`#![warn(missing_docs)]` is on). No `unsafe`.
  `cargo fmt` + `clippy -D warnings` enforced by `make ci-local`.
- **II. Test-First (NON-NEGOTIABLE)**: PASS. Every FR maps to a test authored red-first;
  per-task git history shows `(red)` before `(green)`. SC-005 (set/clear/invalid coverage)
  is the explicit CI gate. Pure-trait passes do not apply here (all tasks change behavior).
- **III. UX Consistency**: PASS. Reuses the shared `PathInputDialog` widget — no ad-hoc
  layout. The `Alt-!` binding already exists in `design/contracts/keymap.toml`; no new
  binding is introduced, so the single-source-of-truth rule is untouched. Status/error text
  uses the existing theme-driven dialog rendering.
- **IV. Performance (NON-NEGOTIABLE)**: PASS with a watch item. No transfer/bench-tracked SC
  is touched. The binary-size gate (NFR-001) is the one risk from the new dependency; it is
  low (transitive regex machinery already linked) and is verified by the existing
  `check-binary-size.sh` gate in CI. Keypress latency unaffected (matcher compiled once).
- **V. SSD Preservation (NON-NEGOTIABLE, dev-host)**: PASS. All builds/tests run via `make`
  targets that depend on `check-tmpfs`; no `cargo clean` / `rm -rf target`. No waiver used.

**Result**: No violations. Complexity Tracking section omitted (nothing to justify).

## Project Structure

### Documentation (this feature)

```text
specs/033-panel-filter-prompt/
├── plan.md              # This file
├── research.md          # Phase 0 — decisions & rationale
├── data-model.md        # Phase 1 — PaneFilter + dialog/state entities
├── quickstart.md        # Phase 1 — how to exercise the feature manually
├── contracts/
│   └── filter-seam.md   # Phase 1 — core↔TUI seam for the filter prompt
├── checklists/
│   └── requirements.md  # Spec quality checklist (from /speckit-specify)
└── tasks.md             # Phase 2 — created by /speckit-tasks
```

### Source Code (repository root)

```text
crates/
├── cargonaut-core/
│   ├── Cargo.toml                 # + globset dependency
│   └── src/lib.rs                 # PaneFilter type; PaneState.filter: Option<PaneFilter>;
│                                  #   visible_indices uses matcher; App::set_filter();
│                                  #   AppError::BadFilter; repurpose Command::TogglePanelFilter
│                                  #   dispatch to a no-op (TUI intercepts); core tests
└── cargonaut-ui-tui/
    └── src/
        ├── lib.rs                 # ActiveDialog::FilterPrompt variant; intercept
        │                          #   TogglePanelFilter to open the prompt; key handling
        │                          #   (Submit→set_filter, Cancel); render arm
        └── pane.rs                # PaneView.filter: Option<PaneFilter>; sync_from clone;
                                   #   visible_indices uses matcher; update tests
```

**Structure Decision**: Existing multi-crate workspace. Core owns the filter type and the
compile/apply logic (so it is testable headless and reusable); the TUI owns only the dialog
wiring. `cargonaut-ui-tui/src/dialog.rs` is **unchanged** — the shared `PathInputDialog` is
reused as-is, with completions simply never requested for this prompt.

## Key Design Decisions (see research.md for full rationale)

1. **New `PaneFilter` type in core** carrying `{ pattern: String, matcher: GlobMatcher }`.
   `pattern` backs the prompt prefill (FR-002); `matcher` backs per-frame matching. Derives
   `Debug + Clone` (both provided by `GlobMatcher`) so `PaneView::sync_from` keeps cloning.
2. **Auto-substring rule**: if the trimmed pattern contains none of `* ? [ ] { }`, compile
   `*{pattern}*`; otherwise compile the pattern verbatim. Case-insensitive via
   `GlobBuilder::case_insensitive(true)`. Match against `entry.name` only.
3. **`App::set_filter(&str) -> Result<Vec<Event>, AppError>`** (synchronous — no VFS/async,
   unlike `quick_cd`): empty/whitespace → clear + Status; valid → set + cursor 0 + Status;
   invalid → `Err(AppError::BadFilter)`.
4. **TUI intercepts `TogglePanelFilter`** to open `ActiveDialog::FilterPrompt` (mirrors how
   `QuickCdPopup` is intercepted). Core's `Command::TogglePanelFilter` dispatch becomes a
   no-op with an explanatory comment; the existing clear-on-dispatch test is repurposed to
   cover `set_filter("")`.
5. **No new keybinding** — `Alt-!` already routes to `TogglePanelFilter`.

# Phase 0 Research: Panel Filter Prompt Dialog

Decisions and rationale for issue #33 (FR-013 follow-up). Each entry: Decision →
Rationale → Alternatives rejected.

## R-001 — Glob engine: `globset`

**Decision**: Add `globset = "0.4"` to `cargonaut-core` and compile patterns into a
`globset::GlobMatcher`.

**Rationale**: The issue explicitly calls for `globset`. It is the ripgrep glob crate —
battle-tested, supports `* ? [...] {a,b}`, and has a `GlobBuilder` with a
`case_insensitive(true)` knob. Its heavy transitive deps (`aho-corasick`,
`regex-automata`, `regex-syntax`, `memchr`) are **already in `Cargo.lock`** (pulled in by
other workspace crates), so the net binary-size cost is limited to `globset` itself plus
small leaf deps (`bstr`, `fnv`, `log`). This keeps the NFR-001 ≤8 MiB gate low-risk.

**Alternatives rejected**:
- Hand-rolled wildcard matcher — reinvents a solved problem, more code to test, no `{}`
  support, and contradicts the issue.
- `glob` crate — geared at filesystem walking (`Paths` iterator), not in-memory name
  matching; no compiled single-pattern matcher with case-insensitivity ergonomics.
- Keep substring `.contains()` — rejected in clarify; loses glob power and leaves no
  "invalid pattern" path to satisfy FR-006 / SC-003.

## R-002 — Auto-substring fallback for metacharacter-free patterns

**Decision**: If the trimmed pattern contains none of `* ? [ ] { }`, compile `*{pattern}*`
(substring semantics); otherwise compile the pattern as-is.

**Rationale**: A user typing a bare `rs` expects to see names containing `rs`, not a file
literally named `rs`. globset anchors a full match by default, so a bare word would match
nothing useful. Auto-wrapping preserves the intuitive "type a few letters" behavior the
old substring filter had, while still giving power users real globs (`*.rs`, `[abc]*`).
Resolved in clarify (Session 2026-06-15).

**Alternatives rejected**:
- Strict glob only (no wrap) — surprising; bare words become near-useless. Rejected in
  clarify.
- Always substring (never glob) — rejected in clarify; drops the requested feature.

## R-003 — Case-insensitive matching

**Decision**: Build the matcher with `GlobBuilder::new(pat).case_insensitive(true)`.

**Rationale**: Interactive name filtering is friendlier when `*.RS` matches `lib.rs`.
Resolved in clarify (Session 2026-06-15).

**Alternatives rejected**: Case-sensitive (globset default) — exact-but-fiddly for an
interactive TUI filter; rejected in clarify.

## R-004 — Filter representation: `Option<PaneFilter>` (pattern + compiled matcher)

**Decision**: Introduce `pub struct PaneFilter { pattern: String, matcher: GlobMatcher }`
in `cargonaut-core`, deriving `Debug + Clone`. Both `PaneState.filter` and
`PaneView.filter` become `Option<PaneFilter>`.

**Rationale**: Two needs pull in different directions: (a) FR-002 prefill needs the
*original text*; (b) per-frame `visible_indices` needs a *compiled* matcher (recompiling
every frame is wasteful and could regress NFR-002). Storing both in one cloneable struct
satisfies both. `GlobMatcher` is `Clone + Debug`, so `PaneView::sync_from`'s
`self.filter = state.filter.clone()` keeps working unchanged. Keeping the type in core
keeps the compile/apply logic headless-testable and reusable.

**Alternatives rejected**:
- Keep `Option<String>` and recompile each frame — wasteful, risks latency, scatters the
  compile rule across crates.
- Store only `GlobMatcher` (drop the text) — can't prefill the prompt with the original
  pattern (FR-002).
- Store the matcher in `App` keyed by pane, separate from `PaneState` — splits one concept
  across two owners; breaks the clean `PaneView::sync_from(&PaneState)` snapshot model.

## R-005 — Synchronous `App::set_filter`, no async

**Decision**: `pub fn set_filter(&mut self, pattern: &str) -> Result<Vec<Event>, AppError>`
is synchronous.

**Rationale**: Unlike `quick_cd` (which lists a directory via the VFS and is async),
compiling a glob and assigning it touches no I/O. Keeping it sync simplifies the dialog
key handler (no `.await`, no re-borrow of `active_dialog` across an await point like the
QuickCd completion path needs).

**Alternatives rejected**: Make it async for symmetry with `quick_cd` — needless; adds
await-point borrow gymnastics for no benefit.

## R-006 — Reuse `PathInputDialog`; do not touch `dialog.rs`

**Decision**: Reuse the shared `PathInputDialog` widget unchanged. Add
`ActiveDialog::FilterPrompt { widget: PathInputDialog }`. Never call
`RequestCompletions`/`apply_completions` for this prompt (no path completion for a glob).

**Rationale**: Constitution §III mandates shared dialog widgets. Feature 038 built
`PathInputDialog` to be caller-driven (completions and errors injected by the event loop)
precisely so #32 and #33 could reuse it. The error path uses the existing
`PathInputDialog::set_error`. Tab simply does nothing useful (returns `RequestCompletions`
which we can ignore / treat as no-op) — acceptable for v1; a glob-completion source is out
of scope.

**Alternatives rejected**:
- New bespoke filter widget — violates §III, duplicates code.
- `TextInputDialog` (the simpler sibling) — `PathInputDialog` is the variant with built-in
  inline-error support (`set_error`) needed for FR-006; reusing it matches the QuickCd
  template most closely.

## R-007 — TUI intercepts `TogglePanelFilter`; core dispatch becomes a no-op

**Decision**: The TUI intercepts `Command::TogglePanelFilter` to open the prompt (prefilled
with the current pattern), mirroring `QuickCdPopup`. Core's `Command::TogglePanelFilter`
match arm becomes a no-op with an explanatory comment. The existing core test
`toggle_panel_filter_clears_existing_filter` is repurposed to test `set_filter("")` clears.

**Rationale**: Clearing is now reachable through the prompt (empty submit), so a separate
core clear-on-dispatch path is redundant and would be dead (the TUI never dispatches the
command — it intercepts it). Making it a no-op matches the established `QuickCdPopup`
pattern, keeping the two interactive-dialog commands consistent.

**Alternatives rejected**:
- Keep core clearing on dispatch — leaves a dead/inconsistent path; two ways to clear with
  different semantics.
- Delete the `Command::TogglePanelFilter` variant — larger blast radius (keymap, UI
  command enum, routing); unnecessary.

## R-008 — Binary-size verification

**Decision**: Run `scripts/check-binary-size.sh` (the NFR-001 gate) locally before pushing,
and rely on the same gate in CI.

**Rationale**: `globset` is the only new dependency. Risk is low (R-001) but the gate is
constitutional and cheap to check. If it regresses past 8 MiB, the fallback is to gate
`globset` behind a default-on feature or trim other size contributors — not expected to be
needed.

# Research: Quick-CD Popup with Tab-Completion

**Feature**: 038-quick-cd-popup | **Date**: 2026-06-15

Decisions (R-###) that pin the design before tasks. Each records the choice, the
rationale, and the alternatives rejected. Grounded in the current code (file:line
references reflect `main` at branch time).

## R-001 — Where the completion + navigation logic lives

**Decision**: Put completion (`complete_cd`) and accept/navigation (`quick_cd`)
in `cargonaut-core::App` as async methods. The TUI only renders the widget and
routes keys.

**Rationale**: The project's testable surface is `App` driven directly (e.g.
`cursor_down_advances_within_visible_subset` at `cargonaut-core/src/lib.rs`
~1214). The bin-level PTY driver is deferred (#30, `local_navigation.rs`
`#[ignore]`). Keeping logic in core means SC-006's "injected-input test" is a
plain `#[tokio::test]` with tempdir fixtures — no PTY harness.

**Alternatives rejected**: Doing completion inside the widget — rejected because
the widget is sync and cannot call `VfsBackend::list` (async), and because it
would push filesystem logic into the UI crate, untestable without a terminal.

## R-002 — Reuse the existing navigation path for accept

**Decision**: On accept, `quick_cd` resolves the typed text to a `VfsPath` and
calls the existing `App::navigate_to(active, path)`
(`cargonaut-core/src/lib.rs` ~982).

**Rationale**: `navigate_to` already (a) lists the target via the backend —
which naturally fails with `AppError` for non-existent / non-directory / denied
targets, satisfying FR-006's "don't navigate on bad path", and (b) pushes the
previous cwd into `dir_history_back` bounded by `directory_depth`, satisfying
FR-005 (history updates identically). No parallel navigation logic.

**Alternatives rejected**: A bespoke "set cwd" that re-implements listing +
history — rejected as duplication that would drift from descend/ascend behavior.

## R-003 — Path resolution (relative vs absolute vs URI)

**Decision**: A resolver maps the typed string to a `VfsPath` against the active
pane's `cwd` (which carries the scheme + authority):

- Input containing `://` → `VfsPath::parse` directly (full URI).
- Input starting with `/` → absolute filesystem path: take active cwd's scheme +
  authority, replace segments by splitting the path on `/` (dropping empties).
- Otherwise → relative: start from active cwd segments, append each typed
  segment; a `..` segment pops one; `.` is skipped.
- A trailing `/` is ignored (FR edge case).

**Rationale**: `VfsPath` is `{scheme, authority, segments}` with `join` appending
a single segment and rejecting `/`/`..` (`cargonaut-vfs/src/types.rs:111`), so the
resolver must split/normalize before constructing the path rather than calling
`join` with raw input. Resolving against the active cwd's scheme/authority keeps
quick-cd backend-agnostic (FR-012, and forward-compatible with remote backends
without exercising them now).

**Alternatives rejected**: Passing the raw string to `VfsPath::join` — rejected,
it panics on `/` and `..`. Restricting input to absolute `file://` URIs only —
rejected as hostile UX (users type plain paths).

## R-004 — Completion candidate sourcing & ordering

**Decision**: `complete_cd(partial)` splits the typed text into
`(dir_prefix, last_segment)`. It resolves `dir_prefix` to a `VfsPath` (R-003),
lists it via `local_fs.list(dir, NameAsc)`, keeps entries whose `meta.kind` is
`Dir` (or a symlink resolving to a dir — treated as dir per normal nav) and whose
name `starts_with(last_segment)`. It then prepends matching entries from the
active pane's `dir_history_back` (most-recent first) whose *final segment* starts
with `last_segment` and that live under `dir_prefix`. Results are de-duplicated
(by full display path), recent-first, then filesystem order. Candidates are
returned as full path strings ready to drop into the buffer.

**Rationale**: FR-008 mandates both sources, directories only, recent-first.
`VfsBackend::list` returns `DirListing { entries: Vec<DirEntry{name, meta}> }`
pre-sorted (`cargonaut-vfs/src/traits.rs:168`); `meta.kind` distinguishes dirs
(`VfsKind::Dir`). `dir_history_back: Vec<VfsPath>` is the T1.24 source
(`cargonaut-core/src/lib.rs:74`).

**Alternatives rejected**: Fuzzy/substring matching — out of scope (spec). Listing
recursively — unnecessary; completion is one level at a time per typed segment.

## R-005 — Tab cycling without re-listing every press

**Decision**: The widget caches the candidate list and the buffer value it was
computed for. Tab behavior:

- If the cache is **valid** (buffer unchanged since last completion) → cycle to
  the next cached candidate in-widget (wrapping). No async call.
- If the cache is **stale** (buffer edited since) → the widget returns
  `PathInputAction::RequestCompletions { text }`; the event loop calls
  `App::complete_cd`, then `widget.apply_completions(candidates)`, which stores
  them, resets the cycle index to 0, and applies the first candidate.
- Empty candidate set → widget keeps text, sets a transient "(no matches)" note
  (FR-009).

**Rationale**: Keeps repeated Tab cheap and deterministic (FR-007 cycle +
wrap), only hitting the backend when the input actually changed. The event loop
is already async (`handle_key`), so the fetch fits without blocking the frame.

**Alternatives rejected**: Re-listing on every Tab — wasteful and could reorder
candidates mid-cycle. Pre-fetching all candidates on open — wrong; the relevant
directory depends on what the user types.

## R-006 — Widget shape & reuse for #32 / #33

**Decision**: Add `PathInputDialog` to `cargonaut-ui-tui::dialog` (beside
`TextInputDialog`), with: prefilled buffer + cursor, char/backspace editing,
Tab-completion cycle state, an optional inline error line, and a `handle_key →
PathInputAction` API (`Edited | RequestCompletions{text} | Submit(String) |
Cancel | Consumed`). Completion candidates and errors are injected by the caller
(`apply_completions`, `set_error`) so the widget stays sync and backend-agnostic
— directly reusable by the tasks panel (#32) and filter prompt (#33), which need
the same "text input + caller-supplied completion/validation" shape.

**Rationale**: Constitution §III wants shared dialog widgets and "no ad-hoc
layouts in feature code." The existing `dialog.rs` widgets are the realized form
of that principle (the `dialog!` macro named in the constitution does not exist
yet — confirmed by grep). Extending the module is the in-spirit choice and keeps
the new widget where the next two features will find it.

**Alternatives rejected**: Extending `TextInputDialog` in place with optional
completion — rejected: it would burden the simple mkdir/pattern callers with
completion state and a richer return type. A brand-new `dialog!` macro — rejected
as out-of-scope scope-creep for one feature; can be a later refactor that folds
all four widgets in.

## R-007 — On-invalid-Enter behavior (clarified)

**Decision**: Keep the prompt open, preserve the typed text, show an inline
error; do not navigate (spec Clarifications + FR-006).

**Rationale**: User-selected in `/speckit-clarify`. Mirrors shell `cd`'s
"tell me and let me fix it." Implemented by: event loop calls `app.quick_cd`; on
`Err(e)`, instead of closing, calls `widget.set_error(e.to_string())` and leaves
`active_dialog`/`mode` unchanged.

**Alternatives rejected**: Close + status error — rejected by the user in
clarify.

## R-008 — Testing strategy (SC-006)

**Decision**: Two layers.
1. **Core** (`cargonaut-core` unit tests, `#[tokio::test]` + `TempDir`): build an
   `App`, call `complete_cd` and `quick_cd` directly, assert candidate
   ordering/dir-only filtering, relative/absolute resolution, successful nav +
   history update, invalid path → `Err` + cwd unchanged, cancel = no-op. This is
   the SC-006 "injected-input" gate per T1.25.
2. **Widget** (`cargonaut-ui-tui::dialog` unit tests): feed `KeyCode` sequences
   to `PathInputDialog`, assert buffer edits, that Tab returns
   `RequestCompletions` when stale and cycles when fresh, Enter→`Submit`,
   Esc→`Cancel`, and a `TestBackend` render shows title/prompt/error.

**Rationale**: Matches the established split (engine tested off-TTY; widget
tested with synthetic keys + `TestBackend`, as the existing dialog tests do at
`cargonaut-ui-tui/src/dialog.rs` ~497). The bin-level PTY E2E stays deferred
(#30); SC-006 is satisfied by the core test, not a PTY driver.

**Alternatives rejected**: A new PTY harness for this feature — rejected; it's the
explicitly-deferred #30 and not required for SC-006.

# Phase 1 Data Model: Panel Filter Prompt Dialog

## Entities

### `PaneFilter` (NEW — `cargonaut-core`)

A compiled, applied filter for one pane.

| Field     | Type                  | Notes                                                    |
|-----------|-----------------------|---------------------------------------------------------|
| `pattern` | `String`              | Original user text; backs prompt prefill (FR-002).      |
| `matcher` | `globset::GlobMatcher`| Compiled, case-insensitive; backs `is_match` per frame. |

Derives: `Debug`, `Clone` (both supplied by `GlobMatcher`).

Associated functions / methods:
- `PaneFilter::compile(pattern: &str) -> Result<PaneFilter, AppError>` — trims; applies the
  auto-substring rule (wrap as `*pattern*` when no `* ? [ ] { }` present); builds a
  case-insensitive matcher; maps `globset::Error` → `AppError::BadFilter`. Caller guarantees
  `pattern` is non-empty after trim (empty → clear path, never compiled).
- `PaneFilter::is_match(&self, name: &str) -> bool` — delegates to `matcher.is_match(name)`.
- `PaneFilter::pattern(&self) -> &str` — accessor for prefill.

**State**: a pane has either `None` (no filter) or `Some(PaneFilter)`. There is no separate
"disabled but remembered" state — clearing sets `None`.

### `PaneState.filter` (CHANGED — `cargonaut-core`)

`Option<String>` → `Option<PaneFilter>`. Consumed by `PaneState::visible_indices`, which
replaces `e.name.contains(pat)` with `pf.is_match(e.name.as_str())`.

### `PaneView.filter` (CHANGED — `cargonaut-ui-tui`)

`Option<String>` → `Option<PaneFilter>`. `PaneView::sync_from` still does
`self.filter = state.filter.clone()`. `PaneView::visible_indices` mirrors the core change.

### `AppError::BadFilter` (NEW — `cargonaut-core`)

```rust
/// A filter pattern failed to compile as a glob.
#[error("bad filter: {0}")]
BadFilter(String),
```

Carries the `globset` error message for inline display via `PathInputDialog::set_error`.

### `ActiveDialog::FilterPrompt` (NEW — `cargonaut-ui-tui`)

```rust
FilterPrompt { widget: PathInputDialog },
```

Mirrors `QuickCd { widget: PathInputDialog }`. The widget is constructed with the focused
pane's current pattern as initial text (empty string if no active filter).

## State transitions (focused pane filter)

```
            invoke filter cmd (Alt-!)
   any state ───────────────────────────▶ FilterPrompt open
                                            (prefilled w/ current pattern)

   FilterPrompt + Submit(non-empty, valid)  ─▶ pane.filter = Some(PaneFilter); cursor = 0;
                                                close; Status "Filter: <pattern>"
   FilterPrompt + Submit(empty/whitespace)  ─▶ pane.filter = None; cursor = 0;
                                                close; Status "Panel filter cleared"
   FilterPrompt + Submit(invalid glob)      ─▶ widget.set_error(msg); stay open;
                                                pane unchanged
   FilterPrompt + edit (Char/Backspace)     ─▶ widget clears any error; stay open
   FilterPrompt + Cancel (Esc)              ─▶ close; pane.filter unchanged
```

Navigation (`Descend`/`Ascend`/cd) does **not** alter `pane.filter` — it persists until
cleared (FR-003c). `visible_indices` re-applies the existing matcher to the new listing.

## Visibility composition

`visible_indices` applies, in order, for each entry:
1. hidden-file mask (`show_hidden` + `meta.is_hidden`) — unchanged.
2. filter: if `Some(pf)`, keep only when `pf.is_match(name)`.

Both panes compute independently, so filtering one never affects the other (FR-009).

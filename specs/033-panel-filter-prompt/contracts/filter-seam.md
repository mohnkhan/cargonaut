# Contract: Core ↔ TUI seam for the filter prompt

This seam mirrors the quick-cd seam (Feature 038). The TUI owns the modal; core owns the
compile-and-apply logic and remains headless-testable.

## Core surface (`cargonaut-core`)

```rust
impl App {
    /// Set or clear the focused pane's filter from raw prompt text.
    ///
    /// - Empty / whitespace-only `pattern` → clears the filter (no error).
    /// - Non-empty valid `pattern` → compiles a case-insensitive glob
    ///   (metacharacter-free patterns are matched as `*pattern*`) and applies
    ///   it to the focused pane; the pane cursor is reset to 0.
    /// - Non-empty invalid `pattern` → returns `Err(AppError::BadFilter(_))`
    ///   and leaves all pane state unchanged.
    ///
    /// Returns the events the caller should apply on success
    /// (`PaneUpdated(active)` + a `Status`).
    pub fn set_filter(&mut self, pattern: &str) -> Result<Vec<Event>, AppError>;
}
```

Guarantees:
- **Focused-pane only**: only `self.active` is mutated (FR-009).
- **Atomic on error**: an invalid pattern mutates nothing (FR-006, SC-003).
- **Cursor reset** on any successful set or clear (FR-004).
- `Command::TogglePanelFilter` dispatched into core is a **no-op** (the TUI intercepts it to
  open the dialog) — documented in the match arm, mirroring `QuickCdPopup`.

## TUI surface (`cargonaut-ui-tui`)

**Open** (intercept inside `dispatch_ui_command`, in the same arm style as
`Command::QuickCdPopup` — this short-circuits before the command is mapped to core; note
this snippet lives in `dispatch_ui_command` while the key-handling snippet below lives in
`handle_key`):
```rust
Command::TogglePanelFilter => {
    let initial = app.active_pane_state()
        .filter.as_ref().map(|f| f.pattern().to_string()).unwrap_or_default();
    *active_dialog = Some(ActiveDialog::FilterPrompt {
        widget: PathInputDialog::new("Filter", "Pattern:", initial),
    });
    *mode = Mode::Dialog;
    return Ok(());
}
```

**Key handling** (dialog branch):
```rust
ActiveDialog::FilterPrompt { widget } => {
    match widget.handle_key(key.code) {
        PathInputAction::Submit(text) => match app.set_filter(&text) {
            Ok(events) => {
                *active_dialog = None;
                *mode = Mode::Pane;
                for ev in events { apply_event(ev, /* … */); }
            }
            Err(e) => {
                if let Some(ActiveDialog::FilterPrompt { widget }) = active_dialog.as_mut() {
                    widget.set_error(e.to_string());   // FR-006: stay open, show error
                }
            }
        },
        PathInputAction::Cancel => { *active_dialog = None; *mode = Mode::Pane; }   // FR-008
        // No path completions for a glob prompt: ignore RequestCompletions.
        PathInputAction::RequestCompletions { .. }
        | PathInputAction::Edited
        | PathInputAction::Consumed => {}
    }
    return Ok(true);
}
```

Notes:
- Unlike QuickCd, `Submit` does **not** special-case empty text in the TUI — `set_filter("")`
  performs the clear and returns the clear `Status`, so the prompt closes on empty submit
  (US2). (QuickCd kept empty open because empty cd is meaningless; empty filter is a valid
  clear.)
- `set_filter` is synchronous → no `.await`, no cross-await re-borrow of `active_dialog`.

**Render** (draw arm):
```rust
ActiveDialog::FilterPrompt { widget } => widget.render(darea, f.buffer_mut(), theme),
```

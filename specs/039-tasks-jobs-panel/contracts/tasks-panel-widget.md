# Contract: TasksPanelDialog widget + TUI wiring

**Feature**: 039-tasks-jobs-panel

Additions in `crates/cargonaut-ui-tui/src/dialog.rs` (widget) and
`crates/cargonaut-ui-tui/src/lib.rs` (modal wiring). Modeled on
`ResumePromptDialog`. Uses the typed `Theme` (`dialog_style()`); no hardcoded
ANSI; no ad-hoc layout (Constitution §III).

## Widget (`dialog.rs`)

```rust
/// One row in the tasks panel, built from a core `JobView`.
#[derive(Debug, Clone)]
pub struct JobRow {
    pub id: TransferId,            // echoed back to target the App method
    pub label: String,            // "<src> → <dst>" (display-shortened)
    pub status_label: String,     // "Running 62%" | "Paused" | "Completed ✓" | ...
    pub can_cancel: bool,
    pub can_pause: bool,
    pub can_resume: bool,
}

/// What the panel asks the event loop to do.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TasksAction {
    Cancel(usize),   // focused row index
    Pause(usize),
    Resume(usize),
    Close,
}

/// Modal list of transfers with per-row cancel/pause/resume.
#[derive(Debug)]
pub struct TasksPanelDialog { /* rows: Vec<JobRow>, state: ListState */ }

impl TasksPanelDialog {
    pub fn new(rows: Vec<JobRow>) -> Self;          // selects row 0 if non-empty
    pub fn set_rows(&mut self, rows: Vec<JobRow>);  // refresh; preserve+clamp selection
    pub fn len(&self) -> usize;
    pub fn is_empty(&self) -> bool;
    pub fn focused_index(&self) -> Option<usize>;
    pub fn focused(&self) -> Option<&JobRow>;
    pub fn handle_key(&mut self, code: KeyCode) -> Option<TasksAction>;
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme);
}
```

### `handle_key` contract
- `Down`/`Char('j')` → move selection down (clamped), return `None`.
- `Up`/`Char('k')` → move selection up (clamped), return `None`.
- `Char('c'|'C')` → `Some(Cancel(focused))`.
- `Char('p'|'P')` → `Some(Pause(focused))`.
- `Char('r'|'R')` → `Some(Resume(focused))`.
- `Esc` → `Some(Close)`.
- Any other key → `None`.
- When the list is empty, navigation/action keys return `None` except `Esc`
  (`Close`); there is no focused index, so `c`/`p`/`r` are inert.
- The widget does **not** itself enforce eligibility (it always reports the action
  for the focused row); the event loop calls the App method, which no-ops on
  ineligible jobs (FR-012). `can_*` flags drive rendering hints only.

### `render` contract
- `Clear`s its rect first (modal).
- Bordered block titled e.g. `"Transfers (N) — [c]ancel [p]ause [r]esume / Esc"`.
- One line per row: `"<label>   <status_label>"`, selection highlighted via
  `ListState`. Long labels truncated to the row width (edge case: long paths).
- Empty list → a single explanatory line ("No transfers") inside the block
  (FR-014), not a blank box.

## TUI wiring (`lib.rs`)

### `ActiveDialog`
Add variant: `TasksPanel { widget: TasksPanelDialog }`.

### Open (in `dispatch_ui_command`, alongside `QuickCdPopup`/`TogglePanelFilter`)
```text
Command::ShowTasksPanel =>
    rows = app.job_views().map(JobRow::from)   // core projection → row model
    *active_dialog = Some(ActiveDialog::TasksPanel { widget: TasksPanelDialog::new(rows) });
    *mode = Mode::Dialog;
    return Ok(());
```
- Only opens if no other modal is active (the dispatch path already guards modal
  stacking; opening sets `Mode::Dialog`). FR-013.

### Key routing (in `handle_key`, alongside other `ActiveDialog` arms)
```text
ActiveDialog::TasksPanel { widget } => match widget.handle_key(key.code) {
    Some(TasksAction::Close) | <F12 again> => { *active_dialog = None; *mode = Mode::Pane; }
    Some(TasksAction::Cancel(i)) => { id = rows[i].id; app.cancel_transfer(id); refresh_rows(widget, app); }
    Some(TasksAction::Pause(i))  => { id = rows[i].id; app.pause_transfer(id);  refresh_rows(widget, app); }
    Some(TasksAction::Resume(i)) => { id = rows[i].id; app.resume_paused(id).await?; refresh_rows(widget, app); }
    None => {}   // navigation / inert key: stay open
}
```
- `refresh_rows` = `widget.set_rows(app.job_views().map(JobRow::from))` (preserves
  selection). The panel stays open after an action so the user can act again.
- F12 while the panel is open closes it (toggle), and never stacks (FR-013).

### Per-frame refresh (FR-008) (in the render path / tick)
Before rendering, if the active dialog is `TasksPanel`, call
`widget.set_rows(app.job_views().map(JobRow::from))` so progress/state update live
without reopening. Selection is preserved and clamped.

### Draw (in `draw_frame` dialog match)
Add: `ActiveDialog::TasksPanel { widget } => widget.render(darea, f.buffer_mut(), theme)`.

### Command mapping
`ui_command_to_core` already maps `U::ShowTasksPanel => AppCommand::ShowTasksPanel`;
since the TUI now intercepts `ShowTasksPanel` to open the modal (returning early),
the core dispatch arm is the no-op described in `core-api.md`.

## Test contract (ui-tui)

- `tasks_panel_handle_key_*`: arrows/`jk` move selection and clamp at ends; `c`/
  `p`/`r` return the matching `TasksAction` for the focused index; `Esc` returns
  `Close`.
- `tasks_panel_render_*` (TestBackend): N rows render N lines with status labels;
  empty list renders the "No transfers" line; selection is highlighted; long
  labels are truncated without overflowing the rect.
- `tasks_panel_set_rows_preserves_selection`: refresh with a shorter list clamps
  the selection in-bounds.
- `show_tasks_panel_opens_modal` (dispatch): dispatching `ShowTasksPanel` sets
  `ActiveDialog::TasksPanel` and `Mode::Dialog`; dispatching again (or `Esc`)
  closes it; no second modal stacks.
- End-to-end (SC-007): open → navigate → pause focused → row shows `Paused` after
  refresh → close leaves panes untouched.

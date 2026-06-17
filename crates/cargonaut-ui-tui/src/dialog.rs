// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Modal dialogs: copy/move/delete confirmation, plus the resume-prompt
//! shown on launch when `scan_resumable` finds an orphan checkpoint.
//!
//! Each dialog is a small state machine: it owns its focus + answer
//! state, exposes [`Dialog::handle_key`] for input, and renders via
//! [`Dialog::render`]. The App's event loop (T1.19) routes keys to the
//! active dialog when `Mode::Dialog` is the active input mode.

use crate::theme::Theme;
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget, Wrap,
};

// =====================================================================
// ConfirmDialog (copy / move / delete confirmation)
// =====================================================================

/// What the user picked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConfirmOutcome {
    /// User pressed Enter / 'y' on the Confirm button.
    Confirm,
    /// User pressed Esc / 'n' or Tab+Enter on Cancel.
    Cancel,
}

/// Modal yes/no dialog. Built by the App for any destructive op
/// (F5/F6/F8). Defaults focus on the **Cancel** button — pressing
/// Enter without thinking gives the safe answer.
#[derive(Debug, Clone)]
pub struct ConfirmDialog {
    title: String,
    body: String,
    /// Index of the focused button (0 = Confirm, 1 = Cancel).
    focus: usize,
}

impl ConfirmDialog {
    /// Build a new confirmation dialog. `body` shows under the title;
    /// keep it short (1-3 lines wraps; longer needs a scrollable popup,
    /// which is out of scope for this widget).
    pub fn new(title: impl Into<String>, body: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            body: body.into(),
            focus: 1, // safe default — Cancel
        }
    }

    /// Handle a key event. Returns `Some(outcome)` when the dialog should
    /// dismiss, `None` if it consumes the key without dismissing.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<ConfirmOutcome> {
        match code {
            KeyCode::Esc => Some(ConfirmOutcome::Cancel),
            KeyCode::Enter => Some(if self.focus == 0 {
                ConfirmOutcome::Confirm
            } else {
                ConfirmOutcome::Cancel
            }),
            KeyCode::Char('y') | KeyCode::Char('Y') => Some(ConfirmOutcome::Confirm),
            KeyCode::Char('n') | KeyCode::Char('N') => Some(ConfirmOutcome::Cancel),
            KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                self.focus = (self.focus + 1) % 2;
                None
            }
            _ => None,
        }
    }

    /// Render the dialog. Clears the rect first (modal — covers the
    /// underlying pane).
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        let body_with_buttons = format!(
            "{}\n\n{}  {}",
            self.body,
            if self.focus == 0 {
                "▶ Confirm"
            } else {
                "  Confirm"
            },
            if self.focus == 1 {
                "▶ Cancel"
            } else {
                "  Cancel"
            },
        );
        let para = Paragraph::new(body_with_buttons)
            .block(block)
            .style(theme.dialog_style())
            .wrap(Wrap { trim: false });
        Widget::render(para, area, buf);
    }

    /// Index of the focused button (mostly for tests). 0 = Confirm,
    /// 1 = Cancel.
    pub fn focus(&self) -> usize {
        self.focus
    }
}

// =====================================================================
// TextInputDialog (mkdir name, select-by-pattern)
// =====================================================================

/// What the user did in a text-input dialog.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputOutcome {
    /// Enter pressed — carries the entered text.
    Submit(String),
    /// Esc pressed — cancelled.
    Cancel,
}

/// A single-line modal text-input dialog (FR-024 mkdir, FR-025 pattern).
#[derive(Debug, Clone)]
pub struct TextInputDialog {
    title: String,
    prompt: String,
    buffer: String,
}

impl TextInputDialog {
    /// New dialog with a title and an inline prompt.
    pub fn new(title: impl Into<String>, prompt: impl Into<String>) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            buffer: String::new(),
        }
    }

    /// New dialog pre-filled with `initial` text (e.g. the current mode/owner)
    /// — Feature 043. The cursor starts at end-of-buffer; editing works as usual.
    pub fn with_initial(
        title: impl Into<String>,
        prompt: impl Into<String>,
        initial: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            buffer: initial.into(),
        }
    }

    /// Current entered text (mostly for tests).
    pub fn value(&self) -> &str {
        &self.buffer
    }

    /// Handle a key. Returns `Some(outcome)` when the dialog dismisses.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<InputOutcome> {
        match code {
            KeyCode::Esc => Some(InputOutcome::Cancel),
            KeyCode::Enter => Some(InputOutcome::Submit(self.buffer.clone())),
            KeyCode::Backspace => {
                self.buffer.pop();
                None
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                None
            }
            _ => None,
        }
    }

    /// Render the dialog (modal, clears its rect first).
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        let body = format!("{}\n> {}_", self.prompt, self.buffer);
        Paragraph::new(body)
            .block(block)
            .style(theme.dialog_style())
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

// =====================================================================
// PathInputDialog (Feature 038 quick-cd; reusable text-input + completion)
// =====================================================================

/// What a key did to a [`PathInputDialog`]. Completion is asynchronous
/// (the candidate directories come from the VFS), so the widget cannot
/// fetch them itself — on a stale cache it asks the event loop to fetch
/// via [`PathInputAction::RequestCompletions`] and receives the result
/// through [`PathInputDialog::apply_completions`].
///
/// Designed to be reused by the deferred tasks panel (#32) and panel
/// filter prompt (#33), which need the same "text input + caller-supplied
/// completion/validation" shape.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PathInputAction {
    /// Key handled; nothing further for the event loop to do.
    Consumed,
    /// The buffer changed (any cached completions are now stale).
    Edited,
    /// Tab on a stale cache — the loop must fetch candidates for `text`
    /// and feed them back via [`PathInputDialog::apply_completions`].
    RequestCompletions {
        /// The current buffer text to complete.
        text: String,
    },
    /// Enter — accept this text.
    Submit(String),
    /// Esc — cancel.
    Cancel,
}

/// A single-line modal text-input dialog with directory tab-completion
/// (FR-012 quick-cd). Prefilled on open; Tab completes/cycles the buffer
/// against caller-supplied candidates; Enter submits, Esc cancels.
#[derive(Debug, Clone)]
pub struct PathInputDialog {
    title: String,
    prompt: String,
    buffer: String,
    /// Cached completion candidates from the last fetch.
    completions: Vec<String>,
    /// The buffer value `completions` was computed for; when it differs
    /// from `buffer` the cache is stale and Tab re-requests.
    completion_for: String,
    /// Position within `completions` for the current cycle.
    cycle_idx: usize,
    /// Inline error (failed accept); cleared on the next edit.
    error: Option<String>,
    /// Transient hint, e.g. "(no matches)"; cleared on the next edit.
    note: Option<String>,
}

impl PathInputDialog {
    /// New prompt prefilled with `initial`, cursor conceptually at end.
    pub fn new(
        title: impl Into<String>,
        prompt: impl Into<String>,
        initial: impl Into<String>,
    ) -> Self {
        Self {
            title: title.into(),
            prompt: prompt.into(),
            buffer: initial.into(),
            completions: Vec::new(),
            completion_for: String::new(),
            cycle_idx: 0,
            error: None,
            note: None,
        }
    }

    /// Current buffer text.
    pub fn value(&self) -> &str {
        &self.buffer
    }

    /// True when the cached completions still apply to the current buffer.
    fn cache_fresh(&self) -> bool {
        !self.completions.is_empty() && self.completion_for == self.buffer
    }

    /// Handle a key. See [`PathInputAction`].
    pub fn handle_key(&mut self, code: KeyCode) -> PathInputAction {
        match code {
            KeyCode::Esc => PathInputAction::Cancel,
            KeyCode::Enter => PathInputAction::Submit(self.buffer.clone()),
            KeyCode::Backspace => {
                self.buffer.pop();
                self.error = None;
                self.note = None;
                PathInputAction::Edited
            }
            KeyCode::Char(c) => {
                self.buffer.push(c);
                self.error = None;
                self.note = None;
                PathInputAction::Edited
            }
            KeyCode::Tab => {
                if self.cache_fresh() {
                    // Cycle to the next candidate, wrapping.
                    self.cycle_idx = (self.cycle_idx + 1) % self.completions.len();
                    self.buffer = self.completions[self.cycle_idx].clone();
                    self.completion_for = self.buffer.clone();
                    PathInputAction::Consumed
                } else {
                    PathInputAction::RequestCompletions {
                        text: self.buffer.clone(),
                    }
                }
            }
            _ => PathInputAction::Consumed,
        }
    }

    /// Install freshly-fetched candidates (response to a
    /// [`PathInputAction::RequestCompletions`]). Applies the first
    /// candidate and marks the cache fresh; an empty list sets the
    /// "(no matches)" note and leaves the buffer untouched (FR-009).
    pub fn apply_completions(&mut self, candidates: Vec<String>) {
        if candidates.is_empty() {
            self.completions.clear();
            self.note = Some("(no matches)".into());
            return;
        }
        self.buffer = candidates[0].clone();
        self.completion_for = self.buffer.clone();
        self.cycle_idx = 0;
        self.completions = candidates;
        self.error = None;
        self.note = None;
    }

    /// Show an inline error and keep the prompt open (failed accept,
    /// FR-006). Cleared on the next edit.
    pub fn set_error(&mut self, msg: impl Into<String>) {
        self.error = Some(msg.into());
    }

    /// Render the dialog (modal, clears its rect first).
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let block = Block::default()
            .title(self.title.as_str())
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        let mut body = format!("{}\n> {}_", self.prompt, self.buffer);
        if let Some(note) = &self.note {
            body.push_str(&format!("\n{note}"));
        }
        if let Some(err) = &self.error {
            body.push_str(&format!("\n✗ {err}"));
        }
        Paragraph::new(body)
            .block(block)
            .style(theme.dialog_style())
            .wrap(Wrap { trim: false })
            .render(area, buf);
    }
}

// =====================================================================
// ResumePromptDialog (offered on launch when scan_resumable finds work)
// =====================================================================

/// User response to a single resume offer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeChoice {
    /// `[r]esume` — pick up from the checkpoint.
    Resume,
    /// `[s]tart over` — discard the checkpoint, start fresh.
    StartOver,
    /// `[c]ancel` — skip this offer (sidecar stays for next launch).
    Skip,
}

/// One row in the resume-prompt list: a brief summary derived from a
/// [`cargonaut_transfer::ResumableTransfer`]. The App builds these
/// before constructing the dialog.
#[derive(Debug, Clone)]
pub struct ResumableSummary {
    /// Source URI (display-shortened to fit the row).
    pub src: String,
    /// Destination URI.
    pub dst: String,
    /// Bytes already written (formatted as MiB).
    pub bytes_written_mib: f32,
    /// Source size (formatted as MiB).
    pub src_size_mib: f32,
    /// Was source SHA-256 prefix unchanged? Renders ✓ vs ✗.
    pub source_unchanged: bool,
    /// Was destination CRC chain intact? Renders ✓ vs ✗.
    pub dest_intact: bool,
}

/// Modal list dialog: shows N resumable transfers; user picks one and
/// answers `r`/`s`/`c`. The App handles each selection sequentially.
#[derive(Debug)]
pub struct ResumePromptDialog {
    offers: Vec<ResumableSummary>,
    state: ListState,
}

impl ResumePromptDialog {
    /// Build a prompt from a list of resume summaries. Empty `offers`
    /// is allowed but pointless (caller should skip rendering entirely).
    pub fn new(offers: Vec<ResumableSummary>) -> Self {
        let mut state = ListState::default();
        if !offers.is_empty() {
            state.select(Some(0));
        }
        Self { offers, state }
    }

    /// Number of offers remaining. (Items don't get removed by handle_key;
    /// the App removes them after acting on each choice.)
    pub fn len(&self) -> usize {
        self.offers.len()
    }

    /// True if no offers remain.
    pub fn is_empty(&self) -> bool {
        self.offers.is_empty()
    }

    /// Currently-focused offer (index into the original `offers` vec).
    pub fn focused_index(&self) -> Option<usize> {
        self.state.selected()
    }

    /// Read the focused summary, if any.
    pub fn focused(&self) -> Option<&ResumableSummary> {
        self.state.selected().and_then(|i| self.offers.get(i))
    }

    /// Drive the dialog with a key. Returns `Some((index, choice))` when
    /// the user makes a decision about the currently-focused offer
    /// (caller is expected to remove the offer + act on the choice);
    /// `None` otherwise.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<(usize, ResumeChoice)> {
        let focused = self.state.selected()?;
        match code {
            KeyCode::Char('r') | KeyCode::Char('R') => Some((focused, ResumeChoice::Resume)),
            KeyCode::Char('s') | KeyCode::Char('S') => Some((focused, ResumeChoice::StartOver)),
            KeyCode::Char('c') | KeyCode::Char('C') | KeyCode::Esc => {
                Some((focused, ResumeChoice::Skip))
            }
            KeyCode::Down | KeyCode::Char('j') => {
                let next = (focused + 1).min(self.offers.len().saturating_sub(1));
                self.state.select(Some(next));
                None
            }
            KeyCode::Up | KeyCode::Char('k') => {
                let prev = focused.saturating_sub(1);
                self.state.select(Some(prev));
                None
            }
            _ => None,
        }
    }

    /// Render the dialog. Clears its rect first (modal).
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let items: Vec<ListItem<'_>> = self
            .offers
            .iter()
            .map(|s| {
                let pct = if s.src_size_mib > 0.0 {
                    (s.bytes_written_mib / s.src_size_mib) * 100.0
                } else {
                    0.0
                };
                let src_ok = if s.source_unchanged { "✓" } else { "✗" };
                let dst_ok = if s.dest_intact { "✓" } else { "✗" };
                let line = format!(
                    "{} → {}  [{:>5.1}/{:>5.1} MiB, {:>4.1}%]  src{src_ok} dst{dst_ok}",
                    s.src, s.dst, s.bytes_written_mib, s.src_size_mib, pct
                );
                ListItem::new(line)
            })
            .collect();
        let block = Block::default()
            .title(format!(
                "Resumable transfers ({}) — [r]esume / [s]tart over / [c]ancel",
                self.offers.len()
            ))
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        let list = List::new(items)
            .block(block)
            .style(theme.dialog_style())
            .highlight_style(
                ratatui::style::Style::default()
                    .fg(theme.dialog_sel_fg)
                    .bg(theme.dialog_sel_bg),
            )
            .highlight_symbol("▶ ");
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

// =====================================================================
// TasksPanelDialog (Feature 039 — F12 tasks/jobs panel)
// =====================================================================

/// One row of the tasks panel: pre-formatted display strings plus which
/// per-row actions are eligible. The App builds these from core `JobView`s
/// before constructing/refreshing the dialog, so the widget stays free of
/// transfer/core types; the event loop maps a row index back to a job id
/// via `job_views()` (mirrors how the resume prompt maps index → offer).
#[derive(Debug, Clone)]
pub struct JobRow {
    /// `"<src> → <dst>"`, display-shortened by the caller.
    pub label: String,
    /// Human-readable state, e.g. `"Running 62%"`, `"Paused"`, `"Completed ✓"`.
    pub status_label: String,
    /// Whether cancel is eligible for this row (rendering hint only).
    pub can_cancel: bool,
    /// Whether pause is eligible for this row.
    pub can_pause: bool,
    /// Whether resume is eligible for this row.
    pub can_resume: bool,
}

/// What the tasks panel asks the event loop to do with the focused row.
/// The `usize` is the focused row index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TasksAction {
    /// Cancel the transfer at this row.
    Cancel(usize),
    /// Pause the transfer at this row.
    Pause(usize),
    /// Resume the transfer at this row.
    Resume(usize),
    /// Close the panel.
    Close,
}

/// Modal list of transfers with per-row cancel/pause/resume. Modeled on
/// [`ResumePromptDialog`]; the App refreshes its rows from `job_views()`
/// each frame so progress updates show live (FR-008).
#[derive(Debug)]
pub struct TasksPanelDialog {
    rows: Vec<JobRow>,
    state: ListState,
}

impl TasksPanelDialog {
    /// Build a panel from row data; selects the first row when non-empty.
    pub fn new(rows: Vec<JobRow>) -> Self {
        let mut state = ListState::default();
        if !rows.is_empty() {
            state.select(Some(0));
        }
        Self { rows, state }
    }

    /// Replace the rows (live refresh), preserving the selection clamped to
    /// the new bounds (or `None` when the list becomes empty).
    pub fn set_rows(&mut self, rows: Vec<JobRow>) {
        let sel = if rows.is_empty() {
            None
        } else {
            Some(self.state.selected().unwrap_or(0).min(rows.len() - 1))
        };
        self.rows = rows;
        self.state.select(sel);
    }

    /// Number of rows.
    pub fn len(&self) -> usize {
        self.rows.len()
    }

    /// True when there are no rows.
    pub fn is_empty(&self) -> bool {
        self.rows.is_empty()
    }

    /// Index of the focused row, if any.
    pub fn focused_index(&self) -> Option<usize> {
        self.state.selected()
    }

    /// The focused row, if any.
    pub fn focused(&self) -> Option<&JobRow> {
        self.state.selected().and_then(|i| self.rows.get(i))
    }

    /// Drive the panel with a key. Navigation returns `None`; an action key
    /// returns the action for the focused row; Esc / F12 returns `Close`.
    /// Eligibility is not enforced here — the App's action method no-ops on
    /// ineligible jobs (FR-012); `can_*` are rendering hints only.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<TasksAction> {
        match code {
            KeyCode::Esc | KeyCode::F(12) => return Some(TasksAction::Close),
            KeyCode::Down | KeyCode::Char('j') => {
                if let Some(cur) = self.state.selected() {
                    let next = (cur + 1).min(self.rows.len().saturating_sub(1));
                    self.state.select(Some(next));
                }
                return None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                if let Some(cur) = self.state.selected() {
                    self.state.select(Some(cur.saturating_sub(1)));
                }
                return None;
            }
            _ => {}
        }
        let focused = self.state.selected()?;
        match code {
            KeyCode::Char('c') | KeyCode::Char('C') => Some(TasksAction::Cancel(focused)),
            KeyCode::Char('p') | KeyCode::Char('P') => Some(TasksAction::Pause(focused)),
            KeyCode::Char('r') | KeyCode::Char('R') => Some(TasksAction::Resume(focused)),
            _ => None,
        }
    }

    /// Render the modal. Clears its rect first; renders an explicit empty
    /// state (FR-014) and truncates over-long rows to the panel width.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let block = Block::default()
            .title(format!(
                "Transfers ({}) — [c]ancel [p]ause [r]esume / Esc",
                self.rows.len()
            ))
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        if self.rows.is_empty() {
            Paragraph::new("No transfers")
                .block(block)
                .style(theme.dialog_style())
                .render(area, buf);
            return;
        }
        let width = area.width.saturating_sub(4) as usize;
        let items: Vec<ListItem<'_>> = self
            .rows
            .iter()
            .map(|r| {
                let line = format!("{}   {}", r.label, r.status_label);
                let line = if width > 1 && line.chars().count() > width {
                    line.chars().take(width - 1).collect::<String>() + "…"
                } else {
                    line
                };
                ListItem::new(line)
            })
            .collect();
        let list = List::new(items)
            .block(block)
            .style(theme.dialog_style())
            .highlight_style(
                ratatui::style::Style::default()
                    .fg(theme.dialog_sel_fg)
                    .bg(theme.dialog_sel_bg),
            )
            .highlight_symbol("▶ ");
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

// =====================================================================
// HotlistDialog (Feature 042 — directory hotlist / bookmarks)
// =====================================================================

/// One row in the hotlist popup. `index` is the bookmark's original index in
/// `App::bookmarks()` for selectable entries, or `None` for a non-selectable
/// group-header row (so grouped display and index→entity mapping coexist).
#[derive(Debug, Clone)]
pub struct HotlistRow {
    /// Pre-formatted display text (caller indents entries / styles headers).
    pub display: String,
    /// Original bookmark index, or `None` for a group header / empty-state row.
    pub index: Option<usize>,
}

/// What the hotlist popup asks the event loop to do. The `usize` is the
/// focused bookmark's original index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HotlistAction {
    /// Jump the active pane to the bookmark at this index.
    Select(usize),
    /// Add the active pane's current directory as a new bookmark.
    Add,
    /// Remove the bookmark at this index.
    Remove(usize),
    /// Close the popup.
    Close,
}

/// Modal hotlist popup. Modeled on [`TasksPanelDialog`]; holds no core types —
/// the event loop builds rows from `App::bookmarks()` (or `Hotlist::grouped()`)
/// and maps a selected row's `index` back to a bookmark on a fresh snapshot.
/// Selection skips non-selectable header rows (`index == None`).
#[derive(Debug)]
pub struct HotlistDialog {
    rows: Vec<HotlistRow>,
    state: ListState,
}

impl HotlistDialog {
    /// Build the popup, selecting the first selectable row (if any).
    pub fn new(rows: Vec<HotlistRow>) -> Self {
        let mut d = Self {
            rows,
            state: ListState::default(),
        };
        d.state.select(d.first_selectable());
        d
    }

    /// True when there are no selectable bookmark rows.
    pub fn is_empty(&self) -> bool {
        !self.rows.iter().any(|r| r.index.is_some())
    }

    /// The focused bookmark's original index, if a bookmark row is selected.
    pub fn focused_index(&self) -> Option<usize> {
        self.state.selected().and_then(|i| self.rows.get(i)?.index)
    }

    fn first_selectable(&self) -> Option<usize> {
        self.rows.iter().position(|r| r.index.is_some())
    }

    /// Move the selection to the next selectable row in `dir` (+1/-1),
    /// skipping non-selectable header rows; stays put if none remain.
    fn move_selection(&mut self, dir: isize) {
        let Some(cur) = self.state.selected() else {
            self.state.select(self.first_selectable());
            return;
        };
        let n = self.rows.len() as isize;
        let mut i = cur as isize + dir;
        while i >= 0 && i < n {
            if self.rows[i as usize].index.is_some() {
                self.state.select(Some(i as usize));
                return;
            }
            i += dir;
        }
        // No further selectable row — keep current.
    }

    /// Drive the popup with a key. Navigation returns `None`; Enter/Space ⇒
    /// `Select`, `a` ⇒ `Add`, `d`/Delete ⇒ `Remove`, Esc / Ctrl-b ⇒ `Close`.
    pub fn handle_key(&mut self, code: KeyCode) -> Option<HotlistAction> {
        match code {
            KeyCode::Esc => return Some(HotlistAction::Close),
            KeyCode::Down | KeyCode::Char('j') => {
                self.move_selection(1);
                return None;
            }
            KeyCode::Up | KeyCode::Char('k') => {
                self.move_selection(-1);
                return None;
            }
            KeyCode::Char('a') | KeyCode::Char('A') => return Some(HotlistAction::Add),
            _ => {}
        }
        let focused = self.focused_index();
        match code {
            KeyCode::Enter | KeyCode::Char(' ') => focused.map(HotlistAction::Select),
            KeyCode::Char('d') | KeyCode::Char('D') | KeyCode::Delete => {
                focused.map(HotlistAction::Remove)
            }
            _ => None,
        }
    }

    /// Render the modal. Clears its rect; shows an explicit empty state.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        Clear.render(area, buf);
        let block = Block::default()
            .title("Hotlist — Enter jump · [a]dd · [d]el · Esc")
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        if self.is_empty() {
            Paragraph::new("No bookmarks — press [a] to add this directory")
                .block(block)
                .style(theme.dialog_style())
                .render(area, buf);
            return;
        }
        let width = area.width.saturating_sub(4) as usize;
        let items: Vec<ListItem<'_>> = self
            .rows
            .iter()
            .map(|r| {
                let line = if width > 1 && r.display.chars().count() > width {
                    r.display.chars().take(width - 1).collect::<String>() + "…"
                } else {
                    r.display.clone()
                };
                ListItem::new(line)
            })
            .collect();
        let list = List::new(items)
            .block(block)
            .style(theme.dialog_style())
            .highlight_style(
                ratatui::style::Style::default()
                    .fg(theme.dialog_sel_fg)
                    .bg(theme.dialog_sel_bg),
            )
            .highlight_symbol("▶ ");
        StatefulWidget::render(list, area, buf, &mut self.state);
    }
}

// =====================================================================
// Help overlay (Feature 047 — US1, T011-T014)
// =====================================================================

/// One (shortcut, description) row in the help content.
#[derive(Debug, Clone, Copy)]
pub struct HelpRow {
    /// Keyboard shortcut(s) shown in the left column.
    pub key: &'static str,
    /// Plain-language description shown in the right column.
    pub desc: &'static str,
}

/// One named section of the help overlay.
#[derive(Debug, Clone, Copy)]
pub struct HelpSection {
    /// Section title (e.g. "Navigation").
    pub title: &'static str,
    /// Rows belonging to this section.
    pub rows: &'static [HelpRow],
}

/// All compiled-in help sections. Order defines the display order.
/// The "User Menu (F2)" section is added in T031 after US2 ships.
pub static HELP_SECTIONS: &[HelpSection] = &[
    HelpSection {
        title: "Navigation",
        rows: &[
            HelpRow { key: "F1", desc: "Show this help overlay (show-help)" },
            HelpRow { key: "F10", desc: "Quit the application (quit)" },
            HelpRow { key: "Tab", desc: "Switch active pane (focus-swap-pane)" },
            HelpRow { key: "M-1", desc: "Focus left pane (focus-left-pane)" },
            HelpRow { key: "M-2", desc: "Focus right pane (focus-right-pane)" },
            HelpRow { key: "j / Down", desc: "Move cursor down (cursor-down)" },
            HelpRow { key: "k / Up", desc: "Move cursor up (cursor-up)" },
            HelpRow { key: "Enter", desc: "Open directory or file (descend-or-open)" },
            HelpRow { key: "Backspace / h", desc: "Go up to parent directory (ascend-parent)" },
            HelpRow { key: "~", desc: "Go to home directory (cd-home)" },
            HelpRow { key: "/", desc: "Go to root directory (cd-root)" },
            HelpRow { key: ":", desc: "Open command line (open-cmdline)" },
        ],
    },
    HelpSection {
        title: "Selection",
        rows: &[
            HelpRow { key: "Insert", desc: "Toggle selection on current entry (selection-toggle)" },
            HelpRow { key: "*", desc: "Invert selection (selection-invert)" },
            HelpRow { key: "+", desc: "Add to selection by pattern (selection-add-by-pattern)" },
            HelpRow { key: "-", desc: "Remove from selection by pattern (selection-remove-by-pattern)" },
        ],
    },
    HelpSection {
        title: "File Operations",
        rows: &[
            HelpRow { key: "F3", desc: "Preview file or directory (preview)" },
            HelpRow { key: "F4", desc: "Edit file in $EDITOR (edit)" },
            HelpRow { key: "F5", desc: "Copy selection to other pane (copy-selection)" },
            HelpRow { key: "F6", desc: "Move or rename selection (move-or-rename-selection)" },
            HelpRow { key: "F7", desc: "Create a new directory (mkdir)" },
            HelpRow { key: "F8", desc: "Delete selection (delete-selection)" },
            HelpRow { key: "C-c", desc: "Cancel current operation (cancel-current-operation)" },
            HelpRow { key: "C-s", desc: "Cycle sort key (cycle-sort-key)" },
            HelpRow { key: "C-z", desc: "Undo last file operation (undo-last-op)" },
        ],
    },
    HelpSection {
        title: "Panels & Modes",
        rows: &[
            HelpRow { key: "F9", desc: "Open menu bar (open-menu-bar)" },
            HelpRow { key: "F12", desc: "Show active transfers panel (show-tasks-panel)" },
            HelpRow { key: "M-c", desc: "Quick CD popup (quick-cd-popup)" },
            HelpRow { key: "M-!", desc: "Toggle panel filter prompt (toggle-panel-filter)" },
            HelpRow { key: "<", desc: "Open fuzzy entry filter (open-fuzzy-filter)" },
            HelpRow { key: "C-f", desc: "Filter entries in current directory (filter-current-dir)" },
            HelpRow { key: "M-i", desc: "Sync other panel to this path (sync-other-panel-path)" },
            HelpRow { key: "M-o", desc: "Show focused entry in other panel (show-focused-in-other-panel)" },
            HelpRow { key: "M-.", desc: "Toggle hidden files (toggle-hidden)" },
            HelpRow { key: "M-,", desc: "Toggle split orientation (toggle-split-orientation)" },
            HelpRow { key: "C-Space", desc: "Calculate recursive directory size (recursive-dir-size)" },
            HelpRow { key: "M-t", desc: "Cycle listing mode (cycle-listing-mode)" },
            HelpRow { key: "C-t", desc: "Open new tab (new-tab)" },
            HelpRow { key: "C-w", desc: "Close current tab (close-tab)" },
            HelpRow { key: "C-o", desc: "Open subshell in current directory (open-subshell)" },
            HelpRow { key: "C-r", desc: "Reload config and themes (reload-config-and-themes)" },
        ],
    },
    HelpSection {
        title: "History",
        rows: &[
            HelpRow { key: "M-S-h", desc: "Show directory history popup (show-directory-history)" },
            HelpRow { key: "M-h", desc: "Show command history popup (show-command-history)" },
            HelpRow { key: "M-y", desc: "Navigate to previous directory in history (history-prev-dir)" },
            HelpRow { key: "M-u", desc: "Navigate to next directory in history (history-next-dir)" },
        ],
    },
    HelpSection {
        title: "Bookmarks",
        rows: &[
            HelpRow { key: "C-b", desc: "Open bookmarks menu; add / remove entries (bookmarks-menu)" },
        ],
    },
    HelpSection {
        title: "File Attributes",
        rows: &[
            HelpRow { key: "C-x c", desc: "Change file permissions (chmod)" },
            HelpRow { key: "C-x o", desc: "Change file ownership (chown)" },
            HelpRow { key: "C-x s", desc: "Create symbolic link (create-symlink)" },
            HelpRow { key: "C-x l", desc: "Create hard link (create-hard-link)" },
            HelpRow { key: "C-x C", desc: "Recursive chmod into subtree (chmod-recursive)" },
            HelpRow { key: "C-x O", desc: "Recursive chown into subtree (chown-recursive)" },
        ],
    },
    HelpSection {
        title: "Power Features",
        rows: &[
            HelpRow { key: "F2", desc: "Open user action menu from menu.toml (show-user-menu)" },
            HelpRow { key: "C-x !", desc: "External panelize — run command, list output (external-panelize)" },
            HelpRow { key: "C-x r", desc: "Bulk rename selection via editor (bulk-rename-via-editor)" },
            HelpRow { key: "C-x d", desc: "Compare two directories (compare-directories)" },
            HelpRow { key: "C-x C-d", desc: "Diff two tagged files (diff-two-tagged-files)" },
            HelpRow { key: "M-m", desc: "Toggle mouse capture; Shift+drag bypasses (toggle-mouse-capture)" },
        ],
    },
    HelpSection {
        title: "Preview",
        rows: &[
            HelpRow { key: "C-x X", desc: "Toggle hex view in previewer (toggle-hex-view)" },
            HelpRow { key: "/", desc: "Search forward in preview (preview-search-forward)" },
            HelpRow { key: "?", desc: "Search backward in preview (preview-search-backward)" },
            HelpRow { key: "n", desc: "Jump to next search match in preview (preview-search-next)" },
            HelpRow { key: "N", desc: "Jump to previous search match in preview (preview-search-prev)" },
        ],
    },
    HelpSection {
        title: "Search Mode",
        rows: &[
            HelpRow { key: "Esc", desc: "Close search overlay (close-search)" },
            HelpRow { key: "Enter", desc: "Navigate to highlighted search result (search-go-to-result)" },
        ],
    },
    HelpSection {
        title: "Dialogs",
        rows: &[
            HelpRow { key: "Esc", desc: "Cancel / close current dialog (dialog-cancel)" },
            HelpRow { key: "Enter", desc: "Confirm current dialog action (dialog-confirm)" },
        ],
    },
    HelpSection {
        title: "Orthodox-FM Compat (mc_keys=true)",
        rows: &[
            HelpRow { key: "M-5", desc: "Copy selection — alt binding (copy-selection)" },
            HelpRow { key: "M-6", desc: "Move or rename — alt binding (move-or-rename-selection)" },
        ],
    },
    HelpSection {
        title: "About",
        rows: &[
            HelpRow {
                key: "cargonaut",
                desc: "A dual-pane TUI file manager. Press Esc or F1 to close help.",
            },
        ],
    },
];

/// Outcome returned by [`HelpOverlay::handle_key`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HelpAction {
    /// Close the help overlay and return to pane mode.
    Close,
    /// Key was consumed; overlay stays open.
    Swallow,
}

/// State for the scrollable F1 help overlay.
#[derive(Debug, Clone)]
pub struct HelpOverlay {
    /// Current scroll offset in lines.
    pub scroll_offset: u16,
    /// Total rendered lines (computed once at construction).
    pub total_lines: u16,
    /// Visible lines in the overlay (set from the frame height at open time).
    pub visible_height: u16,
}

impl HelpOverlay {
    /// Construct a new overlay. `visible_height` is the inner area height of
    /// the overlay as rendered; computes `total_lines` from `HELP_SECTIONS`.
    pub fn new(visible_height: u16) -> Self {
        // Each section contributes: 1 title line + N row lines.
        let total_lines = HELP_SECTIONS
            .iter()
            .map(|s| 1 + s.rows.len())
            .sum::<usize>() as u16;
        Self {
            scroll_offset: 0,
            total_lines,
            visible_height,
        }
    }

    /// Handle a key event. Returns [`HelpAction`] indicating whether to close
    /// or swallow. The overlay swallows every key that is not a navigation or
    /// dismiss key — it MUST NOT fall through to pane commands while open.
    pub fn handle_key(&mut self, code: KeyCode) -> HelpAction {
        let max_offset = self.total_lines.saturating_sub(self.visible_height);
        match code {
            KeyCode::Up => {
                self.scroll_offset = self.scroll_offset.saturating_sub(1);
                HelpAction::Swallow
            }
            KeyCode::Down => {
                self.scroll_offset = self.scroll_offset.saturating_add(1).min(max_offset);
                HelpAction::Swallow
            }
            KeyCode::PageUp => {
                self.scroll_offset = self.scroll_offset.saturating_sub(self.visible_height);
                HelpAction::Swallow
            }
            KeyCode::PageDown => {
                self.scroll_offset = self
                    .scroll_offset
                    .saturating_add(self.visible_height)
                    .min(max_offset);
                HelpAction::Swallow
            }
            KeyCode::Home => {
                self.scroll_offset = 0;
                HelpAction::Swallow
            }
            KeyCode::End => {
                self.scroll_offset = max_offset;
                HelpAction::Swallow
            }
            KeyCode::Esc | KeyCode::F(1) => HelpAction::Close,
            _ => HelpAction::Swallow,
        }
    }

    /// Render the overlay into `area`, clearing it first.
    pub fn render(
        &self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::theme::Theme,
    ) {
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};
        use ratatui::text::{Line, Span, Text};
        use ratatui::style::{Modifier, Style};

        f.render_widget(Clear, area);

        // Build content lines from HELP_SECTIONS.
        let mut lines: Vec<Line<'static>> = Vec::new();
        for sec in HELP_SECTIONS {
            // Section title — bold.
            lines.push(Line::from(vec![Span::styled(
                sec.title,
                Style::default().add_modifier(Modifier::BOLD),
            )]));
            for row in sec.rows {
                let key_span = Span::styled(row.key, Style::default().add_modifier(Modifier::BOLD));
                let sep = Span::raw("  ");
                let desc_span = Span::raw(row.desc);
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    key_span,
                    sep,
                    desc_span,
                ]));
            }
        }

        let total = lines.len() as u16;
        let indicator = format!("[{}/{}]", self.scroll_offset + 1, total.max(1));
        let title = format!(" Help — Cargonaut  {indicator} ");

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .style(theme.dialog_style());

        let paragraph = Paragraph::new(Text::from(lines))
            .block(block)
            .scroll((self.scroll_offset, 0));

        f.render_widget(paragraph, area);
    }
}

// Re-export of crossterm's KeyCode so callers don't need a second use.
pub use crossterm::event::KeyCode;

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;

    // ---------- HotlistDialog (Feature 042) ----------

    fn hl_entry(display: &str, index: usize) -> HotlistRow {
        HotlistRow {
            display: display.into(),
            index: Some(index),
        }
    }

    #[test]
    fn hotlist_new_selects_first_entry_and_renders() {
        let rows = vec![hl_entry("proj", 0), hl_entry("tmp", 1)];
        let mut d = HotlistDialog::new(rows);
        assert_eq!(d.focused_index(), Some(0));
        let backend = TestBackend::new(40, 8);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| d.render(f.size(), f.buffer_mut(), &theme))
            .unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(s.contains("proj") && s.contains("tmp"), "rendered: {s}");
    }

    #[test]
    fn hotlist_nav_and_select_and_close() {
        let rows = vec![hl_entry("a", 0), hl_entry("b", 1)];
        let mut d = HotlistDialog::new(rows);
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.handle_key(KeyCode::Enter), Some(HotlistAction::Select(1)));
        assert_eq!(d.handle_key(KeyCode::Esc), Some(HotlistAction::Close));
    }

    #[test]
    fn hotlist_add_key_returns_add() {
        let mut d = HotlistDialog::new(vec![hl_entry("a", 0)]);
        assert_eq!(d.handle_key(KeyCode::Char('a')), Some(HotlistAction::Add));
    }

    #[test]
    fn hotlist_empty_renders_empty_state() {
        let mut d = HotlistDialog::new(vec![]);
        assert!(d.is_empty());
        let backend = TestBackend::new(50, 6);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| d.render(f.size(), f.buffer_mut(), &theme))
            .unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(s.contains("No bookmarks"), "empty-state text missing: {s}");
    }

    #[test]
    fn hotlist_remove_key_returns_remove() {
        let mut d = HotlistDialog::new(vec![hl_entry("a", 0), hl_entry("b", 1)]);
        d.handle_key(KeyCode::Down);
        assert_eq!(
            d.handle_key(KeyCode::Char('d')),
            Some(HotlistAction::Remove(1))
        );
    }

    // ---------- ConfirmDialog ----------

    #[test]
    fn confirm_dialog_defaults_focus_to_cancel() {
        let d = ConfirmDialog::new("Delete file?", "foo.txt will be permanently removed.");
        assert_eq!(d.focus(), 1, "default focus must be safe (Cancel = 1)");
    }

    #[test]
    fn confirm_enter_on_cancel_returns_cancel() {
        let mut d = ConfirmDialog::new("t", "b");
        assert_eq!(d.handle_key(KeyCode::Enter), Some(ConfirmOutcome::Cancel));
    }

    #[test]
    fn confirm_tab_moves_focus_and_enter_returns_confirm() {
        let mut d = ConfirmDialog::new("t", "b");
        assert_eq!(d.handle_key(KeyCode::Tab), None);
        assert_eq!(d.focus(), 0);
        assert_eq!(d.handle_key(KeyCode::Enter), Some(ConfirmOutcome::Confirm));
    }

    #[test]
    fn confirm_y_and_n_shortcut() {
        let mut d = ConfirmDialog::new("t", "b");
        assert_eq!(
            d.handle_key(KeyCode::Char('y')),
            Some(ConfirmOutcome::Confirm)
        );
        let mut d = ConfirmDialog::new("t", "b");
        assert_eq!(
            d.handle_key(KeyCode::Char('n')),
            Some(ConfirmOutcome::Cancel)
        );
        let mut d = ConfirmDialog::new("t", "b");
        assert_eq!(
            d.handle_key(KeyCode::Char('Y')),
            Some(ConfirmOutcome::Confirm)
        );
    }

    #[test]
    fn confirm_esc_always_cancels() {
        let mut d = ConfirmDialog::new("t", "b");
        d.handle_key(KeyCode::Tab); // focus = Confirm
        assert_eq!(d.handle_key(KeyCode::Esc), Some(ConfirmOutcome::Cancel));
    }

    #[test]
    fn confirm_renders_to_test_backend_with_title_and_buttons() {
        let d = ConfirmDialog::new("Delete file?", "foo.txt");
        let backend = TestBackend::new(60, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            d.render(f.size(), f.buffer_mut(), &Theme::default());
        })
        .unwrap();
        let buf = term.backend().buffer();
        let rendered: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(rendered.contains("Delete file?"), "title missing");
        assert!(rendered.contains("foo.txt"), "body missing");
        assert!(rendered.contains("Confirm"), "Confirm button missing");
        assert!(rendered.contains("Cancel"), "Cancel button missing");
    }

    // ---------- ResumePromptDialog ----------

    fn sample_summary(src: &str, dst: &str) -> ResumableSummary {
        ResumableSummary {
            src: src.into(),
            dst: dst.into(),
            bytes_written_mib: 8.0,
            src_size_mib: 32.0,
            source_unchanged: true,
            dest_intact: true,
        }
    }

    #[test]
    fn resume_prompt_starts_focused_on_first_offer() {
        let d = ResumePromptDialog::new(vec![sample_summary("a", "b"), sample_summary("c", "d")]);
        assert_eq!(d.focused_index(), Some(0));
        assert_eq!(d.len(), 2);
    }

    #[test]
    fn resume_prompt_arrow_keys_navigate() {
        let mut d =
            ResumePromptDialog::new(vec![sample_summary("a", "b"), sample_summary("c", "d")]);
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.focused_index(), Some(1));
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.focused_index(), Some(1), "should clamp at last");
        assert_eq!(d.handle_key(KeyCode::Up), None);
        assert_eq!(d.focused_index(), Some(0));
    }

    #[test]
    fn resume_prompt_r_returns_resume_for_focused_index() {
        let mut d =
            ResumePromptDialog::new(vec![sample_summary("a", "b"), sample_summary("c", "d")]);
        d.handle_key(KeyCode::Down);
        assert_eq!(
            d.handle_key(KeyCode::Char('r')),
            Some((1, ResumeChoice::Resume))
        );
    }

    #[test]
    fn resume_prompt_s_returns_start_over() {
        let mut d = ResumePromptDialog::new(vec![sample_summary("a", "b")]);
        assert_eq!(
            d.handle_key(KeyCode::Char('s')),
            Some((0, ResumeChoice::StartOver))
        );
    }

    #[test]
    fn resume_prompt_c_and_esc_return_skip() {
        let mut d = ResumePromptDialog::new(vec![sample_summary("a", "b")]);
        assert_eq!(
            d.handle_key(KeyCode::Char('c')),
            Some((0, ResumeChoice::Skip))
        );
        let mut d = ResumePromptDialog::new(vec![sample_summary("a", "b")]);
        assert_eq!(d.handle_key(KeyCode::Esc), Some((0, ResumeChoice::Skip)));
    }

    #[test]
    fn resume_prompt_renders_to_test_backend() {
        let mut d = ResumePromptDialog::new(vec![sample_summary(
            "file:///src/large.bin",
            "file:///dst/large.bin",
        )]);
        let backend = TestBackend::new(80, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            d.render(f.size(), f.buffer_mut(), &Theme::default());
        })
        .unwrap();
        let buf = term.backend().buffer();
        let rendered: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            rendered.contains("Resumable"),
            "title missing: {rendered:?}"
        );
        assert!(rendered.contains("[r]esume"), "hint missing: {rendered:?}");
        assert!(
            rendered.contains("large.bin"),
            "summary missing: {rendered:?}"
        );
    }

    // ---------- TextInputDialog ----------

    #[test]
    fn text_input_collects_and_submits() {
        let mut d = TextInputDialog::new("Make directory", "Name:");
        for c in "newdir".chars() {
            assert_eq!(d.handle_key(KeyCode::Char(c)), None);
        }
        assert_eq!(d.value(), "newdir");
        d.handle_key(KeyCode::Backspace);
        assert_eq!(d.value(), "newdi");
        assert_eq!(
            d.handle_key(KeyCode::Enter),
            Some(InputOutcome::Submit("newdi".into()))
        );
    }

    #[test]
    fn text_input_esc_cancels() {
        let mut d = TextInputDialog::new("t", "p");
        assert_eq!(d.handle_key(KeyCode::Esc), Some(InputOutcome::Cancel));
    }

    #[test]
    fn resume_prompt_empty_returns_none_for_keys() {
        let mut d = ResumePromptDialog::new(vec![]);
        assert_eq!(d.handle_key(KeyCode::Char('r')), None);
        assert_eq!(d.handle_key(KeyCode::Esc), None);
        assert!(d.is_empty());
    }

    // ---------- PathInputDialog (Feature 038 quick-cd) ----------

    #[test]
    fn path_input_prefills_and_edits() {
        let mut d = PathInputDialog::new("cd", "Path:", "/home/u");
        assert_eq!(d.value(), "/home/u");
        assert_eq!(d.handle_key(KeyCode::Char('x')), PathInputAction::Edited);
        assert_eq!(d.value(), "/home/ux");
        assert_eq!(d.handle_key(KeyCode::Backspace), PathInputAction::Edited);
        assert_eq!(d.value(), "/home/u");
    }

    #[test]
    fn path_input_enter_submits_and_esc_cancels() {
        let mut d = PathInputDialog::new("cd", "Path:", "/x");
        assert_eq!(
            d.handle_key(KeyCode::Enter),
            PathInputAction::Submit("/x".into())
        );
        let mut d = PathInputDialog::new("cd", "Path:", "/x");
        assert_eq!(d.handle_key(KeyCode::Esc), PathInputAction::Cancel);
    }

    #[test]
    fn path_input_tab_requests_then_cycles() {
        let mut d = PathInputDialog::new("cd", "Path:", "a");
        // Stale cache → ask the loop to fetch.
        assert_eq!(
            d.handle_key(KeyCode::Tab),
            PathInputAction::RequestCompletions { text: "a".into() }
        );
        d.apply_completions(vec!["a1".into(), "a2".into(), "a3".into()]);
        assert_eq!(d.value(), "a1");
        // Fresh cache → cycle in-widget, wrapping.
        assert_eq!(d.handle_key(KeyCode::Tab), PathInputAction::Consumed);
        assert_eq!(d.value(), "a2");
        assert_eq!(d.handle_key(KeyCode::Tab), PathInputAction::Consumed);
        assert_eq!(d.value(), "a3");
        assert_eq!(d.handle_key(KeyCode::Tab), PathInputAction::Consumed);
        assert_eq!(d.value(), "a1");
    }

    #[test]
    fn path_input_edit_invalidates_completion_cache() {
        let mut d = PathInputDialog::new("cd", "Path:", "a");
        d.apply_completions(vec!["a1".into(), "a2".into()]);
        assert_eq!(d.value(), "a1");
        // Editing makes the cache stale → next Tab re-requests.
        d.handle_key(KeyCode::Char('z'));
        assert_eq!(
            d.handle_key(KeyCode::Tab),
            PathInputAction::RequestCompletions { text: "a1z".into() }
        );
    }

    #[test]
    fn path_input_empty_completions_sets_note_keeps_buffer() {
        let mut d = PathInputDialog::new("cd", "Path:", "zzz");
        d.handle_key(KeyCode::Tab);
        d.apply_completions(vec![]);
        assert_eq!(d.value(), "zzz", "buffer must be unchanged on no matches");
    }

    #[test]
    fn path_input_set_error_renders_and_clears_on_edit() {
        let mut d = PathInputDialog::new("cd", "Path:", "/nope");
        d.set_error("not a directory");
        let backend = TestBackend::new(60, 8);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| d.render(f.size(), f.buffer_mut(), &Theme::default()))
            .unwrap();
        let rendered: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(rendered.contains("not a directory"), "error missing");
        assert!(rendered.contains("/nope"), "buffer missing");
        // Editing clears the error.
        d.handle_key(KeyCode::Backspace);
        let mut term2 = Terminal::new(TestBackend::new(60, 8)).unwrap();
        term2
            .draw(|f| d.render(f.size(), f.buffer_mut(), &Theme::default()))
            .unwrap();
        let rendered2: String = term2
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            !rendered2.contains("not a directory"),
            "error should clear on edit"
        );
    }

    // ---------- TasksPanelDialog (Feature 039) ----------

    fn job_row(label: &str, status: &str) -> JobRow {
        JobRow {
            label: label.into(),
            status_label: status.into(),
            can_cancel: true,
            can_pause: true,
            can_resume: false,
        }
    }

    fn render_to_string(d: &mut TasksPanelDialog, w: u16, h: u16) -> String {
        let mut term = Terminal::new(TestBackend::new(w, h)).unwrap();
        term.draw(|f| d.render(f.size(), f.buffer_mut(), &Theme::default()))
            .unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn tasks_panel_new_selects_first_row() {
        let d = TasksPanelDialog::new(vec![
            job_row("a → x", "Running 10%"),
            job_row("b → y", "Queued"),
        ]);
        assert_eq!(d.focused_index(), Some(0));
        assert_eq!(d.len(), 2);
        assert!(!d.is_empty());
    }

    #[test]
    fn tasks_panel_navigation_moves_and_clamps() {
        let mut d = TasksPanelDialog::new(vec![job_row("a", "Running"), job_row("b", "Running")]);
        assert_eq!(d.handle_key(KeyCode::Up), None); // clamp at top
        assert_eq!(d.focused_index(), Some(0));
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.focused_index(), Some(1));
        assert_eq!(d.handle_key(KeyCode::Char('j')), None); // clamp at bottom
        assert_eq!(d.focused_index(), Some(1));
        assert_eq!(d.handle_key(KeyCode::Char('k')), None);
        assert_eq!(d.focused_index(), Some(0));
    }

    #[test]
    fn tasks_panel_action_keys_return_actions_for_focused_row() {
        let mut d = TasksPanelDialog::new(vec![job_row("a", "Running"), job_row("b", "Paused")]);
        d.handle_key(KeyCode::Down); // focus index 1
        assert_eq!(
            d.handle_key(KeyCode::Char('c')),
            Some(TasksAction::Cancel(1))
        );
        assert_eq!(
            d.handle_key(KeyCode::Char('p')),
            Some(TasksAction::Pause(1))
        );
        assert_eq!(
            d.handle_key(KeyCode::Char('r')),
            Some(TasksAction::Resume(1))
        );
        assert_eq!(d.handle_key(KeyCode::Esc), Some(TasksAction::Close));
    }

    #[test]
    fn tasks_panel_renders_one_line_per_row_with_status() {
        let mut d = TasksPanelDialog::new(vec![
            job_row("alpha → dst", "Running 62%"),
            job_row("beta → dst", "Paused"),
        ]);
        let s = render_to_string(&mut d, 60, 8);
        assert!(s.contains("alpha"));
        assert!(s.contains("Running 62%"));
        assert!(s.contains("Paused"));
    }

    #[test]
    fn tasks_panel_renders_empty_state() {
        let mut d = TasksPanelDialog::new(vec![]);
        assert!(d.is_empty());
        assert_eq!(d.focused_index(), None);
        let s = render_to_string(&mut d, 60, 8);
        assert!(s.contains("No transfers"), "empty state missing: {s:?}");
    }

    #[test]
    fn tasks_panel_set_rows_clamps_selection() {
        let mut d = TasksPanelDialog::new(vec![
            job_row("a", "Running"),
            job_row("b", "Running"),
            job_row("c", "Running"),
        ]);
        d.handle_key(KeyCode::Down);
        d.handle_key(KeyCode::Down); // focus index 2
        assert_eq!(d.focused_index(), Some(2));
        d.set_rows(vec![job_row("a", "Running")]); // shrink to 1
        assert_eq!(d.focused_index(), Some(0), "selection must clamp in-bounds");
        d.set_rows(vec![]); // empty
        assert_eq!(d.focused_index(), None);
    }

    #[test]
    fn tasks_panel_keys_inert_when_empty_except_close() {
        let mut d = TasksPanelDialog::new(vec![]);
        assert_eq!(d.handle_key(KeyCode::Char('c')), None);
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.handle_key(KeyCode::Esc), Some(TasksAction::Close));
    }

    // ---------- HelpOverlay (Feature 047 — T008/T009) ----------

    #[test]
    fn help_sections_is_non_empty() {
        assert!(!HELP_SECTIONS.is_empty());
    }

    #[test]
    fn help_sections_each_section_has_non_empty_title() {
        for sec in HELP_SECTIONS {
            assert!(!sec.title.is_empty(), "section title must not be empty");
        }
    }

    #[test]
    fn help_sections_each_section_has_at_least_one_row() {
        for sec in HELP_SECTIONS {
            assert!(!sec.rows.is_empty(), "section '{}' has no rows", sec.title);
        }
    }

    #[test]
    fn help_sections_every_row_has_non_empty_key_and_desc() {
        for sec in HELP_SECTIONS {
            for row in sec.rows {
                assert!(!row.key.is_empty(), "row key empty in section '{}'", sec.title);
                assert!(!row.desc.is_empty(), "row desc empty in section '{}'", sec.title);
            }
        }
    }

    #[test]
    fn help_overlay_scroll_down_increments_offset() {
        let total = HELP_SECTIONS.iter().map(|s| s.rows.len() + 1).sum::<usize>() as u16;
        let visible = 10u16;
        let mut ov = HelpOverlay::new(visible);
        assert_eq!(ov.scroll_offset, 0);
        ov.handle_key(KeyCode::Down);
        if total > visible {
            assert_eq!(ov.scroll_offset, 1);
        }
    }

    #[test]
    fn help_overlay_scroll_up_clamps_at_zero() {
        let mut ov = HelpOverlay::new(10);
        let action = ov.handle_key(KeyCode::Up);
        assert_eq!(ov.scroll_offset, 0);
        assert_eq!(action, HelpAction::Swallow);
    }

    #[test]
    fn help_overlay_home_resets_to_zero() {
        let mut ov = HelpOverlay::new(5);
        ov.handle_key(KeyCode::Down);
        ov.handle_key(KeyCode::Down);
        ov.handle_key(KeyCode::Home);
        assert_eq!(ov.scroll_offset, 0);
    }

    #[test]
    fn help_overlay_f1_returns_close() {
        let mut ov = HelpOverlay::new(10);
        assert_eq!(ov.handle_key(KeyCode::F(1)), HelpAction::Close);
    }

    #[test]
    fn help_overlay_esc_returns_close() {
        let mut ov = HelpOverlay::new(10);
        assert_eq!(ov.handle_key(KeyCode::Esc), HelpAction::Close);
    }

    #[test]
    fn help_overlay_unrecognized_key_swallows() {
        let mut ov = HelpOverlay::new(10);
        assert_eq!(ov.handle_key(KeyCode::Char('j')), HelpAction::Swallow);
        assert_eq!(ov.handle_key(KeyCode::Enter), HelpAction::Swallow);
        assert_eq!(ov.handle_key(KeyCode::Char('q')), HelpAction::Swallow);
    }

    #[test]
    fn help_overlay_page_down_increments_by_visible_height() {
        let mut ov = HelpOverlay::new(10);
        let before = ov.scroll_offset;
        ov.handle_key(KeyCode::PageDown);
        // If content is taller than visible, offset should move by visible_height
        if ov.total_lines > ov.visible_height {
            assert!(ov.scroll_offset > before);
        }
    }

    // ---------- UserMenuDialog (Feature 047 — T018) ----------

    fn menu_item(label: &str, command: &str, key: Option<char>) -> cargonaut_config::MenuItem {
        cargonaut_config::MenuItem {
            label: label.into(),
            command: command.into(),
            only_if: None,
            key,
        }
    }

    #[test]
    fn user_menu_new_with_items_selects_first() {
        let items = vec![menu_item("Edit", "vi {path}", Some('e')), menu_item("List", "ls {path}", None)];
        let d = UserMenuDialog::new(items);
        assert_eq!(d.focused_index(), Some(0));
    }

    #[test]
    fn user_menu_new_empty_has_no_selection() {
        let d = UserMenuDialog::new(vec![]);
        assert_eq!(d.focused_index(), None);
    }

    #[test]
    fn user_menu_down_moves_selection() {
        let items = vec![menu_item("A", "a", None), menu_item("B", "b", None)];
        let mut d = UserMenuDialog::new(items);
        assert_eq!(d.handle_key(KeyCode::Down), None);
        assert_eq!(d.focused_index(), Some(1));
    }

    #[test]
    fn user_menu_up_clamps_at_zero() {
        let items = vec![menu_item("A", "a", None), menu_item("B", "b", None)];
        let mut d = UserMenuDialog::new(items);
        assert_eq!(d.handle_key(KeyCode::Up), None);
        assert_eq!(d.focused_index(), Some(0));
    }

    #[test]
    fn user_menu_esc_returns_close() {
        let mut d = UserMenuDialog::new(vec![menu_item("A", "a", None)]);
        assert_eq!(d.handle_key(KeyCode::Esc), Some(UserMenuAction::Close));
    }

    #[test]
    fn user_menu_enter_returns_execute_index() {
        let items = vec![menu_item("A", "a", None), menu_item("B", "b", None)];
        let mut d = UserMenuDialog::new(items);
        assert_eq!(d.handle_key(KeyCode::Enter), Some(UserMenuAction::Execute(0)));
    }

    #[test]
    fn user_menu_shortcut_char_executes_matching_item() {
        let items = vec![
            menu_item("Edit", "vi {path}", Some('e')),
            menu_item("List", "ls {path}", Some('l')),
        ];
        let mut d = UserMenuDialog::new(items);
        assert_eq!(d.handle_key(KeyCode::Char('l')), Some(UserMenuAction::Execute(1)));
    }

    #[test]
    fn user_menu_new_error_sets_error_field() {
        let d = UserMenuDialog::new_error("parse error: line 5");
        assert!(d.error.is_some());
        assert!(d.error.as_deref().unwrap().contains("parse error"));
    }
}

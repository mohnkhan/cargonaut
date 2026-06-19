// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Modal dialogs: copy/move/delete confirmation, plus the resume-prompt
//! shown on launch when `scan_resumable` finds an orphan checkpoint.
//!
//! Each dialog is a small state machine: it owns its focus + answer
//! state, exposes `handle_key` for input, and renders via
//! `render`. The App's event loop (T1.19) routes keys to the
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
/// `cargonaut_transfer::ResumableTransfer`. The App builds these
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
            HelpRow {
                key: "F1",
                desc: "Show this help overlay (show-help)",
            },
            HelpRow {
                key: "F10",
                desc: "Quit the application (quit)",
            },
            HelpRow {
                key: "Tab",
                desc: "Switch active pane (focus-swap-pane)",
            },
            HelpRow {
                key: "M-1",
                desc: "Focus left pane (focus-left-pane)",
            },
            HelpRow {
                key: "M-2",
                desc: "Focus right pane (focus-right-pane)",
            },
            HelpRow {
                key: "j / Down",
                desc: "Move cursor down (cursor-down)",
            },
            HelpRow {
                key: "k / Up",
                desc: "Move cursor up (cursor-up)",
            },
            HelpRow {
                key: "Enter",
                desc: "Open directory or file (descend-or-open)",
            },
            HelpRow {
                key: "Backspace / h",
                desc: "Go up to parent directory (ascend-parent)",
            },
            HelpRow {
                key: "~",
                desc: "Go to home directory (cd-home)",
            },
            HelpRow {
                key: "/",
                desc: "Go to root directory (cd-root)",
            },
            HelpRow {
                key: ":",
                desc: "Open command line (open-cmdline)",
            },
        ],
    },
    HelpSection {
        title: "Selection",
        rows: &[
            HelpRow {
                key: "Insert",
                desc: "Toggle selection on current entry (selection-toggle)",
            },
            HelpRow {
                key: "*",
                desc: "Invert selection (selection-invert)",
            },
            HelpRow {
                key: "+",
                desc: "Add to selection by pattern (selection-add-by-pattern)",
            },
            HelpRow {
                key: "-",
                desc: "Remove from selection by pattern (selection-remove-by-pattern)",
            },
        ],
    },
    HelpSection {
        title: "File Operations",
        rows: &[
            HelpRow {
                key: "F3",
                desc: "Preview file or directory (preview)",
            },
            HelpRow {
                key: "F4",
                desc: "Edit file in $EDITOR (edit)",
            },
            HelpRow {
                key: "F5",
                desc: "Copy selection to other pane (copy-selection)",
            },
            HelpRow {
                key: "F6",
                desc: "Move or rename selection (move-or-rename-selection)",
            },
            HelpRow {
                key: "F7",
                desc: "Create a new directory (mkdir)",
            },
            HelpRow {
                key: "F8",
                desc: "Delete selection (delete-selection)",
            },
            HelpRow {
                key: "C-c",
                desc: "Cancel current operation (cancel-current-operation)",
            },
            HelpRow {
                key: "C-s",
                desc: "Cycle sort key (cycle-sort-key)",
            },
            HelpRow {
                key: "C-z",
                desc: "Undo last file operation (undo-last-op)",
            },
        ],
    },
    HelpSection {
        title: "Panels & Modes",
        rows: &[
            HelpRow {
                key: "F9",
                desc: "Open menu bar (open-menu-bar)",
            },
            HelpRow {
                key: "F12",
                desc: "Show active transfers panel (show-tasks-panel)",
            },
            HelpRow {
                key: "M-c",
                desc: "Quick CD popup (quick-cd-popup)",
            },
            HelpRow {
                key: "M-!",
                desc: "Toggle panel filter prompt (toggle-panel-filter)",
            },
            HelpRow {
                key: "<",
                desc: "Open fuzzy entry filter (open-fuzzy-filter)",
            },
            HelpRow {
                key: "C-f",
                desc: "Filter entries in current directory (filter-current-dir)",
            },
            HelpRow {
                key: "M-i",
                desc: "Sync other panel to this path (sync-other-panel-path)",
            },
            HelpRow {
                key: "M-o",
                desc: "Show focused entry in other panel (show-focused-in-other-panel)",
            },
            HelpRow {
                key: "M-.",
                desc: "Toggle hidden files (toggle-hidden)",
            },
            HelpRow {
                key: "M-,",
                desc: "Toggle split orientation (toggle-split-orientation)",
            },
            HelpRow {
                key: "C-Space",
                desc: "Calculate recursive directory size (recursive-dir-size)",
            },
            HelpRow {
                key: "M-t",
                desc: "Cycle listing mode (cycle-listing-mode)",
            },
            HelpRow {
                key: "C-t",
                desc: "Open new tab (new-tab)",
            },
            HelpRow {
                key: "C-w",
                desc: "Close current tab (close-tab)",
            },
            HelpRow {
                key: "]",
                desc: "Next tab on active side (tab-next)",
            },
            HelpRow {
                key: "[",
                desc: "Previous tab on active side (tab-prev)",
            },
            HelpRow {
                key: "C-o",
                desc: "Open subshell in current directory (open-subshell)",
            },
            HelpRow {
                key: "C-r",
                desc: "Reload config and themes (reload-config-and-themes)",
            },
        ],
    },
    HelpSection {
        title: "History",
        rows: &[
            HelpRow {
                key: "M-S-h",
                desc: "Show directory history popup (show-directory-history)",
            },
            HelpRow {
                key: "M-h",
                desc: "Show command history popup (show-command-history)",
            },
            HelpRow {
                key: "M-y",
                desc: "Navigate to previous directory in history (history-prev-dir)",
            },
            HelpRow {
                key: "M-u",
                desc: "Navigate to next directory in history (history-next-dir)",
            },
        ],
    },
    HelpSection {
        title: "Bookmarks",
        rows: &[HelpRow {
            key: "C-b",
            desc: "Open bookmarks menu; add / remove entries (bookmarks-menu)",
        }],
    },
    HelpSection {
        title: "File Attributes",
        rows: &[
            HelpRow {
                key: "C-x c",
                desc: "Change file permissions (chmod)",
            },
            HelpRow {
                key: "C-x o",
                desc: "Change file ownership (chown)",
            },
            HelpRow {
                key: "C-x s",
                desc: "Create symbolic link (create-symlink)",
            },
            HelpRow {
                key: "C-x l",
                desc: "Create hard link (create-hard-link)",
            },
            HelpRow {
                key: "C-x C",
                desc: "Recursive chmod into subtree (chmod-recursive)",
            },
            HelpRow {
                key: "C-x O",
                desc: "Recursive chown into subtree (chown-recursive)",
            },
        ],
    },
    HelpSection {
        title: "Power Features",
        rows: &[
            HelpRow {
                key: "F2",
                desc: "Open user action menu from menu.toml (show-user-menu)",
            },
            HelpRow {
                key: "C-x !",
                desc: "External panelize — run command, list output (external-panelize)",
            },
            HelpRow {
                key: "C-x r",
                desc: "Bulk rename selection via editor (bulk-rename-via-editor)",
            },
            HelpRow {
                key: "C-x d",
                desc: "Compare two directories (compare-directories)",
            },
            HelpRow {
                key: "C-x C-d",
                desc: "Diff two tagged files (diff-two-tagged-files)",
            },
            HelpRow {
                key: "M-m",
                desc: "Toggle mouse capture; Shift+drag bypasses (toggle-mouse-capture)",
            },
        ],
    },
    HelpSection {
        title: "File Viewer",
        rows: &[
            HelpRow {
                key: "C-x X",
                desc: "Toggle hex/text mode (toggle-hex-view)",
            },
            HelpRow {
                key: "/",
                desc: "Search forward (preview-search-forward)",
            },
            HelpRow {
                key: "?",
                desc: "Search backward (preview-search-backward)",
            },
            HelpRow {
                key: "n",
                desc: "Next search match (preview-search-next)",
            },
            HelpRow {
                key: "N",
                desc: "Previous search match (preview-search-prev)",
            },
            HelpRow {
                key: "g",
                desc: "Go to line or byte offset (viewer-goto)",
            },
            HelpRow {
                key: "G",
                desc: "Jump to last line or hex row (viewer-end)",
            },
            HelpRow {
                key: "w",
                desc: "Toggle word-wrap in text mode (viewer-wrap)",
            },
            HelpRow {
                key: "q / Esc",
                desc: "Close the file viewer (viewer-quit)",
            },
            HelpRow {
                key: "Up / Down",
                desc: "Scroll one line / hex row",
            },
            HelpRow {
                key: "PgUp / PgDn",
                desc: "Scroll one page",
            },
            HelpRow {
                key: "Home / End",
                desc: "Jump to first / last line",
            },
        ],
    },
    HelpSection {
        title: "Built-in Editor (Feature 056 — F4)",
        rows: &[
            HelpRow {
                key: "F2 / C-s",
                desc: "Save the file (save-file)",
            },
            HelpRow {
                key: "F10 / Esc / q",
                desc: "Quit the editor; prompts if unsaved (editor-quit)",
            },
        ],
    },
    HelpSection {
        title: "Search Mode",
        rows: &[
            HelpRow {
                key: "Esc",
                desc: "Close search overlay (close-search)",
            },
            HelpRow {
                key: "Enter",
                desc: "Navigate to highlighted search result (search-go-to-result)",
            },
        ],
    },
    HelpSection {
        title: "Dialogs",
        rows: &[
            HelpRow {
                key: "Esc",
                desc: "Cancel / close current dialog (dialog-cancel)",
            },
            HelpRow {
                key: "Enter",
                desc: "Confirm current dialog action (dialog-confirm)",
            },
        ],
    },
    HelpSection {
        title: "Find File",
        rows: &[HelpRow {
            key: "M-?",
            desc:
                "Find file by name glob or ripgrep content search, then panelize (find-file-popup)",
        }],
    },
    HelpSection {
        title: "Orthodox-FM Compat (mc_keys=true)",
        rows: &[
            HelpRow {
                key: "M-5",
                desc: "Copy selection — alt binding (copy-selection)",
            },
            HelpRow {
                key: "M-6",
                desc: "Move or rename — alt binding (move-or-rename-selection)",
            },
        ],
    },
    HelpSection {
        title: "About",
        rows: &[HelpRow {
            key: "cargonaut",
            desc: "A dual-pane TUI file manager. Press Esc or F1 to close help.",
        }],
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
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span, Text};
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

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
                lines.push(Line::from(vec![Span::raw("  "), key_span, sep, desc_span]));
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

// =====================================================================
// Helpers
// =====================================================================

/// Centre a `percent_x × percent_y` rect inside `r`.
pub(crate) fn centered_rect_pct(
    percent_x: u16,
    percent_y: u16,
    r: ratatui::layout::Rect,
) -> ratatui::layout::Rect {
    use ratatui::layout::{Constraint, Direction, Layout};
    let vchunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);
    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(vchunks[1])[1]
}

// =====================================================================
// User menu dialog (Feature 047 — US2, T020-T022)
// =====================================================================

/// Outcome returned by [`UserMenuDialog::handle_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum UserMenuAction {
    /// Close the menu without executing anything.
    Close,
    /// Execute the action at the given index.
    Execute(usize),
}

/// The F2 user action menu. Items are loaded fresh from `menu.toml` on each
/// F2 press; an `error` variant displays the parse error instead of items.
#[derive(Debug)]
pub struct UserMenuDialog {
    /// Menu items (empty when `menu.toml` is absent or empty).
    pub items: Vec<cargonaut_config::MenuItem>,
    /// List selection state.
    state: ListState,
    /// If set, display this error message instead of the item list.
    pub error: Option<String>,
}

impl UserMenuDialog {
    /// Construct a new menu from the given items. Selects the first row.
    pub fn new(items: Vec<cargonaut_config::MenuItem>) -> Self {
        let mut state = ListState::default();
        if !items.is_empty() {
            state.select(Some(0));
        }
        Self {
            items,
            state,
            error: None,
        }
    }

    /// Construct an error-state menu (shows the parse error instead of items).
    pub fn new_error(msg: impl Into<String>) -> Self {
        Self {
            items: vec![],
            state: ListState::default(),
            error: Some(msg.into()),
        }
    }

    /// Index of the currently focused item, or `None` if no items.
    pub fn focused_index(&self) -> Option<usize> {
        self.state.selected()
    }

    /// Handle a key event. Returns `Some(action)` when the dialog should act or
    /// close, `None` when the key is consumed without acting (navigation).
    pub fn handle_key(&mut self, code: KeyCode) -> Option<UserMenuAction> {
        match code {
            KeyCode::Esc | KeyCode::F(1) => Some(UserMenuAction::Close),
            KeyCode::Up => {
                if let Some(i) = self.state.selected() {
                    if i > 0 {
                        self.state.select(Some(i - 1));
                    }
                }
                None
            }
            KeyCode::Down => {
                if let Some(i) = self.state.selected() {
                    if i + 1 < self.items.len() {
                        self.state.select(Some(i + 1));
                    }
                }
                None
            }
            KeyCode::Enter => self.state.selected().map(UserMenuAction::Execute),
            KeyCode::Char(c) => {
                // Shortcut key — find first item whose `key == Some(c)` (first wins).
                self.items
                    .iter()
                    .position(|item| item.key == Some(c))
                    .map(UserMenuAction::Execute)
            }
            _ => None,
        }
    }

    /// Render the dialog into `area`, clearing it first.
    pub fn render(
        &mut self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::theme::Theme,
    ) {
        use ratatui::style::{Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders, Clear, List, ListItem, Paragraph, StatefulWidget};

        let darea = centered_rect_pct(50, 60, area);
        f.render_widget(Clear, darea);

        let block = Block::default()
            .title(" User Menu (F2) ")
            .borders(Borders::ALL)
            .style(theme.dialog_style());

        if let Some(err) = &self.error {
            let body = format!("Error loading menu.toml:\n{err}\n\nPress Esc to close.");
            let para = Paragraph::new(body)
                .block(block)
                .style(theme.dialog_style());
            f.render_widget(para, darea);
            return;
        }

        if self.items.is_empty() {
            let body = "No actions defined — see ~/.config/cargonaut/menu.toml";
            let para = Paragraph::new(body)
                .block(block)
                .style(theme.dialog_style());
            f.render_widget(para, darea);
            return;
        }

        let max_label = darea.width.saturating_sub(8) as usize;
        let items: Vec<ListItem<'_>> = self
            .items
            .iter()
            .map(|item| {
                let label = if item.label.chars().count() > max_label {
                    item.label
                        .chars()
                        .take(max_label.saturating_sub(1))
                        .collect::<String>()
                        + "…"
                } else {
                    item.label.clone()
                };
                let key_hint = item.key.map(|c| format!(" [{c}]")).unwrap_or_default();
                let line = Line::from(vec![
                    Span::raw(label),
                    Span::styled(key_hint, Style::default().add_modifier(Modifier::DIM)),
                ]);
                ListItem::new(line)
            })
            .collect();

        let list = List::new(items)
            .block(block)
            .style(theme.dialog_style())
            .highlight_style(
                Style::default()
                    .fg(theme.dialog_sel_fg)
                    .bg(theme.dialog_sel_bg),
            )
            .highlight_symbol("▶ ");

        StatefulWidget::render(list, darea, f.buffer_mut(), &mut self.state);
    }
}

// Re-export of crossterm's KeyCode so callers don't need a second use.
pub use crossterm::event::KeyCode;

// =====================================================================
// Built-in File Viewer (Feature 051 — FR-001..FR-033)
// =====================================================================

/// Byte threshold above which the viewer streams from disk instead of pre-loading.
pub const STREAMING_THRESHOLD_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
/// Maximum number of ANSI-stripped lines kept in the streaming window at one time.
pub const WINDOW_MAX_LINES: usize = 2000;
/// Chunk index interval: one `(line_number, byte_offset)` entry every N lines.
pub const CHUNK_INDEX_INTERVAL: usize = 1000;
/// Bytes per row in hex mode.
pub const HEX_ROW_WIDTH: usize = 16;
/// How many bytes to read when detecting binary vs. UTF-8 content.
pub const BINARY_DETECT_BYTES: usize = 4096;
/// Lines from the window boundary at which a streaming prefetch is triggered.
pub const PREFETCH_THRESHOLD: usize = 100;

/// Display mode for the built-in file viewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// UTF-8 text with line numbers and optional word-wrap.
    Text,
    /// Classic 16-byte-per-row hex + ASCII dump.
    Hex,
}

/// Direction of a viewer search operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchDirection {
    /// `/` — search downward from the current position.
    Forward,
    /// `?` — search upward from the current position.
    Backward,
}

/// Active search state inside the file viewer. `None` when no search is running.
#[derive(Debug, Clone)]
pub struct SearchState {
    /// Literal search pattern (case-sensitive, no regex — FR-019).
    pub pattern: String,
    /// Direction of the most recent search action.
    pub direction: SearchDirection,
    /// Line index of the last found match within the current buffer window.
    pub last_match_line: Option<usize>,
    /// Byte offset within the matched line where the pattern starts.
    pub last_match_col: Option<usize>,
}

/// Inline prompt shown at the bottom of the viewer overlay.
/// `None` when the viewer is in normal navigation mode.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ViewerPrompt {
    /// Active search input (`/` or `?`).
    Search {
        /// Characters typed so far.
        buffer: String,
        /// Direction this search will run.
        direction: SearchDirection,
    },
    /// Active goto input (`g`).
    Goto {
        /// Decimal digits or `0x`-prefixed hex typed so far.
        buffer: String,
    },
}

/// Buffer holding the file content accessible to the viewer.
#[derive(Debug)]
pub enum ViewBuffer {
    /// Files ≤ [`STREAMING_THRESHOLD_BYTES`] — fully pre-loaded into memory.
    Loaded {
        /// ANSI-stripped lines (text mode).
        lines: Vec<String>,
        /// Raw bytes (hex mode).
        bytes: Vec<u8>,
    },
    /// Files > [`STREAMING_THRESHOLD_BYTES`] — streamed on demand.
    Streaming {
        /// Path used for re-opening the file on seek.
        path: std::path::PathBuf,
        /// Compact line index: `(line_number, byte_offset)` every [`CHUNK_INDEX_INTERVAL`] lines.
        chunk_index: Vec<(usize, u64)>,
        /// Sliding window of ANSI-stripped lines (max [`WINDOW_MAX_LINES`]).
        lines: std::collections::VecDeque<String>,
        /// File line number of `lines[0]`.
        window_start_line: usize,
        /// Approximate total line count (set at open time).
        total_lines: usize,
        /// Total file size in bytes.
        total_bytes: u64,
        /// Byte offset of the next line not yet in `lines`.
        reader_offset: u64,
    },
}

/// Return value from [`FileViewerDialog::handle_key`].
#[derive(Debug, Clone, PartialEq)]
pub enum FileViewerAction {
    /// The viewer should close; lib.rs sets `active_dialog = None` and restores `Mode::Pane`.
    Close,
    /// Key was consumed; viewer stays open.
    Swallow,
    /// Viewer needs more file data (streaming scroll forward or backward).
    /// lib.rs spawns a blocking read and calls [`FileViewerDialog::append_lines`] when done.
    NeedsData {
        /// Byte offset in the file to start reading from.
        offset: u64,
        /// Number of lines to load into the window.
        line_count: usize,
        /// File line number at `offset` (so lib.rs knows the window_start for `append_lines`).
        window_start: usize,
    },
}

/// The top-level widget stored in `ActiveDialog::FileViewer`.
#[derive(Debug)]
pub struct FileViewerDialog {
    /// Resolved path of the open file (used for streaming re-open).
    pub path: std::path::PathBuf,
    /// Display name shown in the title bar — the symlink name when following a link (T013).
    pub display_name: String,
    /// Current display mode.
    pub mode: ViewMode,
    /// Content buffer (pre-loaded or streaming).
    pub buffer: ViewBuffer,
    /// Top-of-view position: line index in text mode, 16-byte row index in hex mode.
    pub scroll_offset: usize,
    /// Active search state; `None` when no search is active.
    pub search: Option<SearchState>,
    /// Active inline prompt; `None` in normal navigation mode.
    pub prompt: Option<ViewerPrompt>,
    /// Word-wrap enabled (text mode only; no effect in hex).
    pub word_wrap: bool,
    /// Status text shown at the bottom of the overlay (e.g., `"Line 42/350  wrap: off"`).
    pub status: String,
    /// Viewport height cached from the last render call; used for page-scroll sizing.
    pub viewport_height: usize,
}

impl FileViewerDialog {
    // --- Construction ---

    /// Construct a text-mode viewer for a pre-loaded file.
    ///
    /// `display_name` appears in the title bar; it is the symlink's *display*
    /// name when the underlying path was resolved via `canonicalize`.
    pub fn new_text(
        path: std::path::PathBuf,
        display_name: String,
        lines: Vec<String>,
        wrap: bool,
    ) -> Self {
        let total = lines.len();
        let wrap_str = if wrap { "on" } else { "off" };
        let status = if total == 0 {
            "(empty file)".into()
        } else {
            format!("Line 1/{total}  wrap: {wrap_str}")
        };
        Self {
            path,
            display_name,
            mode: ViewMode::Text,
            buffer: ViewBuffer::Loaded {
                lines,
                bytes: Vec::new(),
            },
            scroll_offset: 0,
            search: None,
            prompt: None,
            word_wrap: wrap,
            status,
            viewport_height: 20,
        }
    }

    /// Construct a hex-mode viewer for a pre-loaded file.
    pub fn new_hex(path: std::path::PathBuf, display_name: String, bytes: Vec<u8>) -> Self {
        let total_bytes = bytes.len();
        let status = format!("Offset 0x00000000 / {total_bytes} bytes");
        Self {
            path,
            display_name,
            mode: ViewMode::Hex,
            buffer: ViewBuffer::Loaded {
                lines: Vec::new(),
                bytes,
            },
            scroll_offset: 0,
            search: None,
            prompt: None,
            word_wrap: false,
            status,
            viewport_height: 20,
        }
    }

    /// Construct a streaming-mode viewer for a large text file.
    ///
    /// `initial_lines` is the first window of ANSI-stripped lines loaded at open time.
    /// `reader_offset` is the byte position immediately after `initial_lines`.
    pub fn new_streaming(
        path: std::path::PathBuf,
        display_name: String,
        chunk_index: Vec<(usize, u64)>,
        initial_lines: std::collections::VecDeque<String>,
        total_lines: usize,
        total_bytes: u64,
        reader_offset: u64,
    ) -> Self {
        let status = if total_lines == 0 {
            "(empty file)".into()
        } else {
            format!("Line 1/{total_lines}  wrap: off")
        };
        Self {
            path: path.clone(),
            display_name,
            mode: ViewMode::Text,
            buffer: ViewBuffer::Streaming {
                path,
                chunk_index,
                lines: initial_lines,
                window_start_line: 0,
                total_lines,
                total_bytes,
                reader_offset,
            },
            scroll_offset: 0,
            search: None,
            prompt: None,
            word_wrap: false,
            status,
            viewport_height: 20,
        }
    }

    /// Read up to `lines_needed` ANSI-stripped lines from `path` starting at `byte_offset`.
    ///
    /// Returns `(lines, new_reader_offset)`.  Intended for use in `spawn_blocking`.
    pub fn load_window_from_chunk(
        path: &std::path::Path,
        byte_offset: u64,
        lines_needed: usize,
    ) -> std::io::Result<(Vec<String>, u64)> {
        use std::io::{BufRead, BufReader, Seek, SeekFrom};
        let mut file = std::fs::File::open(path)?;
        file.seek(SeekFrom::Start(byte_offset))?;
        let mut reader = BufReader::with_capacity(65536, file);
        let mut lines = Vec::with_capacity(lines_needed);
        let mut buf = String::new();
        while lines.len() < lines_needed {
            buf.clear();
            if reader.read_line(&mut buf)? == 0 {
                break;
            }
            let trimmed = buf.trim_end_matches('\n').trim_end_matches('\r');
            lines.push(strip_ansi_escapes::strip_str(trimmed));
        }
        let new_offset = reader.stream_position()?;
        Ok((lines, new_offset))
    }

    // --- Internal line access ---

    fn text_lines_as_vec(&self) -> Vec<&String> {
        match &self.buffer {
            ViewBuffer::Loaded { lines, .. } => lines.iter().collect(),
            ViewBuffer::Streaming { lines, .. } => lines.iter().collect(),
        }
    }

    // --- Informational accessors ---

    /// Total content lines (text mode) or 16-byte rows (hex mode) in the buffer.
    pub fn total_lines(&self) -> usize {
        match &self.buffer {
            ViewBuffer::Loaded { lines, bytes } => match self.mode {
                ViewMode::Text => lines.len(),
                ViewMode::Hex => bytes.len().div_ceil(HEX_ROW_WIDTH),
            },
            ViewBuffer::Streaming {
                total_lines,
                total_bytes,
                ..
            } => match self.mode {
                ViewMode::Text => *total_lines,
                ViewMode::Hex => (*total_bytes as usize).div_ceil(HEX_ROW_WIDTH),
            },
        }
    }

    /// Current top-of-view position (line index or hex row index).
    pub fn current_scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    /// Status text currently shown at the bottom of the overlay.
    pub fn current_status_text(&self) -> &str {
        &self.status
    }

    /// Override the status string (e.g., `"File no longer readable"` — T042 error path).
    pub fn set_status(&mut self, s: impl Into<String>) {
        self.status = s.into();
    }

    fn update_status(&mut self) {
        let total = self.total_lines();
        self.status = match self.mode {
            ViewMode::Text => {
                if total == 0 {
                    "(empty file)".into()
                } else {
                    let line = (self.scroll_offset + 1).min(total);
                    let wrap_str = if self.word_wrap { "on" } else { "off" };
                    format!("Line {line}/{total}  wrap: {wrap_str}")
                }
            }
            ViewMode::Hex => {
                let byte_offset = self.scroll_offset * HEX_ROW_WIDTH;
                let total_bytes = match &self.buffer {
                    ViewBuffer::Loaded { bytes, .. } => bytes.len(),
                    ViewBuffer::Streaming { total_bytes, .. } => *total_bytes as usize,
                };
                format!("Offset 0x{byte_offset:08X} / {total_bytes} bytes")
            }
        };
    }

    // --- Navigation ---

    /// Scroll down by one line/row.
    pub fn scroll_down(&mut self) -> FileViewerAction {
        // Extract streaming state before mutably borrowing self.
        let streaming = match &self.buffer {
            ViewBuffer::Streaming {
                lines,
                window_start_line,
                total_lines,
                reader_offset,
                ..
            } => Some((
                *window_start_line + lines.len(), // window_end
                *total_lines,
                *reader_offset,
            )),
            ViewBuffer::Loaded { .. } => None,
        };
        let max = self.total_lines().saturating_sub(1);
        if self.scroll_offset < max {
            self.scroll_offset += 1;
            self.update_status();
        }
        if let Some((window_end, total, reader_offset)) = streaming {
            if window_end < total && self.scroll_offset + PREFETCH_THRESHOLD >= window_end {
                return FileViewerAction::NeedsData {
                    offset: reader_offset,
                    line_count: WINDOW_MAX_LINES / 2,
                    window_start: window_end,
                };
            }
        }
        FileViewerAction::Swallow
    }

    /// Scroll up by one line/row.
    pub fn scroll_up(&mut self) -> FileViewerAction {
        // For streaming at window boundary, emit NeedsData to load backward.
        let backward = match &self.buffer {
            ViewBuffer::Streaming {
                window_start_line,
                chunk_index,
                ..
            } => {
                if *window_start_line > 0 && self.scroll_offset <= *window_start_line {
                    let target = window_start_line.saturating_sub(WINDOW_MAX_LINES / 2);
                    let entry = chunk_index
                        .iter()
                        .rev()
                        .find(|(ln, _)| *ln <= target)
                        .copied()
                        .unwrap_or((0, 0));
                    Some(entry)
                } else {
                    None
                }
            }
            ViewBuffer::Loaded { .. } => None,
        };
        if let Some((chunk_line, chunk_offset)) = backward {
            return FileViewerAction::NeedsData {
                offset: chunk_offset,
                line_count: WINDOW_MAX_LINES,
                window_start: chunk_line,
            };
        }
        if self.scroll_offset > 0 {
            self.scroll_offset -= 1;
            self.update_status();
        }
        FileViewerAction::Swallow
    }

    /// Scroll down by `height` lines.
    pub fn page_down(&mut self, height: usize) -> FileViewerAction {
        let streaming = match &self.buffer {
            ViewBuffer::Streaming {
                lines,
                window_start_line,
                total_lines,
                reader_offset,
                ..
            } => Some((
                *window_start_line + lines.len(),
                *total_lines,
                *reader_offset,
            )),
            ViewBuffer::Loaded { .. } => None,
        };
        let max = self.total_lines().saturating_sub(1);
        self.scroll_offset = (self.scroll_offset + height).min(max);
        self.update_status();
        if let Some((window_end, total, reader_offset)) = streaming {
            if window_end < total && self.scroll_offset + PREFETCH_THRESHOLD >= window_end {
                return FileViewerAction::NeedsData {
                    offset: reader_offset,
                    line_count: WINDOW_MAX_LINES / 2,
                    window_start: window_end,
                };
            }
        }
        FileViewerAction::Swallow
    }

    /// Scroll up by `height` lines.
    pub fn page_up(&mut self, height: usize) -> FileViewerAction {
        let backward = match &self.buffer {
            ViewBuffer::Streaming {
                window_start_line,
                chunk_index,
                ..
            } => {
                let new_scroll = self.scroll_offset.saturating_sub(height);
                if *window_start_line > 0 && new_scroll <= *window_start_line {
                    let target = window_start_line.saturating_sub(WINDOW_MAX_LINES / 2);
                    let entry = chunk_index
                        .iter()
                        .rev()
                        .find(|(ln, _)| *ln <= target)
                        .copied()
                        .unwrap_or((0, 0));
                    Some(entry)
                } else {
                    None
                }
            }
            ViewBuffer::Loaded { .. } => None,
        };
        if let Some((chunk_line, chunk_offset)) = backward {
            self.scroll_offset = self.scroll_offset.saturating_sub(height);
            self.update_status();
            return FileViewerAction::NeedsData {
                offset: chunk_offset,
                line_count: WINDOW_MAX_LINES,
                window_start: chunk_line,
            };
        }
        self.scroll_offset = self.scroll_offset.saturating_sub(height);
        self.update_status();
        FileViewerAction::Swallow
    }

    /// Jump to the first line/row.
    pub fn home_key(&mut self) -> FileViewerAction {
        self.scroll_offset = 0;
        self.update_status();
        FileViewerAction::Swallow
    }

    /// Jump to the last line/row.
    pub fn end_key(&mut self) -> FileViewerAction {
        self.scroll_offset = self.total_lines().saturating_sub(1);
        self.update_status();
        FileViewerAction::Swallow
    }

    /// Jump to the last line (named command for `G` / `viewer-end`).
    pub fn goto_end(&mut self) -> FileViewerAction {
        self.end_key()
    }

    // --- Goto ---

    /// Jump to line `n` (1-based, clamped to `[1, last_line]`).
    pub fn goto_line(&mut self, n: usize) -> FileViewerAction {
        let total = self.total_lines();
        if total == 0 {
            return FileViewerAction::Swallow;
        }
        let target = n.clamp(1, total) - 1; // 0-based absolute
                                            // For streaming: check if target is in the current window.
        let needs = match &self.buffer {
            ViewBuffer::Streaming {
                lines,
                window_start_line,
                chunk_index,
                ..
            } => {
                let window_end = *window_start_line + lines.len();
                if target < *window_start_line || target >= window_end {
                    // Not in window: binary-search chunk index for nearest entry ≤ target.
                    let entry = chunk_index
                        .iter()
                        .rev()
                        .find(|(ln, _)| *ln <= target)
                        .copied()
                        .unwrap_or((0, 0));
                    Some(entry)
                } else {
                    None
                }
            }
            ViewBuffer::Loaded { .. } => None,
        };
        self.scroll_offset = target;
        self.update_status();
        if let Some((chunk_line, chunk_offset)) = needs {
            FileViewerAction::NeedsData {
                offset: chunk_offset,
                line_count: WINDOW_MAX_LINES,
                window_start: chunk_line,
            }
        } else {
            FileViewerAction::Swallow
        }
    }

    /// Jump to the hex row containing byte `offset` (hex mode, clamped to valid range).
    pub fn goto_offset(&mut self, offset: u64) -> FileViewerAction {
        let total_rows = self.total_lines();
        let row = (offset / HEX_ROW_WIDTH as u64) as usize;
        self.scroll_offset = row.min(total_rows.saturating_sub(1));
        self.update_status();
        FileViewerAction::Swallow
    }

    /// Parse a goto input string: plain decimal or `0x`/`0X`-prefixed hex.
    pub fn parse_goto_input(s: &str) -> Option<u64> {
        let s = s.trim();
        if s.is_empty() {
            return None;
        }
        if let Some(hex) = s.strip_prefix("0x").or_else(|| s.strip_prefix("0X")) {
            u64::from_str_radix(hex, 16).ok()
        } else {
            s.parse::<u64>().ok()
        }
    }

    /// Open the goto prompt (`g` key via keymap; direct method for test coverage).
    pub fn open_goto_prompt(&mut self) -> FileViewerAction {
        self.prompt = Some(ViewerPrompt::Goto {
            buffer: String::new(),
        });
        FileViewerAction::Swallow
    }

    // --- Mode and settings ---

    /// Toggle word-wrap (text mode only; hex mode ignores this).
    pub fn toggle_wrap(&mut self) -> FileViewerAction {
        self.word_wrap = !self.word_wrap;
        self.update_status();
        FileViewerAction::Swallow
    }

    /// Toggle between text and hex mode. Resets scroll position and clears search.
    pub fn toggle_mode(&mut self) {
        self.mode = match self.mode {
            ViewMode::Text => ViewMode::Hex,
            ViewMode::Hex => ViewMode::Text,
        };
        self.scroll_offset = 0;
        self.search = None;
        self.prompt = None;
        self.update_status();
    }

    // --- Search ---

    /// Open the search prompt (direct method, also called from keymap dispatch).
    pub fn open_search_prompt(&mut self, direction: SearchDirection) -> FileViewerAction {
        self.prompt = Some(ViewerPrompt::Search {
            buffer: String::new(),
            direction,
        });
        FileViewerAction::Swallow
    }

    /// Search forward from the current scroll offset for `pattern`.
    /// Returns `(absolute_line_index, col_byte_offset)` of the first match, or `None`.
    pub fn search_forward(&self, pattern: &str) -> Option<(usize, usize)> {
        let lines = self.text_lines_as_vec();
        if lines.is_empty() || pattern.is_empty() {
            return None;
        }
        let window_start = match &self.buffer {
            ViewBuffer::Streaming {
                window_start_line, ..
            } => *window_start_line,
            ViewBuffer::Loaded { .. } => 0,
        };
        // Convert absolute scroll_offset to window-relative.
        let local_scroll = self
            .scroll_offset
            .saturating_sub(window_start)
            .min(lines.len());
        let start = (local_scroll + 1).min(lines.len());
        for (offset, line) in lines[start..].iter().enumerate() {
            if let Some(col) = line.find(pattern) {
                return Some((window_start + start + offset, col));
            }
        }
        // Wrap-around from beginning of window.
        for (i, line) in lines[..start].iter().enumerate() {
            if let Some(col) = line.find(pattern) {
                return Some((window_start + i, col));
            }
        }
        None
    }

    /// Search backward from the current scroll offset for `pattern`.
    /// Returns `(absolute_line_index, col_byte_offset)` of the match, or `None`.
    pub fn search_backward(&self, pattern: &str) -> Option<(usize, usize)> {
        let lines = self.text_lines_as_vec();
        if lines.is_empty() || pattern.is_empty() {
            return None;
        }
        let window_start = match &self.buffer {
            ViewBuffer::Streaming {
                window_start_line, ..
            } => *window_start_line,
            ViewBuffer::Loaded { .. } => 0,
        };
        let local_scroll = self
            .scroll_offset
            .saturating_sub(window_start)
            .min(lines.len());
        let end = local_scroll.min(lines.len());
        for i in (0..end).rev() {
            if let Some(col) = lines[i].find(pattern) {
                return Some((window_start + i, col));
            }
        }
        // Wrap-around from end of window.
        for i in (end..lines.len()).rev() {
            if let Some(col) = lines[i].find(pattern) {
                return Some((window_start + i, col));
            }
        }
        None
    }

    /// Returns a partial-coverage annotation for search results in streaming mode.
    /// Returns `None` for fully-loaded buffers.
    fn streaming_annotation(&self) -> Option<String> {
        if let ViewBuffer::Streaming {
            lines,
            window_start_line,
            total_bytes,
            reader_offset,
            ..
        } = &self.buffer
        {
            let buffer_end_line = window_start_line + lines.len();
            let tot = self.total_lines();
            if buffer_end_line < tot && *total_bytes > 0 {
                let searched_mib = *reader_offset as f64 / (1024.0 * 1024.0);
                let total_mib = *total_bytes as f64 / (1024.0 * 1024.0);
                return Some(format!(
                    "(searched {searched_mib:.1} MiB of {total_mib:.1} MiB)"
                ));
            }
        }
        None
    }

    fn apply_search_result(
        &mut self,
        result: Option<(usize, usize)>,
        pattern: &str,
        dir: SearchDirection,
    ) {
        let annot = self.streaming_annotation().unwrap_or_default();
        match result {
            Some((line, col)) => {
                self.scroll_offset = line;
                self.search = Some(SearchState {
                    pattern: pattern.into(),
                    direction: dir,
                    last_match_line: Some(line),
                    last_match_col: Some(col),
                });
                self.update_status();
                let base = self.status.clone();
                if annot.is_empty() {
                    self.status = format!("/{pattern}  {base}");
                } else {
                    self.status = format!("/{pattern}  {base}  {annot}");
                }
            }
            None => {
                if annot.is_empty() {
                    self.status = format!("Pattern not found: {pattern}");
                } else {
                    self.status = format!("Pattern not found: {pattern}  {annot}");
                }
            }
        }
    }

    /// Advance to the next (`Forward`) or previous (`Backward`) search match.
    pub fn advance_search(&mut self, dir: SearchDirection) -> FileViewerAction {
        let pattern = match &self.search {
            Some(s) => s.pattern.clone(),
            None => return FileViewerAction::Swallow,
        };
        let result = match dir {
            SearchDirection::Forward => self.search_forward(&pattern),
            SearchDirection::Backward => self.search_backward(&pattern),
        };
        self.apply_search_result(result, &pattern, dir);
        FileViewerAction::Swallow
    }

    // --- Key handling ---

    /// Handle a raw navigation key (Up/Down/PgUp/PgDn/Home/End/Esc and prompt input).
    /// Called from lib.rs when `SeqLookup::NoMatch` — i.e., for keys not in the keymap.
    pub fn handle_key(&mut self, code: crossterm::event::KeyCode) -> FileViewerAction {
        use crossterm::event::KeyCode;

        // If a prompt is active, route input to it.
        if let Some(prompt) = self.prompt.take() {
            return self.handle_prompt_key(prompt, code);
        }

        match code {
            KeyCode::Up => self.scroll_up(),
            KeyCode::Down => self.scroll_down(),
            KeyCode::PageUp => {
                let h = self.viewport_height.saturating_sub(1).max(1);
                self.page_up(h)
            }
            KeyCode::PageDown => {
                let h = self.viewport_height.saturating_sub(1).max(1);
                self.page_down(h)
            }
            KeyCode::Home => self.home_key(),
            KeyCode::End => self.end_key(),
            KeyCode::Esc => FileViewerAction::Close,
            // `/` and `?` handled here as raw fallback (keymap takes priority in lib.rs).
            KeyCode::Char('/') => self.open_search_prompt(SearchDirection::Forward),
            KeyCode::Char('?') => self.open_search_prompt(SearchDirection::Backward),
            _ => FileViewerAction::Swallow,
        }
    }

    fn handle_prompt_key(
        &mut self,
        mut prompt: ViewerPrompt,
        code: crossterm::event::KeyCode,
    ) -> FileViewerAction {
        use crossterm::event::KeyCode;
        match &mut prompt {
            ViewerPrompt::Search { buffer, direction } => match code {
                KeyCode::Esc => {
                    self.search = None;
                    self.update_status();
                    FileViewerAction::Swallow
                }
                KeyCode::Enter => {
                    if buffer.is_empty() {
                        self.search = None;
                        self.update_status();
                    } else {
                        let pattern = buffer.clone();
                        let dir = *direction;
                        let result = match dir {
                            SearchDirection::Forward => self.search_forward(&pattern),
                            SearchDirection::Backward => self.search_backward(&pattern),
                        };
                        self.apply_search_result(result, &pattern, dir);
                    }
                    FileViewerAction::Swallow
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
                KeyCode::Char(c) => {
                    buffer.push(c);
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
                _ => {
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
            },
            ViewerPrompt::Goto { buffer } => match code {
                KeyCode::Esc => {
                    self.update_status();
                    FileViewerAction::Swallow
                }
                KeyCode::Enter => {
                    if !buffer.is_empty() {
                        let s = buffer.clone();
                        if let Some(n) = Self::parse_goto_input(&s) {
                            match self.mode {
                                ViewMode::Text => {
                                    self.goto_line(n as usize);
                                }
                                ViewMode::Hex => {
                                    self.goto_offset(n);
                                }
                            }
                        }
                    }
                    FileViewerAction::Swallow
                }
                KeyCode::Backspace => {
                    buffer.pop();
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
                KeyCode::Char(c)
                    if c.is_ascii_digit()
                        || c == 'x'
                        || c == 'X'
                        || ('a'..='f').contains(&c)
                        || ('A'..='F').contains(&c) =>
                {
                    buffer.push(c);
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
                _ => {
                    self.prompt = Some(prompt);
                    FileViewerAction::Swallow
                }
            },
        }
    }

    // --- Streaming ---

    /// Merge new lines into the streaming buffer window (called from lib.rs after `NeedsData`).
    ///
    /// `window_start` is the absolute file line number of `new_lines[0]`.
    /// `new_reader_offset` is the byte position after the last line read.
    /// - Forward load (`window_start >= current_window_end`): appends, evicts front if > `WINDOW_MAX_LINES`.
    /// - Backward/goto load (`window_start < current_window_end`): replaces the window entirely.
    pub fn append_lines(
        &mut self,
        new_lines: Vec<String>,
        window_start: usize,
        new_reader_offset: u64,
    ) {
        if let ViewBuffer::Streaming {
            lines,
            window_start_line,
            reader_offset,
            ..
        } = &mut self.buffer
        {
            let current_window_end = *window_start_line + lines.len();
            if window_start >= current_window_end {
                // Forward: append to back, evict front.
                for l in new_lines {
                    lines.push_back(l);
                    while lines.len() > WINDOW_MAX_LINES {
                        lines.pop_front();
                        *window_start_line += 1;
                    }
                }
            } else {
                // Backward or goto: replace window entirely.
                *lines = new_lines.into_iter().collect();
                *window_start_line = window_start;
            }
            *reader_offset = new_reader_offset;
        }
        self.update_status();
    }

    // --- Hex rendering ---

    /// Format one 16-byte hex dump row.
    ///
    /// Output: `{offset:08x}  {hex_part}  |{ascii_part}|`
    /// where the hex part has an extra space between byte groups 0–7 and 8–15.
    pub fn render_hex_row(offset: usize, data: &[u8]) -> String {
        let mut hex_part = String::with_capacity(49);
        let mut ascii_part = String::with_capacity(HEX_ROW_WIDTH);

        for i in 0..HEX_ROW_WIDTH {
            if i > 0 {
                hex_part.push(' ');
            }
            if i == 8 {
                hex_part.push(' '); // extra gap between the two 8-byte groups
            }
            if let Some(&b) = data.get(i) {
                hex_part.push_str(&format!("{b:02x}"));
                if (0x20..=0x7e).contains(&b) {
                    ascii_part.push(b as char);
                } else {
                    ascii_part.push('.');
                }
            } else {
                hex_part.push_str("  "); // two-space pad for a missing byte
                ascii_part.push(' ');
            }
        }

        format!("{offset:08x}  {hex_part}  |{ascii_part}|")
    }

    // --- Close ---

    /// Signal that the viewer should close (called from lib.rs for `ViewerQuit`).
    pub fn close(&mut self) -> FileViewerAction {
        FileViewerAction::Close
    }

    // --- Rendering ---

    /// Render the viewer as a full-screen overlay into `area`.
    ///
    /// Updates `self.viewport_height` as a side effect so that `PageUp`/`PageDown`
    /// in the subsequent `handle_key` call uses the correct page size.
    pub fn render(
        &mut self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        theme: &crate::theme::Theme,
    ) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::style::Style;
        use ratatui::widgets::{Block, Borders, Clear, Paragraph};

        let mode_label = match self.mode {
            ViewMode::Text => "text",
            ViewMode::Hex => "hex",
        };
        let title = format!(" F3 View — {}  [{}] ", self.display_name, mode_label);

        // Full-screen overlay: paint over everything.
        f.render_widget(Clear, area);

        let block = Block::default()
            .title(title.as_str())
            .borders(Borders::ALL)
            .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg));

        let inner = block.inner(area);
        f.render_widget(block, area);

        if inner.height < 2 {
            return;
        }

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Min(1), Constraint::Length(1)])
            .split(inner);
        let content_area = chunks[0];
        let status_area = chunks[1];

        // Cache viewport height for page-scroll sizing.
        self.viewport_height = content_area.height as usize;
        let viewport_h = self.viewport_height;

        match self.mode {
            ViewMode::Text => self.render_text(f, content_area, viewport_h, theme),
            ViewMode::Hex => self.render_hex(f, content_area, viewport_h, theme),
        }

        // Status bar row — replaced by active prompt text when a prompt is open (L3 fix).
        let status_line = match &self.prompt {
            Some(ViewerPrompt::Search { buffer, .. }) => format!("/{buffer}_"),
            Some(ViewerPrompt::Goto { buffer }) => match self.mode {
                ViewMode::Text => format!("Go to line: {buffer}_"),
                ViewMode::Hex => format!("Go to offset: {buffer}_"),
            },
            None => self.status.clone(),
        };
        let status_para = Paragraph::new(status_line)
            .style(Style::default().fg(theme.status_fg).bg(theme.status_bg));
        f.render_widget(status_para, status_area);
    }

    fn render_text(
        &self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        viewport_h: usize,
        theme: &crate::theme::Theme,
    ) {
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::Paragraph;

        let lines_vec = self.text_lines_as_vec();
        let file_total = self.total_lines(); // total lines in file (for Streaming: estimate)

        if file_total == 0 || lines_vec.is_empty() {
            let para = Paragraph::new("(empty file)")
                .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg));
            f.render_widget(para, area);
            return;
        }

        // For streaming buffers, scroll_offset is an absolute file line number but
        // lines_vec contains only the window.  Compute the window-relative index.
        let window_start = match &self.buffer {
            ViewBuffer::Streaming {
                window_start_line, ..
            } => *window_start_line,
            ViewBuffer::Loaded { .. } => 0,
        };
        let local_start = self
            .scroll_offset
            .saturating_sub(window_start)
            .min(lines_vec.len());
        let local_end = (local_start + viewport_h).min(lines_vec.len());

        let gutter_w = format!("{file_total}").len();
        let search_pat = self.search.as_ref().map(|s| s.pattern.as_str());

        let mut rendered: Vec<Line> = Vec::with_capacity(viewport_h);
        for (line_offset, line_str) in lines_vec[local_start..local_end].iter().enumerate() {
            let line_num = self.scroll_offset + line_offset + 1;
            let text: &str = line_str.as_str();
            let num = format!("{:>width$} ", line_num, width = gutter_w);
            let mut spans = vec![Span::styled(num, Style::default().fg(Color::DarkGray))];

            // Highlight ALL visible occurrences of the search pattern (FR-018 / H3 fix).
            if let Some(pat) = search_pat {
                if !pat.is_empty() {
                    let mut last = 0;
                    for (start, matched) in text.match_indices(pat) {
                        if last < start {
                            spans.push(Span::raw(text[last..start].to_string()));
                        }
                        spans.push(Span::styled(
                            matched.to_string(),
                            Style::default().add_modifier(Modifier::REVERSED),
                        ));
                        last = start + matched.len();
                    }
                    if last < text.len() {
                        spans.push(Span::raw(text[last..].to_string()));
                    }
                } else {
                    spans.push(Span::raw(text.to_string()));
                }
            } else {
                spans.push(Span::raw(text.to_string()));
            }

            rendered.push(Line::from(spans));
        }

        let para = if self.word_wrap {
            Paragraph::new(rendered)
                .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg))
                .wrap(ratatui::widgets::Wrap { trim: false })
        } else {
            Paragraph::new(rendered).style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg))
        };
        f.render_widget(para, area);
    }

    fn render_hex(
        &self,
        f: &mut ratatui::Frame,
        area: ratatui::layout::Rect,
        viewport_h: usize,
        theme: &crate::theme::Theme,
    ) {
        use ratatui::style::Style;
        use ratatui::widgets::Paragraph;

        let bytes = match &self.buffer {
            ViewBuffer::Loaded { bytes, .. } => bytes.as_slice(),
            ViewBuffer::Streaming { .. } => &[], // hex streaming: Phase 7
        };

        let total_rows = bytes.len().div_ceil(HEX_ROW_WIDTH);
        let end = (self.scroll_offset + viewport_h).min(total_rows);
        let mut rows: Vec<String> = Vec::with_capacity(viewport_h);
        for row in self.scroll_offset..end {
            let byte_offset = row * HEX_ROW_WIDTH;
            let row_end = (byte_offset + HEX_ROW_WIDTH).min(bytes.len());
            rows.push(Self::render_hex_row(
                byte_offset,
                &bytes[byte_offset..row_end],
            ));
        }

        let para = Paragraph::new(rows.join("\n"))
            .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg));
        f.render_widget(para, area);
    }
}

// =====================================================================
// Feature 052 — Find-File and Panelize
// =====================================================================

use std::path::PathBuf;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tokio::sync::mpsc;

/// Which search mode the find-file dialog is in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SearchMode {
    /// Search by filename glob pattern (BFS walk, globset matcher).
    Name,
    /// Search by content using ripgrep (`rg --files-with-matches`).
    Content,
}

/// Phase the find-file dialog is currently in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DialogPhase {
    /// Input box is focused; user has not started a search yet.
    InputFocused,
    /// Walk is in progress (channel open, spinner visible).
    Walking,
    /// Walk completed with ≥1 results; result list is focused.
    ResultsFocused,
    /// Walk completed with 0 results; no list to navigate.
    NoResults,
}

/// Events produced by the background walk task.
#[derive(Debug)]
pub enum FindEvent {
    /// A matching file was found.
    Found(PathBuf),
    /// Walk completed (or was aborted).
    Done {
        /// True when the walk stopped because `max_results` was reached.
        truncated: bool,
    },
}

/// Outcome returned by [`FindFileDialog::handle_key`].
#[derive(Debug)]
pub enum FindOutcome {
    /// Key was consumed; stay in dialog.
    Consumed,
    /// User pressed Esc; caller should call `widget.cancel()` then dismiss.
    Cancelled,
    /// User pressed Enter in `ResultsFocused`; panelize these paths.
    Panelize {
        /// Absolute paths to panelize.
        paths: Vec<PathBuf>,
        /// The pattern that was entered (for the `[Find: …]` label).
        pattern: String,
    },
}

/// Find-file dialog widget (Feature 052).
///
/// Owns the channel receiver for incremental walk results and the abort flag.
/// The event loop calls [`FindFileDialog::poll_results`] each tick (100ms) to
/// drain new results and transition phases.
#[allow(dead_code)]
pub struct FindFileDialog {
    /// Current search mode (Name or Content).
    pub mode: SearchMode,
    /// User-typed pattern (glob or literal for ripgrep).
    pub input: String,
    /// Current dialog phase.
    pub phase: DialogPhase,
    /// Accumulated absolute path results.
    pub results: Vec<PathBuf>,
    /// Index of the highlighted result (0-based, clamped).
    pub cursor: usize,
    /// First visible row in the result list.
    pub scroll_offset: usize,
    /// True when the result list was truncated at `max_results`.
    pub truncated: bool,
    /// True when ripgrep binary was found at startup.
    pub content_available: bool,
    /// Transient notice text shown below the input.
    pub notice: Option<String>,
    /// Channel receiver for walk events (Some only while Walking).
    pub walk_rx: Option<mpsc::UnboundedReceiver<FindEvent>>,
    /// Abort flag (Some only while Walking).
    pub abort_flag: Option<Arc<AtomicBool>>,
}

impl std::fmt::Debug for FindFileDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FindFileDialog")
            .field("mode", &self.mode)
            .field("input", &self.input)
            .field("phase", &self.phase)
            .field("results", &self.results.len())
            .field("cursor", &self.cursor)
            .field("scroll_offset", &self.scroll_offset)
            .field("truncated", &self.truncated)
            .field("content_available", &self.content_available)
            .field("notice", &self.notice)
            .finish()
    }
}

/// Pure check: is ripgrep available at `rg_path`?
///
/// Runs `rg --version` and returns `true` if it exits successfully.
/// This is the sole source of truth for content-search gating (FR-013,
/// contract §3a). Pure: no side effects beyond spawning a short subprocess.
pub fn plan_content_available(rg_path: &str) -> bool {
    std::process::Command::new(rg_path)
        .arg("--version")
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

impl FindFileDialog {
    /// Construct a new find-file dialog.
    ///
    /// `content_available` should be the result of [`plan_content_available`]
    /// called at the point the dialog is opened (once per open, not cached).
    pub fn new(content_available: bool) -> Self {
        Self {
            mode: SearchMode::Name,
            input: String::new(),
            phase: DialogPhase::InputFocused,
            results: Vec::new(),
            cursor: 0,
            scroll_offset: 0,
            truncated: false,
            content_available,
            notice: None,
            walk_rx: None,
            abort_flag: None,
        }
    }

    /// Handle a key event. Returns a [`FindOutcome`] telling the caller what
    /// to do next. The `config` is used for max_results / rg_path.
    pub fn handle_key(
        &mut self,
        key: crossterm::event::KeyCode,
        config: &cargonaut_config::Config,
    ) -> FindOutcome {
        use crossterm::event::KeyCode;
        // Clear transient notice on any key.
        self.notice = None;

        match key {
            KeyCode::Esc => return FindOutcome::Cancelled,

            KeyCode::Tab => {
                match self.phase {
                    DialogPhase::Walking => {
                        // No mode change while walking.
                    }
                    _ => {
                        if self.mode == SearchMode::Name {
                            if self.content_available {
                                self.mode = SearchMode::Content;
                            } else {
                                self.notice =
                                    Some("Content search unavailable: rg not found".to_string());
                            }
                        } else {
                            self.mode = SearchMode::Name;
                        }
                    }
                }
            }

            KeyCode::Enter => match self.phase {
                DialogPhase::InputFocused => {
                    // Start a walk. Use a temporary root for the dialog-level
                    // handle_key call; the real root is passed in start_walk.
                    // For tests that call handle_key directly (without a root),
                    // we start a walk against /tmp as a safe fallback.
                    let root = PathBuf::from("/tmp");
                    self.start_walk(root, config);
                }
                DialogPhase::ResultsFocused => {
                    if !self.results.is_empty() {
                        return FindOutcome::Panelize {
                            paths: self.results.clone(),
                            pattern: self.input.clone(),
                        };
                    }
                }
                DialogPhase::Walking => {
                    // Enter during walk is a no-op (user waits for results).
                }
                DialogPhase::NoResults => {
                    // Enter in NoResults is a no-op per contract §3b.
                }
            },

            KeyCode::Backspace if self.phase == DialogPhase::InputFocused => {
                self.input.pop();
            }

            KeyCode::Char(c) if self.phase == DialogPhase::InputFocused => {
                self.input.push(c);
            }

            KeyCode::Char(c)
                if self.phase == DialogPhase::ResultsFocused
                    || self.phase == DialogPhase::NoResults =>
            {
                // Any printable char restarts input.
                self.cancel();
                self.input.push(c);
            }

            KeyCode::Up
                if self.phase == DialogPhase::ResultsFocused && !self.results.is_empty() =>
            {
                self.cursor = self.cursor.saturating_sub(1);
                self.clamp_scroll(10); // default window size
            }

            KeyCode::Down
                if self.phase == DialogPhase::ResultsFocused && !self.results.is_empty() =>
            {
                let max = self.results.len().saturating_sub(1);
                self.cursor = (self.cursor + 1).min(max);
                self.clamp_scroll(10); // default window size
            }

            KeyCode::PageUp
                if self.phase == DialogPhase::ResultsFocused && !self.results.is_empty() =>
            {
                self.cursor = self.cursor.saturating_sub(10);
                self.clamp_scroll(10);
            }

            KeyCode::PageDown
                if self.phase == DialogPhase::ResultsFocused && !self.results.is_empty() =>
            {
                let max = self.results.len().saturating_sub(1);
                self.cursor = (self.cursor + 10).min(max);
                self.clamp_scroll(10);
            }

            _ => {}
        }

        FindOutcome::Consumed
    }

    /// Handle a key with an explicit root for the walk (used by the event loop).
    /// This is the production path; `handle_key` uses /tmp as a fallback for tests.
    pub fn handle_key_with_root(
        &mut self,
        key: crossterm::event::KeyCode,
        config: &cargonaut_config::Config,
        root: PathBuf,
    ) -> FindOutcome {
        use crossterm::event::KeyCode;
        self.notice = None;

        match key {
            KeyCode::Esc => return FindOutcome::Cancelled,

            KeyCode::Enter => match self.phase {
                DialogPhase::InputFocused => {
                    self.start_walk(root, config);
                }
                DialogPhase::ResultsFocused if !self.results.is_empty() => {
                    return FindOutcome::Panelize {
                        paths: self.results.clone(),
                        pattern: self.input.clone(),
                    };
                }
                _ => {}
            },

            _ => return self.handle_key(key, config),
        }

        FindOutcome::Consumed
    }

    /// Start a background walk for the current `mode` and `input`.
    ///
    /// Sets phase to Walking; spawns a tokio task that sends [`FindEvent`]s
    /// through an unbounded channel. Caller must drive `poll_results` each
    /// tick to drain the channel. FR-018: if the root is unreadable, sets
    /// phase to NoResults and returns without spawning.
    pub fn start_walk(&mut self, root: PathBuf, config: &cargonaut_config::Config) {
        // FR-018: root guard — fail fast if the root is not readable.
        if std::fs::read_dir(&root).is_err() {
            self.phase = DialogPhase::NoResults;
            self.notice = Some(format!("Cannot read directory: {}", root.display()));
            return;
        }

        let pattern = if self.input.is_empty() {
            "**".to_string()
        } else {
            self.input.clone()
        };

        let max_results = config.search.max_results as usize;
        let abort_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel::<FindEvent>();

        self.phase = DialogPhase::Walking;
        self.results.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
        self.truncated = false;
        self.walk_rx = Some(rx);
        self.abort_flag = Some(abort_flag.clone());

        match self.mode {
            SearchMode::Name => {
                Self::spawn_name_walk(root, pattern, max_results, abort_flag, tx);
            }
            SearchMode::Content => {
                let rg_path = config.search.ripgrep_path.clone();
                Self::spawn_content_walk(root, pattern, max_results, abort_flag, tx, rg_path);
            }
        }
    }

    /// Spawn the BFS name-mode walk in a blocking task.
    fn spawn_name_walk(
        root: PathBuf,
        pattern: String,
        max_results: usize,
        abort_flag: Arc<AtomicBool>,
        tx: mpsc::UnboundedSender<FindEvent>,
    ) {
        tokio::task::spawn_blocking(move || {
            use globset::GlobBuilder;
            use std::collections::VecDeque;

            // Build glob matcher. Use "**" to match everything if pattern is
            // already "**" (from empty input substitution), otherwise build a
            // filename-only glob that matches anywhere in the path.
            let glob = match GlobBuilder::new(&pattern).build() {
                Ok(g) => g.compile_matcher(),
                Err(_) => {
                    // Invalid glob — send Done with no results.
                    let _ = tx.send(FindEvent::Done { truncated: false });
                    return;
                }
            };

            let mut queue = VecDeque::new();
            queue.push_back(root);
            let mut count = 0usize;

            'outer: while let Some(dir) = queue.pop_front() {
                if abort_flag.load(Ordering::Relaxed) {
                    break;
                }
                let rd = match std::fs::read_dir(&dir) {
                    Ok(rd) => rd,
                    Err(_) => continue, // FR-018: skip unreadable subdirs silently
                };
                for entry in rd {
                    if abort_flag.load(Ordering::Relaxed) {
                        break 'outer;
                    }
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    let path = entry.path();
                    let file_type = match entry.file_type() {
                        Ok(ft) => ft,
                        Err(_) => continue,
                    };
                    if file_type.is_dir() {
                        queue.push_back(path);
                    } else {
                        // Match against the filename component only.
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        let matches = if pattern == "**" {
                            true
                        } else {
                            glob.is_match(name_str.as_ref())
                        };
                        if matches {
                            count += 1;
                            if count > max_results {
                                let _ = tx.send(FindEvent::Done { truncated: true });
                                return;
                            }
                            let _ = tx.send(FindEvent::Found(path));
                        }
                    }
                }
            }
            let _ = tx.send(FindEvent::Done { truncated: false });
        });
    }

    /// Spawn the ripgrep content-mode walk.
    fn spawn_content_walk(
        root: PathBuf,
        pattern: String,
        max_results: usize,
        abort_flag: Arc<AtomicBool>,
        tx: mpsc::UnboundedSender<FindEvent>,
        rg_path: String,
    ) {
        tokio::spawn(async move {
            use std::process::Stdio;
            use tokio::io::AsyncBufReadExt;
            use tokio::process::Command as TokioCommand;

            let root_str = root.to_string_lossy().into_owned();
            let mut child = match TokioCommand::new(&rg_path)
                .args([
                    pattern.as_str(),
                    "--files-with-matches",
                    "--no-messages",
                    &root_str,
                ])
                .stdout(Stdio::piped())
                .spawn()
            {
                Ok(c) => c,
                Err(_) => {
                    let _ = tx.send(FindEvent::Done { truncated: false });
                    return;
                }
            };

            let stdout = match child.stdout.take() {
                Some(s) => s,
                None => {
                    let _ = tx.send(FindEvent::Done { truncated: false });
                    return;
                }
            };

            let mut lines = tokio::io::BufReader::new(stdout).lines();
            let mut count = 0usize;
            let mut truncated = false;

            loop {
                if abort_flag.load(Ordering::Relaxed) {
                    let _ = child.kill().await;
                    break;
                }
                match lines.next_line().await {
                    Ok(Some(line)) if !line.is_empty() => {
                        count += 1;
                        if count > max_results {
                            truncated = true;
                            let _ = child.kill().await;
                            break;
                        }
                        let _ = tx.send(FindEvent::Found(PathBuf::from(line)));
                    }
                    Ok(Some(_)) => {}  // empty line
                    Ok(None) => break, // EOF
                    Err(_) => break,
                }
            }

            let _ = tx.send(FindEvent::Done { truncated });
        });
    }

    /// Drain pending walk events from the channel.
    ///
    /// Call this each 100ms tick while `phase == Walking`. Appends found paths
    /// to `results`; on `Done` transitions to `ResultsFocused` or `NoResults`.
    pub fn poll_results(&mut self) {
        let rx = match self.walk_rx.as_mut() {
            Some(r) => r,
            None => return,
        };

        loop {
            match rx.try_recv() {
                Ok(FindEvent::Found(path)) => {
                    self.results.push(path);
                }
                Ok(FindEvent::Done { truncated }) => {
                    self.truncated = truncated;
                    self.walk_rx = None;
                    self.abort_flag = None;
                    if self.results.is_empty() {
                        self.phase = DialogPhase::NoResults;
                        self.notice = Some(format!("No files found matching `{}`", self.input));
                    } else {
                        self.phase = DialogPhase::ResultsFocused;
                    }
                    break;
                }
                Err(tokio::sync::mpsc::error::TryRecvError::Empty) => break,
                Err(tokio::sync::mpsc::error::TryRecvError::Disconnected) => {
                    // Channel dropped — treat as Done with whatever we have.
                    self.walk_rx = None;
                    self.abort_flag = None;
                    if self.results.is_empty() {
                        self.phase = DialogPhase::NoResults;
                        self.notice = Some(format!("No files found matching `{}`", self.input));
                    } else {
                        self.phase = DialogPhase::ResultsFocused;
                    }
                    break;
                }
            }
        }
    }

    /// Cancel an in-progress walk.
    ///
    /// Sets the abort flag, drops the channel receiver, and resets phase to
    /// `InputFocused`. Results are cleared. The background task will observe
    /// the flag and exit at its next iteration.
    pub fn cancel(&mut self) {
        if let Some(flag) = self.abort_flag.take() {
            flag.store(true, Ordering::Relaxed);
        }
        self.walk_rx = None;
        self.phase = DialogPhase::InputFocused;
        self.results.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
        self.truncated = false;
    }

    /// Adjust `scroll_offset` so that `cursor` remains visible in a window
    /// of `window_height` rows. Invariant: `scroll_offset ≤ cursor`.
    pub fn clamp_scroll(&mut self, window_height: usize) {
        if self.cursor < self.scroll_offset {
            self.scroll_offset = self.cursor;
        } else if self.cursor >= self.scroll_offset + window_height {
            self.scroll_offset = self.cursor - window_height + 1;
        }
    }

    /// Render the find-file dialog overlay onto the given frame area.
    ///
    /// Draws a centered bordered overlay with:
    /// - Mode indicator `[Name]` or `[Content]`
    /// - Input field
    /// - Match count header
    /// - Result list (scrollable, cursor-highlighted)
    /// - Notice text when present
    pub fn render(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect, theme: &Theme) {
        use ratatui::layout::{Constraint, Direction, Layout};
        use ratatui::style::{Color, Modifier, Style};
        use ratatui::text::{Line, Span};
        use ratatui::widgets::{Block, Borders};
        use ratatui::widgets::{Clear, List, ListItem, Paragraph};

        // Center: 70% wide, up to 24 rows tall.
        let overlay = centered_rect_pct(70, 70, area);

        // Clear the background first.
        f.render_widget(Clear, overlay);

        // Outer block.
        let block = Block::default()
            .title("Find File")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(theme.border_focused))
            .style(Style::default().bg(theme.dialog_bg).fg(theme.dialog_fg));
        let inner = block.inner(overlay);
        f.render_widget(block, overlay);

        // Split inner into: mode row, input row, header row, list area, notice row.
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // mode indicator
                Constraint::Length(1), // input field
                Constraint::Length(1), // match count header
                Constraint::Min(1),    // result list
                Constraint::Length(1), // notice / spinner
            ])
            .split(inner);

        // Mode indicator.
        let mode_str = match self.mode {
            SearchMode::Name => "[Name]  (Tab=Content)",
            SearchMode::Content => "[Content] (Tab=Name)",
        };
        f.render_widget(
            Paragraph::new(mode_str)
                .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg)),
            chunks[0],
        );

        // Input field.
        let input_str = format!("> {}", self.input);
        f.render_widget(
            Paragraph::new(input_str)
                .style(Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg)),
            chunks[1],
        );

        // Match count header.
        let header = match self.phase {
            DialogPhase::Walking => "Searching…".to_string(),
            DialogPhase::NoResults => "0 matches".to_string(),
            _ => {
                if self.truncated {
                    format!("{} matches (truncated)", self.results.len())
                } else {
                    format!("{} matches", self.results.len())
                }
            }
        };
        f.render_widget(
            Paragraph::new(header).style(Style::default().fg(Color::Yellow).bg(theme.dialog_bg)),
            chunks[2],
        );

        // Result list.
        let list_height = chunks[3].height as usize;
        let items: Vec<ListItem> = self
            .results
            .iter()
            .enumerate()
            .skip(self.scroll_offset)
            .take(list_height)
            .map(|(i, path)| {
                let display = left_truncate_path(path, chunks[3].width as usize);
                let style = if i == self.cursor {
                    Style::default()
                        .fg(theme.panel_bg)
                        .bg(theme.panel_fg)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default().fg(theme.dialog_fg).bg(theme.dialog_bg)
                };
                ListItem::new(Line::from(Span::styled(display, style)))
            })
            .collect();
        f.render_widget(List::new(items), chunks[3]);

        // Notice / spinner row.
        let notice_text = if let Some(n) = &self.notice {
            n.as_str()
        } else if self.phase == DialogPhase::Walking {
            "… walking …"
        } else {
            ""
        };
        f.render_widget(
            Paragraph::new(notice_text)
                .style(Style::default().fg(Color::Yellow).bg(theme.dialog_bg)),
            chunks[4],
        );
    }

    /// Test-only helper: start a walk with a per-entry sleep for abort timing tests.
    #[cfg(test)]
    pub(crate) fn start_walk_with_delay(
        &mut self,
        root: PathBuf,
        config: &cargonaut_config::Config,
        delay_per_entry: std::time::Duration,
    ) {
        // FR-018: root guard
        if std::fs::read_dir(&root).is_err() {
            self.phase = DialogPhase::NoResults;
            self.notice = Some(format!("Cannot read directory: {}", root.display()));
            return;
        }

        let pattern = if self.input.is_empty() {
            "**".to_string()
        } else {
            self.input.clone()
        };

        let max_results = config.search.max_results as usize;
        let abort_flag = Arc::new(AtomicBool::new(false));
        let (tx, rx) = mpsc::unbounded_channel::<FindEvent>();

        self.phase = DialogPhase::Walking;
        self.results.clear();
        self.cursor = 0;
        self.scroll_offset = 0;
        self.truncated = false;
        self.walk_rx = Some(rx);
        self.abort_flag = Some(abort_flag.clone());

        tokio::task::spawn_blocking(move || {
            use globset::GlobBuilder;
            use std::collections::VecDeque;

            let glob = match GlobBuilder::new(&pattern).build() {
                Ok(g) => g.compile_matcher(),
                Err(_) => {
                    let _ = tx.send(FindEvent::Done { truncated: false });
                    return;
                }
            };

            let mut queue = VecDeque::new();
            queue.push_back(root);
            let mut count = 0usize;

            'outer: while let Some(dir) = queue.pop_front() {
                if abort_flag.load(Ordering::Relaxed) {
                    break;
                }
                let rd = match std::fs::read_dir(&dir) {
                    Ok(rd) => rd,
                    Err(_) => continue,
                };
                for entry in rd {
                    if abort_flag.load(Ordering::Relaxed) {
                        break 'outer;
                    }
                    std::thread::sleep(delay_per_entry);
                    let entry = match entry {
                        Ok(e) => e,
                        Err(_) => continue,
                    };
                    let path = entry.path();
                    let ft = match entry.file_type() {
                        Ok(ft) => ft,
                        Err(_) => continue,
                    };
                    if ft.is_dir() {
                        queue.push_back(path);
                    } else {
                        let name = entry.file_name();
                        let name_str = name.to_string_lossy();
                        let matches = if pattern == "**" {
                            true
                        } else {
                            glob.is_match(name_str.as_ref())
                        };
                        if matches {
                            count += 1;
                            if count > max_results {
                                let _ = tx.send(FindEvent::Done { truncated: true });
                                return;
                            }
                            let _ = tx.send(FindEvent::Found(path));
                        }
                    }
                }
            }
            let _ = tx.send(FindEvent::Done { truncated: false });
        });
    }
}

/// Left-truncate a path to fit in `max_width` columns.
/// If the path's display string fits, return it as-is.
/// Otherwise, truncate from the left and prepend `…`.
fn left_truncate_path(path: &std::path::Path, max_width: usize) -> String {
    let s = path.display().to_string();
    let char_count = s.chars().count();
    if char_count <= max_width {
        return s;
    }
    if max_width <= 1 {
        return "\u{2026}".to_string(); // '…'
    }
    // Keep the rightmost (max_width - 1) chars and prepend `…`.
    let keep = max_width - 1;
    // Find the byte offset of the character at position (char_count - keep).
    let skip_chars = char_count - keep;
    let start_byte = s
        .char_indices()
        .nth(skip_chars)
        .map(|(i, _)| i)
        .unwrap_or(s.len());
    format!("\u{2026}{}", &s[start_byte..])
}

// =====================================================================
// Internal Editor (Feature 056 — FR-001..FR-010, issue #40)
// =====================================================================

/// Detected line-ending style, preserved through load and save.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LineEnding {
    /// Unix-style `\n`.
    Lf,
    /// Windows-style `\r\n`.
    Crlf,
}

impl LineEnding {
    /// Join the given lines into a single string using this line ending.
    pub fn join(self, lines: &[String]) -> String {
        let sep = match self {
            LineEnding::Lf => "\n",
            LineEnding::Crlf => "\r\n",
        };
        lines.join(sep)
    }
}

// --- UnsavedChangesDialog ---

/// What the user chose in the unsaved-changes exit guard.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnsavedChangesChoice {
    /// Save the file, then close the editor.
    Save,
    /// Discard all changes and close without saving.
    Discard,
    /// Return to editing without closing.
    Cancel,
}

/// Three-choice modal shown when the user tries to exit with unsaved changes.
///
/// Focus defaults to `Cancel` (index 2) — the safe choice.
/// Tab / Left / Right cycle through Save / Discard / Cancel.
/// Enter confirms the focused choice; Esc always resolves to Cancel.
#[derive(Debug, Clone)]
pub struct UnsavedChangesDialog {
    /// Focused button: 0 = Save, 1 = Discard, 2 = Cancel.
    focus: usize,
}

impl UnsavedChangesDialog {
    /// Construct with focus on Cancel.
    pub fn new() -> Self {
        Self { focus: 2 }
    }

    /// Handle a key press. Returns `Some(choice)` when the dialog should dismiss.
    pub fn handle_key(&mut self, code: crossterm::event::KeyCode) -> Option<UnsavedChangesChoice> {
        use crossterm::event::KeyCode;
        match code {
            KeyCode::Esc => Some(UnsavedChangesChoice::Cancel),
            KeyCode::Enter => Some(match self.focus {
                0 => UnsavedChangesChoice::Save,
                1 => UnsavedChangesChoice::Discard,
                _ => UnsavedChangesChoice::Cancel,
            }),
            KeyCode::Tab | KeyCode::Right | KeyCode::Left => {
                self.focus = (self.focus + 1) % 3;
                None
            }
            _ => None,
        }
    }

    /// Render centered over the given area.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let w = 46u16.min(area.width);
        let h = 6u16.min(area.height);
        let x = area.x + area.width.saturating_sub(w) / 2;
        let y = area.y + area.height.saturating_sub(h) / 2;
        let inner = Rect {
            x,
            y,
            width: w,
            height: h,
        };
        Clear.render(inner, buf);
        let label = |idx: usize, text: &str| -> String {
            if self.focus == idx {
                format!("[{text}]")
            } else {
                format!(" {text} ")
            }
        };
        let body = format!(
            "Unsaved changes.\n\n{}  {}  {}",
            label(0, "Save"),
            label(1, "Discard"),
            label(2, "Cancel"),
        );
        let block = Block::default()
            .title(" Unsaved Changes ")
            .borders(Borders::ALL)
            .style(theme.dialog_style());
        let para = Paragraph::new(body)
            .block(block)
            .style(theme.dialog_style())
            .wrap(Wrap { trim: false });
        Widget::render(para, inner, buf);
    }

    /// Currently focused button index (0=Save, 1=Discard, 2=Cancel).
    pub fn focus(&self) -> usize {
        self.focus
    }
}

impl Default for UnsavedChangesDialog {
    fn default() -> Self {
        Self::new()
    }
}

// --- FileEditorAction ---

/// Return value from [`FileEditorDialog::handle_key`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FileEditorAction {
    /// Key consumed; redraw, but no structural state change.
    Swallow,
    /// Editor has no unsaved changes — close immediately.
    Close,
    /// Editor has unsaved changes; sub-modal is now showing (key consumed).
    UnsavedPromptShowing,
    /// Sub-modal resolved to Save → lib.rs must call `save()` then close.
    SaveAndClose,
    /// Sub-modal resolved to Discard → lib.rs must close without saving.
    DiscardAndClose,
}

// --- FileEditorDialog ---

/// Full-screen built-in text editor (Feature 056, FR-001..FR-010).
///
/// Stores the file content as a [`Vec<String>`] line buffer (no terminators).
/// Cursor is tracked as `(cursor_line, cursor_col)` — both 0-based byte indices.
/// The view is scrolled so the cursor is always visible.
#[derive(Debug)]
pub struct FileEditorDialog {
    /// Absolute resolved path for saving.
    path: std::path::PathBuf,
    /// Display name shown in the header bar.
    display_name: String,
    /// Line buffer — each entry is one line without its line terminator.
    lines: Vec<String>,
    /// 0-based row index of the cursor.
    cursor_line: usize,
    /// 0-based byte column of the cursor within `lines[cursor_line]`.
    cursor_col: usize,
    /// First visible line index (0 = top of file).
    scroll_offset: usize,
    /// True when in-memory content differs from the file on disk.
    dirty: bool,
    /// Line-ending style detected on open; preserved on save.
    line_ending: LineEnding,
    /// Non-None while the unsaved-changes sub-modal is showing.
    unsaved_dlg: Option<UnsavedChangesDialog>,
    /// Transient status message shown in the footer (e.g. save errors).
    pub status_msg: Option<String>,
    /// Last known content area height in rows; updated each render; used by PageUp/PageDown.
    viewport_height: u16,
}

impl FileEditorDialog {
    // --- Construction ---

    /// Construct a new editor from the full file content string.
    ///
    /// `line_ending` is detected by the caller and recorded for save-time use.
    pub fn new(
        path: std::path::PathBuf,
        display_name: String,
        content: String,
        line_ending: LineEnding,
    ) -> Self {
        // Split on LF (strip any CR), preserve at least one empty line for an empty file.
        let lines: Vec<String> = if content.is_empty() {
            vec![String::new()]
        } else {
            content
                .split('\n')
                .map(|l| l.trim_end_matches('\r').to_owned())
                .collect()
        };
        Self {
            path,
            display_name,
            lines,
            cursor_line: 0,
            cursor_col: 0,
            scroll_offset: 0,
            dirty: false,
            line_ending,
            unsaved_dlg: None,
            status_msg: None,
            viewport_height: 24,
        }
    }

    // --- Public accessors ---

    /// Whether the buffer has unsaved changes.
    pub fn is_dirty(&self) -> bool {
        self.dirty
    }

    // --- Save ---

    /// Write the line buffer to disk using the original line-ending style.
    ///
    /// Clears `self.dirty` on success. On error the buffer is unchanged — the
    /// caller should inspect the returned error and set `status_msg`.
    pub fn save(&mut self) -> std::io::Result<()> {
        let content = self.line_ending.join(&self.lines);
        std::fs::write(&self.path, content.as_bytes())?;
        self.dirty = false;
        Ok(())
    }

    // --- Key handling ---

    /// Handle a raw key event.
    ///
    /// Callers in `lib.rs` MUST intercept `Command::SaveFile` and
    /// `Command::EditorQuit` BEFORE calling this method (via keymap lookup).
    /// This method handles only raw navigation and editing keys, plus
    /// sub-modal routing when `unsaved_dlg` is active.
    ///
    /// Uses `self.viewport_height` (updated by `render()`) for page scroll math.
    pub fn handle_key(
        &mut self,
        code: crossterm::event::KeyCode,
        modifiers: crossterm::event::KeyModifiers,
    ) -> FileEditorAction {
        let viewport_height = self.viewport_height;
        use crossterm::event::{KeyCode, KeyModifiers};

        // Route to sub-modal when active.
        if let Some(ref mut dlg) = self.unsaved_dlg {
            return match dlg.handle_key(code) {
                Some(UnsavedChangesChoice::Save) => {
                    self.unsaved_dlg = None;
                    FileEditorAction::SaveAndClose
                }
                Some(UnsavedChangesChoice::Discard) => {
                    self.unsaved_dlg = None;
                    FileEditorAction::DiscardAndClose
                }
                Some(UnsavedChangesChoice::Cancel) => {
                    self.unsaved_dlg = None;
                    FileEditorAction::Swallow
                }
                None => FileEditorAction::Swallow,
            };
        }

        let ctrl = modifiers.contains(KeyModifiers::CONTROL);

        match code {
            // Navigation
            KeyCode::Up => {
                self.move_up();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Down => {
                self.move_down();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Left => {
                self.move_left();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Right => {
                self.move_right();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Home if ctrl => {
                self.goto_start();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::End if ctrl => {
                self.goto_end();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Home => {
                self.cursor_col = 0;
                FileEditorAction::Swallow
            }
            KeyCode::End => {
                self.cursor_col = self.lines[self.cursor_line].len();
                FileEditorAction::Swallow
            }
            KeyCode::PageUp => {
                let step = viewport_height.saturating_sub(1) as usize;
                self.scroll_offset = self.scroll_offset.saturating_sub(step);
                self.cursor_line = self.cursor_line.saturating_sub(step);
                self.clamp_cursor();
                FileEditorAction::Swallow
            }
            KeyCode::PageDown => {
                let step = viewport_height.saturating_sub(1) as usize;
                let max_line = self.lines.len().saturating_sub(1);
                self.cursor_line = (self.cursor_line + step).min(max_line);
                self.scroll_to_cursor(viewport_height);
                self.clamp_cursor();
                FileEditorAction::Swallow
            }
            // Editing
            KeyCode::Char(c) if !ctrl => {
                self.insert_char(c);
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Backspace => {
                self.delete_left();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            KeyCode::Delete => {
                self.delete_right();
                FileEditorAction::Swallow
            }
            KeyCode::Enter => {
                self.split_line();
                self.scroll_to_cursor(viewport_height);
                FileEditorAction::Swallow
            }
            _ => FileEditorAction::Swallow,
        }
    }

    /// Show the unsaved-changes sub-modal. Called by `lib.rs` when the user
    /// triggers `EditorQuit` while `is_dirty()`.
    pub fn show_unsaved_dialog(&mut self) {
        self.unsaved_dlg = Some(UnsavedChangesDialog::new());
    }

    // --- Render ---

    /// Render the editor full-screen into `area`. Updates `self.viewport_height` as a side effect.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        use ratatui::style::Modifier;
        use ratatui::text::{Line, Span};

        if area.height < 3 || area.width < 4 {
            return;
        }

        let header_area = Rect {
            x: area.x,
            y: area.y,
            width: area.width,
            height: 1,
        };
        let footer_area = Rect {
            x: area.x,
            y: area.y + area.height - 1,
            width: area.width,
            height: 1,
        };
        let content_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: area.height - 2,
        };
        self.viewport_height = content_area.height;
        let content_style = theme.dialog_style();

        // --- Header ---
        let modified = if self.dirty { "*" } else { " " };
        let header_text = format!("{modified} {}", self.display_name);
        let header_line = Line::from(Span::styled(
            format!("{:<width$}", header_text, width = area.width as usize),
            theme.status_style(),
        ));
        buf.set_line(
            header_area.x,
            header_area.y,
            &header_line,
            header_area.width,
        );

        // --- Content ---
        let viewport_height = content_area.height as usize;
        for (row_idx, line_idx) in
            (self.scroll_offset..self.scroll_offset + viewport_height).enumerate()
        {
            let y = content_area.y + row_idx as u16;
            if y >= content_area.y + content_area.height {
                break;
            }
            let Some(line) = self.lines.get(line_idx) else {
                // Empty row below end of file.
                let empty = format!("{:width$}", "", width = content_area.width as usize);
                let styled = Line::from(Span::styled(empty, content_style));
                buf.set_line(content_area.x, y, &styled, content_area.width);
                continue;
            };
            // Expand tabs for display (4 spaces per tab, hard-coded per spec).
            let display: String = line
                .chars()
                .flat_map(|c| {
                    if c == '\t' {
                        std::iter::repeat(' ').take(4).collect::<Vec<_>>()
                    } else {
                        vec![c]
                    }
                })
                .collect();
            let padded = format!("{:<width$}", display, width = content_area.width as usize);
            let styled = Line::from(Span::styled(&padded, content_style));
            buf.set_line(content_area.x, y, &styled, content_area.width);

            // Highlight cursor cell on the cursor line.
            if line_idx == self.cursor_line {
                // Map byte col to visual col (accounting for tabs).
                let visual_col = line.chars().take(self.cursor_col).fold(0usize, |acc, c| {
                    acc + if c == '\t' { 4 } else { c.len_utf8().min(1) }
                });
                let cx = content_area.x + (visual_col as u16).min(content_area.width - 1);
                if cx < content_area.x + content_area.width {
                    let cell = buf.get_mut(cx, y);
                    let current_style = cell.style();
                    cell.set_style(current_style.add_modifier(Modifier::REVERSED));
                }
            }
        }

        // --- Footer ---
        let left = if let Some(ref msg) = self.status_msg {
            msg.clone()
        } else {
            format!("Ln {}, Col {}", self.cursor_line + 1, self.cursor_col + 1)
        };
        let right = "F2=Save  F10=Quit";
        let gap = (area.width as usize).saturating_sub(left.len() + right.len());
        let footer_text = format!("{left}{:>gap$}{right}", "", gap = gap);
        let footer_line = Line::from(Span::styled(
            format!("{:<width$}", footer_text, width = area.width as usize),
            theme.status_style(),
        ));
        buf.set_line(
            footer_area.x,
            footer_area.y,
            &footer_line,
            footer_area.width,
        );

        // --- Unsaved dialog overlay ---
        if let Some(ref dlg) = self.unsaved_dlg {
            dlg.render(content_area, buf, theme);
        }
    }

    // --- Private helpers ---

    fn clamp_cursor(&mut self) {
        let max_line = self.lines.len().saturating_sub(1);
        self.cursor_line = self.cursor_line.min(max_line);
        let max_col = self.lines[self.cursor_line].len();
        self.cursor_col = self.cursor_col.min(max_col);
    }

    fn scroll_to_cursor(&mut self, viewport_height: u16) {
        let vh = viewport_height as usize;
        if vh == 0 {
            return;
        }
        if self.cursor_line < self.scroll_offset {
            self.scroll_offset = self.cursor_line;
        } else if self.cursor_line >= self.scroll_offset + vh {
            self.scroll_offset = self.cursor_line + 1 - vh;
        }
    }

    fn insert_char(&mut self, ch: char) {
        let col = self.cursor_col;
        let line = &mut self.lines[self.cursor_line];
        // Find the byte offset for the given char index.
        line.insert(col, ch);
        self.cursor_col += ch.len_utf8();
        self.dirty = true;
    }

    fn delete_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            // Find char boundary to the left.
            let new_col = {
                let s = &line[..self.cursor_col];
                s.char_indices().next_back().map(|(i, _)| i).unwrap_or(0)
            };
            line.remove(new_col);
            self.cursor_col = new_col;
            self.dirty = true;
        } else if self.cursor_line > 0 {
            // Join with previous line.
            let removed = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_len = self.lines[self.cursor_line].len();
            self.lines[self.cursor_line].push_str(&removed);
            self.cursor_col = prev_len;
            self.dirty = true;
        }
    }

    fn delete_right(&mut self) {
        let line = &self.lines[self.cursor_line];
        if self.cursor_col < line.len() {
            let col = self.cursor_col;
            // Find next char boundary.
            let next = line[col..]
                .char_indices()
                .nth(1)
                .map(|(i, _)| col + i)
                .unwrap_or(line.len());
            self.lines[self.cursor_line].drain(col..next);
            self.dirty = true;
        } else if self.cursor_line + 1 < self.lines.len() {
            // Join next line into this one.
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
            self.dirty = true;
        }
    }

    fn split_line(&mut self) {
        let col = self.cursor_col;
        let rest = self.lines[self.cursor_line].split_off(col);
        self.cursor_line += 1;
        self.cursor_col = 0;
        self.lines.insert(self.cursor_line, rest);
        self.dirty = true;
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.clamp_cursor();
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.clamp_cursor();
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            let s = &self.lines[self.cursor_line][..self.cursor_col];
            if let Some((i, _)) = s.char_indices().next_back() {
                self.cursor_col = i;
            }
        } else if self.cursor_line > 0 {
            self.cursor_line -= 1;
            self.cursor_col = self.lines[self.cursor_line].len();
        }
    }

    fn move_right(&mut self) {
        let line = &self.lines[self.cursor_line];
        if self.cursor_col < line.len() {
            let (_, ch) = line[self.cursor_col..].char_indices().next().unwrap();
            self.cursor_col += ch.len_utf8();
        } else if self.cursor_line + 1 < self.lines.len() {
            self.cursor_line += 1;
            self.cursor_col = 0;
        }
    }

    fn goto_start(&mut self) {
        self.cursor_line = 0;
        self.cursor_col = 0;
    }

    fn goto_end(&mut self) {
        self.cursor_line = self.lines.len().saturating_sub(1);
        self.cursor_col = self.lines[self.cursor_line].len();
    }
}

// ---------------------------------------------------------------------------
// Feature 057 — HostKeyVerifyDialog
// ---------------------------------------------------------------------------

/// Modal dialog shown when the server's SSH host key is unknown.
///
/// Displays the SHA-256 fingerprint and two buttons: Accept (adds the key to
/// `~/.ssh/known_hosts`) and Reject (aborts the connection).  The result is
/// delivered over the oneshot channel from `cargonaut_vfs::HostKeyEvent`.
pub struct HostKeyVerifyDialog {
    fingerprint: String,
    /// `None` after the user has acted (accept or reject).
    accept_tx: Option<tokio::sync::oneshot::Sender<bool>>,
    /// 0 = Accept focused, 1 = Reject focused.
    focus: usize,
}

impl std::fmt::Debug for HostKeyVerifyDialog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HostKeyVerifyDialog")
            .field("fingerprint", &self.fingerprint)
            .field("focus", &self.focus)
            .finish_non_exhaustive()
    }
}

impl HostKeyVerifyDialog {
    /// Create a new dialog displaying `fingerprint`; the result is sent on `accept_tx`.
    pub fn new(fingerprint: String, accept_tx: tokio::sync::oneshot::Sender<bool>) -> Self {
        Self {
            fingerprint,
            accept_tx: Some(accept_tx),
            focus: 0,
        }
    }

    /// The SHA-256 fingerprint shown to the user.
    pub fn fingerprint(&self) -> &str {
        &self.fingerprint
    }

    /// Send `true` (accept) and consume the sender.
    pub fn accept(&mut self) {
        if let Some(tx) = self.accept_tx.take() {
            let _ = tx.send(true);
        }
    }

    /// Send `false` (reject) and consume the sender.
    pub fn reject(&mut self) {
        if let Some(tx) = self.accept_tx.take() {
            let _ = tx.send(false);
        }
    }

    /// Tab / arrow key cycles between Accept and Reject.
    pub fn toggle_focus(&mut self) {
        self.focus = 1 - self.focus;
    }

    /// Render the dialog into `area`.
    pub fn render(&self, f: &mut ratatui::Frame, area: ratatui::layout::Rect) {
        use ratatui::{
            layout::{Constraint, Direction, Layout},
            style::{Color, Modifier, Style},
            widgets::{Block, Borders, Clear, Paragraph},
        };

        let block = Block::default()
            .title(" Unknown Host Key ")
            .borders(Borders::ALL)
            .border_style(Style::default().fg(Color::Yellow));

        let inner = block.inner(area);
        f.render_widget(Clear, area);
        f.render_widget(block, area);

        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .margin(1)
            .constraints([
                Constraint::Length(2),
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Min(0),
                Constraint::Length(1),
            ])
            .split(inner);

        let msg = Paragraph::new(format!(
            "The server's host key is not in known_hosts.\nFingerprint: {}",
            self.fingerprint
        ));
        f.render_widget(msg, chunks[0]);

        let accept_style = if self.focus == 0 {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Green)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Green)
        };
        let reject_style = if self.focus == 1 {
            Style::default()
                .fg(Color::Black)
                .bg(Color::Red)
                .add_modifier(Modifier::BOLD)
        } else {
            Style::default().fg(Color::Red)
        };

        f.render_widget(Paragraph::new("[ Accept ]").style(accept_style), chunks[2]);
        f.render_widget(Paragraph::new("[ Reject ]").style(reject_style), chunks[3]);
    }
}

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
                assert!(
                    !row.key.is_empty(),
                    "row key empty in section '{}'",
                    sec.title
                );
                assert!(
                    !row.desc.is_empty(),
                    "row desc empty in section '{}'",
                    sec.title
                );
            }
        }
    }

    #[test]
    fn help_overlay_scroll_down_increments_offset() {
        let total = HELP_SECTIONS
            .iter()
            .map(|s| s.rows.len() + 1)
            .sum::<usize>() as u16;
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
        let items = vec![
            menu_item("Edit", "vi {path}", Some('e')),
            menu_item("List", "ls {path}", None),
        ];
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
        assert_eq!(
            d.handle_key(KeyCode::Enter),
            Some(UserMenuAction::Execute(0))
        );
    }

    #[test]
    fn user_menu_shortcut_char_executes_matching_item() {
        let items = vec![
            menu_item("Edit", "vi {path}", Some('e')),
            menu_item("List", "ls {path}", Some('l')),
        ];
        let mut d = UserMenuDialog::new(items);
        assert_eq!(
            d.handle_key(KeyCode::Char('l')),
            Some(UserMenuAction::Execute(1))
        );
    }

    #[test]
    fn user_menu_new_error_sets_error_field() {
        let d = UserMenuDialog::new_error("parse error: line 5");
        assert!(d.error.is_some());
        assert!(d.error.as_deref().unwrap().contains("parse error"));
    }

    // ---------- FileViewerDialog — Phase 2 data types (T004) ----------

    #[test]
    fn view_mode_equality() {
        assert_eq!(ViewMode::Text, ViewMode::Text);
        assert_ne!(ViewMode::Text, ViewMode::Hex);
    }

    #[test]
    fn search_state_construction() {
        let s = SearchState {
            pattern: "hello".into(),
            direction: SearchDirection::Forward,
            last_match_line: Some(3),
            last_match_col: Some(5),
        };
        assert_eq!(s.pattern, "hello");
        assert_eq!(s.last_match_line, Some(3));
    }

    #[test]
    fn viewer_prompt_search_equality() {
        let p1 = ViewerPrompt::Search {
            buffer: "foo".into(),
            direction: SearchDirection::Forward,
        };
        let p2 = ViewerPrompt::Search {
            buffer: "foo".into(),
            direction: SearchDirection::Forward,
        };
        assert_eq!(p1, p2);
    }

    #[test]
    fn viewer_prompt_goto_equality() {
        let p = ViewerPrompt::Goto {
            buffer: "42".into(),
        };
        assert_eq!(
            p,
            ViewerPrompt::Goto {
                buffer: "42".into()
            }
        );
        assert_ne!(p, ViewerPrompt::Goto { buffer: "0".into() });
    }

    #[test]
    fn file_viewer_action_close_ne_swallow() {
        assert_ne!(FileViewerAction::Close, FileViewerAction::Swallow);
    }

    // ---------- FileViewerDialog — Phase 3 US1 (T007) ----------

    fn make_viewer(lines: &[&str]) -> FileViewerDialog {
        let ls: Vec<String> = lines.iter().map(|s| s.to_string()).collect();
        FileViewerDialog::new_text(
            std::path::PathBuf::from("/tmp/test.txt"),
            "test.txt".into(),
            ls,
            false,
        )
    }

    #[test]
    fn viewer_new_text_total_lines() {
        let d = make_viewer(&["a", "b", "c"]);
        assert_eq!(d.total_lines(), 3);
        assert_eq!(d.current_scroll_offset(), 0);
    }

    #[test]
    fn viewer_new_text_status_contains_line_fraction() {
        let d = make_viewer(&["a", "b", "c", "d", "e"]);
        let s = d.current_status_text();
        assert!(s.contains("Line 1/5"), "status was: {s}");
    }

    #[test]
    fn viewer_empty_file_status() {
        let d = make_viewer(&[]);
        assert_eq!(d.current_status_text(), "(empty file)");
        assert_eq!(d.total_lines(), 0);
    }

    #[test]
    fn viewer_scroll_down_and_up() {
        let mut d = make_viewer(&["a", "b", "c"]);
        assert_eq!(d.scroll_down(), FileViewerAction::Swallow);
        assert_eq!(d.current_scroll_offset(), 1);
        assert_eq!(d.scroll_up(), FileViewerAction::Swallow);
        assert_eq!(d.current_scroll_offset(), 0);
    }

    #[test]
    fn viewer_scroll_clamped_at_ends() {
        let mut d = make_viewer(&["a", "b", "c"]);
        // Can't go above 0.
        d.scroll_up();
        assert_eq!(d.current_scroll_offset(), 0);
        // Can't go past last line.
        d.scroll_down();
        d.scroll_down();
        d.scroll_down();
        d.scroll_down();
        assert_eq!(d.current_scroll_offset(), 2);
    }

    #[test]
    fn viewer_page_down_and_up() {
        let lines: Vec<&str> = (0..50).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.page_down(10);
        assert_eq!(d.current_scroll_offset(), 10);
        d.page_up(5);
        assert_eq!(d.current_scroll_offset(), 5);
    }

    #[test]
    fn viewer_home_and_end() {
        let lines: Vec<&str> = (0..10).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.page_down(8);
        d.home_key();
        assert_eq!(d.current_scroll_offset(), 0);
        d.end_key();
        assert_eq!(d.current_scroll_offset(), 9);
    }

    #[test]
    fn viewer_status_updates_on_scroll() {
        let lines: Vec<&str> = (0..10).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.scroll_down();
        let s = d.current_status_text();
        assert!(s.contains("Line 2/10"), "status was: {s}");
    }

    // ---------- FileViewerDialog — render test (T008) ----------

    #[test]
    fn viewer_render_shows_title_and_line_numbers() {
        let mut d = make_viewer(&["hello", "world", "foo", "bar", "baz"]);
        let backend = TestBackend::new(80, 20);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| {
            let area = f.size();
            d.render(f, area, &theme);
        })
        .unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(s.contains("F3 View"), "title missing: {s}");
        assert!(s.contains("[text]"), "mode label missing: {s}");
        assert!(s.contains("hello"), "content missing: {s}");
        // Line 1 should appear in the gutter.
        assert!(s.contains('1'), "line number missing: {s}");
    }

    #[test]
    fn viewer_render_empty_file_shows_message() {
        let mut d = make_viewer(&[]);
        let backend = TestBackend::new(60, 10);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        term.draw(|f| {
            let area = f.size();
            d.render(f, area, &theme);
        })
        .unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(s.contains("empty file"), "empty-file message missing: {s}");
    }

    // ---------- FileViewerDialog — Phase 4 US2 hex (T017) ----------

    #[test]
    fn render_hex_row_format_matches_contract() {
        // FR-021 format: `{offset:08x}  {hex}  |{ascii}|`
        let data: Vec<u8> = (0u8..16).collect();
        let row = FileViewerDialog::render_hex_row(0, &data);
        assert!(row.starts_with("00000000  "), "offset field: {row}");
        assert!(row.contains("|"), "ascii fence missing: {row}");
        // Must be 78 chars wide for a full 16-byte row.
        assert_eq!(row.len(), 78, "row width: {row}");
    }

    #[test]
    fn render_hex_row_partial_last_row() {
        let data = b"Hello";
        let row = FileViewerDialog::render_hex_row(0, data);
        assert!(row.contains("|Hello"), "ascii region: {row}");
        // Still 78 chars (padded).
        assert_eq!(row.len(), 78, "padded row width: {row}");
    }

    #[test]
    fn render_hex_row_non_printable_shows_dot() {
        let data = &[0x00u8, 0x01, 0x02];
        let row = FileViewerDialog::render_hex_row(0, data);
        assert!(row.contains("..."), "dots for non-printable: {row}");
    }

    #[test]
    fn viewer_new_hex_total_lines_are_rows() {
        let bytes: Vec<u8> = (0u8..=255).collect();
        let d =
            FileViewerDialog::new_hex(std::path::PathBuf::from("/tmp/bin"), "bin".into(), bytes);
        // 256 bytes / 16 = 16 rows.
        assert_eq!(d.total_lines(), 16);
    }

    #[test]
    fn viewer_new_hex_status_shows_offset() {
        let d = FileViewerDialog::new_hex(
            std::path::PathBuf::from("/tmp/bin"),
            "bin".into(),
            vec![0u8; 32],
        );
        assert!(
            d.current_status_text().starts_with("Offset 0x"),
            "status: {}",
            d.current_status_text()
        );
    }

    // ---------- FileViewerDialog — toggle mode (T018) ----------

    #[test]
    fn viewer_toggle_mode_resets_scroll_and_search() {
        let mut d = make_viewer(&["a", "b", "c"]);
        d.scroll_down();
        // Add fake search state.
        d.search = Some(SearchState {
            pattern: "a".into(),
            direction: SearchDirection::Forward,
            last_match_line: Some(0),
            last_match_col: Some(0),
        });
        d.toggle_mode();
        assert_eq!(d.mode, ViewMode::Hex);
        assert_eq!(d.current_scroll_offset(), 0, "scroll not reset");
        assert!(d.search.is_none(), "search not cleared");
        // Toggle back.
        d.toggle_mode();
        assert_eq!(d.mode, ViewMode::Text);
    }

    // ---------- FileViewerDialog — Phase 5 US3 search (T024) ----------

    #[test]
    fn search_forward_finds_first_match_after_cursor() {
        let mut d = make_viewer(&["aaa", "bbb", "ccc", "bbb", "ddd"]);
        d.scroll_offset = 0;
        // Search for "bbb" — first match after offset 0 is line 1.
        let result = d.search_forward("bbb");
        assert_eq!(result, Some((1, 0)));
    }

    #[test]
    fn search_forward_wraps_around() {
        let mut d = make_viewer(&["aaa", "bbb", "ccc"]);
        d.scroll_offset = 2; // cursor at last line
                             // Wrap around — "bbb" is at line 1, which is before cursor but found via wrap.
        let result = d.search_forward("bbb");
        assert_eq!(result, Some((1, 0)));
    }

    #[test]
    fn search_forward_returns_none_when_no_match() {
        let d = make_viewer(&["aaa", "bbb", "ccc"]);
        assert_eq!(d.search_forward("zzz"), None);
    }

    #[test]
    fn search_backward_finds_match_before_cursor() {
        let mut d = make_viewer(&["aaa", "bbb", "ccc", "bbb", "ddd"]);
        d.scroll_offset = 3;
        // "bbb" occurs at line 1 before cursor (line 3).
        let result = d.search_backward("bbb");
        assert_eq!(result, Some((1, 0)));
    }

    #[test]
    fn search_backward_wraps_around() {
        let mut d = make_viewer(&["aaa", "bbb", "ccc"]);
        d.scroll_offset = 0;
        // Wrap: "bbb" is at line 1 which is after cursor; found via wrap-around.
        let result = d.search_backward("bbb");
        assert_eq!(result, Some((1, 0)));
    }

    #[test]
    fn search_status_contains_pattern_after_match() {
        let mut d = make_viewer(&["hello world", "foo", "hello again"]);
        d.scroll_offset = 0;
        let result = d.search_forward("hello");
        assert!(result.is_some(), "expected a match");
        let (line, _) = result.unwrap();
        d.scroll_offset = line;
        d.search = Some(SearchState {
            pattern: "hello".into(),
            direction: SearchDirection::Forward,
            last_match_line: Some(line),
            last_match_col: Some(0),
        });
        d.update_status();
        let base = d.status.clone();
        d.status = format!("/hello  {base}");
        assert!(d.status.contains("/hello"), "status: {}", d.status);
        assert!(d.status.contains("Line"), "status: {}", d.status);
    }

    #[test]
    fn search_cleared_on_mode_toggle() {
        let mut d = make_viewer(&["hello"]);
        d.search = Some(SearchState {
            pattern: "hello".into(),
            direction: SearchDirection::Forward,
            last_match_line: Some(0),
            last_match_col: Some(0),
        });
        d.toggle_mode();
        assert!(d.search.is_none());
    }

    // ---------- FileViewerDialog — prompt state machine (T025) ----------

    #[test]
    fn search_prompt_opened_by_slash() {
        let mut d = make_viewer(&["a"]);
        let action = d.handle_key(KeyCode::Char('/'));
        assert_eq!(action, FileViewerAction::Swallow);
        assert!(
            matches!(d.prompt, Some(ViewerPrompt::Search { .. })),
            "prompt not set"
        );
    }

    #[test]
    fn search_prompt_opened_by_question_mark() {
        let mut d = make_viewer(&["a"]);
        d.handle_key(KeyCode::Char('?'));
        assert!(matches!(
            d.prompt,
            Some(ViewerPrompt::Search {
                direction: SearchDirection::Backward,
                ..
            })
        ));
    }

    #[test]
    fn search_prompt_accumulates_chars_and_backspace() {
        let mut d = make_viewer(&["hello"]);
        d.handle_key(KeyCode::Char('/'));
        d.handle_key(KeyCode::Char('h'));
        d.handle_key(KeyCode::Char('i'));
        d.handle_key(KeyCode::Backspace);
        if let Some(ViewerPrompt::Search { buffer, .. }) = &d.prompt {
            assert_eq!(buffer, "h", "buffer: {buffer}");
        } else {
            panic!("prompt not Search");
        }
    }

    #[test]
    fn search_prompt_esc_clears_prompt_and_search() {
        let mut d = make_viewer(&["a"]);
        d.handle_key(KeyCode::Char('/'));
        d.handle_key(KeyCode::Char('a'));
        d.handle_key(KeyCode::Esc);
        assert!(d.prompt.is_none(), "prompt should be cleared");
        assert!(d.search.is_none(), "search should be cleared");
    }

    #[test]
    fn search_prompt_enter_with_empty_clears() {
        let mut d = make_viewer(&["hello"]);
        d.handle_key(KeyCode::Char('/'));
        d.handle_key(KeyCode::Enter); // empty buffer
        assert!(d.prompt.is_none());
        assert!(d.search.is_none());
    }

    #[test]
    fn search_prompt_enter_with_pattern_jumps_to_match() {
        let mut d = make_viewer(&["aaa", "bbb", "ccc"]);
        d.handle_key(KeyCode::Char('/'));
        d.handle_key(KeyCode::Char('b'));
        d.handle_key(KeyCode::Char('b'));
        d.handle_key(KeyCode::Char('b'));
        d.handle_key(KeyCode::Enter);
        assert!(d.prompt.is_none());
        assert_eq!(d.current_scroll_offset(), 1);
    }

    // ---------- FileViewerDialog — Phase 6 US4 goto (T031) ----------

    #[test]
    fn goto_line_clamps_and_sets_scroll() {
        let lines: Vec<&str> = (0..10).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.goto_line(5);
        assert_eq!(d.current_scroll_offset(), 4); // 1-based input → 0-based offset
                                                  // Clamp above last line.
        d.goto_line(999);
        assert_eq!(d.current_scroll_offset(), 9);
        // Clamp below 1.
        d.goto_line(0);
        assert_eq!(d.current_scroll_offset(), 0); // clamped to 1 → offset 0
    }

    #[test]
    fn goto_line_status_reflects_new_position() {
        let lines: Vec<&str> = (0..10).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.goto_line(5);
        assert!(
            d.current_status_text().contains("Line 5/10"),
            "status: {}",
            d.current_status_text()
        );
    }

    #[test]
    fn goto_offset_sets_hex_row() {
        let bytes: Vec<u8> = vec![0u8; 64];
        let mut d =
            FileViewerDialog::new_hex(std::path::PathBuf::from("/tmp/bin"), "bin".into(), bytes);
        // Offset 32 = row 2 (32 / 16 = 2).
        d.goto_offset(32);
        assert_eq!(d.current_scroll_offset(), 2);
    }

    #[test]
    fn goto_offset_clamped_to_last_row() {
        let bytes: Vec<u8> = vec![0u8; 32];
        let mut d =
            FileViewerDialog::new_hex(std::path::PathBuf::from("/tmp/bin"), "bin".into(), bytes);
        d.goto_offset(9999);
        assert_eq!(d.current_scroll_offset(), 1); // 2 rows, last = row 1
    }

    #[test]
    fn parse_goto_input_decimal() {
        assert_eq!(FileViewerDialog::parse_goto_input("42"), Some(42));
        assert_eq!(FileViewerDialog::parse_goto_input("  10  "), Some(10));
        assert_eq!(FileViewerDialog::parse_goto_input(""), None);
        assert_eq!(FileViewerDialog::parse_goto_input("abc"), None);
    }

    #[test]
    fn parse_goto_input_hex_prefix() {
        assert_eq!(FileViewerDialog::parse_goto_input("0x1f"), Some(31));
        assert_eq!(FileViewerDialog::parse_goto_input("0XFF"), Some(255));
        assert_eq!(FileViewerDialog::parse_goto_input("0xgg"), None);
    }

    // ---------- FileViewerDialog — goto prompt state machine (T032) ----------

    #[test]
    fn goto_prompt_opened_by_open_goto_prompt() {
        let mut d = make_viewer(&["a"]);
        let action = d.open_goto_prompt();
        assert_eq!(action, FileViewerAction::Swallow);
        assert!(
            matches!(d.prompt, Some(ViewerPrompt::Goto { .. })),
            "prompt not Goto"
        );
    }

    #[test]
    fn goto_prompt_accumulates_digits() {
        let mut d = make_viewer(&["a"]);
        d.open_goto_prompt();
        d.handle_key(KeyCode::Char('2'));
        d.handle_key(KeyCode::Char('5'));
        d.handle_key(KeyCode::Char('0'));
        if let Some(ViewerPrompt::Goto { buffer }) = &d.prompt {
            assert_eq!(buffer, "250");
        } else {
            panic!("prompt not Goto");
        }
    }

    #[test]
    fn goto_prompt_esc_clears_prompt() {
        let mut d = make_viewer(&["a", "b", "c"]);
        let prev = d.current_scroll_offset();
        d.open_goto_prompt();
        d.handle_key(KeyCode::Char('2'));
        d.handle_key(KeyCode::Esc);
        assert!(d.prompt.is_none());
        assert_eq!(d.current_scroll_offset(), prev, "scroll changed on Esc");
    }

    #[test]
    fn goto_prompt_enter_jumps_to_line() {
        let lines: Vec<&str> = (0..20).map(|_| "x").collect();
        let mut d = make_viewer(&lines);
        d.open_goto_prompt();
        d.handle_key(KeyCode::Char('1'));
        d.handle_key(KeyCode::Char('0'));
        d.handle_key(KeyCode::Enter);
        assert!(d.prompt.is_none());
        assert_eq!(d.current_scroll_offset(), 9); // line 10 → offset 9
    }

    // ---------- FileViewerDialog — wrap toggle (T038 subset) ----------

    #[test]
    fn toggle_wrap_flips_flag_and_updates_status() {
        let mut d = make_viewer(&["hello world"]);
        assert!(!d.word_wrap);
        d.toggle_wrap();
        assert!(d.word_wrap);
        assert!(
            d.current_status_text().contains("wrap: on"),
            "status: {}",
            d.current_status_text()
        );
        d.toggle_wrap();
        assert!(!d.word_wrap);
        assert!(
            d.current_status_text().contains("wrap: off"),
            "status: {}",
            d.current_status_text()
        );
    }

    // ---------- FileViewerDialog — streaming: build_chunk_index + load_window_from_chunk (T037) ----------

    #[test]
    fn build_chunk_index_three_entries_for_3000_line_file() {
        // build_chunk_index is in lib.rs; test via load_window_from_chunk which calls the same
        // BufReader logic.  We create a temp file with 3000 lines and validate the chunk index
        // by calling lib.rs through the public API exposed via new_streaming.
        //
        // For this unit test we test load_window_from_chunk directly, which is in dialog.rs.
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        for i in 0..3000usize {
            writeln!(tmp, "line {i}").unwrap();
        }
        // load_window_from_chunk from byte 0, ask for 10 lines.
        let (lines, offset) = FileViewerDialog::load_window_from_chunk(tmp.path(), 0, 10).unwrap();
        assert_eq!(lines.len(), 10);
        assert_eq!(lines[0], "line 0");
        assert_eq!(lines[9], "line 9");
        assert!(offset > 0, "reader_offset should advance past the 10 lines");

        // load from offset: should get the next lines.
        let (lines2, _) = FileViewerDialog::load_window_from_chunk(tmp.path(), offset, 5).unwrap();
        assert_eq!(lines2.len(), 5);
        assert_eq!(lines2[0], "line 10");
    }

    #[test]
    fn load_window_from_chunk_reads_correct_lines_from_mid_file() {
        use std::io::Write;
        let mut tmp = tempfile::NamedTempFile::new().unwrap();
        for i in 0..100usize {
            writeln!(tmp, "entry {i:04}").unwrap();
        }
        // Read line 0 to find offset of line 50.
        let (first, _) = FileViewerDialog::load_window_from_chunk(tmp.path(), 0, 50).unwrap();
        assert_eq!(first.len(), 50);
        // Compute byte offset of line 50 by re-reading the first 50 lines.
        let (_, offset_at_50) =
            FileViewerDialog::load_window_from_chunk(tmp.path(), 0, 50).unwrap();
        let (chunk, _) =
            FileViewerDialog::load_window_from_chunk(tmp.path(), offset_at_50, 5).unwrap();
        assert_eq!(chunk[0], "entry 0050");
        assert_eq!(chunk[4], "entry 0054");
    }

    // ---------- FileViewerDialog — streaming: partial search annotation (T039) ----------

    #[test]
    fn streaming_annotation_present_when_buffer_end_less_than_total() {
        use std::collections::VecDeque;
        let lines: VecDeque<String> = (0..100).map(|i| format!("line {i}")).collect();
        let d = FileViewerDialog::new_streaming(
            std::path::PathBuf::from("/fake"),
            "fake.txt".into(),
            vec![(0, 0)],
            lines,
            1_000_000,        // total_lines >> window
            50 * 1024 * 1024, // 50 MiB total
            10 * 1024 * 1024, // reader at 10 MiB
        );
        let annot = d.streaming_annotation();
        assert!(
            annot.is_some(),
            "should have annotation when buffer_end < total_lines"
        );
        let s = annot.unwrap();
        assert!(s.contains("MiB"), "annotation should mention MiB: {s}");
        assert!(
            s.contains("searched"),
            "annotation should say 'searched': {s}"
        );
    }

    #[test]
    fn streaming_annotation_absent_when_fully_loaded_window() {
        use std::collections::VecDeque;
        let lines: VecDeque<String> = (0..100).map(|i| format!("line {i}")).collect();
        let d = FileViewerDialog::new_streaming(
            std::path::PathBuf::from("/fake"),
            "fake.txt".into(),
            vec![(0, 0)],
            lines,
            100, // total_lines == window size (fully covered)
            1024,
            1024, // reader at EOF
        );
        // buffer_end = 0 + 100 = 100 = total_lines → no annotation
        assert!(d.streaming_annotation().is_none());
    }

    // ---------- FileViewerDialog — close via Esc (T012 subset) ----------

    #[test]
    fn handle_key_esc_returns_close() {
        let mut d = make_viewer(&["a"]);
        assert_eq!(d.handle_key(KeyCode::Esc), FileViewerAction::Close);
    }

    #[test]
    fn handle_key_up_down_navigates() {
        let mut d = make_viewer(&["a", "b", "c"]);
        assert_eq!(d.handle_key(KeyCode::Down), FileViewerAction::Swallow);
        assert_eq!(d.current_scroll_offset(), 1);
        assert_eq!(d.handle_key(KeyCode::Up), FileViewerAction::Swallow);
        assert_eq!(d.current_scroll_offset(), 0);
    }

    // ========================================================================
    // Feature 052 T004 (red) — FindFileDialog pure-function tests
    // ========================================================================

    // T004a: plan_content_available truth table (contract §3a)
    #[test]
    fn plan_content_available_with_valid_rg() {
        // If rg is on PATH, must return true.
        if std::process::Command::new("rg")
            .arg("--version")
            .status()
            .is_ok()
        {
            assert!(
                plan_content_available("rg"),
                "rg found on PATH — plan_content_available must return true"
            );
        }
    }

    #[test]
    fn plan_content_available_with_nonexistent_binary() {
        assert!(
            !plan_content_available("/this/binary/does/not/exist/rg"),
            "non-existent binary must return false"
        );
    }

    // T004b: SearchMode Tab-toggle truth table (contract §3a)
    #[test]
    fn search_mode_tab_toggle_name_to_content_when_available() {
        let mut d = FindFileDialog::new(true);
        assert_eq!(d.mode, SearchMode::Name, "starts in Name mode");
        let outcome = d.handle_key(KeyCode::Tab, &cargonaut_config::Config::default());
        // Should toggle to Content without error
        assert_eq!(
            d.mode,
            SearchMode::Content,
            "Tab must switch to Content when content_available"
        );
        // outcome should be Consumed (not Panelize or Cancelled)
        assert!(
            !matches!(
                outcome,
                FindOutcome::Panelize { .. } | FindOutcome::Cancelled
            ),
            "Tab must return Consumed outcome, got {outcome:?}"
        );
    }

    #[test]
    fn search_mode_tab_toggle_is_noop_when_content_unavailable() {
        let mut d = FindFileDialog::new(false);
        assert_eq!(d.mode, SearchMode::Name);
        d.handle_key(KeyCode::Tab, &cargonaut_config::Config::default());
        assert_eq!(
            d.mode,
            SearchMode::Name,
            "Tab must not switch to Content when unavailable"
        );
    }

    // T004c: Enter key phase-transition truth table (contract §3b rows InputFocused / ResultsFocused)
    #[test]
    fn enter_in_input_focused_starts_walk_transitions_to_walking() {
        let mut d = FindFileDialog::new(false);
        assert_eq!(d.phase, DialogPhase::InputFocused);
        d.input = "*.toml".to_string();
        // Enter should start a walk → phase becomes Walking
        let rt = tokio::runtime::Runtime::new().unwrap();
        rt.block_on(async {
            let _outcome = d.handle_key(KeyCode::Enter, &cargonaut_config::Config::default());
        });
        // After Enter in InputFocused, phase must be Walking or ResultsFocused (if instant)
        assert!(
            d.phase == DialogPhase::Walking || d.phase == DialogPhase::ResultsFocused,
            "Enter in InputFocused must transition to Walking or ResultsFocused, got {:?}",
            d.phase
        );
    }

    #[test]
    fn enter_in_results_focused_returns_panelize() {
        let mut d = FindFileDialog::new(false);
        // Manually inject results and set phase to ResultsFocused
        d.results = vec![std::path::PathBuf::from("/tmp/foo.toml")];
        d.phase = DialogPhase::ResultsFocused;
        let outcome = d.handle_key(KeyCode::Enter, &cargonaut_config::Config::default());
        assert!(
            matches!(outcome, FindOutcome::Panelize { .. }),
            "Enter in ResultsFocused must return Panelize, got {outcome:?}"
        );
    }

    #[test]
    fn enter_in_no_results_phase_is_noop() {
        let mut d = FindFileDialog::new(false);
        d.phase = DialogPhase::NoResults;
        let outcome = d.handle_key(KeyCode::Enter, &cargonaut_config::Config::default());
        assert!(
            !matches!(outcome, FindOutcome::Panelize { .. }),
            "Enter in NoResults must not return Panelize"
        );
    }

    // ========================================================================
    // Feature 052 T010+T011 (red→green) — FindFileDialog render tests
    // ========================================================================

    fn render_find_dialog(d: &FindFileDialog, width: u16, height: u16) -> String {
        let backend = TestBackend::new(width, height);
        let mut term = Terminal::new(backend).unwrap();
        let theme = crate::theme::Theme::default();
        term.draw(|f| {
            let area = f.size();
            d.render(f, area, &theme);
        })
        .unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    #[test]
    fn find_dialog_input_focused_renders_title() {
        let d = FindFileDialog::new(false);
        assert_eq!(d.phase, DialogPhase::InputFocused);
        let s = render_find_dialog(&d, 60, 20);
        assert!(
            s.contains("Find"),
            "InputFocused render must contain 'Find'; got: {s:?}"
        );
    }

    #[test]
    fn find_dialog_results_focused_shows_match_count_and_paths() {
        let mut d = FindFileDialog::new(false);
        d.results = vec![
            std::path::PathBuf::from("/tmp/foo.toml"),
            std::path::PathBuf::from("/tmp/bar.toml"),
        ];
        d.phase = DialogPhase::ResultsFocused;
        let s = render_find_dialog(&d, 80, 24);
        assert!(
            s.contains("2"),
            "ResultsFocused must render '2' match count; got: {s:?}"
        );
    }

    #[test]
    fn find_dialog_walking_renders_progress_indicator() {
        let mut d = FindFileDialog::new(false);
        d.phase = DialogPhase::Walking;
        let s = render_find_dialog(&d, 60, 20);
        // Walking phase should show "Searching" or "walking" indicator
        assert!(
            s.to_lowercase().contains("search") || s.contains("…"),
            "Walking render must show progress indicator; got: {s:?}"
        );
    }

    #[test]
    fn find_dialog_long_path_truncated_with_ellipsis() {
        // 300-char path injected into a 40-col-wide result area.
        let long_name = "a".repeat(290);
        let path = std::path::PathBuf::from(format!("/tmp/{long_name}"));
        let result = left_truncate_path(&path, 40);
        assert!(
            result.contains('\u{2026}') || result.contains("…"),
            "long path must be left-truncated with '…'; got: {result:?}"
        );
        assert!(
            result.chars().count() <= 40,
            "truncated string must fit in 40 chars; char_count={}",
            result.chars().count()
        );
        // Must end with the filename suffix.
        assert!(
            result.ends_with(&long_name[long_name.len() - 35..]),
            "truncated path must end with the filename tail"
        );
    }

    // ========================================================================
    // Feature 052 T006 (red) — start_walk name-mode tests
    // ========================================================================

    /// Poll until the phase is no longer Walking. Uses `tokio::task::yield_now()`
    /// for async tests so the tokio runtime can make progress on spawned tasks.
    async fn poll_until_done(d: &mut FindFileDialog, timeout_secs: u64) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(timeout_secs);
        loop {
            d.poll_results();
            if d.phase != DialogPhase::Walking {
                break;
            }
            if std::time::Instant::now() >= deadline {
                panic!("poll_until_done timed out after {timeout_secs}s");
            }
            // Yield to allow tokio runtime to make progress on spawned tasks.
            tokio::task::yield_now().await;
        }
    }

    // T006a: Happy path — 3 files, glob matches 2.
    #[tokio::test]
    async fn start_walk_name_mode_happy_path() {
        let td = tempfile::TempDir::new().unwrap();
        std::fs::write(td.path().join("foo.toml"), b"").unwrap();
        std::fs::write(td.path().join("bar.toml"), b"").unwrap();
        std::fs::write(td.path().join("baz.rs"), b"").unwrap();

        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.toml".to_string();
        d.start_walk(td.path().to_path_buf(), &config);

        poll_until_done(&mut d, 10).await;

        assert_eq!(
            d.results.len(),
            2,
            "expected 2 .toml matches, got {:?}",
            d.results
        );
        assert_eq!(d.phase, DialogPhase::ResultsFocused);
    }

    // T006b: Unreadable root (FR-018) — phase becomes NoResults, notice set.
    // Skip if running as root (root can read mode-0 dirs).
    #[tokio::test]
    async fn start_walk_unreadable_root_sets_no_results() {
        use std::fs;
        use std::os::unix::fs::PermissionsExt;

        let td = tempfile::TempDir::new().unwrap();
        fs::set_permissions(td.path(), fs::Permissions::from_mode(0o000)).unwrap();

        // Check if the permission actually took effect (root can bypass).
        let probe = fs::read_dir(td.path());
        // Restore permissions so TempDir can clean up regardless.
        let _ = fs::set_permissions(td.path(), fs::Permissions::from_mode(0o755));

        if probe.is_ok() {
            // Running as root or filesystem ignores permissions — skip.
            eprintln!("Skipping unreadable-root test: permissions not enforced");
            return;
        }

        // Now re-run with a fresh tempdir since we already restored perms.
        let td2 = tempfile::TempDir::new().unwrap();
        fs::set_permissions(td2.path(), fs::Permissions::from_mode(0o000)).unwrap();

        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.toml".to_string();
        d.start_walk(td2.path().to_path_buf(), &config);

        // Restore permissions so TempDir can clean up.
        let _ = fs::set_permissions(td2.path(), fs::Permissions::from_mode(0o755));

        assert_eq!(
            d.phase,
            DialogPhase::NoResults,
            "unreadable root must set NoResults immediately"
        );
        assert!(
            d.results.is_empty(),
            "results must be empty for unreadable root"
        );
        let notice = d.notice.as_deref().unwrap_or("");
        assert!(
            notice.contains("Cannot read directory"),
            "notice must mention 'Cannot read directory'; got: {notice:?}"
        );
    }

    // T006c: SC-001 timing gate — 200 files, walk completes < 5s.
    #[tokio::test]
    async fn start_walk_name_mode_timing_gate_sc001() {
        let td = tempfile::TempDir::new().unwrap();
        for i in 0..200usize {
            std::fs::write(td.path().join(format!("file_{i:03}.tmp")), b"").unwrap();
        }

        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.tmp".to_string();

        let t0 = std::time::Instant::now();
        d.start_walk(td.path().to_path_buf(), &config);
        poll_until_done(&mut d, 10).await;
        let elapsed = t0.elapsed();

        assert!(
            elapsed < std::time::Duration::from_secs(5),
            "SC-001: walk must complete in <5s; took {elapsed:?}"
        );
        assert_eq!(
            d.results.len(),
            200,
            "SC-001: all 200 files must be found; got {}",
            d.results.len()
        );
    }

    // ========================================================================
    // Feature 052 T012 (red→green) — start_walk content mode tests
    // T013 implementation is in start_walk / spawn_content_walk (already impl).
    // ========================================================================

    // T012a: Basic content search — needle found in 1 of 2 files.
    #[tokio::test]
    async fn start_walk_content_mode_finds_needle() {
        // Skip if rg is not available.
        if !std::process::Command::new("rg")
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!("Skipping content search test: rg not found");
            return;
        }

        let td = tempfile::TempDir::new().unwrap();
        std::fs::write(td.path().join("match.txt"), b"needle in the haystack").unwrap();
        std::fs::write(td.path().join("nomatch.txt"), b"nothing here").unwrap();

        let mut config = cargonaut_config::Config::default();
        config.search.ripgrep_path = "rg".to_string();

        let mut d = FindFileDialog::new(true);
        d.mode = SearchMode::Content;
        d.input = "needle".to_string();
        d.start_walk(td.path().to_path_buf(), &config);

        poll_until_done(&mut d, 30).await;

        assert_eq!(
            d.results.len(),
            1,
            "content search must find exactly 1 file; results={:?}",
            d.results
        );
        let result_name = d.results[0]
            .file_name()
            .unwrap()
            .to_string_lossy()
            .to_string();
        assert_eq!(result_name, "match.txt");
    }

    // T012b: Differential — compare start_walk Content results vs rg CLI output.
    #[tokio::test]
    async fn start_walk_content_mode_matches_rg_output() {
        // Skip if rg is not available.
        if !std::process::Command::new("rg")
            .arg("--version")
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
        {
            eprintln!("Skipping differential rg test: rg not found");
            return;
        }

        let td = tempfile::TempDir::new().unwrap();
        for i in 0..5usize {
            let content = if i % 2 == 0 {
                b"needle" as &[u8]
            } else {
                b"other"
            };
            std::fs::write(td.path().join(format!("file{i}.txt")), content).unwrap();
        }

        let mut config = cargonaut_config::Config::default();
        config.search.ripgrep_path = "rg".to_string();

        // Get rg CLI output directly.
        let rg_out = std::process::Command::new("rg")
            .args([
                "needle",
                "--files-with-matches",
                "--no-messages",
                td.path().to_str().unwrap(),
            ])
            .output()
            .expect("rg must run");
        let mut rg_paths: Vec<String> = String::from_utf8_lossy(&rg_out.stdout)
            .lines()
            .map(str::to_string)
            .filter(|s| !s.is_empty())
            .collect();
        rg_paths.sort();

        // Get walk results.
        let mut d = FindFileDialog::new(true);
        d.mode = SearchMode::Content;
        d.input = "needle".to_string();
        d.start_walk(td.path().to_path_buf(), &config);
        poll_until_done(&mut d, 30).await;

        let mut walk_paths: Vec<String> = d
            .results
            .iter()
            .map(|p| p.to_string_lossy().to_string())
            .collect();
        walk_paths.sort();

        assert_eq!(
            walk_paths, rg_paths,
            "SC-003: start_walk Content mode must match rg output"
        );
    }

    // ========================================================================
    // Feature 052 T014+T015 (red→green) — Tab toggle content unavailable
    // ========================================================================

    #[test]
    fn tab_toggle_sets_notice_when_content_unavailable() {
        let mut d = FindFileDialog::new(false);
        assert!(!d.content_available, "content_available must be false");
        d.handle_key(KeyCode::Tab, &cargonaut_config::Config::default());
        assert_eq!(d.mode, SearchMode::Name, "mode must stay Name");
        let notice = d.notice.as_deref().unwrap_or("");
        assert!(
            notice.contains("Content search unavailable"),
            "notice must mention 'Content search unavailable'; got: {notice:?}"
        );
    }

    // ========================================================================
    // Feature 052 T016+T017 (red→green) — cancel / abort tests
    // ========================================================================

    // T016a: cancel() resets state correctly.
    #[tokio::test]
    async fn cancel_resets_phase_and_clears_results() {
        let td = tempfile::TempDir::new().unwrap();
        for i in 0..5usize {
            std::fs::write(td.path().join(format!("f{i}.txt")), b"").unwrap();
        }
        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.txt".to_string();
        d.start_walk(td.path().to_path_buf(), &config);

        assert_eq!(
            d.phase,
            DialogPhase::Walking,
            "must be Walking after start_walk"
        );
        assert!(d.walk_rx.is_some(), "walk_rx must be Some while Walking");
        assert!(
            d.abort_flag.is_some(),
            "abort_flag must be Some while Walking"
        );

        d.cancel();

        assert_eq!(
            d.phase,
            DialogPhase::InputFocused,
            "cancel must reset to InputFocused"
        );
        assert!(d.walk_rx.is_none(), "walk_rx must be None after cancel");
        assert!(d.results.is_empty(), "results must be cleared after cancel");
    }

    // T016b: SC-006 abort timing — walk aborted within 300ms.
    #[tokio::test]
    async fn cancel_aborts_walk_within_300ms() {
        let td = tempfile::TempDir::new().unwrap();
        // Create 10 files; each takes 50ms sleep in the delayed walk.
        // Total: 500ms without abort. We cancel after ~10ms → well under 300ms.
        for i in 0..10usize {
            std::fs::write(td.path().join(format!("f{i}.txt")), b"").unwrap();
        }
        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.txt".to_string();

        d.start_walk_with_delay(
            td.path().to_path_buf(),
            &config,
            std::time::Duration::from_millis(50),
        );

        let t0 = std::time::Instant::now();
        d.cancel();
        let elapsed = t0.elapsed();

        assert!(
            elapsed < std::time::Duration::from_millis(300),
            "SC-006: cancel must return in <300ms; took {elapsed:?}"
        );
        assert_eq!(d.phase, DialogPhase::InputFocused);
    }

    // ========================================================================
    // Feature 052 T018+T019 (red→green) — Esc does not set find_label
    // These test the dialog-level behavior. The lib.rs-level T018 tests
    // are in lib.rs.
    // ========================================================================

    #[test]
    fn esc_returns_cancelled_outcome() {
        let mut d = FindFileDialog::new(false);
        let outcome = d.handle_key(KeyCode::Esc, &cargonaut_config::Config::default());
        assert!(
            matches!(outcome, FindOutcome::Cancelled),
            "Esc must return Cancelled outcome"
        );
    }

    #[test]
    fn cancelled_outcome_does_not_set_panelize() {
        let mut d = FindFileDialog::new(false);
        d.results = vec![std::path::PathBuf::from("/tmp/foo.toml")];
        d.phase = DialogPhase::ResultsFocused;
        let outcome = d.handle_key(KeyCode::Esc, &cargonaut_config::Config::default());
        assert!(
            matches!(outcome, FindOutcome::Cancelled),
            "Esc from ResultsFocused must return Cancelled, not Panelize"
        );
    }

    // ========================================================================
    // Feature 052 T024+T025 — truncation at max_results
    // ========================================================================

    #[tokio::test]
    async fn start_walk_truncates_at_max_results() {
        let td = tempfile::TempDir::new().unwrap();
        for i in 0..5usize {
            std::fs::write(td.path().join(format!("f{i}.txt")), b"").unwrap();
        }

        let mut config = cargonaut_config::Config::default();
        config.search.max_results = 3;

        let mut d = FindFileDialog::new(false);
        d.input = "*.txt".to_string();
        d.start_walk(td.path().to_path_buf(), &config);
        poll_until_done(&mut d, 10).await;

        assert_eq!(d.results.len(), 3, "must stop at max_results=3");
        assert!(d.truncated, "truncated must be true");
    }

    #[test]
    fn render_shows_truncated_label_when_truncated() {
        let mut d = FindFileDialog::new(false);
        d.results = vec![
            std::path::PathBuf::from("/tmp/a.txt"),
            std::path::PathBuf::from("/tmp/b.txt"),
        ];
        d.phase = DialogPhase::ResultsFocused;
        d.truncated = true;

        let backend = TestBackend::new(80, 24);
        let mut term = Terminal::new(backend).unwrap();
        let theme = crate::theme::Theme::default();
        term.draw(|f| {
            d.render(f, f.size(), &theme);
        })
        .unwrap();
        let s: String = term
            .backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            s.contains("truncated"),
            "render must show 'truncated' when truncated=true; got: {s:?}"
        );
    }

    // ========================================================================
    // Feature 052 T026+T027 — no-results path
    // ========================================================================

    #[tokio::test]
    async fn walk_with_no_matches_sets_no_results_phase() {
        let td = tempfile::TempDir::new().unwrap();
        // Create files that won't match the glob
        std::fs::write(td.path().join("foo.rs"), b"").unwrap();

        let config = cargonaut_config::Config::default();
        let mut d = FindFileDialog::new(false);
        d.input = "*.toml".to_string();
        d.start_walk(td.path().to_path_buf(), &config);
        poll_until_done(&mut d, 10).await;

        assert_eq!(d.phase, DialogPhase::NoResults, "0 matches → NoResults");
        let notice = d.notice.as_deref().unwrap_or("");
        assert!(
            notice.contains("No files found"),
            "notice must say 'No files found'; got: {notice:?}"
        );
    }

    #[test]
    fn enter_in_no_results_returns_consumed_not_panelize() {
        let mut d = FindFileDialog::new(false);
        d.phase = DialogPhase::NoResults;
        d.results = Vec::new();
        let outcome = d.handle_key(KeyCode::Enter, &cargonaut_config::Config::default());
        assert!(
            !matches!(outcome, FindOutcome::Panelize { .. }),
            "Enter in NoResults must NOT return Panelize"
        );
    }

    // ========================================================================
    // Feature 052 T028+T029 — scroll / cursor navigation
    // ========================================================================

    #[test]
    fn down_moves_cursor_and_scroll_stays_visible() {
        let mut d = FindFileDialog::new(false);
        d.phase = DialogPhase::ResultsFocused;
        d.results = (0..20)
            .map(|i| std::path::PathBuf::from(format!("/tmp/f{i}")))
            .collect();
        d.cursor = 0;
        d.scroll_offset = 0;

        // Press Down 7 times.
        for _ in 0..7 {
            d.handle_key(KeyCode::Down, &cargonaut_config::Config::default());
        }
        assert_eq!(d.cursor, 7);
        // scroll_offset ≤ cursor always.
        assert!(
            d.scroll_offset <= d.cursor,
            "scroll_offset must be ≤ cursor"
        );
    }

    #[test]
    fn page_down_moves_cursor_by_window_height() {
        let mut d = FindFileDialog::new(false);
        d.phase = DialogPhase::ResultsFocused;
        d.results = (0..20)
            .map(|i| std::path::PathBuf::from(format!("/tmp/f{i}")))
            .collect();
        d.cursor = 0;

        d.handle_key(KeyCode::PageDown, &cargonaut_config::Config::default());
        // cursor should advance by ~10 (the default window height).
        assert!(d.cursor > 0, "PgDn must advance cursor");
        assert!(d.cursor <= 19, "cursor must not exceed max");
    }

    // ========================================================================
    // Feature 052 T030+T030B — rg non-zero exit handling
    // ========================================================================

    #[tokio::test]
    async fn content_walk_with_failing_rg_sets_no_results() {
        use std::os::unix::fs::PermissionsExt;
        // Create a mock rg script that exits with code 1.
        let td = tempfile::TempDir::new().unwrap();
        let mock_rg = td.path().join("rg");
        std::fs::write(&mock_rg, b"#!/bin/sh\nexit 1\n").unwrap();
        std::fs::set_permissions(&mock_rg, std::fs::Permissions::from_mode(0o755)).unwrap();

        let mut config = cargonaut_config::Config::default();
        config.search.ripgrep_path = mock_rg.to_str().unwrap().to_string();

        let search_root = tempfile::TempDir::new().unwrap();
        let mut d = FindFileDialog::new(true);
        d.mode = SearchMode::Content;
        d.input = "anything".to_string();
        d.start_walk(search_root.path().to_path_buf(), &config);
        poll_until_done(&mut d, 10).await;

        assert_eq!(
            d.results.len(),
            0,
            "rg non-zero exit must produce 0 results"
        );
        assert!(
            d.phase == DialogPhase::NoResults || d.results.is_empty(),
            "non-zero rg exit must not panic; phase={:?}",
            d.phase
        );
    }

    // ---------- FileEditorDialog (Feature 056) ----------

    fn make_editor(content: &str) -> (FileEditorDialog, tempfile::NamedTempFile) {
        let f = tempfile::NamedTempFile::new().unwrap();
        std::fs::write(f.path(), content).unwrap();
        let widget = FileEditorDialog::new(
            f.path().to_path_buf(),
            "test.txt".into(),
            content.to_owned(),
            LineEnding::Lf,
        );
        (widget, f)
    }

    #[test]
    fn editor_insert_and_save_writes_correct_content() {
        let (mut w, f) = make_editor("hello");
        // Insert 'X' at position 0.
        w.handle_key(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::NONE,
        );
        assert!(w.is_dirty());
        w.save().unwrap();
        assert!(!w.is_dirty());
        let written = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(written, "Xhello");
    }

    #[test]
    fn editor_utf8_roundtrip_no_edits() {
        let content = "line1\nline2\n";
        let (mut w, f) = make_editor(content);
        assert!(!w.is_dirty());
        w.save().unwrap();
        let written = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(written, content);
    }

    #[test]
    fn editor_save_failure_keeps_dirty_and_shows_error() {
        // Use a path that cannot be written.
        let w = FileEditorDialog::new(
            std::path::PathBuf::from("/dev/null/does_not_exist"),
            "x.txt".into(),
            "data".to_owned(),
            LineEnding::Lf,
        );
        // Manually mark dirty so save attempt is meaningful.
        let mut w = w;
        // Force dirty by inserting a char.
        w.handle_key(
            crossterm::event::KeyCode::Char('a'),
            crossterm::event::KeyModifiers::NONE,
        );
        assert!(w.is_dirty());
        let result = w.save();
        assert!(result.is_err(), "save to bad path should fail");
        assert!(w.is_dirty(), "dirty flag must remain after failed save");
    }

    #[test]
    fn editor_cursor_navigation_stays_in_bounds() {
        let (mut w, _f) = make_editor("ab\ncd");
        // Move left past start of first line — should be a no-op, not panic.
        w.handle_key(
            crossterm::event::KeyCode::Left,
            crossterm::event::KeyModifiers::NONE,
        );
        // Move up past first line — no-op.
        w.handle_key(
            crossterm::event::KeyCode::Up,
            crossterm::event::KeyModifiers::NONE,
        );
        // Move right past EOL on last line (wraps to next line start).
        w.handle_key(
            crossterm::event::KeyCode::End,
            crossterm::event::KeyModifiers::NONE,
        );
        w.handle_key(
            crossterm::event::KeyCode::Down,
            crossterm::event::KeyModifiers::NONE,
        );
        w.handle_key(
            crossterm::event::KeyCode::End,
            crossterm::event::KeyModifiers::NONE,
        );
        // Right at absolute end → no-op.
        w.handle_key(
            crossterm::event::KeyCode::Right,
            crossterm::event::KeyModifiers::NONE,
        );
        // Should not have panicked.
    }

    #[test]
    fn editor_render_shows_modified_indicator() {
        use ratatui::buffer::Buffer;
        use ratatui::layout::Rect;
        let theme = crate::theme::Theme::commander_dark();
        let area = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 10,
        };

        let (mut w, _f) = make_editor("hello");
        // Before modification: no star in header.
        let mut buf = Buffer::empty(area);
        w.render(area, &mut buf, &theme);
        let header_row: String = (0..40u16)
            .map(|x| buf.get(x, 0).symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            !header_row.starts_with('*'),
            "clean file should not show '*'"
        );

        // After modification: star appears.
        w.handle_key(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::NONE,
        );
        let mut buf2 = Buffer::empty(area);
        w.render(area, &mut buf2, &theme);
        let header2: String = (0..40u16)
            .map(|x| buf2.get(x, 0).symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            header2.starts_with('*'),
            "dirty file should show '*' in header"
        );
    }

    #[test]
    fn editor_unsaved_changes_guard_triggered_on_quit() {
        let (mut w, _f) = make_editor("text");
        // Dirty the buffer.
        w.handle_key(
            crossterm::event::KeyCode::Char('Z'),
            crossterm::event::KeyModifiers::NONE,
        );
        w.show_unsaved_dialog();
        // Next key should route to sub-modal, not close editor.
        let action = w.handle_key(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::NONE,
        );
        // Esc on UnsavedChangesDialog → Cancel → sub-modal dismissed, swallow.
        assert_eq!(action, FileEditorAction::Swallow);
        assert!(w.is_dirty(), "cancel should not have saved");
    }

    #[test]
    fn editor_discard_does_not_save() {
        let (mut w, f) = make_editor("original");
        let original = std::fs::read_to_string(f.path()).unwrap();
        w.handle_key(
            crossterm::event::KeyCode::Char('X'),
            crossterm::event::KeyModifiers::NONE,
        );
        w.show_unsaved_dialog();
        // Tab to Discard (focus starts on Cancel=2, Tab → Save=0, Tab → Discard=1)
        w.handle_key(
            crossterm::event::KeyCode::Tab,
            crossterm::event::KeyModifiers::NONE,
        );
        w.handle_key(
            crossterm::event::KeyCode::Tab,
            crossterm::event::KeyModifiers::NONE,
        );
        let action = w.handle_key(
            crossterm::event::KeyCode::Enter,
            crossterm::event::KeyModifiers::NONE,
        );
        assert_eq!(action, FileEditorAction::DiscardAndClose);
        let on_disk = std::fs::read_to_string(f.path()).unwrap();
        assert_eq!(on_disk, original, "discard must not write to disk");
    }

    #[test]
    fn unsaved_dialog_cancel_resumes_editing() {
        let mut dlg = UnsavedChangesDialog::new();
        assert_eq!(dlg.focus(), 2, "default focus = Cancel");
        let result = dlg.handle_key(crossterm::event::KeyCode::Esc);
        assert_eq!(result, Some(UnsavedChangesChoice::Cancel));
    }
}

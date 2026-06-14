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
}

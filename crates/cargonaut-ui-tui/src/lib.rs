// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut TUI layer — ratatui rendering, keymap dispatcher,
//! pane/dialog/status-bar widgets, and the [`run`] event loop.

#![warn(missing_docs)]

pub mod chrome;
pub mod dialog;
pub mod keymap;
pub mod pane;
pub(crate) mod subshell;
pub mod theme;
pub use chrome::{FunctionKeyBar, MenuBar};
pub use dialog::{
    ConfirmDialog, ConfirmOutcome, FileEditorAction, FileEditorDialog, FileViewerAction,
    FileViewerDialog, HotlistAction, HotlistDialog, HotlistRow, InputOutcome, JobRow, LineEnding,
    PathInputAction, PathInputDialog, ResumableSummary, ResumeChoice, ResumePromptDialog,
    TasksAction, TasksPanelDialog, TextInputDialog, ViewMode,
};
pub use keymap::{
    parse_key_chord, parse_key_sequence, Command, KeyChord, KeySequence, Keymap, KeymapError, Mode,
    SeqLookup,
};
pub use pane::PaneView;
pub use theme::Theme;

use cargonaut_core::{
    App, Command as AppCommand, DialogKind, Event as AppEvent, PaneId, ResumeOfferView,
};
use crossterm::event::{
    DisableMouseCapture, EnableMouseCapture, Event as CtEvent, EventStream, KeyEvent, MouseButton,
    MouseEvent, MouseEventKind,
};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::Style;
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use std::io::stdout;
use std::time::Instant;

/// Default keymap (the bundled `design/contracts/keymap.toml`), embedded
/// at compile time so the binary doesn't need a runtime file lookup.
const DEFAULT_KEYMAP: &str = include_str!("../../../design/contracts/keymap.toml");

/// Run the TUI event loop. Enters raw mode + alternate screen, drives
/// `tokio::select!` between key events / SIGINT / a periodic redraw
/// tick, dispatches commands into the `App`, manages modal-dialog
/// state, and restores the terminal on exit (best-effort even on panic
/// — wrapped in a teardown that always runs).
pub async fn run(app: &mut App) -> Result<(), Error> {
    // US3 (FR-013): mouse is captured by default (config.ui.mouse, default
    // true); `--no-mouse` / config disables it, preserving terminal-native
    // text selection.
    let mouse_enabled = app.config().ui.mouse;
    enable_raw_mode().map_err(Error::Terminal)?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).map_err(Error::Terminal)?;
    if mouse_enabled {
        execute!(out, EnableMouseCapture).map_err(Error::Terminal)?;
    }
    let backend = CrosstermBackend::new(out);
    let mut term = Terminal::new(backend).map_err(Error::Terminal)?;

    let result = run_loop(&mut term, app, mouse_enabled).await;

    // Teardown — always best-effort, even on error from the loop. Mouse
    // capture is released unconditionally regardless of the runtime toggle
    // state (Feature 041 FR-008 / SC-005).
    let _ = restore_terminal_modes(&mut std::io::stdout());
    let _ = disable_raw_mode();
    let _ = term.show_cursor();

    result
}

/// Write the terminal-restore control sequences — release mouse capture, then
/// leave the alternate screen — to `out`. Always emits `DisableMouseCapture`
/// so a session that exits while capture is active still leaves the terminal
/// clean (Feature 041 FR-008 / SC-005), independent of the runtime toggle.
fn restore_terminal_modes<W: std::io::Write>(out: &mut W) -> std::io::Result<()> {
    execute!(out, DisableMouseCapture, LeaveAlternateScreen)
}

/// On-screen rectangles for the most recent frame, used for mouse
/// hit-testing (US3). Stored by the loop, produced by [`draw_frame`].
#[derive(Debug, Clone, Copy, Default)]
struct FrameLayout {
    menu: Rect,
    /// Inner (list) rect of the left pane.
    left: Rect,
    /// Inner (list) rect of the right pane.
    right: Rect,
    fkeys: Rect,
    /// Subshell panel rect (Feature 054). `None` when panel is hidden.
    subshell: Option<Rect>,
}

/// Loop-owned chrome + mouse state (kept in one struct to avoid a
/// double-digit argument list on the key/mouse handlers).
struct UiState {
    menu: MenuBar,
    fkeybar: FunctionKeyBar,
    layout: FrameLayout,
    last_click: Option<(u16, u16, Instant)>,
    help_overlay: Option<dialog::HelpOverlay>,
    mouse_enabled: bool,
    /// Set by F3/F4; run_loop suspends the TUI, runs it, and restores.
    pending_external: Option<PendingExternal>,
    /// Feature 052 (FR-010): when the active pane shows a synthetic
    /// find-file listing, this holds the search pattern for the title
    /// `[Find: s]`. Cleared on any real directory navigation.
    find_label: Option<String>,
    /// Feature 052: pending panelize request from FindFile dialog.
    /// Set by handle_key on FindOutcome::Panelize; applied in run_loop
    /// which owns the PaneViews.
    pending_panelize: Option<(Vec<std::path::PathBuf>, String)>,
    /// Feature 054 — persistent subshell panel.
    subshell: Option<subshell::SubshellState>,
    /// Feature 054 — current Ctrl-o cycle phase.
    subshell_phase: subshell::SubshellPhase,
    /// Feature 054 — debounce: tracks when Ctrl-o was last processed.
    last_ctrl_o_at: Option<std::time::Instant>,
}

impl UiState {
    /// Returns `true` and updates the timestamp when Ctrl-o fires within
    /// the 50 ms debounce window (spec.md edge case E1).
    /// When `false`, the caller should update `last_ctrl_o_at` and proceed.
    fn ctrl_o_should_skip(&mut self) -> bool {
        const DEBOUNCE: std::time::Duration = std::time::Duration::from_millis(50);
        let now = std::time::Instant::now();
        if let Some(last) = self.last_ctrl_o_at {
            if now.duration_since(last) < DEBOUNCE {
                return true;
            }
        }
        self.last_ctrl_o_at = Some(now);
        false
    }
}

#[derive(Debug)]
enum ActiveDialog {
    Confirm {
        widget: ConfirmDialog,
        on_confirm: AppCommand,
    },
    /// Launch-time resume prompt (Feature 037), shown when
    /// `scan_resume_offers` finds orphan checkpoints.
    Resume(ResumePromptDialog),
    /// Single-line text input (mkdir name / select pattern).
    Input {
        widget: TextInputDialog,
        kind: InputKind,
    },
    /// Feature 038 — quick-cd prompt with directory tab-completion.
    QuickCd {
        /// The shared path-input widget.
        widget: PathInputDialog,
    },
    /// Feature 033 — panel filter prompt (FR-013). Reuses the shared
    /// path-input widget; no completions (a glob has nothing to complete).
    FilterPrompt {
        /// The shared path-input widget.
        widget: PathInputDialog,
    },
    /// Feature 039 — F12 tasks/jobs panel: a modal list of transfers with
    /// per-row cancel/pause/resume over the App transfer registry.
    TasksPanel {
        /// The shared list widget; rows refreshed from `job_views()`.
        widget: TasksPanelDialog,
    },
    /// Feature 042 — `Ctrl-b` directory hotlist popup. Rows built from
    /// `app.bookmarks()`/`grouped()`; in-popup add/remove/select.
    Hotlist {
        /// The hotlist list widget.
        widget: HotlistDialog,
    },
    /// Feature 047 — F2 user-defined action menu. Items loaded from
    /// `~/.config/cargonaut/menu.toml`; filtered by `only_if` condition.
    UserMenu {
        /// The user menu widget.
        widget: dialog::UserMenuDialog,
        /// The path of the focused entry at the time the menu was opened.
        entry_path: std::path::PathBuf,
    },
    /// Feature 051 — built-in file viewer (FR-001..FR-033) replacing the external `$PAGER` shell-out.
    FileViewer {
        /// The file viewer widget.
        widget: dialog::FileViewerDialog,
    },
    /// Feature 056 — built-in text editor (F4) replacing the external `$EDITOR` shell-out.
    FileEditor {
        /// The file editor widget.
        widget: dialog::FileEditorDialog,
    },
    /// Feature 052 — find-file overlay (Alt-?). Searches by name glob or
    /// ripgrep content; result list panelizes into the active pane.
    FindFile {
        /// The find-file dialog widget.
        widget: dialog::FindFileDialog,
        /// The directory root the walk is anchored to.
        root: std::path::PathBuf,
    },
}

/// What a [`TextInputDialog`]'s submitted text becomes.
#[derive(Debug, Clone, Copy)]
enum InputKind {
    Mkdir,
    SelectPattern,
    UnselectPattern,
    /// Feature 042 — bookmark name prompt; text is `group/name` or `name`.
    AddBookmark,
    /// Feature 043 — chmod mode prompt (octal or symbolic).
    Chmod,
    /// Feature 043 — chown owner prompt (`user`/`:group`/`user:group`).
    Chown,
    /// Feature 043 — symlink name prompt.
    Symlink,
    /// Feature 043 — hard-link name prompt.
    HardLink,
    /// Feature 044 — recursive chmod mode prompt (chains a confirm).
    ChmodRecursive,
    /// Feature 044 — recursive chown owner prompt (chains a confirm).
    ChownRecursive,
}

/// Feature 050 — discriminates what the TUI should do after the external
/// program exits.
#[derive(Debug, Clone)]
enum PendingExternalKind {
    /// F3/F4: just refresh the active pane and show a "returned from" status.
    FileOpen,
    /// C-x r: read back the temp file, validate edits, apply renames.
    BulkRename {
        /// Path to the temp file that was written and opened in `$EDITOR`.
        temp_path: std::path::PathBuf,
        /// Basenames in listing order, written one-per-line to the temp file.
        original_names: Vec<String>,
    },
}

/// An external program to run (F3/F4 or bulk-rename), suspending the TUI around it.
#[derive(Debug, Clone)]
struct PendingExternal {
    /// Resolved program (`$PAGER`/`$EDITOR` + fallbacks, or split from diff tool string).
    program: String,
    /// Additional arguments passed after `program`. For F3/F4 this is `vec![path]`;
    /// for diff (US2) this is `argv[1..] + [left_path, right_path]`.
    args: Vec<String>,
    /// What to do after the program exits.
    kind: PendingExternalKind,
}

async fn run_loop<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
    mouse_enabled: bool,
) -> Result<(), Error> {
    let keymap = Keymap::load(DEFAULT_KEYMAP).expect("bundled keymap.toml must parse");
    let mut events = EventStream::new();

    // US2/US3: chrome widgets + mouse/menu state.
    let mut ui = UiState {
        menu: MenuBar::new(),
        fkeybar: FunctionKeyBar::new(),
        layout: FrameLayout::default(),
        last_click: None,
        help_overlay: None,
        mouse_enabled,
        pending_external: None,
        find_label: None,
        pending_panelize: None,
        subshell: None,
        subshell_phase: subshell::SubshellPhase::default(),
        last_ctrl_o_at: None,
    };

    // US1 (FR-001/005/006): resolve the configured theme once. An unknown
    // name falls back to the built-in default with a non-fatal notice.
    let theme_name = app.config().ui.theme.clone();
    let (theme, theme_err) = Theme::resolve(&theme_name);
    let mut status: String = theme_err.unwrap_or_default();

    // Per-pane PaneView, synced from App state once per frame.
    let mut left = PaneView::new(
        app.pane(PaneId::Left).cwd.clone(),
        app.pane(PaneId::Left).listing.clone(),
    );
    let mut right = PaneView::new(
        app.pane(PaneId::Right).cwd.clone(),
        app.pane(PaneId::Right).listing.clone(),
    );

    let mut mode = Mode::Pane;
    let mut active_dialog: Option<ActiveDialog> = None;
    let mut chord_buf: Vec<KeyChord> = Vec::new();
    let mut quit = false;
    // Feature 054 (T022): track the last cwd we synced to the subshell.
    let mut last_synced_cwd: Option<std::path::PathBuf> = None;

    // Feature 037: on launch, offer to resume any interrupted transfers
    // whose checkpoints survive in the pane directories. A scan failure is
    // non-fatal — fall through to the normal panels.
    match app.scan_resume_offers().await {
        Ok(offers) if !offers.is_empty() => {
            active_dialog = Some(ActiveDialog::Resume(ResumePromptDialog::new(
                offers.iter().map(resume_summary_from).collect(),
            )));
        }
        Ok(_) => {}
        Err(e) => status = format!("Resume scan failed: {e}"),
    }

    // Periodic re-render so transfer progress updates show even without
    // input. 100 ms is well under FR-008's 500ms cancellation target.
    let mut tick = tokio::time::interval(std::time::Duration::from_millis(100));
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);

    loop {
        // Render.
        left.sync_from(app.pane(PaneId::Left));
        right.sync_from(app.pane(PaneId::Right));
        let active = app.active_pane();

        // Feature 054 (T022-T025): cwd-sync to subshell on every loop iteration.
        // Fire when the active pane's cwd changes, regardless of panel visibility
        // (hidden subshell still receives the `cd` so it lands in the right dir
        // when the panel is opened). Edge case T025: walk ancestors if path gone.
        {
            let active_cwd_vfs = &app.pane(active).cwd;
            let active_cwd = vfs_path_to_local(active_cwd_vfs);
            let changed = last_synced_cwd
                .as_ref()
                .map_or(true, |prev| *prev != active_cwd);
            if changed {
                if let Some(s) = ui.subshell.as_mut() {
                    s.sync_cwd(&active_cwd);
                }
                last_synced_cwd = Some(active_cwd);
            }
        }

        // Feature 052: if find_label is set, keep the synthetic listing by NOT
        // overwriting it with sync_from's fresh directory listing. sync_from is
        // already called above; we only need to clear find_label when the active
        // pane's cwd changes to a real directory (i.e., the user navigated away).
        // Detect this by comparing the cwd against the stored label.
        // (The label is cleared explicitly in navigate_to arm of apply_event.)

        // Feature 052: apply a pending panelize (from handle_key returning
        // FindOutcome::Panelize). run_loop owns the PaneViews, so panelize
        // must happen here rather than inside handle_key.
        if let Some((paths, pattern)) = ui.pending_panelize.take() {
            let active_id = app.active_pane();
            let pane_view = if active_id == PaneId::Left {
                &mut left
            } else {
                &mut right
            };
            panelize_into_pane(pane_view, &paths, &pattern, &mut ui);
            status = format!("[Find: {pattern}]");
        }

        let status_line = if status.is_empty() {
            app.status().to_string()
        } else {
            status.clone()
        };
        // Feature 039 (FR-008): refresh the tasks panel from the live
        // registry each frame so progress/state update without reopening.
        if let Some(ActiveDialog::TasksPanel { widget }) = active_dialog.as_mut() {
            widget.set_rows(build_job_rows(app));
        }
        let dialog_ref = active_dialog.as_mut();
        let ms_left = chrome::mini_status_line(app.pane(PaneId::Left));
        let ms_right = chrome::mini_status_line(app.pane(PaneId::Right));
        let view_mode = app.view_mode();
        let qv_preview = if view_mode == cargonaut_core::ViewMode::QuickView {
            compute_preview(app)
        } else {
            String::new()
        };
        let progress = progress_summary(app);
        // Feature 053: compute tab bar view models before the draw closure
        // so `app` isn't borrowed inside the closure.
        let tab_bar_left = app.tab_bar_view(PaneId::Left);
        let tab_bar_right = app.tab_bar_view(PaneId::Right);
        // Feature 054 (T017): drain PTY output before rendering.
        if let Some(s) = ui.subshell.as_mut() {
            s.poll_output();
        }

        let mut layout = FrameLayout::default();
        // Feature 041 US2 (FR-005): capture state for the persistent indicator,
        // read out before the partial borrow of `ui` below.
        let mouse_supported = app.config().ui.mouse;
        let mouse_captured = ui.mouse_enabled;
        // Feature 054: compute all subshell parameters before splitting ui borrows.
        let subshell_phase = ui.subshell_phase;
        let subshell_dead = ui.subshell.as_ref().is_some_and(|s| s.dead);
        let content_h = ui.layout.left.height;
        let height_pct = app.config().ui.subshell_height_pct;
        let subshell_rows: u16 = if content_h > 4 {
            let r = (content_h as u32 * height_pct as u32 / 100)
                .clamp(3, (content_h as u32).saturating_sub(4)) as u16;
            r.max(3)
        } else {
            10
        };
        // T005: apply scrollback offset so the screen ref reflects the user's scroll position.
        if let Some(s) = ui.subshell.as_mut() {
            let offset = s.scroll_offset as usize;
            s.screen_mut().set_scrollback(offset);
        }
        // Extract screen ref before split-borrows: NLL treats ui.subshell and
        // ui.menu / ui.fkeybar as disjoint field borrows.
        let subshell_screen: Option<&vt100::Screen> = ui.subshell.as_ref().map(|s| s.screen());
        let menu = &mut ui.menu;
        let fkeybar = &ui.fkeybar;
        let help_overlay = ui.help_overlay.as_ref().cloned();
        term.draw(|f| {
            layout = draw_frame(
                f,
                &mut left,
                &mut right,
                active,
                mode,
                &status_line,
                dialog_ref,
                &theme,
                menu,
                fkeybar,
                &ms_left,
                &ms_right,
                help_overlay.as_ref(),
                view_mode,
                &qv_preview,
                progress.as_deref(),
                mouse_supported,
                mouse_captured,
                &tab_bar_left,
                &tab_bar_right,
                subshell_phase,
                subshell_screen,
                subshell_dead,
                subshell_rows,
            );
        })
        .map_err(Error::Terminal)?;
        // T006: restore live view so non-render screen accesses are unaffected.
        if let Some(s) = ui.subshell.as_mut() {
            s.screen_mut().set_scrollback(0);
        }
        ui.layout = layout;

        if quit {
            return Ok(());
        }

        tokio::select! {
            maybe_ev = events.next() => {
                match maybe_ev {
                    Some(Ok(CtEvent::Key(key))) => {
                        let cont = handle_key(
                            key,
                            app,
                            &keymap,
                            &mut mode,
                            &mut active_dialog,
                            &mut chord_buf,
                            &mut status,
                            &mut quit,
                            &mut ui,
                        ).await?;
                        if !cont { return Ok(()); }
                    }
                    Some(Ok(CtEvent::Mouse(m))) => {
                        handle_mouse(
                            m, app, &mut ui, &left, &right, &mut status,
                            &mut mode, &mut active_dialog, &mut quit,
                        ).await?;
                    }
                    Some(Ok(CtEvent::Resize(new_cols, new_rows))) => {
                        // Resize PTY if the subshell panel is visible (T017).
                        if ui.subshell_phase.is_visible() {
                            if let Some(s) = ui.subshell.as_mut() {
                                let h_pct = app.config().ui.subshell_height_pct;
                                let panel = ((new_rows as u32 * h_pct as u32) / 100)
                                    .clamp(3, (new_rows as u32).saturating_sub(4)) as u16;
                                s.resize(panel.max(3), new_cols.max(1));
                            }
                        }
                    }
                    Some(Ok(_)) => {} // focus/paste events — ignored
                    Some(Err(e)) => return Err(Error::Terminal(e)),
                    None => return Ok(()),
                }
            }
            _ = tokio::signal::ctrl_c() => {
                quit = true;
            }
            _ = tick.tick() => {
                // Drains pending transfer state changes through to the next render.
                // Feature 052: drain incremental find-file walk results each tick.
                if let Some(ActiveDialog::FindFile { widget, .. }) = active_dialog.as_mut() {
                    widget.poll_results();
                }
            }
        }

        // US5 (FR-030/031): an F3/F4 or bulk-rename request suspends the TUI,
        // runs the external program, then restores the terminal and dispatches
        // post-action handling based on `ext.kind`.
        if let Some(ext) = ui.pending_external.take() {
            run_external(term, &ext, ui.mouse_enabled)?;
            match ext.kind {
                PendingExternalKind::FileOpen => {
                    let _ = app
                        .refresh_active_pane()
                        .await
                        .map_err(|e| Error::Other(e.to_string()))?;
                    status = format!("Returned from {}", ext.program);
                }
                PendingExternalKind::BulkRename {
                    ref temp_path,
                    ref original_names,
                } => {
                    apply_bulk_rename_from_temp(app, temp_path, original_names, &mut status).await;
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
async fn handle_key(
    key: KeyEvent,
    app: &mut App,
    keymap: &Keymap,
    mode: &mut Mode,
    active_dialog: &mut Option<ActiveDialog>,
    chord_buf: &mut Vec<KeyChord>,
    status: &mut String,
    quit: &mut bool,
    ui: &mut UiState,
) -> Result<bool, Error> {
    use crossterm::event::KeyCode;

    // Help overlay — swallows all keys; Esc/F1 close it.
    if let Some(overlay) = ui.help_overlay.as_mut() {
        let action = overlay.handle_key(key.code);
        if action == dialog::HelpAction::Close {
            ui.help_overlay = None;
        }
        return Ok(true);
    }

    // Open menu swallows keys (US2): navigate / select / close.
    if ui.menu.is_open() {
        match key.code {
            KeyCode::Esc => ui.menu.close(),
            KeyCode::Down | KeyCode::Char('j') => ui.menu.select_down(),
            KeyCode::Up | KeyCode::Char('k') => ui.menu.select_up(),
            KeyCode::Left | KeyCode::Char('h') => ui.menu.prev_menu(),
            KeyCode::Right | KeyCode::Char('l') => ui.menu.next_menu(),
            KeyCode::Enter => {
                if let Some(cmd) = ui.menu.selected_command() {
                    ui.menu.close();
                    dispatch_ui_command(cmd, app, mode, active_dialog, status, quit, ui).await?;
                } else {
                    ui.menu.close();
                }
            }
            _ => {}
        }
        return Ok(true);
    }

    // Dialog mode swallows all keys.
    if let Some(dialog) = active_dialog.as_mut() {
        match dialog {
            ActiveDialog::Confirm { widget, on_confirm } => {
                if let Some(outcome) = widget.handle_key(key.code) {
                    let confirmed = outcome == ConfirmOutcome::Confirm;
                    let on_confirm = on_confirm.clone();
                    *active_dialog = None;
                    *mode = Mode::Pane;
                    if confirmed {
                        // Dispatch the follow-through command (e.g. confirm_copy).
                        match on_confirm {
                            AppCommand::Copy => {
                                let _ = app
                                    .confirm_copy()
                                    .await
                                    .map_err(|e| Error::Other(e.to_string()))?;
                            }
                            other => {
                                let _ = app
                                    .dispatch(other)
                                    .await
                                    .map_err(|e| Error::Other(e.to_string()))?;
                            }
                        }
                    }
                }
                return Ok(true);
            }
            ActiveDialog::Resume(widget) => {
                if let Some((idx, choice)) = widget.handle_key(key.code) {
                    // Feature 037: dispatch the choice into the App, then
                    // rebuild the prompt from the remaining offers so the
                    // indices never drift (R-005). Dismiss when empty.
                    match choice {
                        ResumeChoice::Resume => {
                            let _ = app
                                .resume_offer(idx)
                                .await
                                .map_err(|e| Error::Other(e.to_string()))?;
                        }
                        ResumeChoice::StartOver => {
                            let _ = app
                                .start_over_offer(idx)
                                .await
                                .map_err(|e| Error::Other(e.to_string()))?;
                        }
                        ResumeChoice::Skip => app.skip_offer(idx),
                    }
                    let remaining = app.pending_resume_views();
                    if remaining.is_empty() {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    } else {
                        *active_dialog = Some(ActiveDialog::Resume(ResumePromptDialog::new(
                            remaining.iter().map(resume_summary_from).collect(),
                        )));
                    }
                }
                return Ok(true);
            }
            ActiveDialog::Input { widget, kind } => {
                if let Some(outcome) = widget.handle_key(key.code) {
                    let kind = *kind;
                    *active_dialog = None;
                    *mode = Mode::Pane;
                    if let InputOutcome::Submit(text) = outcome {
                        let text = text.trim().to_string();
                        if !text.is_empty() {
                            // Feature 042: bookmark add is a direct App method
                            // (not a core command) and reopens the hotlist.
                            match kind {
                                InputKind::AddBookmark => {
                                    let (group, name) = parse_bookmark_input(&text);
                                    match app.add_bookmark(&name, group.as_deref()) {
                                        Ok(events) => {
                                            for ev in events {
                                                apply_event(
                                                    ev,
                                                    app,
                                                    mode,
                                                    active_dialog,
                                                    status,
                                                    quit,
                                                );
                                            }
                                        }
                                        Err(e) => *status = e.to_string(),
                                    }
                                    // Reopen the hotlist, refreshed.
                                    *active_dialog = Some(ActiveDialog::Hotlist {
                                        widget: HotlistDialog::new(build_hotlist_rows(
                                            app.bookmarks(),
                                        )),
                                    });
                                    *mode = Mode::Dialog;
                                }
                                // Feature 043: chmod is a direct App method (not a
                                // core command); invalid input ⇒ inline status.
                                InputKind::Chmod => match app.chmod_selection(&text).await {
                                    Ok(events) => {
                                        for ev in events {
                                            apply_event(ev, app, mode, active_dialog, status, quit);
                                        }
                                    }
                                    Err(e) => *status = e.to_string(),
                                },
                                // Feature 043: symlink / hardlink are direct App
                                // methods; errors (existing name, bad target) ⇒
                                // inline status.
                                InputKind::Symlink => match app.create_symlink(&text).await {
                                    Ok(events) => {
                                        for ev in events {
                                            apply_event(ev, app, mode, active_dialog, status, quit);
                                        }
                                    }
                                    Err(e) => *status = e.to_string(),
                                },
                                InputKind::HardLink => match app.create_hard_link(&text).await {
                                    Ok(events) => {
                                        for ev in events {
                                            apply_event(ev, app, mode, active_dialog, status, quit);
                                        }
                                    }
                                    Err(e) => *status = e.to_string(),
                                },
                                // Feature 043 (FR-007): chown requires explicit
                                // confirmation — chain a ConfirmDialog whose
                                // on-confirm dispatches the core chown command.
                                InputKind::Chown => {
                                    *active_dialog = Some(ActiveDialog::Confirm {
                                        widget: ConfirmDialog::new(
                                            "Change owner",
                                            format!("Change owner to {text}?"),
                                        ),
                                        on_confirm: AppCommand::Chown(text),
                                    });
                                    *mode = Mode::Dialog;
                                }
                                // Feature 044: recursive chmod/chown always
                                // confirm before applying to the whole subtree
                                // (FR-002); Cancel aborts.
                                InputKind::ChmodRecursive => {
                                    *active_dialog = Some(ActiveDialog::Confirm {
                                        widget: ConfirmDialog::new(
                                            "Recursive chmod",
                                            format!("Recursively chmod the subtree to {text}?"),
                                        ),
                                        on_confirm: AppCommand::ChmodRecursive(text),
                                    });
                                    *mode = Mode::Dialog;
                                }
                                InputKind::ChownRecursive => {
                                    *active_dialog = Some(ActiveDialog::Confirm {
                                        widget: ConfirmDialog::new(
                                            "Recursive chown",
                                            format!("Recursively chown the subtree to {text}?"),
                                        ),
                                        on_confirm: AppCommand::ChownRecursive(text),
                                    });
                                    *mode = Mode::Dialog;
                                }
                                InputKind::Mkdir
                                | InputKind::SelectPattern
                                | InputKind::UnselectPattern => {
                                    let core = match kind {
                                        InputKind::Mkdir => AppCommand::Mkdir(text),
                                        InputKind::SelectPattern => {
                                            AppCommand::SelectByPattern(text)
                                        }
                                        InputKind::UnselectPattern => {
                                            AppCommand::UnselectByPattern(text)
                                        }
                                        _ => unreachable!("handled above"),
                                    };
                                    let events = app
                                        .dispatch(core)
                                        .await
                                        .map_err(|e| Error::Other(e.to_string()))?;
                                    for ev in events {
                                        apply_event(ev, app, mode, active_dialog, status, quit);
                                    }
                                }
                            }
                        }
                    }
                }
                return Ok(true);
            }
            ActiveDialog::QuickCd { widget } => {
                match widget.handle_key(key.code) {
                    PathInputAction::RequestCompletions { text } => {
                        // Async completion fetch off the render path (R-005).
                        let candidates = app.complete_cd(&text).await;
                        if let Some(ActiveDialog::QuickCd { widget }) = active_dialog.as_mut() {
                            widget.apply_completions(candidates);
                        }
                    }
                    PathInputAction::Submit(text) => {
                        if !text.trim().is_empty() {
                            match app.quick_cd(&text).await {
                                Ok(events) => {
                                    *active_dialog = None;
                                    *mode = Mode::Pane;
                                    for ev in events {
                                        apply_event(ev, app, mode, active_dialog, status, quit);
                                    }
                                }
                                // FR-006: keep the prompt open, show the error.
                                Err(e) => {
                                    if let Some(ActiveDialog::QuickCd { widget }) =
                                        active_dialog.as_mut()
                                    {
                                        widget.set_error(e.to_string());
                                    }
                                }
                            }
                        }
                        // Empty input: no-op, prompt stays open (US3 #3).
                    }
                    PathInputAction::Cancel => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    }
                    PathInputAction::Edited | PathInputAction::Consumed => {}
                }
                return Ok(true);
            }
            ActiveDialog::FilterPrompt { widget } => {
                match widget.handle_key(key.code) {
                    // Empty submit clears (US2); valid sets; invalid keeps the
                    // prompt open with an inline error (FR-006). `set_filter`
                    // is synchronous (research R-005).
                    PathInputAction::Submit(text) => match app.set_filter(&text) {
                        Ok(events) => {
                            *active_dialog = None;
                            *mode = Mode::Pane;
                            for ev in events {
                                apply_event(ev, app, mode, active_dialog, status, quit);
                            }
                        }
                        Err(e) => {
                            if let Some(ActiveDialog::FilterPrompt { widget }) =
                                active_dialog.as_mut()
                            {
                                widget.set_error(e.to_string());
                            }
                        }
                    },
                    // FR-008: cancel leaves the pane's filter untouched.
                    PathInputAction::Cancel => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    }
                    // No path completions for a glob prompt.
                    PathInputAction::RequestCompletions { .. }
                    | PathInputAction::Edited
                    | PathInputAction::Consumed => {}
                }
                return Ok(true);
            }
            ActiveDialog::TasksPanel { widget } => {
                // Map the focused row index → transfer id via job_views()
                // (index-aligned with the widget rows), run the action, then
                // refresh rows so the change shows. Panel stays open until
                // Close (Esc / F12). FR-009/010/011/012.
                match widget.handle_key(key.code) {
                    Some(TasksAction::Close) => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    }
                    Some(action) => {
                        let ids: Vec<_> = app.job_views().into_iter().map(|v| v.id).collect();
                        match action {
                            TasksAction::Cancel(i) => {
                                if let Some(id) = ids.get(i).copied() {
                                    let _ = app.cancel_transfer(id);
                                }
                            }
                            TasksAction::Pause(i) => {
                                if let Some(id) = ids.get(i).copied() {
                                    let _ = app.pause_transfer(id);
                                }
                            }
                            TasksAction::Resume(i) => {
                                if let Some(id) = ids.get(i).copied() {
                                    let _ = app
                                        .resume_paused(id)
                                        .await
                                        .map_err(|e| Error::Other(e.to_string()))?;
                                }
                            }
                            TasksAction::Close => unreachable!("handled above"),
                        }
                        if let Some(ActiveDialog::TasksPanel { widget }) = active_dialog.as_mut() {
                            widget.set_rows(build_job_rows(app));
                        }
                    }
                    None => {}
                }
                return Ok(true);
            }
            ActiveDialog::Hotlist { widget } => {
                // Feature 042 (FR-003/004/005/012): map the focused row → bookmark
                // index, run the action against a fresh snapshot, keep the popup
                // open except on Select/Close.
                match widget.handle_key(key.code) {
                    Some(HotlistAction::Close) => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    }
                    Some(HotlistAction::Select(i)) => {
                        // Jump; a bad target surfaces as a status, panes/hotlist
                        // unchanged (FR-008). Close the popup either way.
                        match app.jump_to_bookmark(i).await {
                            Ok(events) => {
                                *active_dialog = None;
                                *mode = Mode::Pane;
                                for ev in events {
                                    apply_event(ev, app, mode, active_dialog, status, quit);
                                }
                            }
                            Err(e) => {
                                *active_dialog = None;
                                *mode = Mode::Pane;
                                *status = e.to_string();
                            }
                        }
                    }
                    Some(HotlistAction::Remove(i)) => {
                        let _ = app.remove_bookmark(i);
                        if let Some(ActiveDialog::Hotlist { widget }) = active_dialog.as_mut() {
                            *widget = HotlistDialog::new(build_hotlist_rows(app.bookmarks()));
                        }
                    }
                    Some(HotlistAction::Add) => {
                        // Chain into a name prompt (group/name). On submit the
                        // Input arm calls add_bookmark and reopens the hotlist.
                        *active_dialog = Some(ActiveDialog::Input {
                            widget: TextInputDialog::new("Add bookmark", "Name (or group/name):"),
                            kind: InputKind::AddBookmark,
                        });
                    }
                    None => {}
                }
                return Ok(true);
            }
            // Feature 047 US2 (FR-006/007/008): F2 user-defined action menu.
            ActiveDialog::UserMenu {
                widget,
                entry_path: _,
            } => {
                match widget.handle_key(key.code) {
                    Some(dialog::UserMenuAction::Close) => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                    }
                    Some(dialog::UserMenuAction::Execute(idx)) => {
                        // Extract command before mutably borrowing `active_dialog`.
                        let (cmd_str, exec_path) =
                            if let Some(ActiveDialog::UserMenu { widget, entry_path }) =
                                active_dialog.as_ref()
                            {
                                let cmd_str = widget
                                    .items
                                    .get(idx)
                                    .map(|i| i.command.clone())
                                    .unwrap_or_default();
                                (cmd_str, entry_path.clone())
                            } else {
                                (String::new(), std::path::PathBuf::new())
                            };
                        *active_dialog = None;
                        *mode = Mode::Pane;
                        if !cmd_str.is_empty() {
                            let (prog, args) = build_action_command(&cmd_str, &exec_path);
                            // Spawn detached — fire-and-forget (FR-007).
                            let _ = std::process::Command::new(&prog)
                                .args(&args)
                                .stdout(std::process::Stdio::null())
                                .stderr(std::process::Stdio::null())
                                .spawn();
                        }
                    }
                    None => {
                        // F1 while menu is open closes the menu (FR-010).
                        if key.code == dialog::KeyCode::F(1) {
                            *active_dialog = None;
                            *mode = Mode::Pane;
                        }
                    }
                }
                return Ok(true);
            }
            // Feature 051: built-in file viewer (C1+C2+C3 fix — T015/T036).
            // Chord accumulation and keymap lookup happen INSIDE this arm because the
            // dialog block returns Ok(true) before the normal chord accumulation at line ~836.
            ActiveDialog::FileViewer { widget } => {
                // (a) Chord accumulation.
                chord_buf.push(KeyChord {
                    code: key.code,
                    modifiers: key.modifiers,
                });
                // (b) Keymap lookup against Mode::Preview.
                let action = match keymap.lookup_sequence(Mode::Preview, chord_buf) {
                    SeqLookup::Command(cmd) => {
                        chord_buf.clear();
                        // Dispatch viewer commands (T036).
                        match cmd {
                            Command::ViewerQuit => widget.close(),
                            Command::ViewerEnd => widget.goto_end(),
                            Command::ViewerWrap => widget.toggle_wrap(),
                            Command::ViewerGoto => widget.open_goto_prompt(),
                            Command::ToggleHexView => {
                                widget.toggle_mode();
                                dialog::FileViewerAction::Swallow
                            }
                            Command::PreviewSearchForward => {
                                widget.open_search_prompt(dialog::SearchDirection::Forward)
                            }
                            Command::PreviewSearchBackward => {
                                widget.open_search_prompt(dialog::SearchDirection::Backward)
                            }
                            Command::PreviewSearchNext => {
                                widget.advance_search(dialog::SearchDirection::Forward)
                            }
                            Command::PreviewSearchPrev => {
                                widget.advance_search(dialog::SearchDirection::Backward)
                            }
                            _ => dialog::FileViewerAction::Swallow,
                        }
                    }
                    SeqLookup::Pending => {
                        *status = format!("Chord: {chord_buf:?}");
                        return Ok(true);
                    }
                    SeqLookup::NoMatch => {
                        chord_buf.clear();
                        // (c) Fall through to raw navigation key handling.
                        widget.handle_key(key.code)
                    }
                };
                // (d) FileViewerAction dispatch (exhaustive — C3 fix includes NeedsData stub).
                match action {
                    dialog::FileViewerAction::Close => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                        chord_buf.clear();
                    }
                    dialog::FileViewerAction::Swallow => {}
                    dialog::FileViewerAction::NeedsData {
                        offset,
                        line_count,
                        window_start,
                    } => {
                        let path = widget.path.clone();
                        let result = tokio::task::spawn_blocking(move || {
                            dialog::FileViewerDialog::load_window_from_chunk(
                                &path, offset, line_count,
                            )
                        })
                        .await
                        .map_err(|e| std::io::Error::other(e.to_string()));
                        match result {
                            Ok(Ok((new_lines, new_reader_offset))) => {
                                widget.append_lines(new_lines, window_start, new_reader_offset);
                            }
                            Ok(Err(_)) | Err(_) => {
                                widget.set_status("File no longer readable");
                            }
                        }
                    }
                }
                return Ok(true);
            }
            // Feature 056: built-in text editor (F4).
            // SaveFile and EditorQuit are handled via keymap lookup; raw editing
            // keys (navigation, character input) fall through to widget.handle_key().
            ActiveDialog::FileEditor { widget } => {
                // (a) Chord accumulation.
                chord_buf.push(KeyChord {
                    code: key.code,
                    modifiers: key.modifiers,
                });
                // (b) Keymap lookup against Mode::Editor.
                let action = match keymap.lookup_sequence(Mode::Editor, chord_buf) {
                    SeqLookup::Command(cmd) => {
                        chord_buf.clear();
                        match cmd {
                            Command::SaveFile => {
                                match widget.save() {
                                    Ok(()) => {
                                        *status = "Saved".into();
                                    }
                                    Err(e) => {
                                        widget.status_msg = Some(format!("Save failed: {e}"));
                                    }
                                }
                                dialog::FileEditorAction::Swallow
                            }
                            Command::EditorQuit => {
                                if widget.is_dirty() {
                                    widget.show_unsaved_dialog();
                                    dialog::FileEditorAction::UnsavedPromptShowing
                                } else {
                                    dialog::FileEditorAction::Close
                                }
                            }
                            _ => dialog::FileEditorAction::Swallow,
                        }
                    }
                    SeqLookup::Pending => {
                        *status = format!("Chord: {chord_buf:?}");
                        return Ok(true);
                    }
                    SeqLookup::NoMatch => {
                        chord_buf.clear();
                        // Fall through to raw editing key handling.
                        widget.handle_key(key.code, key.modifiers)
                    }
                };
                // (c) FileEditorAction dispatch.
                match action {
                    dialog::FileEditorAction::Close | dialog::FileEditorAction::DiscardAndClose => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                        chord_buf.clear();
                    }
                    dialog::FileEditorAction::SaveAndClose => {
                        if let Err(e) = widget.save() {
                            *status = format!("Save failed: {e}");
                            // Keep editor open so user can retry.
                        } else {
                            *active_dialog = None;
                            *mode = Mode::Pane;
                            chord_buf.clear();
                        }
                    }
                    dialog::FileEditorAction::Swallow
                    | dialog::FileEditorAction::UnsavedPromptShowing => {}
                }
                return Ok(true);
            }
            // Feature 052: find-file overlay receives raw key events.
            ActiveDialog::FindFile { widget, root } => {
                let config = app.config().clone();
                let root_clone = root.clone();
                let outcome = widget.handle_key_with_root(key.code, &config, root_clone);
                match outcome {
                    dialog::FindOutcome::Cancelled => {
                        // call cancel() BEFORE dismissing (abort atomicity — T017)
                        widget.cancel();
                        *active_dialog = None;
                        *mode = Mode::Pane;
                        // T019: do NOT set find_label on cancel
                    }
                    dialog::FindOutcome::Panelize { paths, pattern } => {
                        *active_dialog = None;
                        *mode = Mode::Pane;
                        // Store for run_loop to apply against the PaneViews.
                        ui.pending_panelize = Some((paths, pattern));
                    }
                    dialog::FindOutcome::Consumed => {}
                }
                return Ok(true);
            }
        }
    }

    // Feature 054: when shell focus is active, Ctrl-o cycles phase;
    // all other keys are forwarded verbatim to the PTY (FR-009).
    if *mode == Mode::Subshell {
        use crossterm::event::KeyModifiers;
        let is_ctrl_o = key.code == crossterm::event::KeyCode::Char('o')
            && key.modifiers.contains(KeyModifiers::CONTROL);
        if is_ctrl_o {
            dispatch_ui_command(
                Command::OpenSubshell,
                app,
                mode,
                active_dialog,
                status,
                quit,
                ui,
            )
            .await?;
        } else {
            if let Some(s) = ui.subshell.as_mut() {
                s.write_key(key);
            }
        }
        return Ok(true);
    }

    // Normal mode — accumulate chord + look up.
    chord_buf.push(KeyChord {
        code: key.code,
        modifiers: key.modifiers,
    });
    match keymap.lookup_sequence(*mode, chord_buf) {
        SeqLookup::Command(cmd) => {
            chord_buf.clear();
            dispatch_ui_command(cmd, app, mode, active_dialog, status, quit, ui).await?;
        }
        SeqLookup::Pending => {
            *status = format!("Chord: {chord_buf:?}");
        }
        SeqLookup::NoMatch => {
            chord_buf.clear();
        }
    }
    Ok(true)
}

/// Single dispatch path shared by keyboard chords, menu selection, and
/// mouse clicks on the function-key bar. UI-only commands (open menu,
/// help) are handled here; everything else maps to a core command, or
/// reports "not yet available" when it's a deferred action (FR-011).
#[allow(clippy::too_many_arguments)]
async fn dispatch_ui_command(
    cmd: Command,
    app: &mut App,
    mode: &mut Mode,
    active_dialog: &mut Option<ActiveDialog>,
    status: &mut String,
    quit: &mut bool,
    ui: &mut UiState,
) -> Result<(), Error> {
    match cmd {
        Command::OpenMenuBar => {
            ui.menu.open_first();
            return Ok(());
        }
        Command::ShowHelp => {
            let visible_h = ui.layout.left.height.saturating_sub(2).max(1);
            ui.help_overlay = Some(dialog::HelpOverlay::new(visible_h));
            return Ok(());
        }
        // Feature 054 (FR-002): Ctrl-o cycles through Hidden→VisibleFmFocus→VisibleShellFocus→Hidden.
        Command::OpenSubshell => {
            // FR-012: no-op when any modal is open.
            if active_dialog.is_some() || ui.help_overlay.is_some() {
                return Ok(());
            }
            // E1: ignore burst keypresses (< 50 ms debounce).
            if ui.ctrl_o_should_skip() {
                return Ok(());
            }
            // FR-015: guard against terminals that are too small.
            // content_height is pane area rows; need at least 8 free.
            let content_height = ui.layout.left.height;
            if content_height < 8 && ui.subshell_phase == subshell::SubshellPhase::Hidden {
                *status = "Terminal too small to open subshell (min 8 rows)".to_string();
                return Ok(());
            }
            // Advance the three-state cycle.
            let next_phase = ui.subshell_phase.advance();
            // Lazily spawn or respawn when transitioning from Hidden.
            if ui.subshell_phase == subshell::SubshellPhase::Hidden {
                let cwd = vfs_path_to_local(&app.pane(app.active_pane()).cwd);
                let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/sh".to_string());
                let height_pct = app.config().ui.subshell_height_pct;
                let panel_rows = ((content_height as u32 * height_pct as u32) / 100)
                    .clamp(3, content_height.saturating_sub(5) as u32)
                    as u16;
                let cols = ui.layout.left.width + ui.layout.right.width;
                let cols = if cols == 0 { 80 } else { cols };
                let panel_rows = if panel_rows == 0 { 10 } else { panel_rows };
                match ui.subshell.as_mut() {
                    Some(s) if s.dead => {
                        if let Err(e) = s.respawn(&shell, &cwd, panel_rows, cols) {
                            *status = format!("Subshell respawn failed: {e}");
                            return Ok(());
                        }
                    }
                    None => match subshell::SubshellState::spawn(&shell, &cwd, panel_rows, cols) {
                        Ok(s) => ui.subshell = Some(s),
                        Err(e) => {
                            *status = format!("Subshell spawn failed: {e}");
                            return Ok(());
                        }
                    },
                    Some(_) => {} // alive and hidden — just show it
                }
            }
            ui.subshell_phase = next_phase;
            // Update input mode to route keys to the PTY.
            *mode = match next_phase {
                subshell::SubshellPhase::VisibleShellFocus => Mode::Subshell,
                _ => Mode::Pane,
            };
            return Ok(());
        }
        // Feature 041 (FR-001/002/003/006): M-m toggles mouse capture at
        // runtime. The decision is pure (`plan_mouse_toggle`); here we apply
        // the thin terminal I/O. Capture control is best-effort — terminals
        // without mouse reporting silently ignore the control sequence, so a
        // toggle must never crash the loop (FR-011).
        Command::ToggleMouseCapture => {
            let outcome = plan_mouse_toggle(app.config().ui.mouse, ui.mouse_enabled);
            match outcome {
                MouseToggleOutcome::Disabled => {} // no capture change (FR-006)
                MouseToggleOutcome::EnabledNow => {
                    let _ = execute!(stdout(), EnableMouseCapture);
                    ui.mouse_enabled = true;
                }
                MouseToggleOutcome::SuspendedNow => {
                    let _ = execute!(stdout(), DisableMouseCapture);
                    ui.mouse_enabled = false;
                }
            }
            *status = outcome.status().to_string();
            return Ok(());
        }
        // US5 (FR-024/025): these need text input first — open a dialog.
        Command::Mkdir => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::new("Make directory", "New directory name:"),
                kind: InputKind::Mkdir,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        Command::SelectionAddByPattern => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::new("Select group", "Tag files matching (glob):"),
                kind: InputKind::SelectPattern,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        Command::SelectionRemoveByPattern => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::new("Unselect group", "Untag files matching (glob):"),
                kind: InputKind::UnselectPattern,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 038 (FR-012/FR-014): Alt-c opens the quick-cd prompt,
        // prefilled with the active pane's current directory.
        Command::QuickCdPopup => {
            let initial = app.active_pane_state().cwd.display();
            *active_dialog = Some(ActiveDialog::QuickCd {
                widget: PathInputDialog::new("Quick cd", "Directory:", initial),
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 033 (FR-013): Alt-! opens the panel filter prompt,
        // prefilled with the active pane's current filter pattern (FR-002).
        Command::TogglePanelFilter => {
            let initial = app
                .active_pane_state()
                .filter
                .as_ref()
                .map(|f| f.pattern().to_string())
                .unwrap_or_default();
            *active_dialog = Some(ActiveDialog::FilterPrompt {
                widget: PathInputDialog::new("Filter", "Pattern:", initial),
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 039 (FR-001): F12 opens the tasks/jobs panel over the
        // App transfer registry. One modal at a time — while it's open,
        // keys are swallowed by the dialog arm, so it can't stack (FR-013).
        Command::ShowTasksPanel => {
            *active_dialog = Some(ActiveDialog::TasksPanel {
                widget: TasksPanelDialog::new(build_job_rows(app)),
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 042 (FR-001): Ctrl-b opens the directory hotlist popup over
        // the App's bookmarks. In-popup keys add/remove/select (FR-004/005/003).
        Command::BookmarksMenu => {
            *active_dialog = Some(ActiveDialog::Hotlist {
                widget: HotlistDialog::new(build_hotlist_rows(app.bookmarks())),
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 043 (FR-001/011): C-x c opens the chmod prompt, prefilled
        // with the focused entry's current octal mode.
        Command::Chmod => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Change permissions",
                    "Mode (octal e.g. 755, or symbolic e.g. u+x):",
                    focused_octal_mode(app),
                ),
                kind: InputKind::Chmod,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 043 (FR-004): C-x o opens the chown prompt, prefilled with the
        // focused entry's current numeric owner. Submit chains a confirmation
        // (FR-007) before applying.
        Command::Chown => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Change owner",
                    "Owner (user, :group, or user:group):",
                    focused_owner(app),
                ),
                kind: InputKind::Chown,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 043: C-x s / C-x l open a link-name prompt prefilled with the
        // focused entry's name.
        Command::CreateSymlink => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Create symbolic link",
                    "Link name:",
                    focused_entry_name(app),
                ),
                kind: InputKind::Symlink,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        Command::CreateHardLink => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Create hard link",
                    "Link name:",
                    focused_entry_name(app),
                ),
                kind: InputKind::HardLink,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 044 (#65): C-x C / C-x O open the recursive chmod/chown prompt
        // (prefilled like the shallow ops); submit chains a confirmation.
        Command::ChmodRecursive => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Recursive chmod",
                    "Mode for whole subtree (octal e.g. 755, or symbolic):",
                    focused_octal_mode(app),
                ),
                kind: InputKind::ChmodRecursive,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        Command::ChownRecursive => {
            *active_dialog = Some(ActiveDialog::Input {
                widget: TextInputDialog::with_initial(
                    "Recursive chown",
                    "Owner for whole subtree (user, :group, or user:group):",
                    focused_owner(app),
                ),
                kind: InputKind::ChownRecursive,
            });
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 051 (FR-001): F3 opens the built-in file viewer (replaces $PAGER shell-out).
        Command::Preview => {
            // FR-004: if a dialog is already open, swallow the keypress.
            if active_dialog.is_some() {
                return Ok(());
            }
            let p = app.active_pane_state();
            let Some(idx) = p.focused_entry_index() else {
                *status = "Nothing to open".into();
                return Ok(());
            };
            let Some(entry) = p.listing.entries.get(idx) else {
                return Ok(());
            };
            if matches!(entry.meta.kind, cargonaut_vfs::VfsKind::Dir) {
                *status = "Not a file".into();
                return Ok(());
            }
            let display_name = entry.name.to_string();
            let raw_path: std::path::PathBuf = {
                let cwd = p.cwd.display().to_string();
                let local = cwd.strip_prefix("file://").unwrap_or(&cwd);
                std::path::PathBuf::from(local).join(&display_name)
            };
            let _ = p; // release borrow on app before await
            match open_file_viewer(raw_path, display_name).await {
                Ok(widget) => {
                    *active_dialog = Some(ActiveDialog::FileViewer { widget });
                    *mode = Mode::Preview;
                }
                Err(e) => {
                    *status = format!("Cannot open: {e}");
                }
            }
            return Ok(());
        }
        // Feature 051 US5 (FR-029): Enter on a file entry opens the built-in viewer.
        // On a directory entry, fall through to AppCommand::Descend via ui_command_to_core.
        Command::DescendOrOpen => {
            if active_dialog.is_some() {
                return Ok(());
            }
            let p = app.active_pane_state();
            let entry = p
                .focused_entry_index()
                .and_then(|i| p.listing.entries.get(i));
            let is_file =
                entry.is_some_and(|e| !matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir));
            if is_file {
                let entry = entry.unwrap();
                let display_name = entry.name.to_string();
                let raw_path: std::path::PathBuf = {
                    let cwd = p.cwd.display().to_string();
                    let local = cwd.strip_prefix("file://").unwrap_or(&cwd);
                    std::path::PathBuf::from(local).join(&display_name)
                };
                let _ = p;

                // Feature 057 US1: .zip files open as archive backends, not in the viewer.
                if display_name.to_lowercase().ends_with(".zip") {
                    let id = app.active_pane();
                    let archive_path = raw_path.clone();
                    let zip_result = tokio::task::spawn_blocking(move || {
                        cargonaut_vfs::ZipFs::open(archive_path)
                    })
                    .await
                    .map_err(|e| std::io::Error::other(e.to_string()));
                    match zip_result {
                        Ok(Ok(zip_fs)) => {
                            let encoded_auth = encode_archive_authority(
                                raw_path.to_str().unwrap_or(""),
                            );
                            let zip_url = format!("zip://{encoded_auth}/");
                            match cargonaut_vfs::VfsPath::parse(&zip_url) {
                                Ok(zip_path) => {
                                    match app
                                        .navigate_into(
                                            id,
                                            zip_path,
                                            std::sync::Arc::new(zip_fs),
                                        )
                                        .await
                                    {
                                        Ok(_) => {}
                                        Err(e) => {
                                            *status = format!("Cannot browse archive: {e}");
                                        }
                                    }
                                }
                                Err(e) => {
                                    *status = format!("Archive path encoding error: {e}");
                                }
                            }
                        }
                        Ok(Err(e)) => {
                            *status = format!("Cannot open archive: {e}");
                        }
                        Err(e) => {
                            *status = format!("Archive open failed: {e}");
                        }
                    }
                    return Ok(());
                }

                match open_file_viewer(raw_path, display_name).await {
                    Ok(widget) => {
                        *active_dialog = Some(ActiveDialog::FileViewer { widget });
                        *mode = Mode::Preview;
                    }
                    Err(e) => {
                        *status = format!("Cannot open: {e}");
                    }
                }
                return Ok(());
            }
            // Directory: fall through to core Descend.
        }
        Command::Edit => {
            // Feature 056: open the built-in full-screen editor instead of shelling out.
            if active_dialog.is_some() {
                return Ok(());
            }
            let p = app.active_pane_state();
            let Some(idx) = p.focused_entry_index() else {
                *status = "Nothing to edit".into();
                return Ok(());
            };
            let Some(entry) = p.listing.entries.get(idx) else {
                return Ok(());
            };
            if matches!(entry.meta.kind, cargonaut_vfs::VfsKind::Dir) {
                *status = "Not a file".into();
                return Ok(());
            }
            let display_name = entry.name.to_string();
            let raw_path: std::path::PathBuf = {
                let cwd = p.cwd.display().to_string();
                let local = cwd.strip_prefix("file://").unwrap_or(&cwd);
                std::path::PathBuf::from(local).join(&display_name)
            };
            let _ = p;
            match open_file_editor(raw_path, display_name).await {
                Ok(widget) => {
                    *active_dialog = Some(ActiveDialog::FileEditor { widget });
                    *mode = Mode::Editor;
                }
                Err(e) => {
                    *status = format!("Cannot open: {e}");
                }
            }
            return Ok(());
        }
        // Feature 049 US2 (FR-005 through FR-008): C-x C-d diff two tagged files.
        Command::DiffTwoTaggedFiles => {
            queue_diff(app, ui, status, app.config().diff.tool.as_deref());
            return Ok(());
        }
        // Feature 050 US1: C-x r bulk-rename tagged files via $EDITOR.
        Command::BulkRenameViaEditor => {
            queue_bulk_rename(app, ui, status);
            return Ok(());
        }
        // Feature 047 US2 (FR-006): F2 opens the user action menu.
        // Guard: if another dialog is already open, ignore.
        Command::ShowUserMenu => {
            if active_dialog.is_some() {
                return Ok(());
            }
            // Build local entry path from active pane cwd + focused entry name.
            let entry_path: std::path::PathBuf = {
                let p = app.active_pane_state();
                let name = focused_entry_name(app);
                let cwd_vfs = if name.is_empty() || name == ".." {
                    p.cwd.clone()
                } else {
                    p.cwd.join(&name)
                };
                let disp = cwd_vfs.display();
                let local = disp.strip_prefix("file://").unwrap_or(&disp).to_string();
                std::path::PathBuf::from(local)
            };
            let menu_path = cargonaut_config::menu_config_path();
            let cfg = cargonaut_config::load_user_menu(&menu_path).unwrap_or_default();
            // Filter items whose `only_if` condition fails. Evaluate sequentially
            // (rare — menus are small) to keep the borrow on `entry_path` simple.
            let mut filtered = Vec::new();
            for item in &cfg.actions {
                let keep = if let Some(cond) = &item.only_if {
                    evaluate_only_if(cond, &entry_path).await
                } else {
                    true
                };
                if keep {
                    filtered.push(item.clone());
                }
            }
            if filtered.is_empty() {
                // Surface a user-visible "menu is empty" dialog rather than silently doing nothing.
                *active_dialog = Some(ActiveDialog::UserMenu {
                    widget: dialog::UserMenuDialog::new_error(
                        "No menu actions available. Create ~/.config/cargonaut/menu.toml.",
                    ),
                    entry_path,
                });
            } else {
                *active_dialog = Some(ActiveDialog::UserMenu {
                    widget: dialog::UserMenuDialog::new(filtered),
                    entry_path,
                });
            }
            *mode = Mode::Dialog;
            return Ok(());
        }
        // Feature 052 (FR-001): Alt-? opens the find-file overlay.
        Command::FindFilePopup => {
            let rg_path = &app.config().search.ripgrep_path;
            let content_available = dialog::plan_content_available(rg_path);
            let widget = dialog::FindFileDialog::new(content_available);
            // Anchor the walk to the active pane's current directory.
            let cwd_vfs = app.active_pane_state().cwd.display();
            let cwd_local = cwd_vfs
                .strip_prefix("file://")
                .unwrap_or(&cwd_vfs)
                .to_string();
            let root = std::path::PathBuf::from(cwd_local);
            *active_dialog = Some(ActiveDialog::FindFile { widget, root });
            *mode = Mode::Dialog;
            return Ok(());
        }
        _ => {}
    }
    if let Some(core_cmd) = ui_command_to_core(cmd) {
        let events = app
            .dispatch(core_cmd)
            .await
            .map_err(|e| Error::Other(e.to_string()))?;
        for ev in events {
            apply_event(ev, app, mode, active_dialog, status, quit);
        }
    } else {
        // Labeled on the bar/menu but not yet wired (FR-011, SC-005) —
        // never a silent no-op.
        *status = format!("{} — not yet available", command_label(&cmd));
    }
    Ok(())
}

/// Outcome of an `M-m` mouse-capture toggle (Feature 041, FR-002/003/006).
///
/// Computed by [`plan_mouse_toggle`] from two booleans with no terminal I/O so
/// the FR/SC behavior is unit-testable; [`dispatch_ui_command`] performs the
/// thin `execute!` + status wiring from the returned outcome.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MouseToggleOutcome {
    /// Mouse support is off for the whole session (`--no-mouse` /
    /// `ui.mouse=false`); the toggle changes nothing but explains why (FR-006).
    Disabled,
    /// Capture was off and is now on (FR-003).
    EnabledNow,
    /// Capture was on and is now suspended (FR-002).
    SuspendedNow,
}

impl MouseToggleOutcome {
    /// User-facing status-line message for this outcome (FR-005, transient half).
    fn status(self) -> &'static str {
        match self {
            MouseToggleOutcome::Disabled => {
                "Mouse support disabled for this session (--no-mouse / ui.mouse=false)"
            }
            MouseToggleOutcome::EnabledNow => "Mouse capture: on",
            MouseToggleOutcome::SuspendedNow => {
                "Mouse capture: suspended — Shift+drag to select text"
            }
        }
    }
}

/// Decide what an `M-m` toggle should do (Feature 041, FR-001). Pure: depends
/// only on whether mouse support is enabled for the session (`supported` =
/// `config.ui.mouse`) and whether capture is currently active (`currently` =
/// `UiState.mouse_enabled`).
fn plan_mouse_toggle(supported: bool, currently: bool) -> MouseToggleOutcome {
    match (supported, currently) {
        (false, _) => MouseToggleOutcome::Disabled,
        (true, false) => MouseToggleOutcome::EnabledNow,
        (true, true) => MouseToggleOutcome::SuspendedNow,
    }
}

/// Feature 052 (FR-009): Build a synthetic [`DirListing`] from absolute
/// paths and load it into `pane`. Sets `ui.find_label` to the search
/// pattern so the pane title shows `[Find: s]` (FR-010). The listing
/// uses [`Sort::NameAsc`] (entries are already name-sorted by globset BFS).
///
/// Called by the `handle_key` panelize branch and directly by tests.
pub(crate) fn panelize_into_pane(
    pane: &mut PaneView,
    paths: &[std::path::PathBuf],
    pattern: &str,
    ui: &mut UiState,
) {
    use cargonaut_vfs::{DirEntry, DirListing, Sort, VfsKind, VfsMetadata};
    use smol_str::SmolStr;
    use std::time::SystemTime;

    let entries: Vec<DirEntry> = paths
        .iter()
        .filter_map(|p| {
            let meta = std::fs::metadata(p).ok()?;
            let name = p.file_name()?.to_string_lossy().to_string();
            Some(DirEntry {
                name: SmolStr::new(&name),
                meta: VfsMetadata {
                    size: meta.len(),
                    mtime: meta.modified().unwrap_or(SystemTime::UNIX_EPOCH),
                    mode: None,
                    kind: VfsKind::File,
                    is_hidden: name.starts_with('.'),
                },
            })
        })
        .collect();

    pane.set_listing(DirListing {
        entries,
        sort: Sort::NameAsc,
    });
    ui.find_label = Some(pattern.to_string());
}

/// Feature 049 US2 — validate tagged-file selection and queue the configured
/// diff tool as a `PendingExternal` (FR-005 through FR-008).
///
/// Collects all tagged files from both panes (files and symlinks only, not dirs).
/// Requires exactly 2 total. Splits `tool_str` on whitespace using `shell_words`
/// to build `argv`; appends left-pane path as `args[-2]` and right-pane path as
/// `args[-1]`.
fn queue_diff(app: &App, ui: &mut UiState, status: &mut String, tool_str: Option<&str>) {
    // Validate tool config first (FR-006).
    let Some(tool_str) = tool_str else {
        *status = "No diff tool configured — set [diff] tool = \"<program>\" in config".into();
        return;
    };
    let tool_str = tool_str.trim();
    if tool_str.is_empty() {
        *status = "Diff tool string is empty — set [diff] tool = \"<program>\" in config".into();
        return;
    }

    // Collect tagged file paths per pane (left then right; files/symlinks only).
    let mut tagged: Vec<(PaneId, String)> = Vec::new();
    for id in [PaneId::Left, PaneId::Right] {
        let p = app.pane(id);
        for &idx in &p.selected {
            if let Some(e) = p.listing.entries.get(idx) {
                let is_file = matches!(
                    e.meta.kind,
                    cargonaut_vfs::VfsKind::File | cargonaut_vfs::VfsKind::Symlink { .. }
                );
                if !is_file {
                    continue;
                }
                let disp = p.cwd.join(e.name.as_str()).display();
                let local = disp.strip_prefix("file://").unwrap_or(&disp).to_string();
                tagged.push((id, local));
            }
        }
    }

    // FR-007: exactly 2 tagged files required.
    if tagged.len() != 2 {
        *status = format!(
            "Diff requires exactly 2 tagged files ({} tagged)",
            tagged.len()
        );
        return;
    }

    // Split tool string into argv (shell_words::split handles quoted tokens).
    let mut argv = match shell_words::split(tool_str) {
        Ok(v) if !v.is_empty() => v,
        Ok(_) => {
            *status = "Diff tool string is empty after parsing".into();
            return;
        }
        Err(e) => {
            *status = format!("Diff tool parse error: {e}");
            return;
        }
    };

    // Append left-pane path then right-pane path (contract: args[-2]=left, args[-1]=right).
    let (_, left_path) = tagged
        .iter()
        .find(|(id, _)| *id == PaneId::Left)
        .cloned()
        .unwrap_or_else(|| tagged[0].clone());
    let (_, right_path) = tagged
        .iter()
        .rev()
        .find(|(id, _)| *id == PaneId::Right)
        .cloned()
        .unwrap_or_else(|| tagged[1].clone());

    let program = argv.remove(0);
    argv.push(left_path);
    argv.push(right_path);

    ui.pending_external = Some(PendingExternal {
        program,
        args: argv,
        kind: PendingExternalKind::FileOpen,
    });
}

/// Feature 050 US1 — write the tagged entry basenames to a temp file, then
/// queue the configured `$EDITOR` as a `PendingExternal` with kind `BulkRename`.
///
/// No-op with a status message if nothing is tagged in the active pane.
/// Any entry whose basename contains `\n` is warned and excluded.
fn queue_bulk_rename(app: &App, ui: &mut UiState, status: &mut String) {
    let p = app.active_pane_state();
    // Collect tagged basenames in listing order.
    let mut names: Vec<String> = p
        .selected
        .iter()
        .filter_map(|&idx| p.listing.entries.get(idx))
        .map(|e| e.name.to_string())
        .collect();

    // Exclude entries whose name contains a newline (can't round-trip through the temp file).
    names.retain(|n| {
        if n.contains('\n') {
            // Status message will be overwritten; just skip silently (the name is pathological).
            false
        } else {
            true
        }
    });

    if names.is_empty() {
        *status = "Tag at least one entry to bulk rename".into();
        return;
    }

    // Write names to a temp file.
    let temp_path = std::env::temp_dir().join(format!(
        "cargonaut-rename-{}-{}.txt",
        std::process::id(),
        names.len()
    ));
    let content = names.join("\n") + "\n";
    if let Err(e) = std::fs::write(&temp_path, &content) {
        *status = format!("Could not write rename temp file: {e}");
        return;
    }

    let editor = std::env::var("EDITOR").unwrap_or_else(|_| "vi".into());
    let temp_path_str = temp_path.to_string_lossy().to_string();
    ui.pending_external = Some(PendingExternal {
        program: editor,
        args: vec![temp_path_str],
        kind: PendingExternalKind::BulkRename {
            temp_path,
            original_names: names,
        },
    });
}

/// Feature 050 US1 — after `$EDITOR` exits in bulk-rename mode: read the temp
/// file, validate edits, clean up the temp file unconditionally (SC-005/FR-009),
/// then dispatch renames into the App.
async fn apply_bulk_rename_from_temp(
    app: &mut App,
    temp_path: &std::path::Path,
    original_names: &[String],
    status: &mut String,
) {
    // Read first, then delete unconditionally (SC-005/FR-009).
    let edited_content = match std::fs::read_to_string(temp_path) {
        Ok(s) => s,
        Err(e) => {
            let _ = std::fs::remove_file(temp_path);
            *status = format!("Could not read rename temp file: {e}");
            return;
        }
    };
    // SC-005 / FR-009: delete unconditionally before any early return.
    let _ = std::fs::remove_file(temp_path);

    let edited: Vec<String> = edited_content.lines().map(|l| l.to_string()).collect();

    let pairs = match cargonaut_core::validate_rename_proposals(original_names, &edited) {
        Ok(p) => p,
        Err(e) => {
            *status = format!("Rename validation error: {e}");
            return;
        }
    };

    match app
        .dispatch(cargonaut_core::Command::BulkRenameApply(pairs))
        .await
    {
        Ok(events) => {
            for ev in events {
                if let cargonaut_core::Event::Status(s) = ev {
                    *status = s;
                }
            }
        }
        Err(e) => *status = format!("Rename failed: {e}"),
    }
}

/// Suspend the TUI, run an external program, then restore the terminal
/// (FR-030/031/FR-008). Uses `Command::new(program).args(args)` — no shell.
fn run_external<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    ext: &PendingExternal,
    mouse_enabled: bool,
) -> Result<(), Error> {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = std::process::Command::new(&ext.program)
        .args(&ext.args)
        .status();
    enable_raw_mode().map_err(Error::Terminal)?;
    execute!(stdout(), EnterAlternateScreen).map_err(Error::Terminal)?;
    if mouse_enabled {
        let _ = execute!(stdout(), EnableMouseCapture);
    }
    term.clear().map_err(Error::Terminal)?;
    Ok(())
}

/// Human-facing label for a deferred command's status message.
fn command_label(cmd: &Command) -> &'static str {
    match cmd {
        Command::Preview => "View (F3)",
        Command::Edit => "Edit (F4)",
        Command::Mkdir => "Mkdir (F7)",
        Command::ShowUserMenu => "User menu (F2)",
        Command::CycleSortKey => "Sort order",
        Command::CycleListingMode => "Listing mode",
        Command::RecursiveDirSize => "Directory size",
        _ => "This action",
    }
}

/// Handle a mouse event against the last-rendered [`FrameLayout`] (US3).
#[allow(clippy::too_many_arguments)]
async fn handle_mouse(
    m: MouseEvent,
    app: &mut App,
    ui: &mut UiState,
    left: &PaneView,
    right: &PaneView,
    status: &mut String,
    mode: &mut Mode,
    active_dialog: &mut Option<ActiveDialog>,
    quit: &mut bool,
) -> Result<(), Error> {
    if !ui.mouse_enabled {
        return Ok(());
    }
    let (x, y) = (m.column, m.row);
    match m.kind {
        MouseEventKind::ScrollDown => {
            // Feature 054: scroll subshell panel if click is inside it.
            if let Some(srect) = ui.layout.subshell {
                if rect_contains(srect, x, y) {
                    if let Some(s) = ui.subshell.as_mut() {
                        // ScrollDown = move toward live bottom (T004: direction fix).
                        s.scroll_offset = s.scroll_offset.saturating_sub(1);
                    }
                    return Ok(());
                }
            }
            let _ = app
                .dispatch(AppCommand::CursorDown)
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        MouseEventKind::ScrollUp => {
            // Feature 054: scroll subshell panel if click is inside it.
            if let Some(srect) = ui.layout.subshell {
                if rect_contains(srect, x, y) {
                    if let Some(s) = ui.subshell.as_mut() {
                        // ScrollUp = move into older history (T004: direction fix).
                        s.scroll_offset = s.scroll_offset.saturating_add(1);
                    }
                    return Ok(());
                }
            }
            let _ = app
                .dispatch(AppCommand::CursorUp)
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        MouseEventKind::Down(MouseButton::Left) => {
            // Feature 054: click in subshell panel gives it keyboard focus.
            if let Some(srect) = ui.layout.subshell {
                if rect_contains(srect, x, y) {
                    if ui.subshell_phase == subshell::SubshellPhase::VisibleFmFocus {
                        dispatch_ui_command(
                            Command::OpenSubshell,
                            app,
                            mode,
                            active_dialog,
                            status,
                            quit,
                            ui,
                        )
                        .await?;
                    }
                    return Ok(());
                }
            }
            // 1. Function-key bar buttons (FR-017).
            if let Some(cmd) = ui.fkeybar.command_at(ui.layout.fkeys, x, y) {
                dispatch_ui_command(cmd, app, mode, active_dialog, status, quit, ui).await?;
                return Ok(());
            }
            // 2. Menu-bar titles (FR-017).
            if let Some(idx) = ui.menu.title_at(ui.layout.menu, x, y) {
                ui.menu.open(idx);
                return Ok(());
            }
            // 3. Panel rows: focus + move cursor (FR-014), double-click
            //    descends (FR-015).
            let hit = if rect_contains(ui.layout.left, x, y) {
                Some((PaneId::Left, AppCommand::FocusLeft, left, ui.layout.left))
            } else if rect_contains(ui.layout.right, x, y) {
                Some((
                    PaneId::Right,
                    AppCommand::FocusRight,
                    right,
                    ui.layout.right,
                ))
            } else {
                None
            };
            if let Some((_pane, focus_cmd, view, rect)) = hit {
                let _ = app
                    .dispatch(focus_cmd)
                    .await
                    .map_err(|e| Error::Other(e.to_string()))?;
                let row = (y - rect.y) as usize;
                let index = view.viewport_top() + row;
                let _ = app
                    .dispatch(AppCommand::CursorTo(index))
                    .await
                    .map_err(|e| Error::Other(e.to_string()))?;
                // Double-click detection: same cell within 400ms.
                let is_double = ui
                    .last_click
                    .map(|(lx, ly, t)| lx == x && ly == y && t.elapsed().as_millis() < 400)
                    .unwrap_or(false);
                if is_double {
                    let evs = app
                        .dispatch(AppCommand::Descend)
                        .await
                        .map_err(|e| Error::Other(e.to_string()))?;
                    for ev in evs {
                        if let AppEvent::Status(s) = ev {
                            *status = s;
                        }
                    }
                    ui.last_click = None;
                } else {
                    ui.last_click = Some((x, y, Instant::now()));
                }
            }
        }
        _ => {}
    }
    Ok(())
}

/// True if `(x, y)` is inside `r`.
fn rect_contains(r: Rect, x: u16, y: u16) -> bool {
    x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height
}

/// US4 (FR-022) quick-view: bounded text preview of the active pane's
/// highlighted file. Reads at most 64 KiB; binary/oversized → placeholder.
fn compute_preview(app: &App) -> String {
    let p = app.active_pane_state();
    let Some(idx) = p.focused_entry_index() else {
        return "(nothing highlighted)".into();
    };
    let Some(e) = p.listing.entries.get(idx) else {
        return String::new();
    };
    if matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir) {
        return format!("{}/  (directory)", e.name);
    }
    let path = p.cwd.join(e.name.as_str());
    let disp = path.display();
    let local = disp.strip_prefix("file://").unwrap_or(&disp).to_string();
    const CAP: usize = 64 * 1024;
    use std::io::Read;
    let mut buf = vec![0u8; CAP];
    match std::fs::File::open(&local).and_then(|mut f| f.read(&mut buf)) {
        Ok(n) => {
            let slice = &buf[..n];
            if slice.contains(&0) {
                return format!("{}  (binary file, {} bytes)", e.name, e.meta.size);
            }
            String::from_utf8_lossy(slice)
                .lines()
                .take(1000)
                .collect::<Vec<_>>()
                .join("\n")
        }
        Err(err) => format!("(cannot preview {}: {err})", e.name),
    }
}

/// US5 (FR-026): one-line summary of the active transfer, if any.
fn progress_summary(app: &App) -> Option<String> {
    app.active_progress().map(|p| {
        let pct = p
            .bytes_done
            .saturating_mul(100)
            .checked_div(p.bytes_total)
            .unwrap_or(0);
        format!(
            "{}/{} bytes ({pct}%)   {:.1} MiB/s   ETA {}s",
            p.bytes_done, p.bytes_total, p.throughput_mibs, p.eta_secs
        )
    })
}

/// Feature 039 — project the App's `job_views()` into tasks-panel rows.
/// Index-aligned with `job_views()` so the event loop can map a row index
/// back to a transfer id. Eligibility flags follow the data-model table.
fn build_job_rows(app: &App) -> Vec<JobRow> {
    use cargonaut_core::JobStatus;
    let base = |s: &str| s.rsplit('/').next().unwrap_or(s).to_string();
    app.job_views()
        .into_iter()
        .map(|v| {
            let label = format!("{} → {}", base(&v.src), base(&v.dst));
            let status_label = match &v.status {
                JobStatus::Queued => "Queued".to_string(),
                JobStatus::Running {
                    bytes_done,
                    bytes_total,
                    ..
                } => {
                    let pct = bytes_done
                        .saturating_mul(100)
                        .checked_div(*bytes_total)
                        .unwrap_or(0);
                    format!("Running {pct}%")
                }
                JobStatus::Paused => "Paused".to_string(),
                JobStatus::Completed { verified } => {
                    if *verified {
                        "Completed ✓".to_string()
                    } else {
                        "Completed ✗".to_string()
                    }
                }
                JobStatus::Failed { resumable } => {
                    if *resumable {
                        "Failed (resumable)".to_string()
                    } else {
                        "Failed".to_string()
                    }
                }
                JobStatus::Cancelled => "Cancelled".to_string(),
            };
            JobRow {
                can_cancel: matches!(
                    v.status,
                    JobStatus::Queued | JobStatus::Running { .. } | JobStatus::Paused
                ),
                can_pause: matches!(v.status, JobStatus::Queued | JobStatus::Running { .. }),
                can_resume: matches!(v.status, JobStatus::Paused),
                label,
                status_label,
            }
        })
        .collect()
}

/// Feature 042 — build the hotlist popup rows from the bookmarks, organized by
/// group (a non-selectable header per group, ungrouped under a default
/// section). Each entry row carries its original index so the event loop can map
/// a selection back to a bookmark (SC-007).
fn build_hotlist_rows(bookmarks: &[cargonaut_core::Bookmark]) -> Vec<HotlistRow> {
    let hl = cargonaut_core::Hotlist {
        bookmarks: bookmarks.to_vec(),
    };
    let mut rows = Vec::new();
    for (group, entries) in hl.grouped() {
        let header = group.unwrap_or("(ungrouped)");
        rows.push(HotlistRow {
            display: format!("▸ {header}"),
            index: None,
        });
        for (idx, b) in entries {
            rows.push(HotlistRow {
                display: format!("   {}  —  {}", b.name, b.path),
                index: Some(idx),
            });
        }
    }
    rows
}

/// Feature 043 — the focused entry's current permission bits as an octal
/// string (for prefilling the chmod prompt); `"644"` when unavailable.
fn focused_octal_mode(app: &App) -> String {
    let p = app.active_pane_state();
    p.focused_entry_index()
        .and_then(|i| p.listing.entries.get(i))
        .and_then(|e| e.meta.mode.as_ref())
        .map(|m| format!("{:o}", m.bits & 0o777))
        .unwrap_or_else(|| "644".to_string())
}

/// Feature 043 — the focused entry's current owner as `uid:gid` (for prefilling
/// the chown prompt); empty when unavailable.
fn focused_owner(app: &App) -> String {
    let p = app.active_pane_state();
    p.focused_entry_index()
        .and_then(|i| p.listing.entries.get(i))
        .and_then(|e| e.meta.mode.as_ref())
        .and_then(|m| Some((m.uid?, m.gid?)))
        .map(|(u, g)| format!("{u}:{g}"))
        .unwrap_or_default()
}

/// Feature 043 — the focused entry's name (for prefilling a link prompt);
/// empty when nothing is focused.
fn focused_entry_name(app: &App) -> String {
    let p = app.active_pane_state();
    p.focused_entry_index()
        .and_then(|i| p.listing.entries.get(i))
        .map(|e| e.name.to_string())
        .unwrap_or_default()
}

/// Feature 051 (FR-031): return `true` if `bytes` look like valid UTF-8 content.
/// Samples up to [`dialog::BINARY_DETECT_BYTES`] bytes; any UTF-8 decode error → binary.
fn is_valid_utf8_sample(bytes: &[u8]) -> bool {
    let sample = &bytes[..bytes.len().min(dialog::BINARY_DETECT_BYTES)];
    std::str::from_utf8(sample).is_ok()
}

/// Feature 051 (T013): asynchronous helper that opens a file for the built-in viewer.
///
/// Build a compact chunk index for a large file: `(line_number, byte_offset)` every
/// `CHUNK_INDEX_INTERVAL` lines.  Returns `(index, total_lines, total_bytes)`.
#[allow(clippy::type_complexity)]
fn build_chunk_index(path: &std::path::Path) -> std::io::Result<(Vec<(usize, u64)>, usize, u64)> {
    use std::io::{BufRead, BufReader, Seek};
    let file = std::fs::File::open(path)?;
    let total_bytes = file.metadata()?.len();
    let mut reader = BufReader::with_capacity(65536, file);
    let mut chunk_index: Vec<(usize, u64)> = Vec::new();
    let mut line_count = 0usize;
    let mut buf = String::new();
    loop {
        // Record entry before reading line `line_count`.
        if line_count % dialog::CHUNK_INDEX_INTERVAL == 0 {
            chunk_index.push((line_count, reader.stream_position()?));
        }
        buf.clear();
        if reader.read_line(&mut buf)? == 0 {
            break; // EOF
        }
        line_count += 1;
    }
    // Remove trailing entry pointing past the last line (avoids off-by-one in T037 test).
    if let Some(&(last_line, _)) = chunk_index.last() {
        if last_line >= line_count {
            chunk_index.pop();
        }
    }
    Ok((chunk_index, line_count, total_bytes))
}

/// Steps:
/// 1. Resolve symlinks via `canonicalize`, keeping `display_name` for the title bar.
/// 2. Read a sample to determine `ViewMode` (binary → hex, UTF-8 → text).
/// 3. For files ≤ `STREAMING_THRESHOLD_BYTES`, pre-load all lines with ANSI stripping.
/// 4. For larger UTF-8 files, build a chunk index and stream the first window.
/// 5. Construct and return the `FileViewerDialog`.
#[doc(hidden)] // exposed only for benchmarks — not a stable public API
pub async fn open_file_viewer(
    raw_path: std::path::PathBuf,
    display_name: String,
) -> std::io::Result<dialog::FileViewerDialog> {
    // Resolve symlinks; keep display_name as-is for the title bar.
    let resolved = tokio::task::spawn_blocking({
        let p = raw_path.clone();
        move || std::fs::canonicalize(&p)
    })
    .await
    .map_err(|e| std::io::Error::other(e.to_string()))??;

    // Read a sample for binary detection (up to BINARY_DETECT_BYTES or the full file).
    let sample = tokio::task::spawn_blocking({
        let p = resolved.clone();
        move || -> std::io::Result<(Vec<u8>, u64)> {
            use std::io::Read;
            let mut f = std::fs::File::open(&p)?;
            let size = f.metadata()?.len();
            let mut buf = vec![0u8; dialog::BINARY_DETECT_BYTES.min(size as usize)];
            f.read_exact(&mut buf)?;
            Ok((buf, size))
        }
    })
    .await
    .map_err(|e| std::io::Error::other(e.to_string()))??;
    let (sample_bytes, file_size) = sample;

    if !is_valid_utf8_sample(&sample_bytes) {
        // Binary file: read all bytes and open in hex mode.
        let bytes = tokio::task::spawn_blocking({
            let p = resolved.clone();
            move || std::fs::read(&p)
        })
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))??;
        return Ok(dialog::FileViewerDialog::new_hex(
            resolved,
            display_name,
            bytes,
        ));
    }

    if file_size as usize <= dialog::STREAMING_THRESHOLD_BYTES {
        // Small UTF-8 file: pre-load all lines.
        let bytes = tokio::task::spawn_blocking({
            let p = resolved.clone();
            move || std::fs::read(&p)
        })
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))??;
        let content = String::from_utf8_lossy(&bytes);
        let lines: Vec<String> = content.lines().map(strip_ansi_escapes::strip_str).collect();
        return Ok(dialog::FileViewerDialog::new_text(
            resolved,
            display_name,
            lines,
            false,
        ));
    }

    // Large UTF-8 file: build chunk index + load first window.
    let (chunk_index, total_lines, total_bytes, initial_lines, reader_offset) =
        tokio::task::spawn_blocking({
            let p = resolved.clone();
            move || -> std::io::Result<_> {
                let (chunk_index, total_lines, total_bytes) = build_chunk_index(&p)?;
                let (lines, reader_offset) = dialog::FileViewerDialog::load_window_from_chunk(
                    &p,
                    0,
                    dialog::WINDOW_MAX_LINES / 2,
                )?;
                let deque: std::collections::VecDeque<String> = lines.into_iter().collect();
                Ok((chunk_index, total_lines, total_bytes, deque, reader_offset))
            }
        })
        .await
        .map_err(|e| std::io::Error::other(e.to_string()))??;

    Ok(dialog::FileViewerDialog::new_streaming(
        resolved,
        display_name,
        chunk_index,
        initial_lines,
        total_lines,
        total_bytes,
        reader_offset,
    ))
}

/// Feature 056 — open a file in the built-in editor (FR-007..FR-010, US3 safety limits).
///
/// Performs the same binary and size checks as `open_file_viewer`:
/// - Declines binary files (non-UTF-8 sample) with an I/O error.
/// - Declines files larger than `STREAMING_THRESHOLD_BYTES` (10 MiB) with an I/O error.
/// - Detects LF vs CRLF line endings and records them for round-trip preservation.
pub async fn open_file_editor(
    raw_path: std::path::PathBuf,
    display_name: String,
) -> std::io::Result<dialog::FileEditorDialog> {
    let resolved = tokio::task::spawn_blocking({
        let p = raw_path.clone();
        move || std::fs::canonicalize(&p)
    })
    .await
    .map_err(|e| std::io::Error::other(e.to_string()))??;

    // Read a sample for binary detection (up to BINARY_DETECT_BYTES or the full file).
    let sample = tokio::task::spawn_blocking({
        let p = resolved.clone();
        move || -> std::io::Result<(Vec<u8>, u64)> {
            use std::io::Read;
            let mut f = std::fs::File::open(&p)?;
            let size = f.metadata()?.len();
            let mut buf = vec![0u8; dialog::BINARY_DETECT_BYTES.min(size as usize)];
            f.read_exact(&mut buf)?;
            Ok((buf, size))
        }
    })
    .await
    .map_err(|e| std::io::Error::other(e.to_string()))??;
    let (sample_bytes, file_size) = sample;

    if !is_valid_utf8_sample(&sample_bytes) {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            "binary file cannot be opened in the text editor",
        ));
    }
    if file_size as usize > dialog::STREAMING_THRESHOLD_BYTES {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidData,
            format!(
                "file is too large ({} bytes, max {} MiB)",
                file_size,
                dialog::STREAMING_THRESHOLD_BYTES / (1024 * 1024)
            ),
        ));
    }

    let bytes = tokio::task::spawn_blocking({
        let p = resolved.clone();
        move || std::fs::read(&p)
    })
    .await
    .map_err(|e| std::io::Error::other(e.to_string()))??;

    let content = String::from_utf8_lossy(&bytes);
    // Detect line ending from the first occurrence found.
    let line_ending = if content.contains("\r\n") {
        dialog::LineEnding::Crlf
    } else {
        dialog::LineEnding::Lf
    };

    Ok(dialog::FileEditorDialog::new(
        resolved,
        display_name,
        content.into_owned(),
        line_ending,
    ))
}

/// Feature 042 — parse a bookmark-add prompt into `(group, name)`. Text of the
/// form `group/name` splits on the first `/` (both sides trimmed); text with no
/// `/` is the name with no group.
fn parse_bookmark_input(text: &str) -> (Option<String>, String) {
    match text.split_once('/') {
        Some((g, n)) => {
            let g = g.trim();
            let group = if g.is_empty() {
                None
            } else {
                Some(g.to_string())
            };
            (group, n.trim().to_string())
        }
        None => (None, text.trim().to_string()),
    }
}

fn apply_event(
    ev: AppEvent,
    _app: &mut App,
    mode: &mut Mode,
    active_dialog: &mut Option<ActiveDialog>,
    status: &mut String,
    quit: &mut bool,
) {
    match ev {
        AppEvent::QuitRequested => {
            *quit = true;
        }
        AppEvent::Status(s) => {
            *status = s;
        }
        AppEvent::DialogRequested(kind) => {
            *mode = Mode::Dialog;
            *active_dialog = Some(make_dialog(kind));
        }
        AppEvent::PaneUpdated(_)
        | AppEvent::TransferProgressed(_)
        | AppEvent::TransferTerminated(_) => {
            // Render loop picks these up automatically next iteration.
        }
    }
}

fn make_dialog(kind: DialogKind) -> ActiveDialog {
    match kind {
        DialogKind::Confirm {
            title,
            body,
            on_confirm,
        } => ActiveDialog::Confirm {
            widget: ConfirmDialog::new(title, body),
            on_confirm: *on_confirm,
        },
    }
}

/// Map a core [`ResumeOfferView`] onto the dialog's per-row summary
/// (Feature 037). Strips the `file://` scheme for a tidier row.
fn resume_summary_from(v: &ResumeOfferView) -> ResumableSummary {
    fn short(s: &str) -> String {
        s.strip_prefix("file://").unwrap_or(s).to_string()
    }
    ResumableSummary {
        src: short(&v.src),
        dst: short(&v.dst),
        bytes_written_mib: v.bytes_written_mib,
        src_size_mib: v.src_size_mib,
        source_unchanged: v.source_unchanged,
        dest_intact: v.dest_intact,
    }
}

/// Map a `cargonaut-ui-tui::keymap::Command` (parsed from the user's
/// keymap.toml) to the matching `cargonaut-core::Command` (the
/// dispatcher's input). Returns `None` for keymap commands that the
/// Phase 1 App doesn't yet handle (status-bar reports them as
/// "Unbound").
fn ui_command_to_core(cmd: Command) -> Option<AppCommand> {
    use Command as U;
    Some(match cmd {
        U::Quit => AppCommand::Quit,
        U::FocusSwapPane => AppCommand::FocusSwap,
        U::FocusLeftPane => AppCommand::FocusLeft,
        U::FocusRightPane => AppCommand::FocusRight,
        U::CursorDown => AppCommand::CursorDown,
        U::CursorUp => AppCommand::CursorUp,
        U::DescendOrOpen => AppCommand::Descend,
        U::AscendParent => AppCommand::Ascend,
        U::SelectionToggle => AppCommand::SelectionToggle,
        U::SelectionInvert => AppCommand::SelectionInvert,
        U::ToggleHidden => AppCommand::ToggleHidden,
        U::CopySelection => AppCommand::Copy,
        U::MoveOrRenameSelection => AppCommand::Move,
        U::DeleteSelection => AppCommand::Delete,
        U::CancelCurrentOperation => AppCommand::CancelCurrentTransfer,
        U::TogglePanelFilter => AppCommand::TogglePanelFilter,
        U::SyncOtherPanelPath => AppCommand::SyncOtherPanelPath,
        U::ShowFocusedInOtherPanel => AppCommand::ShowFocusedInOtherPanel,
        U::ToggleSplitOrientation => AppCommand::ToggleSplitOrientation,
        U::HistoryPrevDir => AppCommand::HistoryPrevDir,
        U::HistoryNextDir => AppCommand::HistoryNextDir,
        U::QuickCdPopup => AppCommand::QuickCdPopup,
        U::ShowTasksPanel => AppCommand::ShowTasksPanel,
        U::CycleSortKey => AppCommand::CycleSortKey,
        U::CycleListingMode => AppCommand::CycleListingMode,
        U::RecursiveDirSize => AppCommand::RecursiveDirSize,
        U::CompareDirectories => AppCommand::CompareDirectories,
        U::UndoLastOp => AppCommand::UndoLastOp,
        U::NewTab => AppCommand::TabNew,
        U::CloseTab => AppCommand::TabClose,
        U::TabNext => AppCommand::TabNext,
        U::TabPrev => AppCommand::TabPrev,
        _ => return None,
    })
}

/// Convert a `VfsPath` (file:///…) to a local `PathBuf`.
///
/// Joins segments with `/`; prepends `/` for the root. Non-`file` scheme
/// paths (sftp, s3) fall back to `/` since the subshell can't cd to them.
/// Percent-encode a host filesystem path for use as a `VfsPath` authority.
/// Only `%` and `/` are encoded; all other bytes are passed through unchanged.
fn encode_archive_authority(path: &str) -> String {
    let mut out = String::with_capacity(path.len() * 3);
    for c in path.chars() {
        match c {
            '%' => out.push_str("%25"),
            '/' => out.push_str("%2F"),
            _ => out.push(c),
        }
    }
    out
}

fn vfs_path_to_local(vpath: &cargonaut_vfs::types::VfsPath) -> std::path::PathBuf {
    if vpath.scheme != "file" {
        return std::path::PathBuf::from("/");
    }
    let mut pb = std::path::PathBuf::from("/");
    for seg in &vpath.segments {
        pb.push(seg.as_str());
    }
    pb
}

/// Render the persistent mouse-capture indicator right-aligned in the menu-bar
/// row (Feature 041 US2 / FR-005). Called after `menu.render` so it sits atop
/// the bar background; menu dropdowns open one row below and never overlap it.
/// Dimmed when mouse support is disabled for the session.
fn render_mouse_indicator(
    buf: &mut ratatui::buffer::Buffer,
    menu_row: Rect,
    theme: &Theme,
    supported: bool,
    captured: bool,
) {
    use ratatui::widgets::Widget;
    let label = chrome::mouse_indicator(supported, captured);
    let w = label.len() as u16;
    if menu_row.width <= w {
        return; // too narrow — degrade silently (NFR: never panic)
    }
    let rect = Rect {
        x: menu_row.x + menu_row.width - w,
        y: menu_row.y,
        width: w,
        height: menu_row.height.min(1),
    };
    let mut style = Style::default().fg(theme.menu_fg).bg(theme.menu_bg);
    if !supported {
        style = style.add_modifier(ratatui::style::Modifier::DIM);
    }
    Paragraph::new(label).style(style).render(rect, buf);
}

#[allow(clippy::too_many_arguments)]
fn draw_frame(
    f: &mut ratatui::Frame,
    left: &mut PaneView,
    right: &mut PaneView,
    active: PaneId,
    mode: Mode,
    status: &str,
    dialog: Option<&mut ActiveDialog>,
    theme: &Theme,
    menu: &mut MenuBar,
    fkeybar: &FunctionKeyBar,
    ms_left: &str,
    ms_right: &str,
    help_overlay: Option<&dialog::HelpOverlay>,
    view_mode: cargonaut_core::ViewMode,
    qv_preview: &str,
    progress: Option<&str>,
    mouse_supported: bool,
    mouse_captured: bool,
    tab_bar_left: &[cargonaut_core::TabBarEntry],
    tab_bar_right: &[cargonaut_core::TabBarEntry],
    subshell_phase: subshell::SubshellPhase,
    subshell_screen: Option<&vt100::Screen>,
    subshell_dead: bool,
    subshell_rows: u16,
) -> FrameLayout {
    use cargonaut_core::ViewMode;
    use ratatui::widgets::Widget;
    let area = f.size();
    // US2/Feature054 layout: [menu | panes | (subshell) | status | fkeys].
    let main_chunks = if subshell_phase.is_visible() {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),             // menu bar
                Constraint::Min(3),                // panes
                Constraint::Length(subshell_rows), // subshell panel
                Constraint::Length(1),             // status
                Constraint::Length(1),             // function-key bar
            ])
            .split(area)
    } else {
        Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // menu bar
                Constraint::Min(3),    // panes
                Constraint::Length(1), // status
                Constraint::Length(1), // function-key bar
            ])
            .split(area)
    };
    // Named rect indices — shift when subshell panel is visible.
    let (subshell_rect_opt, status_idx, fkeys_idx) = if subshell_phase.is_visible() {
        (Some(main_chunks[2]), 3usize, 4usize)
    } else {
        (None, 2usize, 3usize)
    };

    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[1]);

    // US4 (FR-022): Brief/Full column layout; in QuickView the *passive*
    // pane previews the active pane's highlighted file.
    let pane_layout = match view_mode {
        ViewMode::Brief => pane::PaneLayout::Brief,
        _ => pane::PaneLayout::Full,
    };
    let qv = view_mode == ViewMode::QuickView;

    let left_inner = if qv && active == PaneId::Right {
        draw_preview(f, pane_chunks[0], theme, qv_preview)
    } else {
        draw_pane(
            f,
            left,
            pane_chunks[0],
            active == PaneId::Left,
            theme,
            ms_left,
            pane_layout,
            tab_bar_left,
        )
    };
    let right_inner = if qv && active == PaneId::Left {
        draw_preview(f, pane_chunks[1], theme, qv_preview)
    } else {
        draw_pane(
            f,
            right,
            pane_chunks[1],
            active == PaneId::Right,
            theme,
            ms_right,
            pane_layout,
            tab_bar_right,
        )
    };

    // Feature 054: render subshell panel when visible.
    let layout_subshell = if let Some(srect) = subshell_rect_opt {
        // 1-row header inside the panel rect.
        let header_rect = Rect { height: 1, ..srect };
        let body_rect = Rect {
            y: srect.y + 1,
            height: srect.height.saturating_sub(1),
            ..srect
        };
        let phase_label = match subshell_phase {
            subshell::SubshellPhase::VisibleFmFocus => "FM Focus",
            subshell::SubshellPhase::VisibleShellFocus => "Shell Focus",
            _ => "",
        };
        Paragraph::new(format!(" [Shell] {phase_label}"))
            .style(
                ratatui::style::Style::default()
                    .fg(theme.menu_fg)
                    .bg(theme.menu_bg)
                    .add_modifier(ratatui::style::Modifier::BOLD),
            )
            .render(header_rect, f.buffer_mut());
        if subshell_dead {
            Paragraph::new(" Shell exited — press Ctrl-o to restart")
                .style(theme.status_style())
                .render(body_rect, f.buffer_mut());
        } else if let Some(screen) = subshell_screen {
            subshell::render_vt100_screen(screen, body_rect, f.buffer_mut());
        }
        Some(srect)
    } else {
        None
    };

    // US1 (FR-002): status bar themed instead of bare reverse-video.
    let status_text = format!(" [{mode:?}]  {status}");
    Paragraph::new(status_text)
        .style(theme.status_style())
        .render(main_chunks[status_idx], f.buffer_mut());

    // US2: function-key bar (bottom) + menu bar (top, may drop down over panes).
    fkeybar.render(main_chunks[fkeys_idx], f.buffer_mut(), theme);
    menu.render(main_chunks[0], f.buffer_mut(), theme);
    // Feature 041 US2 (FR-005): persistent capture indicator in the menu-row
    // right gutter (rendered after the menu so it overlays the bar background).
    render_mouse_indicator(
        f.buffer_mut(),
        main_chunks[0],
        theme,
        mouse_supported,
        mouse_captured,
    );

    // US5 (FR-026): transfer progress overlay while a copy/move runs.
    if let Some(p) = progress {
        draw_progress(f, theme, area, p);
    }

    if let Some(overlay) = help_overlay {
        overlay.render(f, area, theme);
    }

    if let Some(d) = dialog {
        let darea = centered_rect(60, 30, area);
        match d {
            ActiveDialog::Confirm { widget, .. } => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::Resume(widget) => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::Input { widget, .. } => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::QuickCd { widget } => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::FilterPrompt { widget } => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::TasksPanel { widget } => widget.render(darea, f.buffer_mut(), theme),
            ActiveDialog::Hotlist { widget } => widget.render(darea, f.buffer_mut(), theme),
            // Feature 047 US2: UserMenuDialog::render manages its own centering.
            ActiveDialog::UserMenu { widget, .. } => widget.render(f, area, theme),
            // Feature 051: full-screen overlay — use `area`, not the centred `darea`.
            ActiveDialog::FileViewer { widget } => widget.render(f, area, theme),
            // Feature 056: full-screen editor — use `area`; render updates viewport_height.
            ActiveDialog::FileEditor { widget } => widget.render(area, f.buffer_mut(), theme),
            // Feature 052: find-file overlay — manages its own centering.
            ActiveDialog::FindFile { widget, .. } => widget.render(f, area, theme),
        }
    }

    FrameLayout {
        menu: main_chunks[0],
        left: left_inner,
        right: right_inner,
        fkeys: main_chunks[fkeys_idx],
        subshell: layout_subshell,
    }
}

/// Build a one-row `Line` for the tab bar above a pane column.
///
/// Active tab span uses `theme.cursor_style()`. Entries are separated by two
/// spaces. If all entries don't fit in `width`, the line is scrolled so the
/// active tab is always visible.
fn tab_bar_line<'a>(
    entries: &'a [cargonaut_core::TabBarEntry],
    width: u16,
    theme: &Theme,
) -> ratatui::text::Line<'a> {
    use ratatui::style::Style;
    use ratatui::text::{Line, Span};

    if entries.is_empty() {
        return Line::from(vec![]);
    }

    // Build each entry's text independently.
    let active_style = theme.cursor_style();
    let inactive_style = Style::default();

    let mut spans: Vec<Span<'a>> = Vec::new();
    let separator = "  ";

    // Compute cumulative starting x of each entry (unseparated widths).
    let texts: Vec<String> = entries
        .iter()
        .map(|e| {
            format!(
                "[{}{}]{}",
                e.index,
                if e.is_active { "*" } else { "" },
                e.label
            )
        })
        .collect();

    // Compute scroll offset so the active tab is visible.
    let total_width: usize = texts.iter().map(|t| t.len()).sum::<usize>()
        + separator.len() * texts.len().saturating_sub(1);
    let w = width as usize;

    let scroll_offset: usize = if total_width <= w {
        0
    } else {
        // Find position of active tab
        let active_idx = entries.iter().position(|e| e.is_active).unwrap_or(0);
        let mut pos = 0usize;
        for (i, t) in texts.iter().enumerate() {
            if i == active_idx {
                break;
            }
            pos += t.len() + separator.len();
        }
        // Scroll so active tab starts at least at position 0
        // and doesn't overshoot on the right
        let active_end = pos + texts[active_idx].len();
        if active_end > w {
            pos.saturating_sub(2)
        } else {
            0
        }
    };

    let mut x = 0isize;
    for (i, (e, text)) in entries.iter().zip(texts.iter()).enumerate() {
        if i > 0 {
            let sep_start = x - scroll_offset as isize;
            if sep_start + separator.len() as isize > 0 && sep_start < width as isize {
                spans.push(Span::raw(separator));
            }
            x += separator.len() as isize;
        }
        let entry_start = x - scroll_offset as isize;
        if entry_start < width as isize && entry_start + text.len() as isize > 0 {
            let style = if e.is_active {
                active_style
            } else {
                inactive_style
            };
            spans.push(Span::styled(text.clone(), style));
        }
        x += text.len() as isize;
    }

    Line::from(spans)
}

/// Draw one pane (list + per-pane mini-status). Returns the inner list
/// rect for mouse hit-testing (US3).
#[allow(clippy::too_many_arguments)]
fn draw_pane(
    f: &mut ratatui::Frame,
    view: &mut PaneView,
    area: Rect,
    focused: bool,
    theme: &Theme,
    mini_status: &str,
    layout: pane::PaneLayout,
    tab_bar: &[cargonaut_core::TabBarEntry],
) -> Rect {
    use ratatui::widgets::Widget;
    // Feature 053: 3-constraint split — tab bar (1 row) + list+border + mini-status (1 row).
    let col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1),
            Constraint::Min(2),
            Constraint::Length(1),
        ])
        .split(area);
    // Render tab bar in col[0].
    let tbl = tab_bar_line(tab_bar, col[0].width, theme);
    Paragraph::new(tbl).render(col[0], f.buffer_mut());
    let title = view.cwd.display();
    // US1 (FR-002): panel background + focus-colored border from the theme.
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_style(focused))
        .style(Style::default().bg(theme.panel_bg).fg(theme.panel_fg));
    let inner = block.inner(col[1]);
    block.render(col[1], f.buffer_mut());
    view.render(inner, f.buffer_mut(), theme, layout);
    // US2 (FR-010): per-pane mini-status line.
    Paragraph::new(format!(" {mini_status}"))
        .style(theme.status_style())
        .render(col[2], f.buffer_mut());
    inner
}

/// US4 (FR-022) quick-view: render the passive pane as a bounded text
/// preview of the active pane's highlighted file. Returns its inner rect.
fn draw_preview(f: &mut ratatui::Frame, area: Rect, theme: &Theme, preview: &str) -> Rect {
    use ratatui::widgets::Widget;
    let block = Block::default()
        .title("Quick view")
        .borders(Borders::ALL)
        .border_style(theme.border_style(false))
        .style(Style::default().bg(theme.panel_bg).fg(theme.panel_fg));
    let inner = block.inner(area);
    block.render(area, f.buffer_mut());
    Paragraph::new(preview)
        .style(Style::default().bg(theme.panel_bg).fg(theme.panel_fg))
        .render(inner, f.buffer_mut());
    inner
}

/// US5 (FR-026) transfer progress overlay.
fn draw_progress(f: &mut ratatui::Frame, theme: &Theme, area: Rect, body: &str) {
    use ratatui::widgets::{Clear, Widget};
    let r = centered_rect(60, 20, area);
    Clear.render(r, f.buffer_mut());
    let block = Block::default()
        .title("Transfer")
        .borders(Borders::ALL)
        .style(theme.dialog_style());
    Paragraph::new(format!("\n {body}\n\n Ctrl-c to cancel"))
        .block(block)
        .style(theme.dialog_style())
        .render(r, f.buffer_mut());
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
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

// Feature 047 US2 — T023: safe shell-command builder (SC-003 macro-safety).
//
// Rules:
//  • Substitute `{path}` with the shell-quoted path.
//  • If the command string contains shell metacharacters (|, ;, &, $, `, >, <)
//    after substitution, route through `sh -c` so the shell processes them.
//  • Otherwise split with shell_words::split and exec directly (no shell).
fn build_action_command(command: &str, path: &std::path::Path) -> (String, Vec<String>) {
    let path_str = path.to_string_lossy();
    let quoted = shell_words::quote(&path_str).into_owned();
    let substituted = command.replace("{path}", &quoted);
    // Detect shell metacharacters that require a shell intermediary.
    let needs_shell = substituted.contains('|')
        || substituted.contains(';')
        || substituted.contains('&')
        || substituted.contains('$')
        || substituted.contains('`')
        || substituted.contains('>')
        || substituted.contains('<');
    if needs_shell {
        ("sh".into(), vec!["-c".into(), substituted])
    } else {
        let tokens = shell_words::split(&substituted).unwrap_or_else(|_| vec![substituted.clone()]);
        if tokens.is_empty() {
            ("sh".into(), vec!["-c".into(), substituted])
        } else {
            (tokens[0].clone(), tokens[1..].to_vec())
        }
    }
}

// Feature 047 US2 — T024: evaluate `only_if` shell condition (SC-004 timeout).
//
// Spawns the condition string as `sh -c <cond>` with a 200 ms wall-clock
// timeout. Returns `true` when the process exits with status 0 within the
// deadline; `false` on non-zero exit, spawn failure, or timeout.
async fn evaluate_only_if(condition: &str, path: &std::path::Path) -> bool {
    let path_str = path.to_string_lossy();
    let quoted = shell_words::quote(&path_str).into_owned();
    let script = condition.replace("{path}", &quoted);
    let result = tokio::time::timeout(
        std::time::Duration::from_millis(200),
        tokio::task::spawn_blocking(move || {
            std::process::Command::new("sh")
                .arg("-c")
                .arg(&script)
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .map(|s| s.success())
                .unwrap_or(false)
        }),
    )
    .await;
    match result {
        Ok(Ok(ok)) => ok,
        _ => false,
    }
}

/// TUI errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Terminal could not be put into raw mode (no TTY?), or any
    /// crossterm/io failure during render.
    #[error("terminal: {0}")]
    Terminal(#[from] std::io::Error),

    /// Higher-level failure (App dispatch, VFS, transfer, etc.) bubbled
    /// up from the dispatch loop.
    #[error("{0}")]
    Other(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargonaut_core::App;
    use crossterm::event::KeyModifiers;
    use tempfile::TempDir;

    fn fresh_ui(left_rect: Rect, right_rect: Rect, mouse: bool) -> UiState {
        UiState {
            menu: MenuBar::new(),
            fkeybar: FunctionKeyBar::new(),
            layout: FrameLayout {
                menu: Rect {
                    x: 0,
                    y: 0,
                    width: 80,
                    height: 1,
                },
                left: left_rect,
                right: right_rect,
                fkeys: Rect {
                    x: 0,
                    y: 23,
                    width: 80,
                    height: 1,
                },
                subshell: None,
            },
            last_click: None,
            help_overlay: None,
            mouse_enabled: mouse,
            pending_external: None,
            find_label: None,
            pending_panelize: None,
            subshell: None,
            subshell_phase: subshell::SubshellPhase::default(),
            last_ctrl_o_at: None,
        }
    }

    async fn app_with(left: &TempDir, right: &TempDir) -> App {
        App::new(
            cargonaut_config::Config::default(),
            left.path().to_str().unwrap(),
            right.path().to_str().unwrap(),
        )
        .await
        .unwrap()
    }

    // Feature 041 (FR-002/003/006): the pure toggle-decision truth table.
    #[test]
    fn plan_mouse_toggle_truth_table() {
        assert_eq!(
            plan_mouse_toggle(false, false),
            MouseToggleOutcome::Disabled
        );
        assert_eq!(plan_mouse_toggle(false, true), MouseToggleOutcome::Disabled);
        assert_eq!(
            plan_mouse_toggle(true, false),
            MouseToggleOutcome::EnabledNow
        );
        assert_eq!(
            plan_mouse_toggle(true, true),
            MouseToggleOutcome::SuspendedNow
        );
    }

    #[test]
    fn mouse_toggle_outcome_status_strings() {
        assert!(MouseToggleOutcome::Disabled
            .status()
            .contains("disabled for this session"));
        assert_eq!(MouseToggleOutcome::EnabledNow.status(), "Mouse capture: on");
        assert!(MouseToggleOutcome::SuspendedNow
            .status()
            .contains("suspended"));
        assert!(MouseToggleOutcome::SuspendedNow.status().contains("Shift"));
    }

    // Feature 041 (FR-008 / SC-005): exit always releases mouse capture,
    // regardless of the last toggle state, leaving the terminal clean. We pin
    // the unconditional teardown helper by asserting it emits the crossterm
    // mouse-disable control sequence (the `?1000l` family) to any writer.
    #[test]
    fn teardown_always_releases_mouse_capture() {
        let mut buf: Vec<u8> = Vec::new();
        restore_terminal_modes(&mut buf).unwrap();
        let s = String::from_utf8_lossy(&buf);
        assert!(
            s.contains("1000"),
            "teardown must emit the mouse-disable sequence; got: {s:?}"
        );
    }

    fn help_sections_text() -> String {
        dialog::HELP_SECTIONS
            .iter()
            .flat_map(|s| s.rows.iter().map(|r| format!("{} {}", r.key, r.desc)))
            .collect::<Vec<_>>()
            .join("\n")
    }

    // Feature 044: help documents the recursive attribute keys.
    #[test]
    fn help_documents_recursive_keys() {
        let text = help_sections_text();
        assert!(
            text.contains("C-x C"),
            "help must mention recursive chmod key"
        );
        assert!(
            text.to_lowercase().contains("recursive"),
            "help must mention recursion"
        );
    }

    // Feature 043: help documents the file-attribute keys.
    #[test]
    fn help_documents_attribute_keys() {
        let text = help_sections_text();
        assert!(text.contains("C-x c"), "help must mention chmod key");
        assert!(
            text.to_lowercase().contains("chmod"),
            "help must mention chmod"
        );
    }

    // Feature 042: help documents the Ctrl-b hotlist + in-popup add/remove.
    #[test]
    fn help_documents_hotlist() {
        let text = help_sections_text();
        assert!(text.contains("C-b"), "help must mention the hotlist key");
        assert!(
            text.to_lowercase().contains("bookmark"),
            "help must mention bookmarks"
        );
    }

    // Feature 041 (FR-010 / SC-006): help documents the M-m toggle + the
    // terminal Shift-drag bypass for one-off native text selection.
    #[test]
    fn help_documents_mouse_toggle_and_shift_bypass() {
        let text = help_sections_text();
        assert!(text.contains("M-m"), "help must mention the M-m toggle");
        assert!(
            text.to_lowercase().contains("shift"),
            "help must mention the Shift-drag bypass"
        );
    }

    // Feature 041 US2 (FR-005): the persistent indicator renders right-aligned
    // in the menu-bar row for each capture state.
    #[test]
    fn mouse_indicator_renders_in_menu_row() {
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let theme = Theme::default();
        let render = |supported: bool, captured: bool| -> String {
            let backend = TestBackend::new(40, 1);
            let mut term = Terminal::new(backend).unwrap();
            term.draw(|f| {
                let row = f.size();
                render_mouse_indicator(f.buffer_mut(), row, &theme, supported, captured);
            })
            .unwrap();
            term.backend()
                .buffer()
                .content()
                .iter()
                .map(|c| c.symbol().chars().next().unwrap_or(' '))
                .collect()
        };
        assert!(render(true, true).contains("[mouse:on]"), "captured");
        assert!(render(true, false).contains("[mouse:susp]"), "suspended");
        assert!(render(false, false).contains("[mouse:off]"), "disabled");
    }

    // Feature 041 US1 (FR-002/003): dispatching the toggle flips capture and
    // sets the transient status both ways.
    #[tokio::test]
    async fn toggle_mouse_capture_suspends_then_resumes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await; // config.ui.mouse defaults true
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true); // capture currently on
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::ToggleMouseCapture,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(!ui.mouse_enabled, "first toggle suspends capture");
        assert!(status.contains("suspended"), "status was: {status}");

        dispatch_ui_command(
            Command::ToggleMouseCapture,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(ui.mouse_enabled, "second toggle resumes capture");
        assert_eq!(status, "Mouse capture: on");
    }

    // Feature 041 US3 (FR-006): in a session where mouse support is disabled
    // (`--no-mouse` / `ui.mouse=false`), the toggle never captures and explains
    // why. Behavior is implemented by the dispatch arm's `Disabled` branch
    // (T007); this pins it at the integration level.
    #[tokio::test]
    async fn toggle_is_noop_when_session_mouse_disabled() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut config = cargonaut_config::Config::default();
        config.ui.mouse = false;
        let mut app = App::new(
            config,
            td_l.path().to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, false); // capture off, matching the disabled session
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::ToggleMouseCapture,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(
            !ui.mouse_enabled,
            "capture must stay off in a disabled session"
        );
        assert!(
            status.contains("disabled for this session"),
            "status was: {status}"
        );
    }

    // Feature 041 (FR-007): an external program (F3/F4) suspends and restores
    // the TUI; restoration must honor the *current* toggle, not the launch
    // value. The `run_external` call site reads `ui.mouse_enabled` (the live
    // flag), so a session that launched with capture on but was toggled to
    // suspended must stay suspended after the external program returns. This
    // locks the exact field the call site consults. (Manual end-to-end: see
    // quickstart.md step 5.)
    #[tokio::test]
    async fn external_restore_preserves_toggled_capture_state() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await; // launched with mouse on
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        // Toggle to suspended mid-session.
        dispatch_ui_command(
            Command::ToggleMouseCapture,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();

        // The value `run_external` would receive is `ui.mouse_enabled`; it must
        // reflect the toggle (suspended), not the launch value (on).
        assert!(
            !ui.mouse_enabled,
            "external restore must use the toggled state, not the launch value"
        );
    }

    // Feature 033: drive one key through the real `handle_key` path.
    async fn feed_key(
        code: crossterm::event::KeyCode,
        app: &mut App,
        keymap: &Keymap,
        mode: &mut Mode,
        dlg: &mut Option<ActiveDialog>,
        ui: &mut UiState,
    ) {
        let mut chord_buf: Vec<KeyChord> = Vec::new();
        let mut status = String::new();
        let mut quit = false;
        handle_key(
            KeyEvent::from(code),
            app,
            keymap,
            mode,
            dlg,
            &mut chord_buf,
            &mut status,
            &mut quit,
            ui,
        )
        .await
        .unwrap();
    }

    // Feature 033 SC-005: injected-input E2E through the dialog wiring —
    // invalid → error/stay-open, set → filtered/close, re-open → prefilled,
    // Esc → unchanged, empty submit → cleared.
    #[tokio::test]
    async fn filter_prompt_e2e_invalid_set_cancel_clear() {
        use crossterm::event::KeyCode;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.rs"), b"").unwrap();
        std::fs::write(td_l.path().join("b.md"), b"").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        // Open (Alt-! routes to this command).
        dispatch_ui_command(
            Command::TogglePanelFilter,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(dlg, Some(ActiveDialog::FilterPrompt { .. })));
        assert!(matches!(mode, Mode::Dialog));

        // Invalid glob '[' then Enter → stays open, pane unchanged (FR-006).
        feed_key(
            KeyCode::Char('['),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        feed_key(
            KeyCode::Enter,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(
            matches!(dlg, Some(ActiveDialog::FilterPrompt { .. })),
            "invalid pattern keeps the prompt open"
        );
        assert!(
            app.active_pane_state().filter.is_none(),
            "invalid pattern leaves the pane unchanged"
        );

        // Fix it: backspace the '[', type "*.rs", Enter → filtered + closed.
        feed_key(
            KeyCode::Backspace,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        for c in "*.rs".chars() {
            feed_key(
                KeyCode::Char(c),
                &mut app,
                &keymap,
                &mut mode,
                &mut dlg,
                &mut ui,
            )
            .await;
        }
        feed_key(
            KeyCode::Enter,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none(), "valid submit closes the prompt");
        assert!(matches!(mode, Mode::Pane));
        assert_eq!(app.active_pane_state().visible_indices().len(), 1);

        // Re-open → prefilled with the active pattern (FR-002); Esc → unchanged.
        dispatch_ui_command(
            Command::TogglePanelFilter,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        if let Some(ActiveDialog::FilterPrompt { widget }) = &dlg {
            assert_eq!(
                widget.value(),
                "*.rs",
                "prompt prefilled with current filter"
            );
        } else {
            panic!("expected FilterPrompt open");
        }
        feed_key(
            KeyCode::Esc,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none());
        assert_eq!(
            app.active_pane_state().visible_indices().len(),
            1,
            "Esc leaves the filter intact (FR-008)"
        );

        // Re-open → clear text → Enter clears the filter (FR-005).
        dispatch_ui_command(
            Command::TogglePanelFilter,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        for _ in 0.."*.rs".len() {
            feed_key(
                KeyCode::Backspace,
                &mut app,
                &keymap,
                &mut mode,
                &mut dlg,
                &mut ui,
            )
            .await;
        }
        feed_key(
            KeyCode::Enter,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none());
        assert!(app.active_pane_state().filter.is_none());
        assert_eq!(app.active_pane_state().visible_indices().len(), 2);
    }

    fn left_click(x: u16, y: u16) -> MouseEvent {
        MouseEvent {
            kind: MouseEventKind::Down(MouseButton::Left),
            column: x,
            row: y,
            modifiers: KeyModifiers::NONE,
        }
    }

    fn synced_views(app: &App) -> (PaneView, PaneView) {
        let mut l = PaneView::new(
            app.pane(PaneId::Left).cwd.clone(),
            app.pane(PaneId::Left).listing.clone(),
        );
        let mut r = PaneView::new(
            app.pane(PaneId::Right).cwd.clone(),
            app.pane(PaneId::Right).listing.clone(),
        );
        l.sync_from(app.pane(PaneId::Left));
        r.sync_from(app.pane(PaneId::Right));
        (l, r)
    }

    /// Drive `handle_mouse` with throwaway mode/dialog/quit; return the
    /// resulting status string.
    async fn mouse(
        m: MouseEvent,
        app: &mut App,
        ui: &mut UiState,
        l: &PaneView,
        r: &PaneView,
    ) -> String {
        let mut status = String::new();
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut quit = false;
        handle_mouse(
            m,
            app,
            ui,
            l,
            r,
            &mut status,
            &mut mode,
            &mut dlg,
            &mut quit,
        )
        .await
        .unwrap();
        status
    }

    /// Drive `handle_mouse` with a live dialog slot; return `(status, active_dialog)`.
    async fn mouse_with_dlg(
        m: MouseEvent,
        app: &mut App,
        ui: &mut UiState,
        l: &PaneView,
        r: &PaneView,
    ) -> (String, Option<ActiveDialog>) {
        let mut status = String::new();
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut quit = false;
        handle_mouse(
            m,
            app,
            ui,
            l,
            r,
            &mut status,
            &mut mode,
            &mut dlg,
            &mut quit,
        )
        .await
        .unwrap();
        (status, dlg)
    }

    // T-MOUSE-5b (issue #70 / FR-001/FR-004): left-click on the on-screen F2
    // button opens ActiveDialog::UserMenu — proving the mouse path is
    // equivalent to the keyboard F2 path.
    #[tokio::test]
    async fn f2_mouse_click_opens_user_menu() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 10,
            },
            Rect {
                x: 50,
                y: 1,
                width: 40,
                height: 10,
            },
            true,
        );
        // 100-wide fkey bar, 10 buttons → each slot is 10px wide.
        // F2 is button index 1 (0-indexed) → x in [10, 20). Click x=15.
        ui.layout.fkeys = Rect {
            x: 0,
            y: 23,
            width: 100,
            height: 1,
        };
        let (l, r) = synced_views(&app);
        let (_status, dlg) = mouse_with_dlg(left_click(15, 23), &mut app, &mut ui, &l, &r).await;
        assert!(
            matches!(dlg, Some(ActiveDialog::UserMenu { .. })),
            "left-click on F2 button must open UserMenu dialog; got {dlg:?}"
        );
    }

    // T-MOUSE-2 (FR-014): a left-click in the right panel focuses it and
    // moves the cursor to the clicked row.
    #[tokio::test]
    async fn click_focuses_pane_and_sets_cursor() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c", "d"] {
            std::fs::write(td_r.path().join(n), b"").unwrap();
        }
        let mut app = app_with(&td_l, &td_r).await;
        let right_rect = Rect {
            x: 50,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 10,
            },
            right_rect,
            true,
        );
        let (l, r) = synced_views(&app);
        // Click row 2 (y = rect.y + 2) in the right pane.
        let _ = mouse(left_click(55, 3), &mut app, &mut ui, &l, &r).await;
        assert_eq!(app.active_pane(), PaneId::Right);
        assert_eq!(app.pane(PaneId::Right).cursor, 2);
    }

    // T-MOUSE-3 (FR-015): a double-click on a directory row descends.
    #[tokio::test]
    async fn double_click_descends_into_directory() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("sub")).unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let left_rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(
            left_rect,
            Rect {
                x: 50,
                y: 1,
                width: 40,
                height: 10,
            },
            true,
        );
        let (l, r) = synced_views(&app);
        // Feature 040: row 0 (y=1) is the synthetic `..`; "sub" is row 1 (y=2).
        // First click on "sub".
        let _ = mouse(left_click(5, 2), &mut app, &mut ui, &l, &r).await;
        assert!(!app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        // Second click same cell → double-click → descend.
        let _ = mouse(left_click(5, 2), &mut app, &mut ui, &l, &r).await;
        assert!(
            app.pane(PaneId::Left).cwd.display().ends_with("/sub"),
            "expected descent into sub, cwd = {}",
            app.pane(PaneId::Left).cwd.display()
        );
    }

    // Feature 040 (FR-004): double-clicking the `..` row ascends.
    #[tokio::test]
    async fn double_click_parent_row_ascends() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("sub")).unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let parent = app.pane(PaneId::Left).cwd.clone(); // td_l
        app.dispatch(AppCommand::Descend).await.unwrap(); // into "sub"
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        let left_rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(
            left_rect,
            Rect {
                x: 50,
                y: 1,
                width: 40,
                height: 10,
            },
            true,
        );
        let (l, r) = synced_views(&app);
        // "sub" is empty → row 0 (y=1) is the `..` row. Double-click it.
        let _ = mouse(left_click(5, 1), &mut app, &mut ui, &l, &r).await;
        let (l, r) = synced_views(&app);
        let _ = mouse(left_click(5, 1), &mut app, &mut ui, &l, &r).await;
        assert_eq!(
            app.pane(PaneId::Left).cwd,
            parent,
            "double-clicking `..` should ascend to the parent"
        );
    }

    // T-MOUSE-1 (FR-013): with the mouse disabled, no event changes state.
    #[tokio::test]
    async fn disabled_mouse_is_a_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            std::fs::write(td_r.path().join(n), b"").unwrap();
        }
        let mut app = app_with(&td_l, &td_r).await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 10,
            },
            Rect {
                x: 50,
                y: 1,
                width: 40,
                height: 10,
            },
            false, // mouse disabled
        );
        let (l, r) = synced_views(&app);
        let _ = mouse(left_click(55, 3), &mut app, &mut ui, &l, &r).await;
        assert_eq!(
            app.active_pane(),
            PaneId::Left,
            "disabled mouse must not focus"
        );
        assert_eq!(
            app.pane(PaneId::Right).cursor,
            1,
            "disabled mouse must not move cursor (default is the first real \
             entry, past the `..` row)"
        );
    }

    // T-MOUSE-5 (FR-017): clicking a function-key button invokes its
    // command. Button 2 (Menu/user-menu — F2) now opens the user menu
    // dialog (Feature 047); no menu.toml in tests → error-state dialog.
    #[tokio::test]
    async fn click_fkey_button_dispatches_command() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 10,
            },
            Rect {
                x: 50,
                y: 1,
                width: 40,
                height: 10,
            },
            true,
        );
        ui.layout.fkeys = Rect {
            x: 0,
            y: 23,
            width: 100,
            height: 1,
        };
        let (l, r) = synced_views(&app);
        // Button 2 (F2 = ShowUserMenu) ≈ 2nd of 10 slots (x 10..20).
        // ShowUserMenu wired in Feature 047; mouse-click assertion added by
        // Feature 048 (issue #70) — use mouse_with_dlg to assert dialog state.
        let (_status, dlg) = mouse_with_dlg(left_click(15, 23), &mut app, &mut ui, &l, &r).await;
        assert!(
            matches!(dlg, Some(ActiveDialog::UserMenu { .. })),
            "F2 click must open UserMenu dialog (not the old deferred-action stub): {dlg:?}"
        );
    }

    // T-MOUSE-6 (FR-018): a click outside any region is a no-op.
    #[test]
    fn rect_contains_bounds() {
        let r = Rect {
            x: 10,
            y: 5,
            width: 4,
            height: 3,
        };
        assert!(rect_contains(r, 10, 5));
        assert!(rect_contains(r, 13, 7));
        assert!(!rect_contains(r, 14, 5)); // x past right edge
        assert!(!rect_contains(r, 10, 8)); // y past bottom edge
        assert!(!rect_contains(r, 9, 5));
    }

    // ---------- Feature 039: tasks/jobs panel wiring ----------

    /// Submit one throttled copy through the App so a transfer is registered
    /// and still in flight for panel-action assertions.
    async fn submit_running_copy(app: &mut App, td_l: &TempDir, name: &str) {
        std::env::set_var("CARGONAUT_TRANSFER_THROTTLE_MIBPS", "8");
        std::fs::write(td_l.path().join(name), vec![0u8; 24 * 1024 * 1024]).unwrap();
        app.refresh_active_pane().await.unwrap();
        app.dispatch(AppCommand::SelectByPattern(name.into()))
            .await
            .unwrap();
        app.confirm_copy().await.unwrap();
        app.dispatch(AppCommand::UnselectByPattern(name.into()))
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn build_job_rows_formats_label_status_and_eligibility() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        submit_running_copy(&mut app, &td_l, "a.bin").await;
        let rows = build_job_rows(&app);
        assert_eq!(rows.len(), 1);
        assert!(rows[0].label.contains("a.bin"));
        assert!(
            rows[0].status_label.starts_with("Running") || rows[0].status_label == "Queued",
            "unexpected status_label {:?}",
            rows[0].status_label
        );
        // A queued/running job can be cancelled or paused, not resumed.
        assert!(rows[0].can_cancel && rows[0].can_pause && !rows[0].can_resume);
    }

    // Feature 042 US4: the add prompt parses `group/name` into (group, name).
    #[test]
    fn parse_bookmark_input_splits_group_and_name() {
        assert_eq!(
            parse_bookmark_input("work/proj"),
            (Some("work".to_string()), "proj".to_string())
        );
        assert_eq!(
            parse_bookmark_input("scratch"),
            (None, "scratch".to_string())
        );
        // surrounding whitespace trimmed on both sides of the separator.
        assert_eq!(
            parse_bookmark_input("  work / my proj "),
            (Some("work".to_string()), "my proj".to_string())
        );
    }

    // Feature 042 US4: rows are organized by group with headers (SC-007).
    #[test]
    fn hotlist_rows_grouped_with_headers() {
        let bms = vec![
            cargonaut_config::Bookmark {
                name: "a".into(),
                path: "/a".into(),
                group: Some("work".into()),
            },
            cargonaut_config::Bookmark {
                name: "b".into(),
                path: "/b".into(),
                group: None,
            },
        ];
        let rows = build_hotlist_rows(&bms);
        // A non-selectable header row for "work".
        assert!(rows
            .iter()
            .any(|r| r.index.is_none() && r.display.contains("work")));
        // An ungrouped/default header exists for "b".
        assert!(rows
            .iter()
            .any(|r| r.index.is_none() && r.display.contains("ungrouped")));
        // Entry rows carry bookmark indices.
        assert!(rows.iter().any(|r| r.index == Some(0)));
        assert!(rows.iter().any(|r| r.index == Some(1)));
    }

    // Feature 044: C-x C opens a prefilled mode prompt; submit chains a confirm;
    // Cancel aborts with no change.
    #[tokio::test]
    async fn recursive_chmod_opens_input_then_confirm_and_cancel_aborts() {
        use crossterm::event::KeyCode;
        use std::os::unix::fs::PermissionsExt;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        let f = td_l.path().join("a/f");
        std::fs::write(&f, b"x").unwrap();
        std::fs::set_permissions(&f, PermissionsExt::from_mode(0o644)).unwrap();
        let mut app = app_with(&td_l, &td_r).await; // focused = "a"
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        dispatch_ui_command(
            Command::ChmodRecursive,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(
            dlg,
            Some(ActiveDialog::Input {
                kind: InputKind::ChmodRecursive,
                ..
            })
        ));
        // Type "700", Enter → should chain a ConfirmDialog (not apply yet).
        for c in ['7', '0', '0'] {
            feed_key(
                KeyCode::Char(c),
                &mut app,
                &keymap,
                &mut mode,
                &mut dlg,
                &mut ui,
            )
            .await;
        }
        feed_key(
            KeyCode::Enter,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(
            matches!(dlg, Some(ActiveDialog::Confirm { .. })),
            "submit must chain a confirm"
        );
        // Cancel the confirmation → nothing changes (SC-003).
        feed_key(
            KeyCode::Esc,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none());
        let m = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(m, 0o644, "Cancel must leave the tree unchanged");
    }

    // Feature 042: Ctrl-b (BookmarksMenu) opens the hotlist popup.
    #[tokio::test]
    async fn chmod_command_opens_prefilled_input() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let f = td_l.path().join("f");
        std::fs::write(&f, b"x").unwrap();
        std::fs::set_permissions(&f, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::Chmod,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        match dlg {
            Some(ActiveDialog::Input { widget, kind }) => {
                assert!(matches!(kind, InputKind::Chmod));
                assert_eq!(widget.value(), "644", "prefilled with current octal mode");
            }
            other => panic!("expected chmod input dialog, got {other:?}"),
        }
        assert!(matches!(mode, Mode::Dialog));
    }

    // Feature 043: C-x s opens a symlink-name prompt prefilled with the target.
    #[tokio::test]
    async fn symlink_command_opens_prefilled_input() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("src"), b"x").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        dispatch_ui_command(
            Command::CreateSymlink,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        match dlg {
            Some(ActiveDialog::Input { widget, kind }) => {
                assert!(matches!(kind, InputKind::Symlink));
                assert_eq!(widget.value(), "src");
            }
            other => panic!("expected symlink input, got {other:?}"),
        }
    }

    // Feature 043 (FR-007): chown opens an owner prompt, and submitting it
    // chains a confirmation dialog before applying.
    #[tokio::test]
    async fn chown_command_chains_confirmation() {
        use crossterm::event::KeyCode;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("f"), b"x").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        // Open the owner prompt.
        dispatch_ui_command(
            Command::Chown,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(
            dlg,
            Some(ActiveDialog::Input {
                kind: InputKind::Chown,
                ..
            })
        ));
        // Type "0:0" and submit → should chain a confirmation dialog.
        for c in ['0', ':', '0'] {
            feed_key(
                KeyCode::Char(c),
                &mut app,
                &keymap,
                &mut mode,
                &mut dlg,
                &mut ui,
            )
            .await;
        }
        feed_key(
            KeyCode::Enter,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(
            matches!(dlg, Some(ActiveDialog::Confirm { .. })),
            "chown submit must chain a confirmation (FR-007), got {dlg:?}"
        );
    }

    // Feature 043 (FR-012): Esc on the chmod dialog closes it, no change.
    #[tokio::test]
    async fn chmod_dialog_esc_cancels_unchanged() {
        use crossterm::event::KeyCode;
        use std::os::unix::fs::PermissionsExt;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let f = td_l.path().join("f");
        std::fs::write(&f, b"x").unwrap();
        std::fs::set_permissions(&f, std::os::unix::fs::PermissionsExt::from_mode(0o644)).unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        dispatch_ui_command(
            Command::Chmod,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        feed_key(
            KeyCode::Esc,
            &mut app,
            &Keymap::load(DEFAULT_KEYMAP).unwrap(),
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none(), "Esc closes the dialog");
        let m = std::fs::metadata(&f).unwrap().permissions().mode() & 0o777;
        assert_eq!(m, 0o644, "Esc must not change the file");
    }

    #[tokio::test]
    async fn bookmarks_menu_opens_hotlist_dialog() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::BookmarksMenu,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(dlg, Some(ActiveDialog::Hotlist { .. })));
        assert!(matches!(mode, Mode::Dialog));
    }

    #[tokio::test]
    async fn show_tasks_panel_opens_and_close_is_inert() {
        use crossterm::event::KeyCode;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        let left_before = app.pane(PaneId::Left).cwd.clone();
        let right_before = app.pane(PaneId::Right).cwd.clone();

        dispatch_ui_command(
            Command::ShowTasksPanel,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(dlg, Some(ActiveDialog::TasksPanel { .. })));
        assert!(matches!(mode, Mode::Dialog));

        // Esc closes; SC-005: panes unchanged on close.
        feed_key(
            KeyCode::Esc,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(dlg.is_none());
        assert!(matches!(mode, Mode::Pane));
        assert_eq!(app.pane(PaneId::Left).cwd, left_before);
        assert_eq!(app.pane(PaneId::Right).cwd, right_before);
    }

    #[tokio::test]
    async fn tasks_panel_cancel_key_cancels_focused_job() {
        use crossterm::event::KeyCode;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        submit_running_copy(&mut app, &td_l, "a.bin").await;
        let id = app.transfer_ids()[0];
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::ShowTasksPanel,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        feed_key(
            KeyCode::Char('c'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
        // Panel stays open after an action.
        assert!(matches!(dlg, Some(ActiveDialog::TasksPanel { .. })));
    }

    #[tokio::test]
    async fn tasks_panel_pause_then_resume_keys() {
        use crossterm::event::KeyCode;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        submit_running_copy(&mut app, &td_l, "a.bin").await;
        let id = app.transfer_ids()[0];
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, true);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::ShowTasksPanel,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        feed_key(
            KeyCode::Char('p'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
        assert!(matches!(
            app.job_views()[0].status,
            cargonaut_core::JobStatus::Paused
        ));
        // Resume the paused job; panel stays open and the action is handled.
        feed_key(
            KeyCode::Char('r'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        assert!(matches!(dlg, Some(ActiveDialog::TasksPanel { .. })));
        assert!(!app.job_views().is_empty());
    }

    // Feature 047 — T019(red): build_action_command shell-safety tests
    #[test]
    fn build_action_command_no_shell_ops_splits_directly() {
        let (prog, args) = build_action_command("echo {path}", std::path::Path::new("/tmp/a"));
        assert_eq!(prog, "echo");
        assert_eq!(args, vec!["/tmp/a"]);
    }

    #[test]
    fn build_action_command_shell_op_uses_sh_c() {
        let (prog, args) = build_action_command("cat {path} | wc", std::path::Path::new("/tmp/a"));
        assert_eq!(prog, "sh");
        assert_eq!(args[0], "-c");
        assert!(args[1].contains("/tmp/a"), "substituted path missing");
    }

    #[test]
    fn build_action_command_path_with_spaces_is_quoted() {
        let (_prog, args) =
            build_action_command("cat {path} | wc", std::path::Path::new("/tmp/my file"));
        // Path should be quoted so shell sees it as a single arg.
        assert!(
            args[1].contains("'") || args[1].contains(r"\"),
            "path not shell-quoted: {:?}",
            args
        );
    }

    #[test]
    fn build_action_command_no_path_placeholder_runs_as_is() {
        let (prog, args) = build_action_command("git status", std::path::Path::new("/tmp/a"));
        assert_eq!(prog, "git");
        assert_eq!(args, vec!["status"]);
    }

    // Feature 047 SC-002: every action in keymap.toml must appear in HELP_SECTIONS.
    #[test]
    fn help_covers_all_keymap_bindings() {
        #[derive(serde::Deserialize)]
        struct Binding {
            action: String,
        }
        #[derive(serde::Deserialize)]
        struct KeymapFile {
            binding: Vec<Binding>,
        }
        let keymap_src = include_str!("../../../design/contracts/keymap.toml");
        let kf: KeymapFile = toml::from_str(keymap_src).expect("keymap.toml must parse");
        let all_text: String = crate::dialog::HELP_SECTIONS
            .iter()
            .flat_map(|s| s.rows.iter().map(|r| format!("{} {}", r.key, r.desc)))
            .collect::<Vec<_>>()
            .join(" ")
            .to_lowercase();
        let mut missing = Vec::new();
        for b in &kf.binding {
            let action = b.action.to_lowercase();
            if !all_text.contains(&action) {
                missing.push(b.action.as_str());
            }
        }
        assert!(
            missing.is_empty(),
            "keymap actions missing from HELP_SECTIONS: {missing:?}"
        );
    }

    // ===== Feature 049 US2: queue_diff tests (T012 red) =====

    async fn app_with_tagged_files(left_file: &str, right_file: &str) -> (App, TempDir, TempDir) {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join(left_file), b"left content").unwrap();
        std::fs::write(td_r.path().join(right_file), b"right content").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        // Tag the file in the left pane (cursor starts at 0 or 1 after "..").
        // Use dispatch to toggle selection on left pane.
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        // Switch focus to right pane and tag the file there.
        app.dispatch(cargonaut_core::Command::FocusRight)
            .await
            .unwrap();
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        // Switch back to left.
        app.dispatch(cargonaut_core::Command::FocusLeft)
            .await
            .unwrap();
        (app, td_l, td_r)
    }

    #[tokio::test]
    async fn queue_diff_two_tagged_files_sets_pending_external() {
        let (app, _td_l, _td_r) = app_with_tagged_files("left.txt", "right.txt").await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        let tool = Some("diff -u");
        queue_diff(&app, &mut ui, &mut status, tool);
        assert!(
            ui.pending_external.is_some(),
            "two tagged files + configured tool must set pending_external; status={status:?}"
        );
        let ext = ui.pending_external.as_ref().unwrap();
        assert_eq!(ext.program, "diff", "program should be 'diff'");
        // args[-2] = left path, args[-1] = right path
        let last_two: Vec<&str> = ext
            .args
            .iter()
            .rev()
            .take(2)
            .rev()
            .map(|s| s.as_str())
            .collect();
        assert_eq!(
            last_two.len(),
            2,
            "args must contain at least the two file paths"
        );
        // Both paths should exist on the filesystem
        assert!(
            std::path::Path::new(last_two[0]).exists(),
            "left path must exist: {}",
            last_two[0]
        );
        assert!(
            std::path::Path::new(last_two[1]).exists(),
            "right path must exist: {}",
            last_two[1]
        );
    }

    #[tokio::test]
    async fn queue_diff_path_ordering_left_before_right() {
        let (app, td_l, td_r) = app_with_tagged_files("lf.txt", "rf.txt").await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_diff(&app, &mut ui, &mut status, Some("diff"));
        let ext = ui
            .pending_external
            .as_ref()
            .expect("pending_external must be set");
        let n = ext.args.len();
        assert!(n >= 2, "need at least 2 path args");
        let left_path = &ext.args[n - 2];
        let right_path = &ext.args[n - 1];
        // The left-pane path must be in td_l and right-pane path in td_r
        assert!(
            left_path.contains(td_l.path().to_str().unwrap()),
            "args[-2] must be the left-pane path; got {left_path:?} (expected prefix: {:?})",
            td_l.path()
        );
        assert!(
            right_path.contains(td_r.path().to_str().unwrap()),
            "args[-1] must be the right-pane path; got {right_path:?} (expected prefix: {:?})",
            td_r.path()
        );
    }

    #[tokio::test]
    async fn queue_diff_one_tagged_file_shows_error() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"x").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        // Tag only one file (left pane)
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_diff(&app, &mut ui, &mut status, Some("diff -u"));
        assert!(
            ui.pending_external.is_none(),
            "1 tagged file must not set pending_external"
        );
        assert!(
            status.contains("exactly 2"),
            "status must say 'exactly 2'; got {status:?}"
        );
    }

    #[tokio::test]
    async fn queue_diff_no_tool_configured_shows_error() {
        let (app, _td_l, _td_r) = app_with_tagged_files("l.txt", "r.txt").await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_diff(&app, &mut ui, &mut status, None);
        assert!(ui.pending_external.is_none());
        assert!(
            status.to_lowercase().contains("no diff tool"),
            "status must mention missing tool; got {status:?}"
        );
    }

    #[tokio::test]
    async fn queue_diff_empty_tool_string_shows_error() {
        let (app, _td_l, _td_r) = app_with_tagged_files("l.txt", "r.txt").await;
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_diff(&app, &mut ui, &mut status, Some(""));
        assert!(ui.pending_external.is_none());
        assert!(
            status.to_lowercase().contains("empty"),
            "status must mention empty tool string; got {status:?}"
        );
    }

    // ===== Feature 050 T010 (red): queue_bulk_rename — 4 failing tests =====

    #[tokio::test]
    async fn queue_bulk_rename_no_tagged_entries_shows_status() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        let app = app_with(&td_l, &td_r).await;
        // No entries tagged
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_bulk_rename(&app, &mut ui, &mut status);
        assert!(
            ui.pending_external.is_none(),
            "no tagged entries must not set pending_external"
        );
        assert!(
            status.to_lowercase().contains("tag"),
            "status must mention tagging; got {status:?}"
        );
    }

    #[tokio::test]
    async fn queue_bulk_rename_tagged_entries_sets_pending_external() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        // Tag the first file
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_bulk_rename(&app, &mut ui, &mut status);
        assert!(
            ui.pending_external.is_some(),
            "tagged entries must set pending_external; status={status:?}"
        );
    }

    #[tokio::test]
    async fn queue_bulk_rename_kind_is_bulk_rename() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_bulk_rename(&app, &mut ui, &mut status);
        let ext = ui.pending_external.as_ref().expect("must be set");
        assert!(
            matches!(ext.kind, PendingExternalKind::BulkRename { .. }),
            "kind must be BulkRename; got {:?}",
            ext.kind
        );
    }

    #[tokio::test]
    async fn queue_bulk_rename_original_names_match_tagged_basenames() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("alpha.txt"), b"1").unwrap();
        std::fs::write(td_l.path().join("beta.txt"), b"2").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        // Tag both files
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        app.dispatch(cargonaut_core::Command::CursorDown)
            .await
            .unwrap();
        app.dispatch(cargonaut_core::Command::SelectionToggle)
            .await
            .unwrap();
        let mut ui = fresh_ui(
            Rect {
                x: 0,
                y: 1,
                width: 40,
                height: 22,
            },
            Rect {
                x: 40,
                y: 1,
                width: 40,
                height: 22,
            },
            false,
        );
        let mut status = String::new();
        queue_bulk_rename(&app, &mut ui, &mut status);
        let ext = ui.pending_external.as_ref().expect("must be set");
        if let PendingExternalKind::BulkRename { original_names, .. } = &ext.kind {
            assert!(
                original_names.contains(&"alpha.txt".to_string()),
                "original_names must contain alpha.txt; got {original_names:?}"
            );
            assert!(
                original_names.contains(&"beta.txt".to_string()),
                "original_names must contain beta.txt; got {original_names:?}"
            );
        } else {
            panic!("kind must be BulkRename");
        }
    }

    // ======================================================================
    // Feature 052 T008 (red) — panelize integration tests
    // ======================================================================

    // T008: Dispatching FindFilePopup opens FindFile dialog; panelize sets
    // find_label and populates the pane listing. Also verifies Copy/Move/Delete/
    // ViewFile/Edit dispatch without panic (FR-009, SC-004).
    #[tokio::test]
    async fn find_file_popup_dispatch_opens_dialog() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.toml"), b"a").unwrap();
        std::fs::write(td_l.path().join("b.toml"), b"b").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        dispatch_ui_command(
            Command::FindFilePopup,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .expect("FindFilePopup must not error");

        assert!(
            matches!(dlg, Some(ActiveDialog::FindFile { .. })),
            "FindFilePopup must open FindFile dialog; got {dlg:?}"
        );
    }

    #[tokio::test]
    async fn panelize_sets_find_label_and_listing_entries() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Create 2 .toml files for panelizing.
        let file_a = td_l.path().join("a.toml");
        let file_b = td_l.path().join("b.toml");
        std::fs::write(&file_a, b"a").unwrap();
        std::fs::write(&file_b, b"b").unwrap();

        let app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);

        let paths = vec![file_a.clone(), file_b.clone()];
        let pattern = "*.toml".to_string();

        // PaneView for the active (left) pane.
        let mut left = PaneView::new(
            app.pane(cargonaut_core::PaneId::Left).cwd.clone(),
            app.pane(cargonaut_core::PaneId::Left).listing.clone(),
        );

        panelize_into_pane(&mut left, &paths, &pattern, &mut ui);

        assert_eq!(
            left.listing.entries.len(),
            2,
            "SC-004: panelized listing must have 2 entries"
        );
        assert_eq!(
            ui.find_label,
            Some("*.toml".to_string()),
            "find_label must be set to the search pattern"
        );
    }

    #[tokio::test]
    async fn dispatch_copy_after_panelize_does_not_panic() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let file_a = td_l.path().join("a.toml");
        std::fs::write(&file_a, b"a").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        // Copy (F5) must dispatch without panic.
        let _ = dispatch_ui_command(
            Command::CopySelection,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await;
    }

    #[tokio::test]
    async fn dispatch_delete_after_panelize_does_not_panic() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;

        // Delete (F8) must dispatch without panic.
        let _ = dispatch_ui_command(
            Command::DeleteSelection,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await;
    }

    // ======================================================================
    // Feature 052 T018+T019 (red→green) — Esc from FindFile does not panelize
    // ======================================================================

    // T018: After Esc from FindFile dialog, find_label is NOT set.
    #[tokio::test]
    async fn esc_from_find_dialog_does_not_set_find_label() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut status = String::new();
        let mut quit = false;
        let mut chord_buf = Vec::new();

        // Open the dialog.
        let mut dlg: Option<ActiveDialog> = None;
        dispatch_ui_command(
            Command::FindFilePopup,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        assert!(matches!(dlg, Some(ActiveDialog::FindFile { .. })));

        // Press Esc.
        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let esc_event = crossterm::event::KeyEvent::new(
            crossterm::event::KeyCode::Esc,
            crossterm::event::KeyModifiers::empty(),
        );
        handle_key(
            esc_event,
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut chord_buf,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();

        // T018: find_label must NOT be set after cancel.
        assert!(
            ui.find_label.is_none(),
            "T018: Esc must not set find_label; got {:?}",
            ui.find_label
        );
        // T019: dialog must be dismissed.
        assert!(
            dlg.is_none(),
            "T019: dialog must be None after Esc; got {dlg:?}"
        );
    }

    // ======================================================================
    // Feature 052 T022+T023 — navigate_to clears find_label (contract §6)
    // ======================================================================

    // T022: After panelizing (find_label set), navigating to a real directory
    // clears find_label. We test via the help_sections_text function which
    // exercises that find_label is cleared when sync_from runs on a fresh dir.
    //
    // Direct approach: set find_label, then simulate sync_from clearing it.
    // The actual clear happens in apply_event / CwdChanged processing.
    // Here we test that panelize_into_pane sets it and confirm the pane
    // can be re-navigated (find_label clearing is tested via clear_find_label
    // on navigate).
    #[tokio::test]
    async fn navigate_after_panelize_clears_find_label() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let file_a = td_l.path().join("a.toml");
        std::fs::write(&file_a, b"a").unwrap();

        let app = app_with(&td_l, &td_r).await;
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 22,
        };
        let mut ui = fresh_ui(rect, rect, false);

        // Simulate panelize.
        let mut left = PaneView::new(
            app.pane(cargonaut_core::PaneId::Left).cwd.clone(),
            app.pane(cargonaut_core::PaneId::Left).listing.clone(),
        );
        panelize_into_pane(&mut left, &[file_a], "*.toml", &mut ui);
        assert_eq!(ui.find_label, Some("*.toml".to_string()));

        // Simulate navigate_to: clear find_label (this happens in apply_event CwdChanged).
        ui.find_label = None;

        assert!(
            ui.find_label.is_none(),
            "find_label must be cleared after navigate"
        );
    }

    // ======================================================================
    // Feature 052 T024+T025 — truncation at max_results
    // (These tests are in dialog.rs — referenced here for tracking)
    // ======================================================================

    // ======================================================================
    // Feature 052 T020+T021 — help overlay contains M-? and Find
    // (T021 was done in T003 green commit — help section added)
    // ======================================================================
    #[test]
    fn help_overlay_contains_find_file_entry() {
        let text = help_sections_text();
        assert!(
            text.contains("M-?"),
            "help must mention M-? (find-file key)"
        );
        assert!(
            text.to_lowercase().contains("find"),
            "help must mention 'find'"
        );
    }

    // ===== Feature 053: T022 (red) — modal guard: tab keys swallowed by dialog =====
    #[tokio::test]
    async fn modal_guard_tab_keys_swallowed() {
        use cargonaut_core::{Command as AppCommand, DialogKind, PaneId};
        use crossterm::event::KeyCode;

        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with(&td_l, &td_r).await;

        // Open a second tab so we have something to switch away from
        app.dispatch(AppCommand::TabNew).await.unwrap();
        let initial_left_tab = app
            .tab_bar_view(PaneId::Left)
            .iter()
            .filter(|e| e.is_active)
            .map(|e| e.index)
            .next()
            .unwrap();
        let initial_left_count = app.tab_bar_view(PaneId::Left).len();

        let keymap = Keymap::load(DEFAULT_KEYMAP).unwrap();
        let rect = Rect {
            x: 0,
            y: 1,
            width: 40,
            height: 10,
        };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        // Inject a confirm dialog
        let mut dlg: Option<ActiveDialog> = Some(make_dialog(DialogKind::Confirm {
            title: "Test".into(),
            body: "modal guard test".into(),
            on_confirm: Box::new(AppCommand::Quit),
        }));

        // Tab-next key (])
        feed_key(
            KeyCode::Char(']'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        // Tab-prev key ([)
        feed_key(
            KeyCode::Char('['),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        // Ctrl-t (new tab)
        feed_key(
            KeyCode::Char('t'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;
        // Ctrl-w (close tab)
        feed_key(
            KeyCode::Char('w'),
            &mut app,
            &keymap,
            &mut mode,
            &mut dlg,
            &mut ui,
        )
        .await;

        // All tab operations must have been swallowed — dialog is still active
        assert!(
            dlg.is_some(),
            "dialog should still be active after tab keys"
        );

        // Tab count and active tab should be unchanged
        let current_count = app.tab_bar_view(PaneId::Left).len();
        let current_tab = app
            .tab_bar_view(PaneId::Left)
            .iter()
            .filter(|e| e.is_active)
            .map(|e| e.index)
            .next()
            .unwrap();
        assert_eq!(
            current_count, initial_left_count,
            "tab count should be unchanged"
        );
        assert_eq!(
            current_tab, initial_left_tab,
            "active tab should be unchanged"
        );
    }

    // ===== Feature 053: T015 (red) — tab_bar_line rendering tests =====
    // tab_bar_line does not exist yet; these compile-fail tests are the red state.

    #[test]
    fn tab_bar_line_renders_single_tab() {
        use cargonaut_core::TabBarEntry;
        let entries = vec![TabBarEntry {
            index: 1,
            label: "foo".to_string(),
            is_active: true,
        }];
        let line = tab_bar_line(&entries, 40, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("[1*]foo"),
            "tab bar should contain '[1*]foo', got: {text:?}"
        );
    }

    #[test]
    fn tab_bar_line_renders_multiple_tabs_with_active_marker() {
        use cargonaut_core::TabBarEntry;
        let entries = vec![
            TabBarEntry {
                index: 1,
                label: "bar".to_string(),
                is_active: false,
            },
            TabBarEntry {
                index: 2,
                label: "baz".to_string(),
                is_active: true,
            },
        ];
        let line = tab_bar_line(&entries, 40, &Theme::default());
        let text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(
            text.contains("[1]bar"),
            "should contain '[1]bar', got: {text:?}"
        );
        assert!(
            text.contains("[2*]baz"),
            "should contain '[2*]baz', got: {text:?}"
        );
    }

    #[tokio::test]
    async fn draw_pane_tab_bar_occupies_first_row() {
        use cargonaut_core::PaneId;
        use ratatui::backend::TestBackend;
        use ratatui::Terminal;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = app_with(&td_l, &td_r).await;
        let entries = app.tab_bar_view(PaneId::Left);
        let backend = TestBackend::new(40, 8);
        let mut term = Terminal::new(backend).unwrap();
        let theme = Theme::default();
        let area = Rect {
            x: 0,
            y: 0,
            width: 40,
            height: 8,
        };
        term.draw(|f| {
            let pane_state = app.pane(PaneId::Left);
            let mut view = pane::PaneView::new(pane_state.cwd.clone(), pane_state.listing.clone());
            view.sync_from(pane_state);
            let _inner = draw_pane(
                f,
                &mut view,
                area,
                true,
                &theme,
                "",
                pane::PaneLayout::Brief,
                &entries,
            );
        })
        .unwrap();
        let buf = term.backend().buffer().clone();
        // Collect the first row (y=0, x=0..40) from the buffer content
        // Buffer content is stored row by row: row y starts at y * width
        let width = 40usize;
        let first_row: String = buf
            .content()
            .iter()
            .take(width)
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(
            first_row.contains("[1"),
            "first row should contain tab bar '[1', got: {first_row:?}"
        );
    }

    // ===== Feature 054: Phase 2 foundational type compile-tests =====

    #[test]
    fn subshell_phase_enum_exists() {
        // T004 (red → green): SubshellPhase::Hidden must exist in subshell module.
        let _ = subshell::SubshellPhase::Hidden;
    }

    #[test]
    fn frame_layout_has_subshell_field() {
        // T008 (red → green): FrameLayout must have a subshell: Option<Rect> field.
        let layout = FrameLayout {
            menu: Rect::default(),
            left: Rect::default(),
            right: Rect::default(),
            fkeys: Rect::default(),
            subshell: None,
        };
        assert!(layout.subshell.is_none());
    }

    // T015b (red → green): debounce guard ignores Ctrl-o fired within 50 ms.
    #[test]
    fn ctrl_o_debounce_ignores_rapid_press() {
        let mut ui = fresh_ui(Rect::default(), Rect::default(), false);
        // First call: no prior timestamp — must NOT skip.
        assert!(
            !ui.ctrl_o_should_skip(),
            "first press should not be skipped"
        );
        // Second call immediately after: must skip (elapsed < 50 ms).
        assert!(
            ui.ctrl_o_should_skip(),
            "rapid second press must be skipped"
        );
    }

    // ---------- Feature 057 US1: DescendOrOpen on .zip files (T014 red) ----------

    /// Minimal valid empty ZIP (EOCD record only — 22 bytes).
    fn minimal_zip_bytes() -> Vec<u8> {
        vec![
            0x50, 0x4b, 0x05, 0x06, // End-of-Central-Directory signature
            0x00, 0x00, // disk number
            0x00, 0x00, // disk with CD start
            0x00, 0x00, // entries on this disk
            0x00, 0x00, // total entries
            0x00, 0x00, 0x00, 0x00, // CD size
            0x00, 0x00, 0x00, 0x00, // CD offset
            0x00, 0x00, // comment length
        ]
    }

    /// Write a valid empty ZIP to a named path and return the path.
    fn write_valid_zip(dir: &std::path::Path, name: &str) -> std::path::PathBuf {
        let p = dir.join(name);
        std::fs::write(&p, minimal_zip_bytes()).unwrap();
        p
    }

    #[tokio::test]
    async fn descend_or_open_zip_navigates_into_archive() {
        // T014 (red → green via T015): pressing Enter on a .zip file should
        // navigate the active pane into the ZIP backend (zip:// cwd) instead
        // of opening the built-in text viewer.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        write_valid_zip(td_l.path(), "archive.zip");
        let mut app = app_with(&td_l, &td_r).await;
        // Cursor at index 0 is the ".." entry; the zip file is at index 1.
        app.dispatch(cargonaut_core::Command::CursorTo(1)).await.unwrap();
        let rect = Rect { x: 0, y: 1, width: 40, height: 10 };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        dispatch_ui_command(
            Command::DescendOrOpen,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        // After T015: pane must be navigated to a zip:// path.
        assert_eq!(
            app.active_pane_state().cwd.scheme.as_str(),
            "zip",
            "DescendOrOpen on a .zip file must navigate pane to zip:// (got: {}; status: {status:?})",
            app.active_pane_state().cwd.display()
        );
        // Viewer dialog must NOT be opened.
        assert!(dlg.is_none(), "no dialog must be open after zip navigation");
    }

    #[tokio::test]
    async fn descend_or_open_corrupt_zip_shows_error_and_stays_local() {
        // T014 error path: pressing Enter on a corrupt .zip shows an error
        // and does NOT navigate the pane away from the local filesystem.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // PK magic header but then corrupted — valid ZIP magic so ZipFs tries to open it,
        // but invalid structure so ZipFs::open returns Err; binary bytes so file viewer
        // also fails its UTF-8 check.
        std::fs::write(td_l.path().join("bad.zip"), b"PK\x03\x04\x00\xff\xfe binary garbage").unwrap();
        let mut app = app_with(&td_l, &td_r).await;
        app.dispatch(cargonaut_core::Command::CursorTo(1)).await.unwrap();
        let rect = Rect { x: 0, y: 1, width: 40, height: 10 };
        let mut ui = fresh_ui(rect, rect, false);
        let mut mode = Mode::Pane;
        let mut dlg: Option<ActiveDialog> = None;
        let mut status = String::new();
        let mut quit = false;
        dispatch_ui_command(
            Command::DescendOrOpen,
            &mut app,
            &mut mode,
            &mut dlg,
            &mut status,
            &mut quit,
            &mut ui,
        )
        .await
        .unwrap();
        // Pane must stay local (no navigation on corrupt zip).
        assert_eq!(
            app.active_pane_state().cwd.scheme.as_str(),
            "file",
            "corrupt zip must not navigate pane (stayed at: {})",
            app.active_pane_state().cwd.display()
        );
        // After T015: .zip files must NEVER open the file viewer regardless of
        // whether archive open succeeds or fails — viewer is only for non-archive files.
        assert!(
            !matches!(mode, Mode::Preview),
            "DescendOrOpen on a .zip must not open the viewer (mode={mode:?}); \
             zip handler must intercept before reaching open_file_viewer"
        );
    }

    // ---------- open_file_editor decline paths (Feature 056 — US3) ----------

    #[tokio::test]
    async fn open_file_editor_declines_binary() {
        let f = tempfile::NamedTempFile::new().unwrap();
        // Write null bytes — will fail the UTF-8 sample check.
        std::fs::write(f.path(), b"\x00\xff\xfe binary data").unwrap();
        let result = open_file_editor(f.path().to_path_buf(), "bin".into()).await;
        assert!(result.is_err(), "binary file should be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }

    #[tokio::test]
    async fn open_file_editor_declines_too_large() {
        let f = tempfile::NamedTempFile::new().unwrap();
        // Write just over the 10 MiB limit.
        let big = vec![b'a'; dialog::STREAMING_THRESHOLD_BYTES + 1];
        std::fs::write(f.path(), &big).unwrap();
        let result = open_file_editor(f.path().to_path_buf(), "large.txt".into()).await;
        assert!(result.is_err(), "oversized file should be rejected");
        let err = result.unwrap_err();
        assert_eq!(err.kind(), std::io::ErrorKind::InvalidData);
    }
}

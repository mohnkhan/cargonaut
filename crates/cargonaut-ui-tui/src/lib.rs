// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut TUI layer — ratatui rendering, keymap dispatcher,
//! pane/dialog/status-bar widgets, and the [`run`] event loop.

#![warn(missing_docs)]

pub mod chrome;
pub mod dialog;
pub mod keymap;
pub mod pane;
pub mod theme;
pub use chrome::{FunctionKeyBar, MenuBar};
pub use dialog::{
    ConfirmDialog, ConfirmOutcome, InputOutcome, JobRow, PathInputAction, PathInputDialog,
    ResumableSummary, ResumeChoice, ResumePromptDialog, TasksAction, TasksPanelDialog,
    TextInputDialog,
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

    // Teardown — always best-effort, even on error from the loop.
    let _ = execute!(std::io::stdout(), DisableMouseCapture);
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = term.show_cursor();

    result
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
}

/// Loop-owned chrome + mouse state (kept in one struct to avoid a
/// double-digit argument list on the key/mouse handlers).
struct UiState {
    menu: MenuBar,
    fkeybar: FunctionKeyBar,
    layout: FrameLayout,
    last_click: Option<(u16, u16, Instant)>,
    help_open: bool,
    mouse_enabled: bool,
    /// Set by F3/F4; run_loop suspends the TUI, runs it, and restores.
    pending_external: Option<PendingExternal>,
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
}

/// What a [`TextInputDialog`]'s submitted text becomes.
#[derive(Debug, Clone, Copy)]
enum InputKind {
    Mkdir,
    SelectPattern,
    UnselectPattern,
}

/// An external program to run (F3/F4), suspending the TUI around it.
#[derive(Debug, Clone)]
struct PendingExternal {
    /// Resolved program (`$PAGER`/`$EDITOR` + fallbacks).
    program: String,
    /// Local filesystem path argument.
    path: String,
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
        help_open: false,
        mouse_enabled,
        pending_external: None,
    };

    // US1 (FR-001/005/006): resolve the configured theme once. An unknown
    // name falls back to the built-in default with a non-fatal notice.
    let theme_name = app.config().ui.theme.clone();
    let theme = Theme::resolve(&theme_name);
    let mut status: String = if Theme::builtin(&theme_name).is_none() {
        format!("Unknown theme {theme_name:?} — using {}", theme.name)
    } else {
        String::new()
    };

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
        let mut layout = FrameLayout::default();
        let menu = &mut ui.menu;
        let fkeybar = &ui.fkeybar;
        let help_open = ui.help_open;
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
                help_open,
                view_mode,
                &qv_preview,
                progress.as_deref(),
            );
        })
        .map_err(Error::Terminal)?;
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
                    Some(Ok(CtEvent::Resize(_, _))) => {
                        // Loop iter will re-render.
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
            }
        }

        // US5 (FR-030/031): an F3/F4 request suspends the TUI, runs the
        // external pager/editor, then restores the terminal + refreshes.
        if let Some(ext) = ui.pending_external.take() {
            run_external(term, &ext, ui.mouse_enabled)?;
            let _ = app
                .refresh_active_pane()
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
            status = format!("Returned from {}", ext.program);
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

    // Help overlay swallows the next key (any key dismisses it).
    if ui.help_open {
        ui.help_open = false;
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
                            let core = match kind {
                                InputKind::Mkdir => AppCommand::Mkdir(text),
                                InputKind::SelectPattern => AppCommand::SelectByPattern(text),
                                InputKind::UnselectPattern => AppCommand::UnselectByPattern(text),
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
        }
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
            ui.help_open = true;
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
        // US5 (FR-030/031): F3/F4 shell out to $PAGER / $EDITOR.
        Command::Preview => {
            queue_external(app, ui, status, ExternalTool::Pager);
            return Ok(());
        }
        Command::Edit => {
            queue_external(app, ui, status, ExternalTool::Editor);
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
            MouseToggleOutcome::SuspendedNow => "Mouse capture: suspended — Shift+drag to select text",
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

/// Which external tool F3/F4 launch.
#[derive(Debug, Clone, Copy)]
enum ExternalTool {
    Pager,
    Editor,
}

/// US5 (FR-030/031): resolve the external tool + focused file and queue
/// it for run_loop to execute (suspending the TUI). No-op with a status
/// message if nothing suitable is focused.
fn queue_external(app: &App, ui: &mut UiState, status: &mut String, tool: ExternalTool) {
    let p = app.active_pane_state();
    let Some(idx) = p.focused_entry_index() else {
        *status = "Nothing to open".into();
        return;
    };
    let Some(e) = p.listing.entries.get(idx) else {
        return;
    };
    if matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir) {
        *status = format!("{} is a directory", e.name);
        return;
    }
    let path = p.cwd.join(e.name.as_str());
    let disp = path.display();
    let local = disp.strip_prefix("file://").unwrap_or(&disp).to_string();
    let program = match tool {
        ExternalTool::Pager => std::env::var("PAGER").unwrap_or_else(|_| "less".into()),
        ExternalTool::Editor => std::env::var("EDITOR").unwrap_or_else(|_| "vi".into()),
    };
    ui.pending_external = Some(PendingExternal {
        program,
        path: local,
    });
}

/// Suspend the TUI, run an external program on a file, then restore the
/// terminal (FR-030/031). Uses `Command::new(prog).arg(path)` — no shell —
/// per the constitution's macro-safety rule.
fn run_external<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    ext: &PendingExternal,
    mouse_enabled: bool,
) -> Result<(), Error> {
    let _ = disable_raw_mode();
    let _ = execute!(stdout(), LeaveAlternateScreen, DisableMouseCapture);
    let _ = std::process::Command::new(&ext.program)
        .arg(&ext.path)
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
            let _ = app
                .dispatch(AppCommand::CursorDown)
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        MouseEventKind::ScrollUp => {
            let _ = app
                .dispatch(AppCommand::CursorUp)
                .await
                .map_err(|e| Error::Other(e.to_string()))?;
        }
        MouseEventKind::Down(MouseButton::Left) => {
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
        _ => return None,
    })
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
    help_open: bool,
    view_mode: cargonaut_core::ViewMode,
    qv_preview: &str,
    progress: Option<&str>,
) -> FrameLayout {
    use cargonaut_core::ViewMode;
    use ratatui::widgets::Widget;
    let area = f.size();
    // US2 layout: [menu bar | panes | status | fkey bar].
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(1), // menu bar
            Constraint::Min(3),    // panes
            Constraint::Length(1), // status
            Constraint::Length(1), // function-key bar
        ])
        .split(area);
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
        )
    };

    // US1 (FR-002): status bar themed instead of bare reverse-video.
    let status_text = format!(" [{mode:?}]  {status}");
    Paragraph::new(status_text)
        .style(theme.status_style())
        .render(main_chunks[2], f.buffer_mut());

    // US2: function-key bar (bottom) + menu bar (top, may drop down over panes).
    fkeybar.render(main_chunks[3], f.buffer_mut(), theme);
    menu.render(main_chunks[0], f.buffer_mut(), theme);

    // US5 (FR-026): transfer progress overlay while a copy/move runs.
    if let Some(p) = progress {
        draw_progress(f, theme, area, p);
    }

    if help_open {
        draw_help(f, theme, area);
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
        }
    }

    FrameLayout {
        menu: main_chunks[0],
        left: left_inner,
        right: right_inner,
        fkeys: main_chunks[3],
    }
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
) -> Rect {
    use ratatui::widgets::Widget;
    // Split the column into the list (with border) + a 1-row mini-status.
    let col = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(2), Constraint::Length(1)])
        .split(area);
    let title = view.cwd.display();
    // US1 (FR-002): panel background + focus-colored border from the theme.
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(theme.border_style(focused))
        .style(Style::default().bg(theme.panel_bg).fg(theme.panel_fg));
    let inner = block.inner(col[0]);
    block.render(col[0], f.buffer_mut());
    view.render(inner, f.buffer_mut(), theme, layout);
    // US2 (FR-010): per-pane mini-status line.
    Paragraph::new(format!(" {mini_status}"))
        .style(theme.status_style())
        .render(col[1], f.buffer_mut());
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

/// Minimal help overlay (F1). The full hypertext help viewer is deferred.
fn draw_help(f: &mut ratatui::Frame, theme: &Theme, area: Rect) {
    use ratatui::widgets::{Clear, Widget};
    let r = centered_rect(60, 50, area);
    Clear.render(r, f.buffer_mut());
    let body = "\
Cargonaut — quick help\n\
\n\
  Arrows / j k     move cursor      Tab        switch pane\n\
  Enter            enter directory  Insert     tag file\n\
  F5 Copy  F6 Move  F8 Delete  F7 Mkdir*  F9 Menu  F10 Quit\n\
  F3 View*  F4 Edit*   (* not yet available)\n\
  Mouse: click to focus/move, double-click to enter, wheel to scroll\n\
\n\
  Press any key to close.";
    let block = Block::default()
        .title("Help")
        .borders(Borders::ALL)
        .style(theme.dialog_style());
    Paragraph::new(body)
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
            },
            last_click: None,
            help_open: false,
            mouse_enabled: mouse,
            pending_external: None,
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
        assert_eq!(plan_mouse_toggle(false, false), MouseToggleOutcome::Disabled);
        assert_eq!(plan_mouse_toggle(false, true), MouseToggleOutcome::Disabled);
        assert_eq!(plan_mouse_toggle(true, false), MouseToggleOutcome::EnabledNow);
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
        assert!(MouseToggleOutcome::SuspendedNow.status().contains("suspended"));
        assert!(MouseToggleOutcome::SuspendedNow.status().contains("Shift"));
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
        // First click on row 0 (the only entry, "sub").
        let _ = mouse(left_click(5, 1), &mut app, &mut ui, &l, &r).await;
        assert!(!app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        // Second click same cell → double-click → descend.
        let _ = mouse(left_click(5, 1), &mut app, &mut ui, &l, &r).await;
        assert!(
            app.pane(PaneId::Left).cwd.display().ends_with("/sub"),
            "expected descent into sub, cwd = {}",
            app.pane(PaneId::Left).cwd.display()
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
            0,
            "disabled mouse must not move cursor"
        );
    }

    // T-MOUSE-5 (FR-017): clicking a function-key button invokes its
    // command. Button 2 (Menu/user-menu) is deferred → status message.
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
        // Button 2 (Menu = user menu) ≈ 2nd of 10 slots (x 10..20) → deferred.
        let status = mouse(left_click(15, 23), &mut app, &mut ui, &l, &r).await;
        assert!(
            status.contains("not yet available"),
            "expected deferred-action notice, got {status:?}"
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
}

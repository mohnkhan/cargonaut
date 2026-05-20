// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut TUI layer — ratatui rendering, keymap dispatcher,
//! pane/dialog/status-bar widgets, and the [`run`] event loop.

#![warn(missing_docs)]

pub mod dialog;
pub mod keymap;
pub mod pane;
pub use dialog::{
    ConfirmDialog, ConfirmOutcome, ResumableSummary, ResumeChoice, ResumePromptDialog,
};
pub use keymap::{
    parse_key_chord, parse_key_sequence, Command, KeyChord, KeySequence, Keymap, KeymapError, Mode,
    SeqLookup,
};
pub use pane::PaneView;

use cargonaut_core::{App, Command as AppCommand, DialogKind, Event as AppEvent, PaneId};
use crossterm::event::{Event as CtEvent, EventStream, KeyEvent};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use futures::StreamExt;
use ratatui::backend::CrosstermBackend;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, Borders, Paragraph};
use ratatui::Terminal;
use std::io::stdout;

/// Default keymap (the bundled `design/contracts/keymap.toml`), embedded
/// at compile time so the binary doesn't need a runtime file lookup.
const DEFAULT_KEYMAP: &str = include_str!("../../../design/contracts/keymap.toml");

/// Run the TUI event loop. Enters raw mode + alternate screen, drives
/// `tokio::select!` between key events / SIGINT / a periodic redraw
/// tick, dispatches commands into the `App`, manages modal-dialog
/// state, and restores the terminal on exit (best-effort even on panic
/// — wrapped in a teardown that always runs).
pub async fn run(app: &mut App) -> Result<(), Error> {
    enable_raw_mode().map_err(Error::Terminal)?;
    let mut out = stdout();
    execute!(out, EnterAlternateScreen).map_err(Error::Terminal)?;
    let backend = CrosstermBackend::new(out);
    let mut term = Terminal::new(backend).map_err(Error::Terminal)?;

    let result = run_loop(&mut term, app).await;

    // Teardown — always best-effort, even on error from the loop.
    let _ = disable_raw_mode();
    let _ = execute!(std::io::stdout(), LeaveAlternateScreen);
    let _ = term.show_cursor();

    result
}

#[derive(Debug)]
enum ActiveDialog {
    Confirm {
        widget: ConfirmDialog,
        on_confirm: AppCommand,
    },
    #[allow(dead_code)]
    Resume(ResumePromptDialog),
}

async fn run_loop<B: ratatui::backend::Backend>(
    term: &mut Terminal<B>,
    app: &mut App,
) -> Result<(), Error> {
    let keymap = Keymap::load(DEFAULT_KEYMAP).expect("bundled keymap.toml must parse");
    let mut events = EventStream::new();

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
    let mut status: String = String::new();
    let mut quit = false;

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
        let dialog_ref = active_dialog.as_mut();
        term.draw(|f| {
            draw_frame(
                f,
                &mut left,
                &mut right,
                active,
                mode,
                &status_line,
                dialog_ref,
            );
        })
        .map_err(Error::Terminal)?;

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
                        ).await?;
                        if !cont { return Ok(()); }
                    }
                    Some(Ok(CtEvent::Resize(_, _))) => {
                        // Loop iter will re-render.
                    }
                    Some(Ok(_)) => {} // mouse / focus events — ignored for Phase 1
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
) -> Result<bool, Error> {
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
                if let Some((_idx, _choice)) = widget.handle_key(key.code) {
                    // T1.14/T1.15: actually dispatch the resume here.
                    // For Phase 1 MVP we just dismiss after the first answer.
                    *active_dialog = None;
                    *mode = Mode::Pane;
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
            if let Some(core_cmd) = ui_command_to_core(cmd) {
                let events = app
                    .dispatch(core_cmd)
                    .await
                    .map_err(|e| Error::Other(e.to_string()))?;
                for ev in events {
                    apply_event(ev, app, mode, active_dialog, status, quit);
                }
            } else {
                *status = format!("Unbound: {cmd:?}");
            }
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
        _ => return None,
    })
}

fn draw_frame(
    f: &mut ratatui::Frame,
    left: &mut PaneView,
    right: &mut PaneView,
    active: PaneId,
    mode: Mode,
    status: &str,
    dialog: Option<&mut ActiveDialog>,
) {
    let area = f.size();
    let main_chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(3), Constraint::Length(1)])
        .split(area);
    let pane_chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(main_chunks[0]);

    draw_pane(f, left, pane_chunks[0], active == PaneId::Left);
    draw_pane(f, right, pane_chunks[1], active == PaneId::Right);

    let status_text = format!(" [{mode:?}]  {status}");
    let para = Paragraph::new(status_text).style(Style::default().add_modifier(Modifier::REVERSED));
    use ratatui::widgets::Widget;
    para.render(main_chunks[1], f.buffer_mut());

    if let Some(d) = dialog {
        let darea = centered_rect(60, 30, area);
        match d {
            ActiveDialog::Confirm { widget, .. } => widget.render(darea, f.buffer_mut()),
            ActiveDialog::Resume(widget) => widget.render(darea, f.buffer_mut()),
        }
    }
}

fn draw_pane(f: &mut ratatui::Frame, view: &mut PaneView, area: Rect, focused: bool) {
    let title = view.cwd.display();
    let border_style = if focused {
        Style::default().add_modifier(Modifier::BOLD)
    } else {
        Style::default()
    };
    let block = Block::default()
        .title(title)
        .borders(Borders::ALL)
        .border_style(border_style);
    let inner = block.inner(area);
    use ratatui::widgets::Widget;
    block.render(area, f.buffer_mut());
    view.render(inner, f.buffer_mut());
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

//! Cargonaut core — application state, event loop, command dispatch.
//!
//! The UI layer emits `Command`s into the App; the App applies them to
//! `AppState` and emits `Event`s back for the UI to re-render.

#![warn(missing_docs)]

/// User-driven commands. UI emits these.
#[derive(Debug, Clone)]
pub enum Command {
    /// Move pane cursor down.
    CursorDown,
    /// Move pane cursor up.
    CursorUp,
    /// Descend into selected entry (or open a file).
    Descend,
    /// Go to the parent directory.
    Ascend,
    /// Swap pane focus.
    FocusSwap,
    /// Toggle selection on current cursor.
    SelectionToggle,
    /// F5 — copy selection to opposite pane.
    Copy,
    /// F6 — move/rename.
    Move,
    /// F8 — delete selection.
    Delete,
    /// Ctrl-c — cancel in-flight operation.
    Cancel,
    /// F10 — quit.
    Quit,
}

/// Events back to the UI. App emits these after applying commands.
#[derive(Debug, Clone)]
pub enum Event {
    /// Pane state changed; UI should re-render.
    PaneUpdated(PaneId),
    /// Transfer progress updated.
    TransferProgress(TransferId, u64, u64),
    /// Transfer completed (success or failure).
    TransferCompleted(TransferId, Result<(), String>),
    /// Modal dialog request.
    DialogRequested(DialogKind),
    /// Status message for the bottom bar.
    Status(String),
}

/// Pane identifier (left/right today; tabs later).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    /// Left pane.
    Left,
    /// Right pane.
    Right,
}

/// Re-export from the transfer crate for convenience.
pub use cargonaut_transfer::TransferId;

/// Kinds of modal dialogs the UI may need to show.
#[derive(Debug, Clone)]
pub enum DialogKind {
    /// "Are you sure?" for destructive ops.
    Confirm(String),
    /// Resume a previously-interrupted transfer.
    Resume(Vec<String>),
    /// Conflict: destination exists.
    Conflict { src: String, dst: String },
}

/// Application root. T1.19 implements.
pub struct App {
    // ... pane state, transfer registry, config, plugin host (Phase 3), ...
}

impl App {
    /// Construct from loaded config + two initial paths.
    pub fn new(_config: cargonaut_config::Config, _left: &str, _right: &str) -> Self {
        unimplemented!("T1.19")
    }

    /// Apply a command and emit follow-up events.
    pub async fn dispatch(&mut self, _cmd: Command) -> Vec<Event> {
        unimplemented!("T1.19")
    }
}

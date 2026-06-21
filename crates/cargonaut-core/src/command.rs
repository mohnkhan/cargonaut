// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `command` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// User-driven actions the App responds to. UI layer parses keymap →
/// Command and feeds them into `App::dispatch`.
#[derive(Debug, Clone)]
pub enum Command {
    /// Move active pane's cursor down one visible entry.
    CursorDown,
    /// Move active pane's cursor up one visible entry.
    CursorUp,
    /// FR-014 — set the active pane's cursor to an absolute position in
    /// the visible subset (used by mouse clicks; clamps to range). Lives
    /// in core so a clicked cursor survives the per-frame `sync_from`.
    CursorTo(usize),
    /// Descend into the focused directory (or open the focused file).
    Descend,
    /// Ascend to the parent directory.
    Ascend,
    /// Swap active pane focus.
    FocusSwap,
    /// Focus the left pane explicitly.
    FocusLeft,
    /// Focus the right pane explicitly.
    FocusRight,
    /// Toggle selection on the cursor's current entry.
    SelectionToggle,
    /// Invert the entire selection.
    SelectionInvert,
    /// Toggle hidden-file visibility on the active pane.
    ToggleHidden,
    /// FR-013 Alt-! — open the panel filter prompt for the active pane.
    /// The popup is a UI-side modal (Feature 033); the set/clear logic
    /// lives in [`App::set_filter`]. Dispatching this command directly into
    /// core is a no-op — the TUI intercepts Alt-! to open the dialog.
    TogglePanelFilter,
    /// FR-014 Alt-i — copy the OTHER pane's cwd into the active pane.
    SyncOtherPanelPath,
    /// FR-014 Alt-o — open the focused entry's directory in the OTHER pane
    /// (keeps focus on the origin pane). No-op if focused entry isn't a dir.
    ShowFocusedInOtherPanel,
    /// FR-015 Alt-, — cycle split orientation horizontal ↔ vertical.
    ToggleSplitOrientation,
    /// FR-011 Alt-y — step to the previous dir in the active pane's
    /// back-history (no-op if empty).
    HistoryPrevDir,
    /// FR-011 Alt-u — step to the next dir in the active pane's
    /// forward-history (populated only after `HistoryPrevDir`).
    HistoryNextDir,
    /// FR-012 Alt-c — open the quick-cd popup. The popup is a UI-side
    /// modal (Feature 038); the completion + navigation logic lives in
    /// [`App::complete_cd`] / [`App::quick_cd`]. Dispatching this command
    /// directly into core is a no-op — the TUI intercepts Alt-c to open
    /// the dialog.
    QuickCdPopup,
    /// FR-016 F12 — show the in-flight transfers panel. Phase 1: stub
    /// (status message listing active transfer count); the panel
    /// widget lands in the next polish PR.
    ShowTasksPanel,
    /// F5 — copy selection (or focused entry) to the opposite pane.
    Copy,
    /// F6 — move/rename selection to the opposite pane.
    Move,
    /// F8 — delete selection.
    Delete,
    /// Ctrl-c — cancel the most recently submitted transfer.
    CancelCurrentTransfer,
    /// FR-021 — cycle the active pane's sort key (name → ext → size → mtime).
    CycleSortKey,
    /// FR-021 — flip the active pane's sort direction.
    ToggleSortReverse,
    /// FR-022 — cycle the global listing view (brief → full → quick-view).
    CycleListingMode,
    /// FR-023 — compute the recursive size of the focused directory.
    RecursiveDirSize,
    /// FR-024 — create a directory with the given name in the active pane.
    Mkdir(String),
    /// FR-025 — tag visible entries whose name matches the glob.
    SelectByPattern(String),
    /// FR-025 — untag visible entries whose name matches the glob.
    UnselectByPattern(String),
    /// Feature 043 (#46) — change ownership of the selection to `user[:group]`
    /// (routed through a confirmation dialog; see `chown_selection`).
    Chown(String),
    /// Feature 044 (#65) — recursively chmod the selection's subtree(s) to the
    /// given mode (routed through a confirmation dialog).
    ChmodRecursive(String),
    /// Feature 044 (#65) — recursively chown the selection's subtree(s)
    /// (routed through a confirmation dialog).
    ChownRecursive(String),
    /// F10 — quit cargonaut.
    Quit,
    /// Feature 049 — compare both panels' visible listings and additively
    /// mark all differing entries (name-only, size-differ, or hash-differ).
    CompareDirectories,
    /// Feature 050 — apply the validated rename pairs produced by the editor
    /// temp-file round-trip (called by the TUI after editor exits).
    BulkRenameApply(Vec<(String, String)>),
    /// Feature 050 — undo the most recent reversible file operation.
    UndoLastOp,
    /// Feature 053 — open a new tab on the active side, cloning the current pane state.
    TabNew,
    /// Feature 053 — close the active tab on the active side (no-op when only one tab).
    TabClose,
    /// Feature 053 — cycle to the next tab on the active side (wraps around).
    TabNext,
    /// Feature 053 — cycle to the previous tab on the active side (wraps around).
    TabPrev,
}

/// State changes the App emits back to the UI.
#[derive(Debug, Clone)]
pub enum Event {
    /// One pane's state changed; UI should re-render that pane.
    PaneUpdated(PaneId),
    /// A modal dialog should be shown.
    DialogRequested(DialogKind),
    /// Transfer just progressed (poll its `watch::Receiver` for details).
    TransferProgressed(TransferId),
    /// Transfer just terminated (Completed/Failed/Canceled).
    TransferTerminated(TransferId),
    /// Status-bar message.
    Status(String),
    /// App should exit cleanly.
    QuitRequested,
}

/// Kinds of modal dialogs the App may request.
#[derive(Debug, Clone)]
pub enum DialogKind {
    /// "Are you sure?" for a destructive op. `body` is shown verbatim.
    Confirm {
        /// Dialog title (e.g. "Delete 3 files?").
        title: String,
        /// Dialog body (e.g. listing of files).
        body: String,
        /// The Command to dispatch if the user confirms.
        on_confirm: Box<Command>,
    },
}

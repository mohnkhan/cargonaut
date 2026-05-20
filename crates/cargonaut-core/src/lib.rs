// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut core — application state, event loop, command dispatch.
//!
//! The UI layer emits `Command`s into the [`App`] via [`App::dispatch`];
//! the App applies them to its internal state and returns
//! [`Event`]s for the UI to re-render or react to.
//!
//! The [`App`] owns:
//! - [`Config`](cargonaut_config::Config) — runtime settings.
//! - Two [`PaneState`]s — cwd, listing snapshot, cursor, selection, etc.
//! - A `TransferRegistry` — active `TransferJob`s keyed by `TransferId`.
//! - A status-bar message + the currently-active pane id.
//!
//! The full `tokio::select!` event loop (input ↔ transfer progress) lives
//! in the binary (T1.21); this crate just provides the dispatch surface
//! and state machine so it stays testable without a terminal attached.

#![warn(missing_docs)]

use cargonaut_transfer::{
    submit_transfer, TransferError, TransferId, TransferJob, TransferOptions, TransferState,
};
use cargonaut_vfs::{DirListing, LocalFs, Sort, VfsBackend, VfsError, VfsPath};
use std::collections::{BTreeSet, HashMap};
use std::sync::Arc;
use thiserror::Error;

// =====================================================================
// PaneId / PaneState
// =====================================================================

/// Pane identifier (left/right today; tabs later — FR-202 in Phase 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    /// Left pane.
    Left,
    /// Right pane.
    Right,
}

impl PaneId {
    /// The other pane (Left ↔ Right).
    pub fn other(self) -> Self {
        match self {
            PaneId::Left => PaneId::Right,
            PaneId::Right => PaneId::Left,
        }
    }
}

/// Pure state for one pane. Renderable by the UI (ui-tui's `PaneView`
/// builds itself from a `&PaneState` per frame) and mutated by the
/// `App::dispatch` state machine.
#[derive(Debug, Clone)]
pub struct PaneState {
    /// Directory currently being viewed.
    pub cwd: VfsPath,
    /// Sorted listing snapshot for `cwd`.
    pub listing: DirListing,
    /// Cursor position within the **visible** subset (filter + hidden-masked).
    pub cursor: usize,
    /// Selected indices into `listing.entries`.
    pub selected: BTreeSet<usize>,
    /// Show Unix dotfiles in the listing.
    pub show_hidden: bool,
    /// Substring filter (placeholder for FR-013 globset).
    pub filter: Option<String>,
}

impl PaneState {
    /// Indices that pass the visibility filters, in listing order.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.listing
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if !self.show_hidden && e.meta.is_hidden {
                    return None;
                }
                if let Some(pat) = &self.filter {
                    if !e.name.as_str().contains(pat.as_str()) {
                        return None;
                    }
                }
                Some(i)
            })
            .collect()
    }

    /// Absolute index in `listing.entries` of the cursor's current entry.
    pub fn focused_entry_index(&self) -> Option<usize> {
        self.visible_indices().get(self.cursor).copied()
    }
}

// =====================================================================
// Command / Event / DialogKind
// =====================================================================

/// User-driven actions the App responds to. UI layer parses keymap →
/// Command and feeds them into `App::dispatch`.
#[derive(Debug, Clone)]
pub enum Command {
    /// Move active pane's cursor down one visible entry.
    CursorDown,
    /// Move active pane's cursor up one visible entry.
    CursorUp,
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
    /// F5 — copy selection (or focused entry) to the opposite pane.
    Copy,
    /// F6 — move/rename selection to the opposite pane.
    Move,
    /// F8 — delete selection.
    Delete,
    /// Ctrl-c — cancel the most recently submitted transfer.
    CancelCurrentTransfer,
    /// F10 — quit cargonaut.
    Quit,
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

// =====================================================================
// Errors
// =====================================================================

/// Errors from constructing or driving the App.
#[derive(Debug, Error)]
pub enum AppError {
    /// VFS-level failure.
    #[error("vfs: {0}")]
    Vfs(#[from] VfsError),

    /// Transfer-engine failure.
    #[error("transfer: {0}")]
    Transfer(#[from] TransferError),

    /// Path argument couldn't be parsed as a `file://` URI.
    #[error("bad path: {0}")]
    BadPath(String),
}

// =====================================================================
// App
// =====================================================================

/// Application root. Owns config + two panes + transfer registry +
/// active-dialog state. Dispatch is async because some commands (cd,
/// copy) call into the VFS / transfer engine.
pub struct App {
    config: cargonaut_config::Config,
    panes: [PaneState; 2],
    active: PaneId,
    local_fs: Arc<dyn VfsBackend>,
    transfers: HashMap<TransferId, TransferJob>,
    /// IDs in submit order — used by `CancelCurrentTransfer`.
    transfer_order: Vec<TransferId>,
    status: String,
}

impl App {
    /// Build an App with the two given starting paths. Paths may be
    /// absolute (`/home/me`) or `file://` URIs; relative paths are
    /// rejected. Lists both directories synchronously so the caller
    /// learns about NotFound up front.
    pub async fn new(
        config: cargonaut_config::Config,
        left: &str,
        right: &str,
    ) -> Result<Self, AppError> {
        let local_fs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let left_p = parse_path(left)?;
        let right_p = parse_path(right)?;
        let left_listing = local_fs.list(&left_p, Sort::NameAsc).await?;
        let right_listing = local_fs.list(&right_p, Sort::NameAsc).await?;

        let show_hidden = config.ui.show_hidden;

        let panes = [
            PaneState {
                cwd: left_p,
                listing: left_listing,
                cursor: 0,
                selected: BTreeSet::new(),
                show_hidden,
                filter: None,
            },
            PaneState {
                cwd: right_p,
                listing: right_listing,
                cursor: 0,
                selected: BTreeSet::new(),
                show_hidden,
                filter: None,
            },
        ];

        Ok(Self {
            config,
            panes,
            active: PaneId::Left,
            local_fs,
            transfers: HashMap::new(),
            transfer_order: Vec::new(),
            status: String::new(),
        })
    }

    /// Read-only access to the App's config.
    pub fn config(&self) -> &cargonaut_config::Config {
        &self.config
    }

    /// Which pane currently has focus.
    pub fn active_pane(&self) -> PaneId {
        self.active
    }

    /// Read-only access to a specific pane.
    pub fn pane(&self, id: PaneId) -> &PaneState {
        let idx = pane_idx(id);
        &self.panes[idx]
    }

    /// Read-only access to the active pane.
    pub fn active_pane_state(&self) -> &PaneState {
        self.pane(self.active)
    }

    /// Current status-bar message.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Snapshot of active transfer ids in submit order.
    pub fn transfer_ids(&self) -> Vec<TransferId> {
        self.transfer_order.clone()
    }

    /// Borrow a transfer by id (for the UI to read its `watch::Receiver`).
    pub fn transfer(&self, id: TransferId) -> Option<&TransferJob> {
        self.transfers.get(&id)
    }

    /// Apply a command. Returns the events the UI should react to.
    /// Many commands are state-only (no events beyond `PaneUpdated`);
    /// destructive ones request dialogs; `Copy`/`Move` spawn transfers.
    pub async fn dispatch(&mut self, cmd: Command) -> Result<Vec<Event>, AppError> {
        use Command::*;
        match cmd {
            CursorDown => {
                let p = self.active_pane_mut();
                let v = p.visible_indices();
                if !v.is_empty() {
                    p.cursor = (p.cursor + 1).min(v.len() - 1);
                }
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            CursorUp => {
                let p = self.active_pane_mut();
                p.cursor = p.cursor.saturating_sub(1);
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            Descend => self.descend_into_focused().await,
            Ascend => self.ascend_to_parent().await,
            FocusSwap => {
                self.active = self.active.other();
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            FocusLeft => {
                self.active = PaneId::Left;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            FocusRight => {
                self.active = PaneId::Right;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            SelectionToggle => {
                let p = self.active_pane_mut();
                if let Some(idx) = p.focused_entry_index() {
                    if !p.selected.insert(idx) {
                        p.selected.remove(&idx);
                    }
                }
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            SelectionInvert => {
                let p = self.active_pane_mut();
                let all_visible: BTreeSet<usize> = p.visible_indices().into_iter().collect();
                let new_sel: BTreeSet<usize> =
                    all_visible.difference(&p.selected).copied().collect();
                p.selected = new_sel;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            ToggleHidden => {
                let p = self.active_pane_mut();
                p.show_hidden = !p.show_hidden;
                p.cursor = 0;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            Copy => self.request_copy_confirmation(),
            Move => self.request_move_confirmation(),
            Delete => self.request_delete_confirmation(),
            CancelCurrentTransfer => {
                if let Some(id) = self.transfer_order.last().copied() {
                    if let Some(job) = self.transfers.get(&id) {
                        job.cancel.cancel();
                        Ok(vec![Event::Status(format!("Canceled transfer {id:?}"))])
                    } else {
                        Ok(vec![])
                    }
                } else {
                    Ok(vec![Event::Status("No active transfer to cancel".into())])
                }
            }
            Quit => Ok(vec![Event::QuitRequested]),
        }
    }

    /// Actually start a copy from the active pane's selection (or focused
    /// entry) to the opposite pane's cwd. Caller invokes this *after* the
    /// user confirms the dialog.
    pub async fn confirm_copy(&mut self) -> Result<Vec<Event>, AppError> {
        let src_pane = self.active;
        let dst_pane = src_pane.other();
        let entries = self.selection_or_focused(src_pane);
        if entries.is_empty() {
            return Ok(vec![Event::Status("Nothing selected".into())]);
        }
        let dst_cwd = self.pane(dst_pane).cwd.clone();
        let opts = TransferOptions {
            checkpoint_interval_bytes: u64::from(self.config.transfer.checkpoint_interval_mib)
                * 1024
                * 1024,
            verify_after_copy: self.config.transfer.verify_after_copy,
            ..Default::default()
        };
        let mut events = Vec::new();
        for entry_name in entries {
            let src_path = self.pane(src_pane).cwd.join(&entry_name);
            let dst_path = dst_cwd.join(&entry_name);
            let job = submit_transfer(
                Arc::clone(&self.local_fs),
                src_path,
                Arc::clone(&self.local_fs),
                dst_path,
                opts.clone(),
            )
            .await?;
            let id = job.id;
            self.transfers.insert(id, job);
            self.transfer_order.push(id);
            events.push(Event::TransferProgressed(id));
        }
        Ok(events)
    }

    /// Reload the active pane's listing from disk. Cursor + selection are
    /// preserved if the cursor is still in bounds; otherwise clamped.
    pub async fn refresh_active_pane(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let cwd = self.pane(id).cwd.clone();
        let listing = self.local_fs.list(&cwd, Sort::NameAsc).await?;
        let p = self.active_pane_mut();
        p.listing = listing;
        p.selected.clear();
        let v = p.visible_indices();
        if v.is_empty() {
            p.cursor = 0;
        } else {
            p.cursor = p.cursor.min(v.len() - 1);
        }
        Ok(vec![Event::PaneUpdated(id)])
    }

    // -------- internals --------

    fn active_pane_mut(&mut self) -> &mut PaneState {
        let idx = pane_idx(self.active);
        &mut self.panes[idx]
    }

    fn pane_mut(&mut self, id: PaneId) -> &mut PaneState {
        let idx = pane_idx(id);
        &mut self.panes[idx]
    }

    /// Names of entries the user "means" by their current selection:
    /// the tagged set if non-empty, else the focused entry alone.
    fn selection_or_focused(&self, id: PaneId) -> Vec<String> {
        let p = self.pane(id);
        if !p.selected.is_empty() {
            p.selected
                .iter()
                .filter_map(|i| p.listing.entries.get(*i))
                .map(|e| e.name.to_string())
                .collect()
        } else {
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .map(|e| vec![e.name.to_string()])
                .unwrap_or_default()
        }
    }

    async fn descend_into_focused(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let target = {
            let p = self.pane(id);
            let entry = p
                .focused_entry_index()
                .and_then(|i| p.listing.entries.get(i));
            entry.map(|e| (e.name.to_string(), e.meta.kind.clone()))
        };
        let Some((name, kind)) = target else {
            return Ok(vec![]);
        };
        if !matches!(kind, cargonaut_vfs::VfsKind::Dir) {
            // Descend on a file is a no-op for now (T1.21 will open via $EDITOR / openers).
            return Ok(vec![Event::Status(format!("{name} is not a directory"))]);
        }
        let new_cwd = self.pane(id).cwd.join(&name);
        let listing = self.local_fs.list(&new_cwd, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        p.cwd = new_cwd;
        p.listing = listing;
        p.cursor = 0;
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    async fn ascend_to_parent(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let Some(parent) = self.pane(id).cwd.parent() else {
            return Ok(vec![Event::Status("Already at root".into())]);
        };
        let listing = self.local_fs.list(&parent, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        p.cwd = parent;
        p.listing = listing;
        p.cursor = 0;
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    fn request_copy_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to copy".into())]);
        }
        let dst = self.pane(self.active.other()).cwd.display();
        let body = format!(
            "Copy {} item(s) to {dst}:\n  {}",
            names.len(),
            names.join("\n  ")
        );
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Copy".into(),
            body,
            on_confirm: Box::new(Command::Copy),
        })])
    }

    fn request_move_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to move".into())]);
        }
        let body = format!("Move {} item(s)", names.len());
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Move".into(),
            body,
            on_confirm: Box::new(Command::Move),
        })])
    }

    fn request_delete_confirmation(&self) -> Result<Vec<Event>, AppError> {
        let names = self.selection_or_focused(self.active);
        if names.is_empty() {
            return Ok(vec![Event::Status("Nothing selected to delete".into())]);
        }
        let body = format!(
            "Permanently delete {} item(s)?\n  {}",
            names.len(),
            names.join("\n  ")
        );
        Ok(vec![Event::DialogRequested(DialogKind::Confirm {
            title: "Delete".into(),
            body,
            on_confirm: Box::new(Command::Delete),
        })])
    }
}

fn pane_idx(id: PaneId) -> usize {
    match id {
        PaneId::Left => 0,
        PaneId::Right => 1,
    }
}

fn parse_path(s: &str) -> Result<VfsPath, AppError> {
    if let Some(rest) = s.strip_prefix("file://") {
        return VfsPath::parse(&format!("file://{}", rest))
            .map_err(|e| AppError::BadPath(e.to_string()));
    }
    if !s.starts_with('/') {
        return Err(AppError::BadPath(format!(
            "{s:?} must be absolute or a file:// URI"
        )));
    }
    VfsPath::parse(&format!("file://{}", s)).map_err(|e| AppError::BadPath(e.to_string()))
}

/// Snapshot helper: poll a `TransferJob::state` once. Mostly for the
/// UI to render the current state without owning a `watch::Receiver`
/// borrow across awaits.
pub fn transfer_state_snapshot(job: &TransferJob) -> TransferState {
    job.state.borrow().clone()
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;
    use tokio::fs;

    async fn make_app(td_left: &TempDir, td_right: &TempDir) -> App {
        let config = cargonaut_config::Config::default();
        App::new(
            config,
            td_left.path().to_str().unwrap(),
            td_right.path().to_str().unwrap(),
        )
        .await
        .unwrap()
    }

    #[tokio::test]
    async fn new_loads_both_pane_listings() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"").await.unwrap();
        fs::write(td_l.path().join("b"), b"").await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 2);
        assert_eq!(app.pane(PaneId::Right).listing.entries.len(), 0);
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn new_rejects_relative_path() {
        let config = cargonaut_config::Config::default();
        let res = App::new(config, "relative/path", "/tmp").await;
        assert!(matches!(res, Err(AppError::BadPath(_))));
    }

    #[tokio::test]
    async fn cursor_down_advances_within_visible_subset() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 1);
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
    }

    #[tokio::test]
    async fn focus_swap_toggles_active_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.active_pane(), PaneId::Left);
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(app.active_pane(), PaneId::Right);
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn selection_toggle_marks_focused_entry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(app.pane(PaneId::Left).selected.contains(&0));
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(!app.pane(PaneId::Left).selected.contains(&0));
    }

    #[tokio::test]
    async fn descend_into_subdir_then_ascend_back() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        fs::write(td_l.path().join("sub/x"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let parent_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 1);
        app.dispatch(Command::Ascend).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, parent_cwd);
    }

    #[tokio::test]
    async fn copy_with_no_selection_emits_status_not_dialog() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::Copy).await.unwrap();
        assert!(
            events.iter().any(|e| matches!(e, Event::Status(_))),
            "expected Status event, got {events:?}"
        );
        assert!(
            !events
                .iter()
                .any(|e| matches!(e, Event::DialogRequested(_))),
            "no dialog when nothing to copy"
        );
    }

    #[tokio::test]
    async fn copy_with_selection_requests_confirmation() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"hello").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        let events = app.dispatch(Command::Copy).await.unwrap();
        let has_dialog = events
            .iter()
            .any(|e| matches!(e, Event::DialogRequested(DialogKind::Confirm { .. })));
        assert!(has_dialog, "expected Confirm dialog, got {events:?}");
    }

    #[tokio::test]
    async fn confirm_copy_spawns_a_transfer() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"hello").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        app.dispatch(Command::Copy).await.unwrap(); // request dialog
        let events = app.confirm_copy().await.unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::TransferProgressed(_))));
        assert_eq!(app.transfer_ids().len(), 1);
    }

    #[tokio::test]
    async fn quit_emits_quit_requested() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::Quit).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::QuitRequested)));
    }

    #[tokio::test]
    async fn toggle_hidden_resets_cursor() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join(".hidden"), b"").await.unwrap();
        fs::write(td_l.path().join("visible"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Initially show_hidden=false (per Config default); cursor at 0 sees "visible".
        let initial = app.active_pane_state().focused_entry_index();
        assert!(initial.is_some());
        app.dispatch(Command::ToggleHidden).await.unwrap();
        assert!(app.pane(PaneId::Left).show_hidden);
        // Cursor reset to 0; both files now visible.
        assert_eq!(app.pane(PaneId::Left).cursor, 0);
        assert_eq!(app.pane(PaneId::Left).visible_indices().len(), 2);
    }

    #[tokio::test]
    async fn cancel_current_transfer_signals_cancel_on_latest() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let big: Vec<u8> = vec![0u8; 8 * 1024 * 1024];
        fs::write(td_l.path().join("big"), &big).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        app.dispatch(Command::Copy).await.unwrap();
        app.confirm_copy().await.unwrap();
        let id = app.transfer_ids()[0];
        let events = app.dispatch(Command::CancelCurrentTransfer).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
        // The cancellation token must be triggered.
        assert!(app.transfer(id).unwrap().cancel.is_cancelled());
    }
}

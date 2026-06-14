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
    resume_transfer, scan_resumable, submit_transfer, ResumableTransfer, TransferError, TransferId,
    TransferJob, TransferOptions, TransferState,
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
    /// Active sort order for this pane's listing (FR-021).
    pub sort: Sort,
    /// Substring filter (placeholder for FR-013 globset).
    pub filter: Option<String>,
    /// FR-011 back history: cwds visited before the current one, most
    /// recent at the end. Bounded by `Config::ui.history.directory_depth`.
    pub dir_history_back: Vec<VfsPath>,
    /// FR-011 forward history: only populated after [`Command::HistoryPrevDir`].
    /// Cleared on any non-history navigation (descend / ascend / sync).
    pub dir_history_fwd: Vec<VfsPath>,
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
    /// FR-013 Alt-! — clear the active pane's filter. (Setting a filter
    /// requires the prompt dialog — deferred; Phase 1 just clears.)
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
    /// F10 — quit cargonaut.
    Quit,
}

/// FR-022 — the global listing/preview view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Names only (compact).
    Brief,
    /// Name + size + mtime + permissions.
    Full,
    /// The passive panel previews the active panel's highlighted file.
    QuickView,
}

/// FR-026 — user-facing snapshot of an in-flight transfer.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ProgressView {
    /// Bytes copied so far.
    pub bytes_done: u64,
    /// Total bytes to copy.
    pub bytes_total: u64,
    /// Estimated seconds remaining.
    pub eta_secs: u32,
    /// Current throughput in MiB/s.
    pub throughput_mibs: f32,
}

/// Feature 037 — UI-agnostic projection of one resumable transfer found
/// on launch. Lets the UI build its resume prompt without depending on
/// the transfer crate's types (mirrors [`ProgressView`]). The UI maps
/// this onto its own per-row summary widget.
#[derive(Debug, Clone, PartialEq)]
pub struct ResumeOfferView {
    /// Source URI/path, as recorded in the checkpoint.
    pub src: String,
    /// Destination URI/path, as recorded in the checkpoint.
    pub dst: String,
    /// Bytes already written, expressed in MiB.
    pub bytes_written_mib: f32,
    /// Source size, expressed in MiB.
    pub src_size_mib: f32,
    /// True if the source's content fingerprint is unchanged since the
    /// checkpoint was written.
    pub source_unchanged: bool,
    /// True if the partial destination still matches the checkpoint's
    /// integrity chain.
    pub dest_intact: bool,
}

impl ViewMode {
    /// Cycle Brief → Full → QuickView → Brief.
    pub fn next(self) -> Self {
        match self {
            ViewMode::Brief => ViewMode::Full,
            ViewMode::Full => ViewMode::QuickView,
            ViewMode::QuickView => ViewMode::Brief,
        }
    }
}

/// FR-015 split orientation for the two-pane layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrient {
    /// Side-by-side panes (default — the classic orthodox look).
    Horizontal,
    /// Stacked panes (left = top, right = bottom).
    Vertical,
}

impl SplitOrient {
    /// Cycle to the other orientation.
    pub fn toggle(self) -> Self {
        match self {
            SplitOrient::Horizontal => SplitOrient::Vertical,
            SplitOrient::Vertical => SplitOrient::Horizontal,
        }
    }
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
    /// Feature 037 — resumable transfers discovered on launch, in scan
    /// order. Drained one at a time as the user answers the resume prompt.
    pending_resumes: Vec<ResumableTransfer>,
    status: String,
    split: SplitOrient,
    view_mode: ViewMode,
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
                sort: Sort::NameAsc,
                filter: None,
                dir_history_back: Vec::new(),
                dir_history_fwd: Vec::new(),
            },
            PaneState {
                cwd: right_p,
                listing: right_listing,
                cursor: 0,
                selected: BTreeSet::new(),
                show_hidden,
                sort: Sort::NameAsc,
                filter: None,
                dir_history_back: Vec::new(),
                dir_history_fwd: Vec::new(),
            },
        ];

        Ok(Self {
            config,
            panes,
            active: PaneId::Left,
            local_fs,
            transfers: HashMap::new(),
            transfer_order: Vec::new(),
            pending_resumes: Vec::new(),
            status: String::new(),
            split: SplitOrient::Horizontal,
            view_mode: ViewMode::Full,
        })
    }

    /// The current global listing/preview view mode (FR-022).
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// FR-026 — a UI-friendly projection of the most recent in-flight
    /// transfer, or `None` if nothing is running. Lets the UI render a
    /// progress dialog without depending on the transfer crate's types.
    pub fn active_progress(&self) -> Option<ProgressView> {
        for id in self.transfer_order.iter().rev() {
            if let Some(job) = self.transfers.get(id) {
                if let TransferState::Running {
                    bytes_done,
                    bytes_total,
                    eta_secs,
                    throughput_mibs,
                } = transfer_state_snapshot(job)
                {
                    return Some(ProgressView {
                        bytes_done,
                        bytes_total,
                        eta_secs,
                        throughput_mibs,
                    });
                }
            }
        }
        None
    }

    /// Current split orientation. UI reads this to lay out the two panes.
    pub fn split_orient(&self) -> SplitOrient {
        self.split
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
            CursorTo(n) => {
                let p = self.active_pane_mut();
                let v = p.visible_indices();
                if v.is_empty() {
                    p.cursor = 0;
                } else {
                    p.cursor = n.min(v.len() - 1);
                }
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
            TogglePanelFilter => {
                // FR-013 Phase 1: just clear the filter when invoked.
                // A future iteration ships the glob-pattern prompt
                // dialog and replaces this with a request_filter_dialog().
                let p = self.active_pane_mut();
                p.filter = None;
                p.cursor = 0;
                Ok(vec![
                    Event::PaneUpdated(self.active),
                    Event::Status("Panel filter cleared".into()),
                ])
            }
            SyncOtherPanelPath => self.sync_other_panel_path().await,
            ShowFocusedInOtherPanel => self.show_focused_in_other_panel().await,
            ToggleSplitOrientation => {
                self.split = self.split.toggle();
                Ok(vec![
                    Event::PaneUpdated(PaneId::Left),
                    Event::PaneUpdated(PaneId::Right),
                ])
            }
            HistoryPrevDir => self.history_prev_dir().await,
            HistoryNextDir => self.history_next_dir().await,
            // Feature 038: opened UI-side (the TUI builds the dialog and
            // calls `complete_cd`/`quick_cd`). A direct dispatch is a no-op.
            QuickCdPopup => Ok(vec![]),
            ShowTasksPanel => {
                let n = self.transfer_order.len();
                Ok(vec![Event::Status(format!(
                    "Tasks panel not yet implemented (T1.29 stub) — {n} active transfer(s)"
                ))])
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
            CycleSortKey => {
                let p = self.active_pane_mut();
                p.sort = next_sort_key(p.sort);
                let label = sort_label(p.sort);
                let mut evs = self.relist_active().await?;
                evs.push(Event::Status(format!("Sort: {label}")));
                Ok(evs)
            }
            ToggleSortReverse => {
                let p = self.active_pane_mut();
                p.sort = match p.sort {
                    Sort::NameAsc => Sort::NameDesc,
                    Sort::NameDesc => Sort::NameAsc,
                    other => other,
                };
                let label = sort_label(p.sort);
                let mut evs = self.relist_active().await?;
                evs.push(Event::Status(format!("Sort: {label}")));
                Ok(evs)
            }
            CycleListingMode => {
                self.view_mode = self.view_mode.next();
                Ok(vec![Event::Status(format!("View: {:?}", self.view_mode))])
            }
            RecursiveDirSize => self.recursive_dir_size().await,
            Mkdir(name) => self.mkdir(&name).await,
            SelectByPattern(pat) => Ok(self.select_by_pattern(&pat, true)),
            UnselectByPattern(pat) => Ok(self.select_by_pattern(&pat, false)),
            Quit => Ok(vec![Event::QuitRequested]),
        }
    }

    /// Re-list the active pane's cwd with its current sort; clamp cursor.
    async fn relist_active(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let (cwd, sort) = {
            let p = self.pane(id);
            (p.cwd.clone(), p.sort)
        };
        let listing = self.local_fs.list(&cwd, sort).await?;
        let p = self.pane_mut(id);
        p.listing = listing;
        let v = p.visible_indices();
        p.cursor = if v.is_empty() {
            0
        } else {
            p.cursor.min(v.len() - 1)
        };
        Ok(vec![Event::PaneUpdated(id)])
    }

    /// FR-024 — create a directory in the active pane's cwd; refresh.
    async fn mkdir(&mut self, name: &str) -> Result<Vec<Event>, AppError> {
        let name = name.trim();
        if name.is_empty() || name.contains('/') {
            return Ok(vec![Event::Status(format!(
                "Invalid directory name {name:?}"
            ))]);
        }
        let target = self.active_pane_state().cwd.join(name);
        match self.local_fs.mkdir(&target, false).await {
            Ok(()) => {
                let mut evs = self.refresh_active_pane().await?;
                evs.push(Event::Status(format!("Created {name}")));
                Ok(evs)
            }
            Err(e) => Ok(vec![Event::Status(format!("mkdir failed: {e}"))]),
        }
    }

    /// FR-025 — tag (or untag) visible entries whose name matches `pat`.
    fn select_by_pattern(&mut self, pat: &str, add: bool) -> Vec<Event> {
        let id = self.active;
        let visible = self.pane(id).visible_indices();
        let mut matched = 0usize;
        let p = self.pane_mut(id);
        for i in visible {
            let name = p.listing.entries[i].name.to_string();
            if glob_match(pat, &name) {
                matched += 1;
                if add {
                    p.selected.insert(i);
                } else {
                    p.selected.remove(&i);
                }
            }
        }
        let verb = if add { "Tagged" } else { "Untagged" };
        vec![
            Event::PaneUpdated(id),
            Event::Status(format!(
                "{verb} {matched} entr{}",
                if matched == 1 { "y" } else { "ies" }
            )),
        ]
    }

    /// FR-023 — recursive size of the focused directory (bounded walk).
    async fn recursive_dir_size(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let target = {
            let p = self.pane(id);
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .and_then(|e| match &e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => {
                        Some((e.name.to_string(), p.cwd.join(e.name.as_str())))
                    }
                    _ => None,
                })
        };
        let Some((name, path)) = target else {
            return Ok(vec![Event::Status("Not a directory".into())]);
        };
        // Bounded breadth-first walk; cap node count so a huge tree can't
        // wedge the UI (FR-023).
        const NODE_CAP: usize = 200_000;
        let mut total: u64 = 0;
        let mut nodes = 0usize;
        let mut stack = vec![path];
        let mut truncated = false;
        while let Some(dir) = stack.pop() {
            let listing = match self.local_fs.list(&dir, Sort::NameAsc).await {
                Ok(l) => l,
                Err(_) => continue,
            };
            for e in listing.entries {
                nodes += 1;
                if nodes > NODE_CAP {
                    truncated = true;
                    break;
                }
                match e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => stack.push(dir.join(e.name.as_str())),
                    _ => total += e.meta.size,
                }
            }
            if truncated {
                break;
            }
        }
        let suffix = if truncated { " (truncated)" } else { "" };
        Ok(vec![Event::Status(format!(
            "{name}: {total} bytes{suffix}"
        ))])
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
        let opts = self.transfer_opts();
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

    /// Transfer options derived from the active config (checkpoint
    /// interval + post-copy verification). Shared by fresh copies and
    /// resumed/started-over transfers so they behave identically.
    fn transfer_opts(&self) -> TransferOptions {
        TransferOptions {
            checkpoint_interval_bytes: u64::from(self.config.transfer.checkpoint_interval_mib)
                * 1024
                * 1024,
            verify_after_copy: self.config.transfer.verify_after_copy,
            ..Default::default()
        }
    }

    // -----------------------------------------------------------------
    // Feature 037 — resume-on-launch seam.
    //
    // The binary calls `scan_resume_offers` once on startup; the UI
    // renders the returned views, then routes the user's choice back into
    // `resume_offer` / `start_over_offer` / `skip_offer` by index. After
    // each choice the UI rebuilds its dialog from `pending_resume_views`
    // (so indices never drift). All transfer-crate interaction stays here.
    // -----------------------------------------------------------------

    /// Scan both pane directories (non-recursively, de-duplicated) for
    /// orphan checkpoint sidecars and remember any found as pending resume
    /// offers. Returns UI-friendly projections in scan order. Safe to call
    /// once on launch; an empty result means "nothing to resume" (the hot
    /// path — no prompt). (FR-001/002/003)
    pub async fn scan_resume_offers(&mut self) -> Result<Vec<ResumeOfferView>, AppError> {
        self.pending_resumes.clear();
        let mut scanned: Vec<VfsPath> = Vec::new();
        for id in [PaneId::Left, PaneId::Right] {
            let dir = self.pane(id).cwd.clone();
            if scanned.contains(&dir) {
                continue;
            }
            scanned.push(dir.clone());
            let found = scan_resumable(Arc::clone(&self.local_fs), dir).await?;
            self.pending_resumes.extend(found);
        }
        Ok(self.pending_resume_views())
    }

    /// Project the current pending resume offers to UI views, in order.
    /// Pure (no I/O). Used by the UI to rebuild its prompt after each
    /// choice.
    pub fn pending_resume_views(&self) -> Vec<ResumeOfferView> {
        self.pending_resumes.iter().map(resume_offer_view).collect()
    }

    /// Resume the offer at `index`: continue the transfer from its
    /// checkpoint and register it like any other in-flight transfer. On a
    /// validation failure (e.g. the destination changed) the offer is
    /// dropped and a status message is returned — never a corrupt copy.
    /// (FR-005/006/009, SC-005)
    pub async fn resume_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        if index >= self.pending_resumes.len() {
            return Ok(vec![Event::Status("No such resume offer".into())]);
        }
        let rt = self.pending_resumes.remove(index);
        let opts = self.transfer_opts();
        match resume_transfer(
            Arc::clone(&self.local_fs),
            Arc::clone(&self.local_fs),
            rt.checkpoint,
            opts,
        )
        .await
        {
            Ok(job) => {
                let id = job.id;
                self.transfers.insert(id, job);
                self.transfer_order.push(id);
                Ok(vec![Event::TransferProgressed(id)])
            }
            Err(e) => Ok(vec![Event::Status(format!("Cannot resume: {e}"))]),
        }
    }

    /// Start the offer at `index` over from scratch: discard its
    /// checkpoint sidecar and submit a fresh copy (which truncates the
    /// partial destination). (FR-007)
    pub async fn start_over_offer(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        if index >= self.pending_resumes.len() {
            return Ok(vec![Event::Status("No such resume offer".into())]);
        }
        let rt = self.pending_resumes.remove(index);
        // Discard the stale checkpoint so a future scan won't re-offer it.
        let _ = std::fs::remove_file(&rt.checkpoint_path);
        let src = parse_path(&rt.checkpoint.src_uri)?;
        let dst = parse_path(&rt.checkpoint.dst_uri)?;
        let opts = self.transfer_opts();
        let job = submit_transfer(
            Arc::clone(&self.local_fs),
            src,
            Arc::clone(&self.local_fs),
            dst,
            opts,
        )
        .await?;
        let id = job.id;
        self.transfers.insert(id, job);
        self.transfer_order.push(id);
        Ok(vec![Event::TransferProgressed(id)])
    }

    /// Skip the offer at `index`: start no transfer and leave the
    /// checkpoint sidecar on disk so it is offered again next launch.
    /// (FR-008)
    pub fn skip_offer(&mut self, index: usize) {
        if index < self.pending_resumes.len() {
            self.pending_resumes.remove(index);
        }
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
        self.navigate_to(id, new_cwd).await
    }

    async fn sync_other_panel_path(&mut self) -> Result<Vec<Event>, AppError> {
        let active = self.active;
        let other = active.other();
        let other_cwd = self.pane(other).cwd.clone();
        self.navigate_to(active, other_cwd).await
    }

    async fn show_focused_in_other_panel(&mut self) -> Result<Vec<Event>, AppError> {
        let active = self.active;
        let other = active.other();
        let target = {
            let p = self.pane(active);
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .and_then(|e| match &e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => Some(p.cwd.join(e.name.as_str())),
                    _ => None,
                })
        };
        let Some(target) = target else {
            return Ok(vec![Event::Status(
                "Focused entry isn't a directory".into(),
            )]);
        };
        // navigate_to acts on `other`, not `active` — FR-014: focus stays put.
        self.navigate_to(other, target).await
    }

    async fn ascend_to_parent(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let Some(parent) = self.pane(id).cwd.parent() else {
            return Ok(vec![Event::Status("Already at root".into())]);
        };
        self.navigate_to(id, parent).await
    }

    /// FR-011 history-aware navigation. Pushes the OLD cwd onto the
    /// pane's back-history (bounded by `Config::ui.history.directory_depth`)
    /// and clears the forward-history. Called by every non-history nav
    /// entry point (descend, ascend, sync, show-in-other).
    async fn navigate_to(&mut self, id: PaneId, new_cwd: VfsPath) -> Result<Vec<Event>, AppError> {
        let listing = self.local_fs.list(&new_cwd, Sort::NameAsc).await?;
        let depth = self.config.ui.history.directory_depth as usize;
        let p = self.pane_mut(id);
        let old_cwd = std::mem::replace(&mut p.cwd, new_cwd);
        if depth > 0 {
            p.dir_history_back.push(old_cwd);
            while p.dir_history_back.len() > depth {
                p.dir_history_back.remove(0);
            }
        }
        p.dir_history_fwd.clear();
        p.listing = listing;
        p.cursor = 0;
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    /// Feature 038 (FR-012/R-003): resolve quick-cd input text to a
    /// `VfsPath` relative to the active pane's cwd.
    ///
    /// - text containing `://` is parsed as a full URI;
    /// - text starting with `/` is an absolute path under the active
    ///   pane's scheme/authority (rooted at the active backend);
    /// - otherwise it is relative to the active pane's cwd.
    ///
    /// `.` segments are skipped, `..` pops one segment (saturating at
    /// root), empty segments (e.g. a trailing `/`) are ignored. The path
    /// is not checked for existence here — that happens in `navigate_to`.
    fn resolve_cd_target(&self, text: &str) -> Result<VfsPath, AppError> {
        if text.contains("://") {
            return VfsPath::parse(text).map_err(|e| AppError::BadPath(e.to_string()));
        }
        let active_cwd = &self.active_pane_state().cwd;
        let (mut path, rest) = if let Some(stripped) = text.strip_prefix('/') {
            // Absolute: walk to the root of the active backend, keeping
            // its scheme + authority, then apply the typed segments.
            let mut root = active_cwd.clone();
            while let Some(parent) = root.parent() {
                root = parent;
            }
            (root, stripped)
        } else {
            (active_cwd.clone(), text)
        };
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if let Some(parent) = path.parent() {
                        path = parent;
                    }
                }
                s => path = path.join(s),
            }
        }
        Ok(path)
    }

    /// Feature 038 (FR-004/005/006/012/013): accept a quick-cd path for
    /// the active pane. Resolves `path_text` relative to the active cwd
    /// and navigates via the normal [`Self::navigate_to`] path (which
    /// lists the target first, so a non-existent / non-directory /
    /// permission-denied target returns `Err` with App state unchanged).
    ///
    /// Empty / whitespace-only input is a no-op (`Ok` with no events).
    pub async fn quick_cd(&mut self, path_text: &str) -> Result<Vec<Event>, AppError> {
        let trimmed = path_text.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        let target = self.resolve_cd_target(trimmed)?;
        let id = self.active;
        self.navigate_to(id, target).await
    }

    /// Feature 038 (FR-007/008/009): compute directory completion
    /// candidates for the active pane from `partial` quick-cd input.
    ///
    /// The final path segment is the prefix to complete; earlier segments
    /// name the directory to list (resolved relative to the active cwd).
    /// Returns full path strings (URI form), ordered recent-visited
    /// matches first (most-recent first), then filesystem children in
    /// backend sort order, de-duplicated. Only directories are returned;
    /// an empty result means "nothing to complete". Read-only.
    pub async fn complete_cd(&self, partial: &str) -> Vec<String> {
        let (dir_text, last) = match partial.rfind('/') {
            Some(i) => (&partial[..=i], &partial[i + 1..]),
            None => ("", partial),
        };
        let dir = if dir_text.is_empty() {
            self.active_pane_state().cwd.clone()
        } else {
            match self.resolve_cd_target(dir_text) {
                Ok(p) => p,
                Err(_) => return Vec::new(),
            }
        };

        // Recent-visited matches first (most-recent at the end of the
        // history vec, so iterate in reverse): a recent dir matches when
        // it lives directly under `dir` and its final segment shares the
        // prefix.
        let active = self.active_pane_state();
        let mut out: Vec<String> = Vec::new();
        for hist in active.dir_history_back.iter().rev() {
            if hist.parent().as_ref() == Some(&dir) {
                if let Some(name) = hist.segments.last() {
                    if name.as_str().starts_with(last) {
                        let s = hist.display();
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
            }
        }

        // Then filesystem children that are directories and match.
        if let Ok(listing) = self.local_fs.list(&dir, Sort::NameAsc).await {
            for e in listing.entries {
                if matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir)
                    && e.name.as_str().starts_with(last)
                {
                    let s = dir.join(e.name.as_str()).display();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }

    async fn history_prev_dir(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let prev = self.pane_mut(id).dir_history_back.pop();
        let Some(prev) = prev else {
            return Ok(vec![Event::Status("No prior directory".into())]);
        };
        let listing = self.local_fs.list(&prev, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        let cur = std::mem::replace(&mut p.cwd, prev);
        p.dir_history_fwd.push(cur);
        p.listing = listing;
        p.cursor = 0;
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    async fn history_next_dir(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let next = self.pane_mut(id).dir_history_fwd.pop();
        let Some(next) = next else {
            return Ok(vec![Event::Status("No forward directory".into())]);
        };
        let listing = self.local_fs.list(&next, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        let cur = std::mem::replace(&mut p.cwd, next);
        p.dir_history_back.push(cur);
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

/// Cycle the sort *key* (FR-021): name → ext → size → mtime → name.
/// Reverse direction is a separate toggle.
fn next_sort_key(s: Sort) -> Sort {
    match s {
        Sort::NameAsc | Sort::NameDesc => Sort::ExtAsc,
        Sort::ExtAsc => Sort::SizeDesc,
        Sort::SizeDesc => Sort::MtimeDesc,
        Sort::MtimeDesc => Sort::NameAsc,
    }
}

/// Human-facing label for a sort order (surfaced in the status bar).
fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::NameAsc => "name",
        Sort::NameDesc => "name (reverse)",
        Sort::ExtAsc => "extension",
        Sort::SizeDesc => "size",
        Sort::MtimeDesc => "modified",
    }
}

/// Minimal shell-glob matcher supporting `*` (any run) and `?` (one char).
/// Dependency-free (avoids pulling regex/globset for FR-025).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // Iterative backtracking matcher.
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark): (Option<usize>, usize) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

/// Project a [`ResumableTransfer`] into the UI-facing [`ResumeOfferView`].
fn resume_offer_view(rt: &ResumableTransfer) -> ResumeOfferView {
    const MIB: f32 = 1024.0 * 1024.0;
    ResumeOfferView {
        src: rt.checkpoint.src_uri.clone(),
        dst: rt.checkpoint.dst_uri.clone(),
        bytes_written_mib: rt.checkpoint.bytes_written as f32 / MIB,
        src_size_mib: rt.checkpoint.src_size as f32 / MIB,
        source_unchanged: rt.source_unchanged,
        dest_intact: rt.dest_intact,
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
    async fn cursor_to_sets_absolute_position_and_clamps() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::CursorTo(2)).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
        // Out-of-range clamps to last visible entry.
        app.dispatch(Command::CursorTo(99)).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
    }

    #[tokio::test]
    async fn cursor_to_survives_resync_via_pane_state() {
        // A clicked cursor must be authoritative: reading pane state back
        // reflects it (the UI's PaneView::sync_from copies state.cursor).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c", "d"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::CursorTo(3)).await.unwrap();
        assert_eq!(app.active_pane_state().cursor, 3);
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

    #[test]
    fn glob_match_basic() {
        assert!(glob_match("*.rs", "lib.rs"));
        assert!(glob_match("*.rs", ".rs"));
        assert!(!glob_match("*.rs", "lib.toml"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("read*e", "readme"));
        assert!(glob_match("read*e", "readsome"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactly"));
    }

    #[tokio::test]
    async fn cycle_sort_key_reorders_listing() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Names ascending: a, b, z; sizes: a=3, b=1, z=2.
        fs::write(td_l.path().join("a"), b"xxx").await.unwrap();
        fs::write(td_l.path().join("b"), b"x").await.unwrap();
        fs::write(td_l.path().join("z"), b"xx").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.pane(PaneId::Left).sort, Sort::NameAsc);
        // name -> ext
        app.dispatch(Command::CycleSortKey).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::ExtAsc);
        // ext -> size (desc): largest first = a(3), z(2), b(1)
        app.dispatch(Command::CycleSortKey).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::SizeDesc);
        assert_eq!(app.pane(PaneId::Left).listing.entries[0].name.as_str(), "a");
    }

    #[tokio::test]
    async fn toggle_sort_reverse_flips_name_order() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::ToggleSortReverse).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::NameDesc);
        assert_eq!(app.pane(PaneId::Left).listing.entries[0].name.as_str(), "c");
    }

    #[tokio::test]
    async fn cycle_listing_mode_rotates_view() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.view_mode(), ViewMode::Full);
        app.dispatch(Command::CycleListingMode).await.unwrap();
        assert_eq!(app.view_mode(), ViewMode::QuickView);
        app.dispatch(Command::CycleListingMode).await.unwrap();
        assert_eq!(app.view_mode(), ViewMode::Brief);
    }

    #[tokio::test]
    async fn mkdir_creates_directory_and_refreshes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::Mkdir("newdir".into())).await.unwrap();
        assert!(td_l.path().join("newdir").is_dir());
        assert!(app
            .pane(PaneId::Left)
            .listing
            .entries
            .iter()
            .any(|e| e.name.as_str() == "newdir"));
    }

    #[tokio::test]
    async fn mkdir_rejects_invalid_name_without_crash() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let evs = app.dispatch(Command::Mkdir("a/b".into())).await.unwrap();
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Invalid"))));
        assert!(!td_l.path().join("a").exists());
    }

    #[tokio::test]
    async fn select_by_pattern_tags_matches_and_unselect_removes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["one.rs", "two.rs", "note.txt"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert_eq!(app.pane(PaneId::Left).selected.len(), 2);
        app.dispatch(Command::UnselectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert_eq!(app.pane(PaneId::Left).selected.len(), 0);
    }

    #[tokio::test]
    async fn select_by_pattern_zero_match_reports_zero() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("x.txt"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let evs = app
            .dispatch(Command::SelectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Tagged 0"))));
    }

    #[tokio::test]
    async fn recursive_dir_size_sums_tree() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("d")).await.unwrap();
        fs::write(td_l.path().join("d/a"), b"hello").await.unwrap(); // 5
        fs::create_dir(td_l.path().join("d/sub")).await.unwrap();
        fs::write(td_l.path().join("d/sub/b"), b"hi").await.unwrap(); // 2
        let mut app = make_app(&td_l, &td_r).await;
        // Cursor on "d" (only dir; NameAsc → first entry).
        let evs = app.dispatch(Command::RecursiveDirSize).await.unwrap();
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::Status(s) if s.contains("7 bytes"))),
            "expected 7 bytes total, got {evs:?}"
        );
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
    async fn toggle_split_orientation_cycles_horizontal_vertical() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.split_orient(), SplitOrient::Horizontal);
        app.dispatch(Command::ToggleSplitOrientation).await.unwrap();
        assert_eq!(app.split_orient(), SplitOrient::Vertical);
        app.dispatch(Command::ToggleSplitOrientation).await.unwrap();
        assert_eq!(app.split_orient(), SplitOrient::Horizontal);
    }

    #[tokio::test]
    async fn toggle_panel_filter_clears_existing_filter() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        {
            let p = app.active_pane_state();
            assert!(p.filter.is_none());
        }
        // Manually plant a filter, then dispatch the toggle to clear.
        // (Setting via dispatch requires the prompt dialog, deferred.)
        // We reach in by mutating App's internal panes via dispatch is
        // not possible without exposing more state, so just test the
        // toggle clears when nothing's set (idempotent).
        app.dispatch(Command::TogglePanelFilter).await.unwrap();
        assert!(app.active_pane_state().filter.is_none());
    }

    #[tokio::test]
    async fn sync_other_panel_path_copies_other_pane_cwd_into_active() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_r.path().join("over-there"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Active = Left initially. Sync into left from right.
        app.dispatch(Command::SyncOtherPanelPath).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, app.pane(PaneId::Right).cwd);
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 1);
    }

    #[tokio::test]
    async fn show_focused_in_other_panel_navigates_other_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        tokio::fs::write(td_l.path().join("sub/x"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Cursor on "sub" — but it's the only entry, so cursor=0 hits it.
        app.dispatch(Command::ShowFocusedInOtherPanel)
            .await
            .unwrap();
        // Right pane (other) now shows sub/'s contents.
        assert!(app.pane(PaneId::Right).cwd.display().ends_with("/sub"));
        assert_eq!(app.pane(PaneId::Right).listing.entries.len(), 1);
        // Active pane unchanged per FR-014.
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn show_focused_in_other_panel_is_noop_on_file() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_l.path().join("file"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app
            .dispatch(Command::ShowFocusedInOtherPanel)
            .await
            .unwrap();
        // Status event but no PaneUpdated.
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
        assert!(!events.iter().any(|e| matches!(e, Event::PaneUpdated(_))));
    }

    #[tokio::test]
    async fn descend_pushes_back_history_clears_forward() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let initial_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        let p = app.pane(PaneId::Left);
        assert_eq!(p.dir_history_back, vec![initial_cwd]);
        assert!(p.dir_history_fwd.is_empty());
    }

    #[tokio::test]
    async fn history_prev_dir_pops_back_pushes_to_forward() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let initial_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        let sub_cwd = app.pane(PaneId::Left).cwd.clone();

        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        let p = app.pane(PaneId::Left);
        assert_eq!(p.cwd, initial_cwd);
        assert!(p.dir_history_back.is_empty());
        assert_eq!(p.dir_history_fwd, vec![sub_cwd]);
    }

    #[tokio::test]
    async fn history_next_dir_returns_after_prev() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::Descend).await.unwrap();
        let sub_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        app.dispatch(Command::HistoryNextDir).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, sub_cwd);
        assert!(app.pane(PaneId::Left).dir_history_fwd.is_empty());
    }

    #[tokio::test]
    async fn history_prev_dir_with_empty_history_is_noop_with_status() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::HistoryPrevDir).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
    }

    #[tokio::test]
    async fn descend_after_prev_drops_forward_history() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("a")).await.unwrap();
        tokio::fs::create_dir(td_l.path().join("b")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Descend into "a", then back, then descend into "b" — forward
        // should be cleared.
        // (Cursor starts at 0 = "a" since NameAsc.)
        app.dispatch(Command::Descend).await.unwrap();
        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        assert!(!app.pane(PaneId::Left).dir_history_fwd.is_empty());
        // Move cursor to "b" then descend.
        app.dispatch(Command::CursorDown).await.unwrap();
        app.dispatch(Command::Descend).await.unwrap();
        assert!(app.pane(PaneId::Left).dir_history_fwd.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_popup_dispatch_is_noop_in_core() {
        // Feature 038: the UI intercepts Alt-c to open the dialog; reaching
        // core's dispatch is a no-op (no status stub anymore).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::QuickCdPopup).await.unwrap();
        assert!(events.is_empty());
    }

    // ---------- Feature 038: quick-cd resolution + navigation (US1) ----------

    #[tokio::test]
    async fn quick_cd_absolute_path_navigates_active_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let target = format!("{}/sub", td_l.path().to_str().unwrap());
        app.quick_cd(&target).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        // Inactive pane untouched (FR-013).
        assert!(app
            .pane(PaneId::Right)
            .cwd
            .display()
            .ends_with(td_r.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn quick_cd_relative_path_resolves_against_cwd() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.quick_cd("sub").await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
    }

    #[tokio::test]
    async fn quick_cd_dotdot_ascends() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();
        app.quick_cd("sub").await.unwrap();
        app.quick_cd("..").await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, base);
    }

    #[tokio::test]
    async fn quick_cd_trailing_slash_ignored() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.quick_cd("sub/").await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
    }

    #[tokio::test]
    async fn quick_cd_records_history() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();
        app.quick_cd("sub").await.unwrap();
        assert_eq!(app.pane(PaneId::Left).dir_history_back, vec![base]);
    }

    #[tokio::test]
    async fn quick_cd_empty_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let evs = app.quick_cd("   ").await.unwrap();
        assert!(evs.is_empty());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_nonexistent_errors_without_navigating() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let res = app.quick_cd("/no/such/place/xyz123").await;
        assert!(res.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_file_target_errors_without_navigating() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("afile"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let res = app.quick_cd("afile").await;
        assert!(res.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
    }

    // ---------- Feature 038: completion (US2) ----------

    #[tokio::test]
    async fn complete_cd_unique_prefix_single_candidate() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        fs::create_dir(td_l.path().join("docs")).await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("sr").await;
        assert_eq!(c.len(), 1);
        assert!(c[0].ends_with("/src"));
    }

    #[tokio::test]
    async fn complete_cd_multiple_matches_in_sort_order() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for d in ["app", "apple", "apply"] {
            fs::create_dir(td_l.path().join(d)).await.unwrap();
        }
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("app").await;
        assert_eq!(c.len(), 3);
        assert!(c[0].ends_with("/app"));
        assert!(c[1].ends_with("/apple"));
        assert!(c[2].ends_with("/apply"));
    }

    #[tokio::test]
    async fn complete_cd_excludes_files() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("data")).await.unwrap();
        fs::write(td_l.path().join("database"), b"x").await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("dat").await;
        assert_eq!(c.len(), 1);
        assert!(c[0].ends_with("/data"));
    }

    #[tokio::test]
    async fn complete_cd_recent_dir_ordered_first() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("alpha")).await.unwrap();
        fs::create_dir(td_l.path().join("apex")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Visit alpha then ascend, so dir_history_back contains [base, alpha].
        app.quick_cd("alpha").await.unwrap();
        app.quick_cd("..").await.unwrap();
        let c = app.complete_cd("a").await;
        // alpha is recent → first; apex is filesystem-only → after; deduped.
        assert_eq!(c.len(), 2, "got {c:?}");
        assert!(c[0].ends_with("/alpha"), "recent dir must lead: {c:?}");
        assert!(c[1].ends_with("/apex"), "{c:?}");
    }

    #[tokio::test]
    async fn complete_cd_no_match_is_empty() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert!(app.complete_cd("zzz").await.is_empty());
    }

    /// SC-006 injected-input gate (T1.25 origin): drive the full quick-cd
    /// flow against the engine — complete → accept (success), a cancel
    /// path (read-only, zero side effects), and error-recovery (bad path
    /// rejected without mutation, then a valid accept succeeds).
    #[tokio::test]
    async fn quick_cd_end_to_end_complete_accept_cancel_and_recover() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();

        // --- Cancel path: completing is read-only; not accepting leaves
        // both panes untouched (SC-004).
        let candidates = app.complete_cd("sr").await;
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].ends_with("/src"));
        assert_eq!(
            app.pane(PaneId::Left).cwd,
            base,
            "complete_cd mutated state"
        );
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());

        // --- Error-recovery: accept a bad path → Err, no nav; then a
        // valid accept → Ok (SC-005 then SC-001).
        let bad = app.quick_cd("/no/such/dir/zzz").await;
        assert!(bad.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, base, "bad accept navigated");

        // --- Accept the completed candidate → active pane moves (SC-001),
        // previous cwd recorded (FR-005), other pane unchanged (FR-013).
        let right_before = app.pane(PaneId::Right).cwd.clone();
        app.quick_cd(&candidates[0]).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/src"));
        assert_eq!(app.pane(PaneId::Left).dir_history_back, vec![base]);
        assert_eq!(app.pane(PaneId::Right).cwd, right_before);
    }

    #[tokio::test]
    async fn show_tasks_panel_emits_status_with_transfer_count() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::ShowTasksPanel).await.unwrap();
        let has_status = events.iter().any(|e| match e {
            Event::Status(s) => s.contains("0 active"),
            _ => false,
        });
        assert!(
            has_status,
            "expected '0 active transfer' status, got {events:?}"
        );
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

    // =================================================================
    // Feature 037 — resume-on-launch seam (T005, T007, T008, T013, T014)
    // =================================================================

    use sha2::{Digest, Sha256};

    fn file_uri(p: &std::path::Path) -> String {
        format!("file://{}", p.to_str().unwrap())
    }

    /// Stage a genuinely-resumable checkpoint: write the full source, a
    /// partial destination (first `bytes_written` bytes), and a matching
    /// sidecar in `dst_dir`. Returns the destination file path. The
    /// resulting offer validates (`source_unchanged` + `dest_intact`).
    async fn stage_checkpoint(
        src_dir: &std::path::Path,
        dst_dir: &std::path::Path,
        name: &str,
        full: &[u8],
        bytes_written: usize,
        interval: usize,
    ) -> std::path::PathBuf {
        assert!(
            bytes_written % interval == 0,
            "checkpoint at interval boundary"
        );
        let src = src_dir.join(name);
        let dst = dst_dir.join(name);
        fs::write(&src, full).await.unwrap();
        fs::write(&dst, &full[..bytes_written]).await.unwrap();

        let prefix_len = full.len().min(1024 * 1024);
        let mut h = Sha256::new();
        h.update(&full[..prefix_len]);
        let src_sha256_prefix: [u8; 32] = h.finalize().into();

        let chunk_crcs: Vec<u32> = full[..bytes_written]
            .chunks(interval)
            .map(crc32fast::hash)
            .collect();

        let cp = cargonaut_transfer::TransferCheckpoint {
            version: cargonaut_transfer::TransferCheckpoint::VERSION,
            job_id: "11111111-1111-4111-8111-111111111111".into(),
            src_uri: file_uri(&src),
            src_size: full.len() as u64,
            src_sha256_prefix,
            dst_uri: file_uri(&dst),
            bytes_written: bytes_written as u64,
            chunk_crcs,
            chunk_size_bytes: interval as u64,
            created_at: 0,
            last_update_at: 0,
        };
        let sidecar = dst_dir.join(format!(".cargonaut-transfer-{}.json", cp.job_id));
        fs::write(&sidecar, serde_json::to_vec(&cp).unwrap())
            .await
            .unwrap();
        dst
    }

    async fn wait_completed(app: &App, id: TransferId) -> TransferState {
        let mut rx = app.transfer(id).unwrap().state.clone();
        tokio::time::timeout(std::time::Duration::from_secs(30), async {
            loop {
                {
                    let s = rx.borrow();
                    if matches!(
                        *s,
                        TransferState::Completed { .. }
                            | TransferState::Failed { .. }
                            | TransferState::Canceled
                    ) {
                        return s.clone();
                    }
                }
                if rx.changed().await.is_err() {
                    return TransferState::Failed {
                        error: "sender dropped".into(),
                        resumable: false,
                    };
                }
            }
        })
        .await
        .expect("transfer did not terminate in 30s")
    }

    // ---- T005: projection scaffolding ----

    #[tokio::test]
    async fn pending_resume_views_empty_on_fresh_app() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert!(app.pending_resume_views().is_empty());
    }

    // ---- T007: scan_resume_offers ----

    #[tokio::test]
    async fn scan_finds_offer_in_a_pane_dir() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full = vec![0xABu8; 4096];
        stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 2048, 1024).await;
        // Right pane is the destination dir holding the sidecar.
        let mut app = make_app(&td_src, &td_dst).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert_eq!(offers.len(), 1, "expected one resumable offer");
        assert_eq!(app.pending_resume_views().len(), 1);
        assert!(offers[0].source_unchanged && offers[0].dest_intact);
    }

    #[tokio::test]
    async fn scan_finds_nothing_when_no_sidecars() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("plain.txt"), b"hi")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert!(offers.is_empty());
        assert!(app.pending_resume_views().is_empty());
    }

    #[tokio::test]
    async fn scan_ignores_malformed_sidecar() {
        // FR-010: a garbage sidecar must not error or appear as an offer.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(
            td_r.path().join(".cargonaut-transfer-bogus.json"),
            b"{ not valid json ",
        )
        .await
        .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let offers = app.scan_resume_offers().await.unwrap();
        assert!(
            offers.is_empty(),
            "malformed sidecar must not yield an offer"
        );
    }

    // ---- T008: resume_offer ----

    #[tokio::test]
    async fn resume_offer_completes_and_matches_source() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.resume_offer(0).await.unwrap();
        let id = match events.first() {
            Some(Event::TransferProgressed(id)) => *id,
            other => panic!("expected TransferProgressed, got {other:?}"),
        };
        assert!(app.pending_resume_views().is_empty(), "offer consumed");

        let final_state = wait_completed(&app, id).await;
        assert!(
            matches!(final_state, TransferState::Completed { sha256_match: true }),
            "expected Completed{{sha256_match:true}}, got {final_state:?}"
        );
        assert_eq!(fs::read(&dst).await.unwrap(), full, "dst must equal src");
    }

    #[tokio::test]
    async fn resume_offer_fails_safe_on_changed_destination() {
        // FR-009 / SC-005: if the partial destination no longer matches the
        // checkpoint, resume must refuse rather than corrupt it.
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;
        // Corrupt the partial destination after staging.
        fs::write(&dst, vec![0xFFu8; 4096]).await.unwrap();

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.resume_offer(0).await.unwrap();
        // No successful transfer was registered; a status explains why.
        assert!(
            app.transfer_ids().is_empty(),
            "no transfer should be registered on a fail-safe refusal"
        );
        assert!(
            events.iter().any(|e| matches!(e, Event::Status(_))),
            "expected a status message, got {events:?}"
        );
    }

    // ---- T013: start_over_offer ----

    #[tokio::test]
    async fn start_over_discards_checkpoint_and_copies_fresh() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full: Vec<u8> = (0..8192u32).map(|i| (i % 251) as u8).collect();
        let dst =
            stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 4096, 1024).await;
        let sidecar = td_dst
            .path()
            .join(".cargonaut-transfer-11111111-1111-4111-8111-111111111111.json");
        assert!(sidecar.exists());

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        let events = app.start_over_offer(0).await.unwrap();
        let id = match events.first() {
            Some(Event::TransferProgressed(id)) => *id,
            other => panic!("expected TransferProgressed, got {other:?}"),
        };
        assert!(
            !sidecar.exists(),
            "start over must remove the stale sidecar"
        );
        assert!(app.pending_resume_views().is_empty());

        let final_state = wait_completed(&app, id).await;
        assert!(matches!(final_state, TransferState::Completed { .. }));
        assert_eq!(fs::read(&dst).await.unwrap(), full);
    }

    // ---- T014: skip_offer ----

    #[tokio::test]
    async fn skip_offer_starts_nothing_and_keeps_sidecar() {
        let td_src = TempDir::new().unwrap();
        let td_dst = TempDir::new().unwrap();
        let full = vec![0x33u8; 4096];
        stage_checkpoint(td_src.path(), td_dst.path(), "big.bin", &full, 2048, 1024).await;
        let sidecar = td_dst
            .path()
            .join(".cargonaut-transfer-11111111-1111-4111-8111-111111111111.json");

        let mut app = make_app(&td_src, &td_dst).await;
        app.scan_resume_offers().await.unwrap();
        app.skip_offer(0);
        assert!(app.transfer_ids().is_empty(), "skip starts no transfer");
        assert!(
            app.pending_resume_views().is_empty(),
            "offer dropped from memory"
        );
        assert!(sidecar.exists(), "skip leaves the sidecar on disk");

        // A fresh scan re-discovers the skipped transfer.
        let offers = app.scan_resume_offers().await.unwrap();
        assert_eq!(
            offers.len(),
            1,
            "skipped transfer is offered again next launch"
        );
    }
}

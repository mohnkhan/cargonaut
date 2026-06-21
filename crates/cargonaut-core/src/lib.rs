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

pub(crate) use cargonaut_transfer::{
    resume_transfer, scan_resumable, submit_transfer, ResumableTransfer, TransferError,
    TransferJob, TransferOptions, TransferState,
};
/// Re-exported transfer identity + mode so UI layers can name jobs without
/// depending on `cargonaut-transfer` directly (mirrors the existing
/// projection seam — see [`JobView`], [`ProgressView`]).
pub use cargonaut_transfer::{TransferId, TransferMode};
// Feature 042 — re-export hotlist types so the UI layer can name them without a
// direct `cargonaut-config` dependency (mirrors the transfer-type re-exports).
pub use cargonaut_config::{Bookmark, Hotlist};
pub(crate) use cargonaut_vfs::{
    DirListing, LocalFs, Sort, VfsBackend, VfsError, VfsPath, VfsRegistry,
};
pub(crate) use globset::{GlobBuilder, GlobMatcher};
pub(crate) use std::collections::{BTreeSet, HashMap, HashSet};
pub(crate) use std::sync::Arc;
pub(crate) use thiserror::Error;

// Feature 059 — implementation split into cohesive submodules.
mod app;
mod attrs;
mod command;
mod compare;
mod error;
mod fsops;
mod history;
mod hotlist;
mod jobs;
mod nav;
mod pane;
mod rename;
mod tabs;
#[cfg(test)]
mod test_support;
mod transfers;

pub use command::{Command, DialogKind, Event};
pub use error::{AppError, UndoEntry};
pub use jobs::{transfer_state_snapshot, JobStatus, JobView, ProgressView, ResumeOfferView};
pub use pane::{
    glob_match, FocusedRow, PaneFilter, PaneId, PaneState, SplitOrient, TabBarEntry, ViewMode,
};
pub use rename::validate_rename_proposals;

#[allow(unused_imports)]
pub(crate) use attrs::{attr_status, recursive_status, RECURSE_NODE_CAP};
#[allow(unused_imports)]
pub(crate) use jobs::{crc32_partial, job_status_from, resume_offer_view};
#[allow(unused_imports)]
pub(crate) use nav::{next_sort_key, parse_path, sort_label};
#[allow(unused_imports)]
pub(crate) use pane::pane_idx;

/// Holds all tab state for one pane side (left or right). Private to
/// `cargonaut-core`; callers always access the active tab via [`App::pane`].
///
/// Invariants (enforced by all mutation methods):
/// - `tabs.len() >= 1` at all times
/// - `active_tab < tabs.len()` at all times
#[derive(Debug, Clone)]
struct SideState {
    /// Ordered list of directory tabs. Always non-empty.
    tabs: Vec<PaneState>,
    /// Index of the currently visible tab.
    active_tab: usize,
}

/// Application root. Owns config + two pane sides (each with a tab list) +
/// transfer registry + active-dialog state. Dispatch is async because some
/// commands (cd, copy) call into the VFS / transfer engine.
pub struct App {
    config: cargonaut_config::Config,
    /// Per-side tab state. Index 0 = Left, 1 = Right (via [`pane_idx`]).
    sides: [SideState; 2],
    active: PaneId,
    /// Feature 057 — scheme+authority dispatch for all VFS backends.
    registry: Arc<VfsRegistry>,
    transfers: HashMap<TransferId, TransferJob>,
    /// IDs in submit order — used by `CancelCurrentTransfer`.
    transfer_order: Vec<TransferId>,
    /// Feature 039 — ids the user has paused (vs. cancelled). Source of
    /// truth for the [`JobStatus::Paused`] classification and for resume
    /// eligibility; the engine's `TransferState` can't express "paused".
    paused: HashSet<TransferId>,
    /// Feature 037 — resumable transfers discovered on launch, in scan
    /// order. Drained one at a time as the user answers the resume prompt.
    pending_resumes: Vec<ResumableTransfer>,
    status: String,
    split: SplitOrient,
    view_mode: ViewMode,
    /// Feature 042 — the directory hotlist, loaded at construction and
    /// persisted to `hotlist_path` on every add/remove.
    hotlist: cargonaut_config::Hotlist,
    /// Feature 042 — where the hotlist is persisted (resolved at construction;
    /// overridable in tests).
    hotlist_path: std::path::PathBuf,
    /// Feature 050 — the most recent reversible file operation, or `None`
    /// if nothing is undoable (session start, after undo, or after Delete).
    undo_log: Option<UndoEntry>,
}

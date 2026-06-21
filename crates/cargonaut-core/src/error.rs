// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `error` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

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

    /// A filter pattern failed to compile as a glob (FR-006).
    #[error("bad filter: {0}")]
    BadFilter(String),

    /// A bookmark could not be created (e.g. blank name) — Feature 042.
    #[error("bad bookmark: {0}")]
    BadBookmark(String),

    /// A file-attribute request was invalid (bad mode/owner/link input) —
    /// Feature 043.
    #[error("bad attribute: {0}")]
    BadAttr(String),
}

/// Feature 050 — a single reversible file-operation recorded by the App.
/// The undo log holds at most one entry; it is overwritten on each new
/// operation that supports undo. Session-scoped and non-persistent.
#[derive(Debug, Clone)]
pub enum UndoEntry {
    /// One or more `std::fs::rename` calls: `(old_name, new_name)` pairs.
    /// The undo reverses each pair (`new_name → old_name`), all in the
    /// same directory as the active pane at the time of the rename.
    Rename {
        /// Local filesystem path for the rename directory (e.g. `/tmp/foo`).
        /// Stored as a plain String (not VfsPath) to keep the variant small.
        dir: String,
        /// `(new_name, old_name)` — already reversed so undo just iterates.
        pairs: Vec<(String, String)>,
    },
    /// Copies submitted to the transfer engine: destination paths to delete.
    Copy {
        /// Absolute `file://` paths of the copies to remove on undo.
        copies: Vec<VfsPath>,
    },
    /// Move operations (scaffold — not yet populated in Feature 050).
    Move {
        /// `(destination, source)` — reversed pairs for undo.
        pairs: Vec<(VfsPath, VfsPath)>,
    },
    /// Delete operations — cannot be undone.
    Delete,
}

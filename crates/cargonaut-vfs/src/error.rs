// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Unified VFS error type.

use thiserror::Error;

/// Errors any VFS backend may return.
#[derive(Debug, Error)]
pub enum VfsError {
    /// Path does not exist.
    #[error("not found: {0}")]
    NotFound(String),

    /// Permission denied at the OS / remote layer.
    #[error("permission denied: {0}")]
    PermissionDenied(String),

    /// Caller asked for a capability the backend doesn't support.
    #[error("unsupported operation: {0}")]
    Unsupported(&'static str),

    /// I/O error from the underlying OS or network.
    #[error("io error: {0}")]
    Io(#[from] std::io::Error),

    /// Authentication or credential failure (remote backends).
    #[error("auth failed: {0}")]
    AuthFailed(String),

    /// Other backend-specific error.
    #[error("backend error: {0}")]
    Other(String),
}

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut TUI layer — ratatui rendering, keymap dispatcher,
//! pane/dialog/status-bar widgets.

#![warn(missing_docs)]

/// Run the TUI event loop, dispatching key events to the App and rendering
/// state changes. T1.07 + T1.17 + T1.18 + T1.19 implement.
///
/// Returns when the user presses F10 (quit) or the App emits a fatal error.
pub async fn run(_app: cargonaut_core::App) -> Result<(), Error> {
    unimplemented!("T1.07 — see design/tasks.md")
}

/// TUI errors.
#[derive(Debug, thiserror::Error)]
pub enum Error {
    /// Terminal could not be put into raw mode (no TTY?).
    #[error("terminal: {0}")]
    Terminal(#[from] std::io::Error),
}

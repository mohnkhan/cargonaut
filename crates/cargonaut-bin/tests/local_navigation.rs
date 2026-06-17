// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Feature 045 — PTY binary-level navigation smoke tests (T1.07, issue #30).
//
// Launches the real `cargonaut` binary under a PTY and asserts that keyboard
// navigation sequences produce the expected TUI output changes:
//
//   nav_cursor_arrow_keys — Down/Up keys move the cursor through the listing.
//   nav_descend_enter     — Enter on a subdirectory changes the active pane CWD.
//   nav_ascend_backspace  — Backspace returns the pane to the parent directory.
//
// Gated behind `CARGONAUT_PTY_TESTS=1` (same as the resume smoke test from
// Feature 037). CI sets the flag. Unix-only.

#![cfg(unix)]

#[path = "common/mod.rs"]
mod common;
use common::*;

use std::io::Write;
use std::time::Duration;

#[test]
fn nav_cursor_arrow_keys() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }
    assert!(false, "not yet implemented: nav_cursor_arrow_keys");
}

#[test]
fn nav_descend_enter() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }
    assert!(false, "not yet implemented: nav_descend_enter");
}

#[test]
fn nav_ascend_backspace() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }
    assert!(false, "not yet implemented: nav_ascend_backspace");
}

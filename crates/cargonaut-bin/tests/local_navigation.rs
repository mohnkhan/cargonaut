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
//
// Cursor start position: `default_cursor()` lands on the first real entry
// ("aaa"), not the ".." parent row. One Down moves to the second entry ("bbb"),
// a second Down to the third ("ccc"), and one Up retreats back to "bbb".

#![cfg(unix)]

#[path = "common/mod.rs"]
mod common;
use common::*;

use std::io::Write;
use std::time::Duration;

fn make_fixture() -> (tempfile::TempDir, tempfile::TempDir) {
    let left = tempfile::tempdir().unwrap();
    let right = tempfile::tempdir().unwrap();
    std::fs::create_dir(left.path().join("aaa")).unwrap();
    std::fs::create_dir(left.path().join("bbb")).unwrap();
    std::fs::create_dir(left.path().join("ccc")).unwrap();
    (left, right)
}

/// Send F10 and wait for the binary to exit; kill it if it does not exit
/// within the deadline.
fn quit_and_cleanup(
    mut child: Box<dyn portable_pty::Child + Send + Sync>,
    w: &mut Box<dyn Write + Send>,
    pid: u32,
) {
    w.write_all(KEY_F10).unwrap();
    w.flush().unwrap();
    // Block in a thread so we can apply a deadline without try_wait.
    let (tx, rx) = std::sync::mpsc::channel::<()>();
    std::thread::spawn(move || {
        let _ = child.wait();
        let _ = tx.send(());
    });
    if rx.recv_timeout(Duration::from_secs(5)).is_err() {
        sigkill(pid);
    }
}

/// US1: Arrow keys move the cursor through the listing.
///
/// The TUI cursor starts at "aaa" (the first real entry; `default_cursor()`
/// skips the synthetic ".." row). Each Down advance is observed by checking
/// the delta buffer for the newly-highlighted entry name.
#[test]
fn nav_cursor_arrow_keys() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }

    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let (left, right) = make_fixture();
    let (child, mut w, sink) = spawn(exe, left.path(), right.path());
    let pid = child.process_id().unwrap();

    // Wait for the TUI to render its first frame (function-key bar "Quit" label).
    assert!(
        wait_until(Duration::from_secs(5), || output_contains(&sink, "Quit")),
        "TUI did not start within 5s"
    );

    // Cursor starts at "aaa" (default_cursor = first real entry).
    // Down → cursor moves from aaa to bbb.
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_DOWN).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "bbb"
        )),
        "cursor did not reach bbb after first Down (started at aaa)"
    );

    // Down → cursor advances from bbb to ccc.
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_DOWN).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "ccc"
        )),
        "cursor did not reach ccc after second Down"
    );

    // Up → cursor retreats from ccc to bbb.
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_UP).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "bbb"
        )),
        "cursor did not retreat to bbb after Up"
    );

    quit_and_cleanup(child, &mut w, pid);
}

/// US2: Enter key descends into a subdirectory (CWD change visible in pane title).
///
/// Cursor starts at "aaa". One Down moves to "bbb"; Enter descends into "bbb"
/// and the pane title reflects the new path (contains "bbb").
#[test]
fn nav_descend_enter() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }

    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let (left, right) = make_fixture();
    let (child, mut w, sink) = spawn(exe, left.path(), right.path());
    let pid = child.process_id().unwrap();

    assert!(
        wait_until(Duration::from_secs(5), || output_contains(&sink, "Quit")),
        "TUI did not start within 5s"
    );

    // Cursor starts at "aaa". One Down moves to "bbb".
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_DOWN).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "bbb"
        )),
        "cursor did not reach bbb"
    );

    // Enter → descend into bbb; pane title should contain "bbb".
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_ENTER).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "bbb"
        )),
        "pane did not descend into bbb after Enter"
    );

    quit_and_cleanup(child, &mut w, pid);
}

/// US3: Backspace while inside a subdirectory ascends to the parent (CWD change).
///
/// Cursor starts at "aaa". Enter descends into "aaa" (which is an empty dir
/// — only ".." is visible inside). Backspace returns to the parent directory.
/// The ascent is detected by "bbb" and "ccc" appearing in the delta: those
/// entries were absent from the empty "aaa" listing but become visible once
/// the pane is back in the parent directory. Ratatui's differential renderer
/// re-emits them as new rows when the listing changes completely.
#[test]
fn nav_ascend_backspace() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run PTY navigation tests");
        return;
    }

    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let (left, right) = make_fixture();
    let (child, mut w, sink) = spawn(exe, left.path(), right.path());
    let pid = child.process_id().unwrap();

    assert!(
        wait_until(Duration::from_secs(5), || output_contains(&sink, "Quit")),
        "TUI did not start within 5s"
    );

    // Cursor starts at "aaa". Enter descends into "aaa".
    // "aaa" appears in the delta via the pane title showing the new CWD.
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_ENTER).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || delta_contains(
            &sink, prev, "aaa"
        )),
        "pane did not descend into aaa after Enter"
    );

    // Backspace → ascend. "aaa" is empty so "bbb" and "ccc" only appear in the
    // parent listing. Their presence in new bytes confirms the pane is back in
    // the parent directory. (The pane title prefix is unchanged by the
    // differential renderer, so left_name is not a reliable signal here.)
    let prev = sink.lock().unwrap().len();
    w.write_all(KEY_BACKSPACE).unwrap();
    w.flush().unwrap();
    assert!(
        wait_until(Duration::from_secs(5), || {
            delta_contains(&sink, prev, "bbb") && delta_contains(&sink, prev, "ccc")
        }),
        "pane did not ascend back to parent after Backspace (bbb/ccc not in new bytes)"
    );

    quit_and_cleanup(child, &mut w, pid);
}

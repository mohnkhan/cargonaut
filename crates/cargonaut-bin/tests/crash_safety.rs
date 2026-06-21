// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 061 — gated PTY crash-safety integration test (SC-001, SC-002).
//!
//! Spawns the real binary under a pseudo-terminal with
//! `CARGONAUT_PANIC_INJECT=render`. The render recovery boundary catches the
//! injected panic on three consecutive frames, then escalates to a clean fatal
//! exit (research R7). We then assert:
//!
//! - the terminal was restored — the PTY output contains the leave-alternate-
//!   screen sequence emitted by teardown (SC-001 proxy: the shell is not left in
//!   the alternate screen / raw mode);
//! - a crash report was written with version, platform, location, and a
//!   backtrace (SC-002), into an isolated `XDG_DATA_HOME`.
//!
//! Gated behind `CARGONAUT_PTY_TESTS=1` so plain `cargo test` stays fast; CI
//! sets it (same pattern as Feature 037).

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn pty_enabled() -> bool {
    std::env::var("CARGONAUT_PTY_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

#[test]
fn fatal_render_panic_restores_terminal_and_writes_report() {
    if !pty_enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run crash-safety PTY test");
        return;
    }

    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let data = std::env::temp_dir().join(format!("cargonaut-crash-it-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&data);
    std::fs::create_dir_all(&data).unwrap();

    let pty = native_pty_system();
    let pair = pty
        .openpty(PtySize {
            rows: 40,
            cols: 120,
            pixel_width: 0,
            pixel_height: 0,
        })
        .unwrap();

    let mut cmd = CommandBuilder::new(exe);
    cmd.arg(data.as_os_str()); // left pane (any readable dir)
    cmd.arg(data.as_os_str()); // right pane
    cmd.env("TERM", "xterm-256color");
    cmd.env("CARGONAUT_PANIC_INJECT", "render");
    cmd.env("XDG_DATA_HOME", data.as_os_str());
    cmd.env("HOME", data.as_os_str());

    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);

    // Drain the PTY master so the child never blocks on a full buffer.
    let mut reader = pair.master.try_clone_reader().unwrap();
    let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink2 = Arc::clone(&sink);
    std::thread::spawn(move || {
        let mut buf = [0u8; 4096];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => sink2.lock().unwrap().extend_from_slice(&buf[..n]),
            }
        }
    });

    // Wait for the child to exit (it should die quickly after 3 render panics).
    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("binary did not exit within 15s after injected render panic");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    // Non-zero exit for a fatal crash.
    assert!(!status.success(), "expected non-zero exit on fatal panic");

    // SC-001: teardown restored the terminal (left the alternate screen).
    let output = String::from_utf8_lossy(&sink.lock().unwrap()).into_owned();
    assert!(
        output.contains("\u{1b}[?1049l"),
        "terminal not restored: missing leave-alternate-screen sequence"
    );

    // SC-002: a crash report exists with the required fields.
    let report = std::fs::read_dir(data.join("cargonaut"))
        .unwrap()
        .filter_map(|e| e.ok())
        .map(|e| e.path())
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .map(|n| n.starts_with("crash-") && n.ends_with(".log"))
                .unwrap_or(false)
        })
        .expect("a crash-*.log must be written");
    let body = std::fs::read_to_string(&report).unwrap();
    for needle in [
        "version:",
        "platform:",
        "## Panic",
        "location:",
        "## Backtrace",
    ] {
        assert!(
            body.contains(needle),
            "crash report missing {needle:?}\n{body}"
        );
    }

    let _ = std::fs::remove_dir_all(&data);
}

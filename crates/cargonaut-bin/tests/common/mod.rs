// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Shared helpers for PTY integration tests (nav smoke + resume smoke).
// Named mod.rs inside a common/ subdirectory so Cargo does not treat
// this file as a standalone test binary root.

#![allow(dead_code)]

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

pub const KEY_DOWN: &[u8] = b"\x1b[B";
pub const KEY_UP: &[u8] = b"\x1b[A";
pub const KEY_ENTER: &[u8] = b"\r";
pub const KEY_BACKSPACE: &[u8] = b"\x7f";
pub const KEY_F10: &[u8] = b"\x1b[21~";

/// Active cargonaut process under a PTY: child handle, PTY master writer,
/// and the shared output buffer drained by a background thread.
pub type PtyHandle = (
    Box<dyn portable_pty::Child + Send + Sync>,
    Box<dyn Write + Send>,
    Arc<Mutex<Vec<u8>>>,
);

/// Returns true when the `CARGONAUT_PTY_TESTS` env var is set to `"1"`.
pub fn enabled() -> bool {
    std::env::var("CARGONAUT_PTY_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Spawn the cargonaut binary under a real PTY. A background thread drains
/// the PTY master so the child never blocks on a full output buffer.
pub fn spawn(exe: &str, left: &Path, right: &Path) -> PtyHandle {
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
    cmd.arg(left.as_os_str());
    cmd.arg(right.as_os_str());
    cmd.env("TERM", "xterm-256color");

    let child = pair.slave.spawn_command(cmd).unwrap();
    let mut reader = pair.master.try_clone_reader().unwrap();
    let writer = pair.master.take_writer().unwrap();

    let sink = Arc::new(Mutex::new(Vec::<u8>::new()));
    let sink2 = Arc::clone(&sink);
    std::thread::spawn(move || {
        let _keep = pair; // hold master/slave open until the child closes the PTY
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => sink2.lock().unwrap().extend_from_slice(&buf[..n]),
            }
        }
    });
    (child, writer, sink)
}

/// True if the entire accumulated PTY output contains `needle`.
pub fn output_contains(sink: &Arc<Mutex<Vec<u8>>>, needle: &str) -> bool {
    let guard = sink.lock().unwrap();
    String::from_utf8_lossy(&guard).contains(needle)
}

/// True if new bytes written after `prev_len` contain `needle`.
/// Used to isolate assertions to bytes generated since the last action,
/// preventing false positives from the cumulative output buffer.
pub fn delta_contains(sink: &Arc<Mutex<Vec<u8>>>, prev_len: usize, needle: &str) -> bool {
    let guard = sink.lock().unwrap();
    let start = prev_len.min(guard.len());
    String::from_utf8_lossy(&guard[start..]).contains(needle)
}

/// Poll `cond` every 50 ms until it returns `true` or `deadline` elapses.
pub fn wait_until<F: Fn() -> bool>(deadline: Duration, cond: F) -> bool {
    let start = Instant::now();
    while start.elapsed() < deadline {
        if cond() {
            return true;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    false
}

/// Send SIGKILL to the given process ID. Last-resort cleanup if the binary
/// does not exit cleanly within a test deadline.
pub fn sigkill(pid: u32) {
    let _ = std::process::Command::new("kill")
        .args(["-9", &pid.to_string()])
        .status();
}

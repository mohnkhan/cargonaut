// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 061/062 — gated PTY crash-safety integration tests (SC-001, SC-002).
//!
//! Spawns the real binary under a pseudo-terminal with a `CARGONAUT_PANIC_INJECT`
//! site. Both the render (Feature 061) and input (Feature 062) recovery
//! boundaries recover the injected panic three times, then escalate to a clean
//! fatal exit (research R7). We assert the terminal is restored (leave-alternate-
//! screen sequence) and a complete crash report is written.
//!
//! Gated behind `CARGONAUT_PTY_TESTS=1` (CI sets it).

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn pty_enabled() -> bool {
    std::env::var("CARGONAUT_PTY_TESTS")
        .map(|v| v == "1")
        .unwrap_or(false)
}

/// Run the binary under a PTY with the given inject site, optionally sending
/// `keys` (each a byte slice). Returns `(exited_nonzero, pty_output, data_dir)`.
fn run_with_inject(site: &str, keys: &[&[u8]]) -> (bool, String, std::path::PathBuf) {
    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let data = std::env::temp_dir().join(format!("cargonaut-crash-{site}-{}", std::process::id()));
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
    cmd.arg(data.as_os_str());
    cmd.arg(data.as_os_str());
    cmd.env("TERM", "xterm-256color");
    cmd.env("CARGONAUT_PANIC_INJECT", site);
    cmd.env("XDG_DATA_HOME", data.as_os_str());
    cmd.env("HOME", data.as_os_str());

    let mut child = pair.slave.spawn_command(cmd).unwrap();
    drop(pair.slave);

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

    // Send any keystrokes (input-inject needs events to fire the panic).
    if !keys.is_empty() {
        let mut writer = pair.master.take_writer().unwrap();
        for k in keys {
            std::thread::sleep(Duration::from_millis(120));
            let _ = writer.write_all(k);
            let _ = writer.flush();
        }
    }

    let deadline = Instant::now() + Duration::from_secs(15);
    let status = loop {
        if let Some(s) = child.try_wait().unwrap() {
            break s;
        }
        if Instant::now() > deadline {
            let _ = child.kill();
            panic!("binary did not exit within 15s after injected {site} panic");
        }
        std::thread::sleep(Duration::from_millis(50));
    };

    let output = String::from_utf8_lossy(&sink.lock().unwrap()).into_owned();
    (!status.success(), output, data)
}

fn assert_restored_and_reported(output: &str, data: &std::path::Path) {
    // SC-001: teardown left the alternate screen.
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
}

#[test]
fn fatal_render_panic_restores_terminal_and_writes_report() {
    if !pty_enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run crash-safety PTY test");
        return;
    }
    // Render inject fires every frame → 3 recovered → escalate, no keys needed.
    let (nonzero, output, data) = run_with_inject("render", &[]);
    assert!(nonzero, "expected non-zero exit on fatal panic");
    assert_restored_and_reported(&output, &data);
    let _ = std::fs::remove_dir_all(&data);
}

#[test]
fn fatal_input_panic_restores_terminal_and_writes_report() {
    if !pty_enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run crash-safety PTY test");
        return;
    }
    // Input inject fires per key → send 3 Down keys → 3 recovered → escalate.
    const KEY_DOWN: &[u8] = b"\x1b[B";
    let (nonzero, output, data) = run_with_inject("input", &[KEY_DOWN, KEY_DOWN, KEY_DOWN]);
    assert!(nonzero, "expected non-zero exit on fatal input panic");
    assert_restored_and_reported(&output, &data);
    let _ = std::fs::remove_dir_all(&data);
}

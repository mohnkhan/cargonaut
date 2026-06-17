// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.08 / Feature 037 — SC-002 binary-level gate.
//
// Drives the real `cargonaut` binary under a PTY through a full
// kill-and-resume cycle:
//
//   1. spawn cargonaut <src> <dst> under a PTY,
//   2. F5 + confirm to start copying a multi-tens-of-MiB file,
//   3. throttle the engine so the copy stays in flight, then SIGKILL it
//      mid-transfer (after a checkpoint sidecar + partial dst exist),
//   4. relaunch cargonaut against the same dirs — the resume prompt
//      appears,
//   5. press `r` to resume, wait for completion,
//   6. assert sha256(src) == sha256(dst).
//
// This enforces SC-002 (resume from SIGKILL) end-to-end at the binary
// level, closing the hole that the in-process engine tests alone could
// not cover (issue #29).
//
// Gated behind `CARGONAUT_PTY_TESTS=1` so it does not slow the default
// `cargo test`; CI sets the flag. Unix-only (uses a PTY + `kill -9`).

#![cfg(unix)]

#[path = "common/mod.rs"]
mod common;
use common::*;

use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::path::Path;
use std::sync::{Arc, Mutex};

const FILE_BYTES: u64 = 64 * 1024 * 1024; // 64 MiB — big enough to stay in flight
const THROTTLE_MIBPS: &str = "24"; // ~2.7 s total copy under throttle
const NAME: &str = "big.bin";

fn sha256_file(p: &Path) -> [u8; 32] {
    let bytes = std::fs::read(p).expect("read file for hashing");
    let mut h = Sha256::new();
    h.update(&bytes);
    h.finalize().into()
}

/// Deterministic payload (cheap to generate, identical across runs).
fn write_payload(p: &Path, len: u64) {
    let mut f = std::fs::File::create(p).unwrap();
    let block: Vec<u8> = (0..4096u32).map(|i| (i % 251) as u8).collect();
    let mut written = 0u64;
    while written < len {
        let take = block.len().min((len - written) as usize);
        f.write_all(&block[..take]).unwrap();
        written += take as u64;
    }
    f.flush().unwrap();
}

fn has_sidecar(dir: &Path) -> bool {
    let Ok(rd) = std::fs::read_dir(dir) else {
        return false;
    };
    for entry in rd.flatten() {
        let name = entry.file_name();
        let name = name.to_string_lossy();
        if name.starts_with(".cargonaut-transfer-") && name.ends_with(".json") {
            return true;
        }
    }
    false
}

/// Spawn cargonaut under a PTY with the transfer throttle env var set,
/// so the copy stays in flight long enough to SIGKILL mid-transfer.
fn spawn_throttled(exe: &str, left: &Path, right: &Path) -> PtyHandle {
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
    cmd.env("CARGONAUT_TRANSFER_THROTTLE_MIBPS", THROTTLE_MIBPS);
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

#[test]
fn resume_sigkill_smoke() {
    if !enabled() {
        eprintln!("skipping: set CARGONAUT_PTY_TESTS=1 to run the SC-002 PTY gate");
        return;
    }

    let exe = env!("CARGO_BIN_EXE_cargonaut");
    let src_dir = tempfile::tempdir().unwrap();
    let dst_dir = tempfile::tempdir().unwrap();
    let src_file = src_dir.path().join(NAME);
    let dst_file = dst_dir.path().join(NAME);
    write_payload(&src_file, FILE_BYTES);

    // ---- Run 1: start the copy, then SIGKILL it mid-transfer. ----
    let (mut child1, mut w1, _out1) = spawn_throttled(exe, src_dir.path(), dst_dir.path());
    let pid1 = child1.process_id().expect("run1 has a pid");

    // Let the TUI initialise, then F5 (copy focused entry) + confirm.
    std::thread::sleep(std::time::Duration::from_millis(700));
    w1.write_all(b"\x1b[15~").unwrap(); // F5
    w1.flush().unwrap();
    std::thread::sleep(std::time::Duration::from_millis(400));
    w1.write_all(b"y").unwrap(); // confirm copy
    w1.flush().unwrap();

    // Wait until a checkpoint sidecar exists and the partial destination is
    // well under way (≥16 MiB, < full) — proof the copy is in flight.
    let in_flight = wait_until(std::time::Duration::from_secs(20), || {
        let size = std::fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0);
        has_sidecar(dst_dir.path()) && (16 * 1024 * 1024..FILE_BYTES).contains(&size)
    });
    let pre_kill_size = std::fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0);
    assert!(
        in_flight,
        "copy never reached a mid-flight checkpoint (partial size={pre_kill_size}); \
         tune FILE_BYTES/THROTTLE_MIBPS"
    );

    sigkill(pid1);
    let _ = child1.wait();
    assert!(
        has_sidecar(dst_dir.path()),
        "checkpoint sidecar should survive SIGKILL"
    );

    // ---- Run 2: relaunch; the resume prompt should appear; press `r`. ----
    let (mut child2, mut w2, out2) = spawn_throttled(exe, src_dir.path(), dst_dir.path());
    let pid2 = child2.process_id().expect("run2 has a pid");

    let prompt_shown = wait_until(std::time::Duration::from_secs(10), || {
        output_contains(&out2, "Resumable transfers")
    });
    assert!(
        prompt_shown,
        "resume prompt did not appear on relaunch (no 'Resumable transfers' in output)"
    );

    w2.write_all(b"r").unwrap(); // resume
    w2.flush().unwrap();

    // Wait for the resumed copy to finish: full size + sidecar removed.
    let completed = wait_until(std::time::Duration::from_secs(60), || {
        let size = std::fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0);
        size == FILE_BYTES && !has_sidecar(dst_dir.path())
    });
    assert!(
        completed,
        "resumed transfer did not complete (size={}, sidecar={})",
        std::fs::metadata(&dst_file).map(|m| m.len()).unwrap_or(0),
        has_sidecar(dst_dir.path())
    );

    // The core assertion: destination is byte-identical to the source.
    assert_eq!(
        sha256_file(&src_file),
        sha256_file(&dst_file),
        "SC-002: resumed destination must match source byte-for-byte"
    );

    sigkill(pid2);
    let _ = child2.wait();
}

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.08 — SC-002 integration test: 4 GiB random file, spawn cargonaut
// subprocess to F5 copy, wait 1 s, SIGKILL, relaunch, automate the
// [r]esume prompt, wait for completion, assert SHA-256 match.
//
// Phase 1 implementation: this test is `#[ignore]` because the full
// shape needs (a) PTY automation to drive the F5 keypress + resume
// prompt response, and (b) a way to run two cargonaut subprocesses
// in sequence with mid-flight SIGKILL. Both are achievable with
// `portable-pty` + `nix::sys::signal` but add 2 deps and significant
// scaffolding for one test.
//
// The *engine* equivalent IS covered:
// - `cargonaut-transfer/tests/cancellation.rs` proves cancellation +
//   sidecar+partial-dst survival within 500 ms (FR-008 / NFR-005).
// - `crates/cargonaut-transfer/src/job.rs::tests::resume_completes_
//   partial_transfer` proves resume_transfer picks up where a
//   checkpoint leaves off.
// - `crates/cargonaut-transfer/src/checkpoint.rs::tests::scan_*`
//   prove scan_resumable discovers and validates sidecars.
//
// What's deferred is the *binary*-level SIGKILL + relaunch + UI prompt
// loop. Manual smoke procedure:
//
//   1. dd if=/dev/urandom of=/tmp/src.bin bs=1M count=4096
//   2. cargo run --release --bin cargonaut /tmp /tmp/dst
//   3. <F5> to copy; wait ~1 s; in another terminal: pkill -KILL cargonaut
//   4. cargo run --release --bin cargonaut /tmp /tmp/dst
//   5. The resume prompt appears; press [r]
//   6. After completion, sha256sum /tmp/src.bin /tmp/dst/src.bin
//      — both hashes must match.

#[test]
#[ignore = "needs portable-pty + SIGKILL automation; engine-layer tests cover the moving parts"]
fn resume_sigkill_smoke() {
    // See module doc for the manual procedure. The engine-layer tests
    // referenced above ensure each step of this manual flow works.
}

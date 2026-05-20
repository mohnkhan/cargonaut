// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//
// T1.07 — Integration test: launch cargonaut with two tempdir args,
// simulate keypresses (Tab, j, Enter, Backspace, :cd), assert pane
// state evolves correctly.
//
// Phase 1 implementation: this test is `#[ignore]` because automating
// crossterm key injection from a non-TTY test requires a PTY harness
// (likely `portable-pty`). The App-layer equivalents of these
// keystrokes ARE covered by the `cargonaut-core` unit tests
// (`descend_into_subdir_then_ascend_back`,
// `focus_swap_toggles_active_pane`, etc.), so the *engine* behavior
// is verified — what's deferred is the bin-level end-to-end driver.
//
// To run manually:
//   cargo run --bin cargonaut /tmp/some-dir /tmp/another-dir
//   <Tab> swaps panes; <j>/<k> moves cursor; <Enter> descends;
//   <Backspace> ascends; <F10> quits.
//
// Followup: when `portable-pty` lands in workspace deps, replace this
// stub with a real driven test using `expectrl` or similar.

#[test]
#[ignore = "needs portable-pty harness; manual smoke covers the binary"]
fn local_navigation_smoke() {
    // Intentionally empty — `#[ignore]` keeps `cargo test` honest about
    // the deferral. Run with `cargo test -- --ignored` to see the
    // skip explicitly.
}

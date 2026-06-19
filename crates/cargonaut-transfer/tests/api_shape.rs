// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! T040: Compile-only verification that `submit_transfer` already accepts
//! `Arc<dyn VfsBackend>` for both source and destination backends.
//!
//! This test was expected to require a transfer-crate API change; instead it
//! confirms the assumption "transfer engine already supports Arc<dyn VfsBackend>"
//! from Feature 057 research (analysis finding M6).

use cargonaut_transfer::{submit_transfer, TransferOptions};
use cargonaut_vfs::{LocalFs, VfsBackend, VfsPath};
use std::sync::Arc;

/// Compile-only test: verifies `submit_transfer` signature accepts
/// `Arc<dyn VfsBackend>` for both src and dst. The function is never
/// called at runtime (the test completes without executing the closure).
///
/// If the signature ever changes to not accept `Arc<dyn VfsBackend>`,
/// this will fail at compile time — catching the regression early.
#[allow(dead_code)]
fn assert_submit_transfer_accepts_arc_dyn() {
    let local: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let src_path = VfsPath::parse("file:///tmp/src.txt").unwrap();
    let dst_path = VfsPath::parse("file:///tmp/dst.txt").unwrap();
    let _opts = TransferOptions::default();

    // Type-check: confirm submit_transfer accepts (Arc<dyn VfsBackend>, VfsPath, Arc<dyn VfsBackend>, VfsPath, TransferOptions).
    // We use std::future::Future to capture the return type without running it.
    let _future = submit_transfer(
        Arc::clone(&local),
        src_path,
        Arc::clone(&local),
        dst_path,
        _opts,
    );
    // Future is dropped here — never awaited, never polled.
}

#[test]
fn transfer_engine_api_shape_is_correct() {
    // Runtime sanity: the compile-only assertion above verified the signature.
    // This empty test ensures the file is included in `cargo test`.
    let _ = assert_submit_transfer_accepts_arc_dyn as fn();
}

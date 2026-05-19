// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pins down `VfsBackend`'s object-safety + dyn-dispatch contract.
//!
//! `design/data-model.md` defines `VfsRef = (Arc<dyn VfsBackend>, VfsPath)`,
//! and the transfer engine, UI, and audit log all dispatch through that
//! `Arc<dyn VfsBackend>`. If the trait stops being object-safe (e.g. someone
//! adds a generic method or a `Self`-returning method), every caller breaks.
//! This test fails at compile time in that case.

use cargonaut_vfs::{ByteRange, LocalFs, VfsBackend, VfsCaps};
use std::sync::Arc;

const fn _assert_send_sync<T: ?Sized + Send + Sync>() {}
const _: () = {
    _assert_send_sync::<dyn VfsBackend>();
    _assert_send_sync::<VfsCaps>();
    _assert_send_sync::<ByteRange>();
};

#[test]
fn local_fs_round_trips_through_arc_dyn() {
    let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    assert_eq!(backend.scheme(), "file");
    let caps = backend.caps();
    assert!(caps.contains(VfsCaps::SEEKABLE));
    assert!(caps.contains(VfsCaps::RANDOM_WRITE));
    assert!(caps.contains(VfsCaps::ATOMIC_RENAME));
}

#[test]
fn caps_are_stable_per_instance() {
    let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let first = backend.caps();
    for _ in 0..16 {
        assert_eq!(backend.caps(), first, "VfsBackend::caps must be stable");
    }
}

#[test]
fn scheme_is_stable_per_instance() {
    let backend: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
    let first = backend.scheme();
    for _ in 0..16 {
        assert_eq!(backend.scheme(), first);
    }
}

#[test]
fn byte_range_full_constant() {
    assert_eq!(ByteRange::FULL.start, 0);
    assert!(ByteRange::FULL.end.is_none());
}

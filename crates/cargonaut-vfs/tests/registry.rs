// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Integration tests for VfsRegistry — scheme+authority dispatch.
//! T006: external test suite for VfsRegistry (FR-001).

use cargonaut_vfs::VfsPath;
use cargonaut_vfs::{LocalFs, VfsBackend, VfsCaps, VfsRegistry};
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::sync::Arc;

fn local_backend() -> Arc<dyn VfsBackend> {
    Arc::new(LocalFs::new())
}

#[test]
fn file_scheme_resolves_to_local() {
    let reg = VfsRegistry::new(local_backend());
    let path = VfsPath::parse("file:///tmp/foo").unwrap();
    let b = reg.resolve(&path).expect("file:// must always resolve");
    assert_eq!(b.scheme(), "file");
}

#[test]
fn unknown_scheme_resolves_to_none() {
    let reg = VfsRegistry::new(local_backend());
    let path = VfsPath::parse("s3://mybucket/key").unwrap();
    assert!(reg.resolve(&path).is_none());
}

#[test]
fn registered_remote_resolves_correctly() {
    let mut reg = VfsRegistry::new(local_backend());
    let remote: Arc<dyn VfsBackend> = local_backend();
    reg.register_remote("sftp://alice@host:22", Arc::clone(&remote));
    let path = VfsPath {
        scheme: SmolStr::new("sftp"),
        authority: Some(SmolStr::new("alice@host:22")),
        segments: SmallVec::new(),
    };
    let resolved = reg.resolve(&path).expect("registered remote must resolve");
    assert!(Arc::ptr_eq(&resolved, &remote));
}

#[test]
fn re_register_overwrites_previous() {
    let mut reg = VfsRegistry::new(local_backend());
    let first: Arc<dyn VfsBackend> = local_backend();
    let second: Arc<dyn VfsBackend> = local_backend();
    reg.register_remote("sftp://host:22", Arc::clone(&first));
    reg.register_remote("sftp://host:22", Arc::clone(&second));
    let path = VfsPath {
        scheme: SmolStr::new("sftp"),
        authority: Some(SmolStr::new("host:22")),
        segments: SmallVec::new(),
    };
    let resolved = reg.resolve(&path).unwrap();
    assert!(
        Arc::ptr_eq(&resolved, &second),
        "second registration must overwrite first"
    );
}

#[test]
fn local_accessor_returns_seekable_backend() {
    let reg = VfsRegistry::new(local_backend());
    let b = reg.local();
    assert!(
        b.caps().contains(VfsCaps::SEEKABLE),
        "local backend must be seekable"
    );
}

#[test]
fn different_authorities_are_independent() {
    let mut reg = VfsRegistry::new(local_backend());
    let sftp1: Arc<dyn VfsBackend> = local_backend();
    let sftp2: Arc<dyn VfsBackend> = local_backend();
    reg.register_remote("sftp://host1:22", Arc::clone(&sftp1));
    reg.register_remote("sftp://host2:22", Arc::clone(&sftp2));

    let path1 = VfsPath {
        scheme: SmolStr::new("sftp"),
        authority: Some(SmolStr::new("host1:22")),
        segments: SmallVec::new(),
    };
    let path2 = VfsPath {
        scheme: SmolStr::new("sftp"),
        authority: Some(SmolStr::new("host2:22")),
        segments: SmallVec::new(),
    };

    assert!(Arc::ptr_eq(&reg.resolve(&path1).unwrap(), &sftp1));
    assert!(Arc::ptr_eq(&reg.resolve(&path2).unwrap(), &sftp2));
}

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Archive VFS backends — read-only backends for browsing ZIP and TAR
//! archives as if they were directories.
//!
//! Both backends use the same `VfsPath` encoding convention:
//! - `authority` = archive host-filesystem path with `/` → `%2F`
//! - `segments` = in-archive entry path
//!
//! Example: `zip://home%2Fuser%2Farchive.zip/subdir/file.txt`
//!   - authority decoded = `/home/user/archive.zip`
//!   - entry path = `subdir/file.txt`

pub mod zip_fs;
pub mod tar_fs;

pub use zip_fs::ZipFs;
pub use tar_fs::TarFs;

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut VFS — virtual-filesystem abstraction.
//!
//! Defines [`VfsBackend`] (the trait every backend implements) plus the
//! pure data types every adapter speaks: [`VfsPath`], [`VfsMetadata`],
//! [`DirListing`], etc.
//!
//! Phase 1 ships only [`LocalFs`]. Phases 2+ add SFTP, S3, archive, etc.
//! as sibling crates that implement [`VfsBackend`].

#![warn(missing_docs)]

pub mod error;
pub mod local;
pub mod mode;
pub mod traits;
pub mod types;

pub use error::VfsError;
pub use local::LocalFs;
pub use mode::{ModeError, ModeSpec};
pub use traits::{ByteRange, VfsBackend, VfsCaps, WriteMode};
pub use types::{DirEntry, DirListing, FileMode, Sort, VfsKind, VfsMetadata, VfsPath};

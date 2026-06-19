// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Cargonaut VFS — virtual-filesystem abstraction.
//!
//! Defines [`VfsBackend`] (the trait every backend implements) plus the
//! pure data types every adapter speaks: [`VfsPath`], [`VfsMetadata`],
//! [`DirListing`], etc.
//!
//! Feature 057 adds archive backends ([`ZipFs`], [`TarFs`]) and remote
//! backends ([`SftpFs`], [`FtpFs`]), plus a [`VfsRegistry`] for scheme
//! dispatch.

#![warn(missing_docs)]

pub mod error;
pub mod local;
pub mod mode;
pub mod owner;
pub mod registry;
pub mod traits;
pub mod types;

#[cfg(feature = "archives")]
pub mod archive;
#[cfg(feature = "remote")]
pub mod remote;

pub use error::VfsError;
pub use local::LocalFs;
pub use mode::{ModeError, ModeSpec};
pub use owner::{parse_owner, OwnerError};
pub use registry::VfsRegistry;
pub use traits::{ByteRange, VfsBackend, VfsCaps, WriteMode};
pub use types::{DirEntry, DirListing, FileMode, Sort, VfsKind, VfsMetadata, VfsPath};

#[cfg(feature = "archives")]
pub use archive::{TarFs, ZipFs};
#[cfg(feature = "remote")]
pub use remote::{FtpFs, SftpFs};

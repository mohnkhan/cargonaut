//! Cargonaut VFS — virtual-filesystem abstraction.
//!
//! Defines [`VfsBackend`] (the trait every backend implements) plus the
//! pure data types every adapter speaks: [`VfsPath`], [`VfsMetadata`],
//! [`DirListing`], etc.
//!
//! Phase 1 ships only [`LocalFs`]. Phases 2+ add SFTP, S3, archive, etc.
//! as sibling crates that implement [`VfsBackend`].

#![warn(missing_docs)]

pub mod types;
pub mod traits;
pub mod local;
pub mod error;

pub use error::VfsError;
pub use local::LocalFs;
pub use traits::{VfsBackend, VfsCaps, WriteMode, ByteRange, Sort};
pub use types::{VfsPath, VfsMetadata, VfsKind, FileMode, DirListing, DirEntry};

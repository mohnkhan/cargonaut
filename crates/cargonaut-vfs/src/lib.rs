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
pub use archive::{TarCompression, TarFs, ZipFs};
#[cfg(feature = "remote")]
pub use remote::{FtpFs, FtpOps, HostKeyEvent, SftpCredentials, SftpFs, SftpOps};

/// SFTP-specific items, including test helpers.
#[cfg(feature = "remote")]
pub mod sftp {
    pub use crate::remote::sftp_fs::{HostKeyEvent, SftpCredentials, SftpFs, SftpOps};

    /// Test helpers for asserting SFTP security properties (e.g. credential
    /// redaction in log output).
    pub mod testing {
        use std::io;
        use std::sync::{Arc, Mutex};

        /// A writer that captures formatted log lines into a shared `Vec<String>`
        /// for assertions in tests.
        pub struct CaptureWriter {
            captured: Arc<Mutex<Vec<String>>>,
        }

        impl CaptureWriter {
            /// Create a new writer that appends to `captured`.
            pub fn new(captured: Arc<Mutex<Vec<String>>>) -> Self {
                Self { captured }
            }
        }

        impl io::Write for CaptureWriter {
            fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
                if let Ok(s) = std::str::from_utf8(buf) {
                    if let Ok(mut guard) = self.captured.lock() {
                        guard.push(s.to_owned());
                    }
                }
                Ok(buf.len())
            }

            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
    }
}

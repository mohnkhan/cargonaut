//! Pure data types shared across every VFS backend.

use serde::{Deserialize, Serialize};
use smallvec::SmallVec;
use smol_str::SmolStr;
use std::time::SystemTime;

/// A scheme-aware path. `file:///etc`, `sftp://user@host/var/log`, `s3://bucket/key`.
///
/// Round-trip invariant: `VfsPath::display(VfsPath::parse(s)) == s` for all valid `s`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct VfsPath {
    /// e.g. `"file"`, `"sftp"`, `"s3"`.
    pub scheme: SmolStr,
    /// e.g. `Some("user@host:22")` for sftp; `None` for file.
    pub authority: Option<SmolStr>,
    /// Path segments. Never contain `/` or `..`.
    pub segments: SmallVec<[SmolStr; 8]>,
}

impl VfsPath {
    /// Parse a `scheme://authority/seg1/seg2` URI.
    /// Returns [`ParseError`] for malformed input.
    pub fn parse(_s: &str) -> Result<Self, ParseError> {
        // T1.04: implement with proptest round-trip + edge cases.
        unimplemented!("T1.04 — see design/tasks.md")
    }

    /// Render the path back to a URI string.
    pub fn display(&self) -> String {
        unimplemented!("T1.04")
    }

    /// Return the parent path, or `None` if at root.
    pub fn parent(&self) -> Option<Self> {
        unimplemented!("T1.04")
    }

    /// Append a single segment.
    pub fn join(&self, _segment: &str) -> Self {
        unimplemented!("T1.04")
    }
}

/// Errors from parsing a [`VfsPath`].
#[derive(Debug, thiserror::Error)]
pub enum ParseError {
    /// The URI string was malformed.
    #[error("malformed URI: {0}")]
    Malformed(String),
}

/// Permissions + ownership (Unix-style). Some backends may set fields to `None`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FileMode {
    /// rwxrwxrwx bits (low 9 bits of octal mode).
    pub bits: u32,
    /// Unix uid; `None` on backends without ownership.
    pub uid: Option<u32>,
    /// Unix gid.
    pub gid: Option<u32>,
}

/// What kind of filesystem object this is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum VfsKind {
    /// Regular file.
    File,
    /// Directory.
    Dir,
    /// Symbolic link (with its target).
    Symlink {
        /// Target path (may be relative or absolute).
        target: Box<VfsPath>,
    },
    /// Other (FIFO, device, etc.).
    Other,
}

/// Metadata for a single VFS entry.
#[derive(Debug, Clone)]
pub struct VfsMetadata {
    /// File size in bytes (0 for directories).
    pub size: u64,
    /// Last modification time.
    pub mtime: SystemTime,
    /// Permission bits.
    pub mode: Option<FileMode>,
    /// File kind.
    pub kind: VfsKind,
    /// True if the name starts with `.` (Unix convention).
    pub is_hidden: bool,
}

/// One entry in a directory listing.
#[derive(Debug, Clone)]
pub struct DirEntry {
    /// File name (no directory component).
    pub name: SmolStr,
    /// Metadata for this entry.
    pub meta: VfsMetadata,
}

/// A complete directory listing, pre-sorted per [`Sort`].
#[derive(Debug, Clone)]
pub struct DirListing {
    /// Entries in display order.
    pub entries: Vec<DirEntry>,
    /// The sort applied at construction time.
    pub sort: Sort,
}

/// Sort key for [`DirListing`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Sort {
    /// Alphabetical, ascending.
    NameAsc,
    /// Alphabetical, descending.
    NameDesc,
    /// By size, largest first.
    SizeDesc,
    /// By mtime, newest first.
    MtimeDesc,
    /// By extension then name.
    ExtAsc,
}

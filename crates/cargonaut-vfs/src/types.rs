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

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn segment_strategy() -> impl Strategy<Value = SmolStr> {
        "[a-zA-Z0-9._-]{1,12}"
            .prop_map(SmolStr::new)
            .prop_filter("segment must not be \"..\"", |s| s.as_str() != "..")
    }

    fn authority_strategy() -> impl Strategy<Value = SmolStr> {
        "[a-zA-Z0-9._:@-]{1,24}".prop_map(SmolStr::new)
    }

    fn vfs_path_strategy() -> impl Strategy<Value = VfsPath> {
        let scheme_idx = 0u8..4;
        let authority = proptest::option::of(authority_strategy());
        let segments = proptest::collection::vec(segment_strategy(), 0..10);
        (scheme_idx, authority, segments).prop_map(|(idx, auth, segs)| {
            let scheme = ["file", "sftp", "s3", "ssh+http"][idx as usize];
            let authority = if scheme == "file" { None } else { auth };
            VfsPath {
                scheme: SmolStr::new(scheme),
                authority,
                segments: SmallVec::from_vec(segs),
            }
        })
    }

    proptest! {
        #[test]
        fn parse_display_roundtrip(p in vfs_path_strategy()) {
            let s = p.display();
            let parsed = VfsPath::parse(&s).expect("display output must parse");
            prop_assert_eq!(parsed, p);
        }
    }

    #[test]
    fn parse_basic_file() {
        let p = VfsPath::parse("file:///etc/passwd").unwrap();
        assert_eq!(p.scheme.as_str(), "file");
        assert_eq!(p.authority, None);
        assert_eq!(
            p.segments.iter().map(|s| s.as_str()).collect::<Vec<_>>(),
            vec!["etc", "passwd"]
        );
    }

    #[test]
    fn parse_basic_sftp() {
        let p = VfsPath::parse("sftp://user@host:22/var/log").unwrap();
        assert_eq!(p.scheme.as_str(), "sftp");
        assert_eq!(p.authority.as_deref(), Some("user@host:22"));
        assert_eq!(p.segments.len(), 2);
    }

    #[test]
    fn parse_root_file() {
        let p = VfsPath::parse("file:///").unwrap();
        assert_eq!(p.segments.len(), 0);
        assert_eq!(p.authority, None);
        assert_eq!(p.display(), "file:///");
    }

    #[test]
    fn parse_root_with_authority() {
        let p = VfsPath::parse("sftp://host/").unwrap();
        assert_eq!(p.segments.len(), 0);
        assert_eq!(p.authority.as_deref(), Some("host"));
        assert_eq!(p.display(), "sftp://host/");
    }

    #[test]
    fn parse_rejects_dotdot() {
        assert!(VfsPath::parse("file:///foo/../etc").is_err());
    }

    #[test]
    fn parse_rejects_empty_segment() {
        assert!(VfsPath::parse("file:///foo//bar").is_err());
        assert!(VfsPath::parse("file:///foo/").is_err());
    }

    #[test]
    fn parse_rejects_missing_scheme_separator() {
        assert!(VfsPath::parse("/etc/passwd").is_err());
        assert!(VfsPath::parse("foo").is_err());
    }

    #[test]
    fn parse_rejects_invalid_scheme() {
        assert!(VfsPath::parse("://host/x").is_err());
        assert!(VfsPath::parse("1bad://host/x").is_err());
    }

    #[test]
    fn parent_of_root_is_none() {
        let root = VfsPath::parse("file:///").unwrap();
        assert!(root.parent().is_none());
    }

    #[test]
    fn parent_pops_last_segment() {
        let p = VfsPath::parse("file:///etc/passwd").unwrap();
        let parent = p.parent().unwrap();
        assert_eq!(parent.display(), "file:///etc");
    }

    #[test]
    fn join_appends_segment() {
        let p = VfsPath::parse("file:///etc").unwrap();
        let child = p.join("passwd");
        assert_eq!(child.display(), "file:///etc/passwd");
    }

    #[test]
    fn join_from_root() {
        let root = VfsPath::parse("file:///").unwrap();
        let child = root.join("home");
        assert_eq!(child.display(), "file:///home");
    }

    #[test]
    #[should_panic(expected = "join")]
    fn join_rejects_slash() {
        let p = VfsPath::parse("file:///etc").unwrap();
        let _ = p.join("a/b");
    }

    #[test]
    #[should_panic(expected = "join")]
    fn join_rejects_dotdot() {
        let p = VfsPath::parse("file:///etc").unwrap();
        let _ = p.join("..");
    }
}

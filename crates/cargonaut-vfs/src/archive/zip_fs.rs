// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`ZipFs`] — read-only [`VfsBackend`] for `zip://` paths.
//!
//! T013 (green): full implementation driven by T012 test suite.

use crate::{
    ByteRange, DirEntry, DirListing, Sort, VfsBackend, VfsCaps, VfsError, VfsKind, VfsMetadata,
    VfsPath, WriteMode,
};
use async_trait::async_trait;
use futures::io::Cursor;
use futures::{AsyncRead, AsyncWrite};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::io::{self, Read};
use std::path::PathBuf;
use std::pin::Pin;
use std::sync::Arc;
use std::time::SystemTime;

// ─── Index ────────────────────────────────────────────────────────────────────

/// Metadata for one entry in the cached in-memory index.
#[derive(Debug, Clone)]
struct EntryMeta {
    size: u64,
    is_dir: bool,
    mtime: SystemTime,
}

/// In-memory entry index, built once in `ZipFs::open()`.
///
/// Keys are normalized entry names (no leading `/`, no trailing `/` for dirs).
/// Path-traversal entries are silently dropped during index construction.
#[derive(Debug)]
struct ZipIndex {
    /// entry name (normalized) → metadata
    entries: HashMap<String, EntryMeta>,
}

impl ZipIndex {
    /// Build the index by scanning all entries in the archive.
    fn build(archive_path: &PathBuf) -> Result<Self, VfsError> {
        let file = std::fs::File::open(archive_path).map_err(VfsError::Io)?;
        let mut archive = zip::ZipArchive::new(file)
            .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

        let mut entries = HashMap::new();
        for i in 0..archive.len() {
            let entry = archive
                .by_index(i)
                .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

            // Drop path-traversal entries silently.
            let safe_name = match entry.enclosed_name() {
                Some(n) => n.to_string_lossy().into_owned(),
                None => {
                    tracing::warn!(name = %entry.name(), "zip: dropping unsafe path-traversal entry");
                    continue;
                }
            };
            if safe_name.is_empty() {
                continue;
            }

            // Normalize: strip trailing slash from directory names.
            let key = safe_name.trim_end_matches('/').to_string();
            if key.is_empty() {
                continue;
            }

            let is_dir = entry.is_dir();
            let size = if is_dir { 0 } else { entry.size() };

            // Use UNIX epoch as mtime fallback — zip DateTime conversion is optional.
            let mtime = SystemTime::UNIX_EPOCH;

            entries.insert(
                key,
                EntryMeta {
                    size,
                    is_dir,
                    mtime,
                },
            );
        }

        Ok(ZipIndex { entries })
    }
}

// ─── VfsPath ↔ ZIP entry path conversion ─────────────────────────────────────

/// Convert VfsPath segments into the zip entry name prefix to match against.
/// An empty segment list corresponds to the archive root.
fn segments_to_zip_prefix(path: &VfsPath) -> String {
    path.segments
        .iter()
        .map(|s| s.as_str())
        .collect::<Vec<_>>()
        .join("/")
}

// ─── ZipFs ────────────────────────────────────────────────────────────────────

/// Read-only VFS backend that exposes a ZIP archive as a directory tree.
///
/// The in-memory entry index is built once at `open()` time; subsequent
/// `list()` / `stat()` calls are O(n) scans over that index.
/// `read_stream()` re-opens the archive to extract the entry bytes.
#[derive(Debug)]
pub struct ZipFs {
    /// Absolute path to the archive on the host filesystem.
    pub(crate) archive_path: PathBuf,
    /// Cached in-memory entry index.
    index: Arc<ZipIndex>,
}

impl ZipFs {
    /// Open a ZIP archive and build the in-memory entry index.
    ///
    /// Returns `VfsError::Io` for corrupt or inaccessible archives.
    pub fn open(archive_path: PathBuf) -> Result<Self, VfsError> {
        let index = ZipIndex::build(&archive_path)?;
        Ok(ZipFs {
            archive_path,
            index: Arc::new(index),
        })
    }
}

// ─── VfsBackend impl ─────────────────────────────────────────────────────────

#[async_trait]
impl VfsBackend for ZipFs {
    fn scheme(&self) -> &'static str {
        "zip"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::empty()
    }

    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError> {
        let prefix = segments_to_zip_prefix(path);

        // Collect direct children of `prefix` from the index.
        let mut children: Vec<DirEntry> = Vec::new();
        let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (name, meta) in &self.index.entries {
            let child_name = if prefix.is_empty() {
                // Root: keep entries with no "/" in them (direct root children).
                if name.contains('/') {
                    // This entry is nested — add the first path component as a synthetic dir.
                    let top = name.split('/').next().unwrap_or("").to_string();
                    if !top.is_empty() && seen_dirs.insert(top.clone()) {
                        // Add a synthetic directory entry for the first component.
                        children.push(DirEntry {
                            name: SmolStr::new(&top),
                            meta: VfsMetadata {
                                size: 0,
                                mtime: SystemTime::UNIX_EPOCH,
                                mode: None,
                                kind: VfsKind::Dir,
                                is_hidden: top.starts_with('.'),
                            },
                        });
                    }
                    continue;
                }
                name.clone()
            } else {
                // Subdir: name must start with "prefix/"
                let strip_prefix = format!("{prefix}/");
                match name.strip_prefix(strip_prefix.as_str()) {
                    None => continue,
                    Some(rest) => {
                        if rest.is_empty() {
                            continue;
                        }
                        if rest.contains('/') {
                            // Deeper nested — add synthetic dir for the next component.
                            let next = rest.split('/').next().unwrap_or("").to_string();
                            if !next.is_empty() && seen_dirs.insert(next.clone()) {
                                children.push(DirEntry {
                                    name: SmolStr::new(&next),
                                    meta: VfsMetadata {
                                        size: 0,
                                        mtime: SystemTime::UNIX_EPOCH,
                                        mode: None,
                                        kind: VfsKind::Dir,
                                        is_hidden: next.starts_with('.'),
                                    },
                                });
                            }
                            continue;
                        }
                        rest.to_string()
                    }
                }
            };

            // Direct child — add as file or dir.
            let kind = if meta.is_dir {
                VfsKind::Dir
            } else {
                VfsKind::File
            };
            // Skip directory entries that already appeared as synthetic dirs.
            if meta.is_dir && seen_dirs.contains(&child_name) {
                continue;
            }
            children.push(DirEntry {
                name: SmolStr::new(&child_name),
                meta: VfsMetadata {
                    size: meta.size,
                    mtime: meta.mtime,
                    mode: None,
                    kind,
                    is_hidden: child_name.starts_with('.'),
                },
            });
        }

        // Verify the prefix actually exists if the listing is empty.
        if children.is_empty() && !prefix.is_empty() {
            let exists = self.index.entries.contains_key(&prefix)
                || self
                    .index
                    .entries
                    .keys()
                    .any(|k| k.starts_with(&format!("{prefix}/")));
            if !exists {
                return Err(VfsError::NotFound(format!("no such directory: {prefix}")));
            }
        }

        // Sort the listing.
        match sort {
            Sort::NameAsc => children.sort_by(|a, b| a.name.cmp(&b.name)),
            Sort::NameDesc => children.sort_by(|a, b| b.name.cmp(&a.name)),
            Sort::SizeDesc => children.sort_by_key(|e| std::cmp::Reverse(e.meta.size)),
            Sort::MtimeDesc => children.sort_by_key(|e| std::cmp::Reverse(e.meta.mtime)),
            Sort::ExtAsc => children.sort_by(|a, b| {
                let ea = a.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
                let eb = b.name.rsplit_once('.').map(|(_, e)| e).unwrap_or("");
                ea.cmp(eb).then(a.name.cmp(&b.name))
            }),
        }

        Ok(DirListing {
            entries: children,
            sort,
        })
    }

    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        let key = segments_to_zip_prefix(path);
        if key.is_empty() {
            // Root of the archive — always exists as a directory.
            return Ok(VfsMetadata {
                size: 0,
                mtime: SystemTime::UNIX_EPOCH,
                mode: None,
                kind: VfsKind::Dir,
                is_hidden: false,
            });
        }
        match self.index.entries.get(&key) {
            Some(meta) => Ok(VfsMetadata {
                size: meta.size,
                mtime: meta.mtime,
                mode: None,
                kind: if meta.is_dir {
                    VfsKind::Dir
                } else {
                    VfsKind::File
                },
                is_hidden: key.starts_with('.') || key.contains("/."),
            }),
            None => {
                // Check if it exists as a directory prefix.
                let prefix = format!("{key}/");
                if self.index.entries.keys().any(|k| k.starts_with(&prefix)) {
                    return Ok(VfsMetadata {
                        size: 0,
                        mtime: SystemTime::UNIX_EPOCH,
                        mode: None,
                        kind: VfsKind::Dir,
                        is_hidden: false,
                    });
                }
                Err(VfsError::NotFound(format!("not found in archive: {key}")))
            }
        }
    }

    async fn read_stream(
        &self,
        path: &VfsPath,
        range: ByteRange,
    ) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError> {
        // Only FULL reads supported — ZIP is not seekable.
        if range.start != 0 || range.end.is_some() {
            return Err(VfsError::Unsupported(
                "ZipFs: only FULL byte ranges are supported",
            ));
        }

        let key = segments_to_zip_prefix(path);
        // Verify the entry exists and is a file.
        match self.index.entries.get(&key) {
            None => return Err(VfsError::NotFound(format!("not found in archive: {key}"))),
            Some(m) if m.is_dir => {
                return Err(VfsError::Other(
                    "cannot read a directory as a file stream".to_string(),
                ))
            }
            Some(_) => {}
        }

        let archive_path = self.archive_path.clone();
        let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, VfsError> {
            let file = std::fs::File::open(&archive_path).map_err(VfsError::Io)?;
            let mut archive = zip::ZipArchive::new(file)
                .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

            // Find the entry by matching its normalized name.
            let mut found_idx = None;
            for i in 0..archive.len() {
                let entry = archive
                    .by_index(i)
                    .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;
                let entry_name = match entry.enclosed_name() {
                    Some(n) => n.to_string_lossy().trim_end_matches('/').to_string(),
                    None => continue,
                };
                if entry_name == key {
                    found_idx = Some(i);
                    break;
                }
            }

            let idx =
                found_idx.ok_or_else(|| VfsError::NotFound(format!("entry not found: {key}")))?;
            let mut entry = archive
                .by_index(idx)
                .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

            // Encrypted entries: the zip crate returns an error on read.
            let mut buf = Vec::with_capacity(entry.size() as usize);
            entry.read_to_end(&mut buf).map_err(|e| {
                if e.kind() == io::ErrorKind::InvalidData {
                    VfsError::PermissionDenied("encrypted archive entry".to_string())
                } else {
                    VfsError::Io(e)
                }
            })?;
            Ok(buf)
        })
        .await
        .map_err(|e| VfsError::Other(format!("spawn_blocking join error: {e}")))??;

        let cursor = Cursor::new(bytes);
        Ok(Box::pin(cursor))
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("ZipFs is read-only"))
    }
}

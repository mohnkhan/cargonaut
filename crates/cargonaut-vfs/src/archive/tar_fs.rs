// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`TarFs`] — read-only [`VfsBackend`] for `tar://` paths.
//!
//! T020 (green): full implementation driven by T019 test suite.

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

// ─── Compression codec ────────────────────────────────────────────────────────

/// Compression codec detected from the archive file extension.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TarCompression {
    /// Uncompressed `.tar`.
    None,
    /// Gzip-compressed `.tar.gz` / `.tgz`.
    Gz,
    /// Bzip2-compressed `.tar.bz2` / `.tbz2`.
    Bz2,
    /// XZ-compressed `.tar.xz` / `.txz`.
    Xz,
}

impl TarCompression {
    /// Detect compression from a file extension (case-insensitive).
    pub fn from_extension(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.ends_with(".tar.gz") || lower.ends_with(".tgz") {
            Some(Self::Gz)
        } else if lower.ends_with(".tar.bz2") || lower.ends_with(".tbz2") {
            Some(Self::Bz2)
        } else if lower.ends_with(".tar.xz") || lower.ends_with(".txz") {
            Some(Self::Xz)
        } else if lower.ends_with(".tar") {
            Some(Self::None)
        } else {
            None
        }
    }
}

// ─── Index ────────────────────────────────────────────────────────────────────

/// Metadata for one entry in the in-memory index.
#[derive(Debug, Clone)]
struct EntryMeta {
    size: u64,
    is_dir: bool,
    mtime: SystemTime,
    /// Sequential index of this entry in the archive (for read_stream re-scan).
    seq_idx: usize,
}

/// In-memory entry index, built once in `TarFs::open()`.
///
/// Keys are normalized entry paths (no leading `/`, no trailing `/`).
/// Path-traversal entries (`../`) are silently dropped.
#[derive(Debug)]
struct TarIndex {
    /// entry path → metadata
    entries: HashMap<String, EntryMeta>,
}

impl TarIndex {
    fn build(archive_path: &PathBuf, compression: TarCompression) -> Result<Self, VfsError> {
        let file = std::fs::File::open(archive_path).map_err(VfsError::Io)?;
        let mut entries = HashMap::new();

        let res = match compression {
            TarCompression::None => scan_tar(tar::Archive::new(file), &mut entries),
            TarCompression::Gz => {
                let dec = flate2::read::GzDecoder::new(file);
                scan_tar(tar::Archive::new(dec), &mut entries)
            }
            TarCompression::Bz2 => {
                let dec = bzip2::read::BzDecoder::new(file);
                scan_tar(tar::Archive::new(dec), &mut entries)
            }
            TarCompression::Xz => {
                let dec = xz2::read::XzDecoder::new(file);
                scan_tar(tar::Archive::new(dec), &mut entries)
            }
        };
        res?;
        Ok(TarIndex { entries })
    }
}

/// Generic scan over any `tar::Archive<R: Read>`.
fn scan_tar<R: Read>(mut archive: tar::Archive<R>, entries: &mut HashMap<String, EntryMeta>) -> Result<(), VfsError> {
    let all = archive
        .entries()
        .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

    let mut seq_idx = 0usize;
    for entry_result in all {
        let mut entry = entry_result
            .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

        let raw_path = entry
            .path()
            .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?
            .to_string_lossy()
            .into_owned();

        // Drop path-traversal entries.
        if raw_path.contains("../") || raw_path.starts_with("../") || raw_path == ".." {
            tracing::warn!(path = %raw_path, "tar: dropping unsafe path-traversal entry");
            // Still consume the entry to advance the stream.
            io::copy(&mut entry, &mut io::sink())
                .map_err(VfsError::Io)?;
            seq_idx += 1;
            continue;
        }

        // Normalize: strip leading `./` and trailing `/`.
        let key = raw_path
            .trim_start_matches("./")
            .trim_end_matches('/')
            .to_string();
        if key.is_empty() {
            io::copy(&mut entry, &mut io::sink()).map_err(VfsError::Io)?;
            seq_idx += 1;
            continue;
        }

        let is_dir = entry.header().entry_type().is_dir();
        let size = entry.header().size().unwrap_or(0);
        let mtime_secs = entry.header().mtime().unwrap_or(0);
        let mtime = SystemTime::UNIX_EPOCH
            + std::time::Duration::from_secs(mtime_secs);

        entries.insert(key, EntryMeta { size, is_dir, mtime, seq_idx });

        // Drain entry bytes to advance the stream.
        io::copy(&mut entry, &mut io::sink()).map_err(VfsError::Io)?;
        seq_idx += 1;
    }
    Ok(())
}

// ─── VfsPath ↔ TAR entry path ─────────────────────────────────────────────────

fn segments_to_tar_prefix(path: &VfsPath) -> String {
    path.segments.iter().map(|s| s.as_str()).collect::<Vec<_>>().join("/")
}

// ─── TarFs ────────────────────────────────────────────────────────────────────

/// Read-only VFS backend that exposes a TAR archive as a directory tree.
///
/// TAR is a sequential format — range reads are not supported.
/// `read_stream()` re-scans the archive from the beginning to reach the target entry.
#[derive(Debug)]
pub struct TarFs {
    /// Absolute path to the archive file on the host filesystem.
    pub(crate) archive_path: PathBuf,
    /// Detected compression codec.
    pub(crate) compression: TarCompression,
    /// In-memory index built at `open()` time.
    index: Arc<TarIndex>,
}

impl TarFs {
    /// Open a TAR archive and build the in-memory entry index.
    ///
    /// Returns `VfsError::Io` for corrupt or inaccessible archives.
    pub fn open(archive_path: PathBuf, compression: TarCompression) -> Result<Self, VfsError> {
        let index = TarIndex::build(&archive_path, compression)?;
        Ok(TarFs {
            archive_path,
            compression,
            index: Arc::new(index),
        })
    }
}

// ─── VfsBackend impl ─────────────────────────────────────────────────────────

#[async_trait]
impl VfsBackend for TarFs {
    fn scheme(&self) -> &'static str {
        "tar"
    }

    fn caps(&self) -> VfsCaps {
        VfsCaps::empty()
    }

    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError> {
        let prefix = segments_to_tar_prefix(path);
        let mut children: Vec<DirEntry> = Vec::new();
        let mut seen_dirs: std::collections::HashSet<String> = std::collections::HashSet::new();

        for (name, meta) in &self.index.entries {
            let child_name = if prefix.is_empty() {
                // Root: keep entries with no "/" — top-level entries.
                if name.contains('/') {
                    let top = name.split('/').next().unwrap_or("").to_string();
                    if !top.is_empty() && seen_dirs.insert(top.clone()) {
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
                let strip_prefix = format!("{prefix}/");
                match name.strip_prefix(strip_prefix.as_str()) {
                    None => continue,
                    Some(rest) => {
                        if rest.is_empty() {
                            continue;
                        }
                        if rest.contains('/') {
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

            let kind = if meta.is_dir { VfsKind::Dir } else { VfsKind::File };
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

        if children.is_empty() && !prefix.is_empty() {
            let exists = self.index.entries.contains_key(&prefix)
                || self.index.entries.keys().any(|k| k.starts_with(&format!("{prefix}/")));
            if !exists {
                return Err(VfsError::NotFound(format!("no such directory: {prefix}")));
            }
        }

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

        Ok(DirListing { entries: children, sort })
    }

    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError> {
        let key = segments_to_tar_prefix(path);
        if key.is_empty() {
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
                kind: if meta.is_dir { VfsKind::Dir } else { VfsKind::File },
                is_hidden: key.starts_with('.') || key.contains("/."),
            }),
            None => {
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
        // TAR is sequential — range reads are not supported.
        if range.start != 0 || range.end.is_some() {
            return Err(VfsError::Unsupported("TarFs: only FULL byte ranges are supported"));
        }

        let key = segments_to_tar_prefix(path);
        let meta = match self.index.entries.get(&key) {
            None => return Err(VfsError::NotFound(format!("not found in archive: {key}"))),
            Some(m) if m.is_dir => return Err(VfsError::Other("cannot read a directory as a file stream".to_string())),
            Some(m) => m,
        };
        let target_seq_idx = meta.seq_idx;

        let archive_path = self.archive_path.clone();
        let compression = self.compression;

        let bytes = tokio::task::spawn_blocking(move || -> Result<Vec<u8>, VfsError> {
            let file = std::fs::File::open(&archive_path).map_err(VfsError::Io)?;
            match compression {
                TarCompression::None => read_entry(tar::Archive::new(file), target_seq_idx),
                TarCompression::Gz => read_entry(tar::Archive::new(flate2::read::GzDecoder::new(file)), target_seq_idx),
                TarCompression::Bz2 => read_entry(tar::Archive::new(bzip2::read::BzDecoder::new(file)), target_seq_idx),
                TarCompression::Xz => read_entry(tar::Archive::new(xz2::read::XzDecoder::new(file)), target_seq_idx),
            }
        })
        .await
        .map_err(|e| VfsError::Other(format!("spawn_blocking join error: {e}")))??;

        Ok(Box::pin(Cursor::new(bytes)))
    }

    async fn write_stream(
        &self,
        _path: &VfsPath,
        _offset: u64,
        _mode: WriteMode,
    ) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn unlink(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn rmdir(&self, _path: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn rename(&self, _src: &VfsPath, _dest: &VfsPath) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }

    async fn mkdir(&self, _path: &VfsPath, _recursive: bool) -> Result<(), VfsError> {
        Err(VfsError::Unsupported("TarFs is read-only"))
    }
}

/// Re-open the archive and read the entry at `seq_idx` into a Vec<u8>.
fn read_entry<R: Read>(mut archive: tar::Archive<R>, target_seq_idx: usize) -> Result<Vec<u8>, VfsError> {
    let all = archive
        .entries()
        .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

    for (idx, entry_result) in all.enumerate() {
        let mut entry = entry_result
            .map_err(|e| VfsError::Io(io::Error::new(io::ErrorKind::InvalidData, e)))?;

        if idx == target_seq_idx {
            let mut buf = Vec::new();
            entry.read_to_end(&mut buf).map_err(VfsError::Io)?;
            return Ok(buf);
        }

        // Skip this entry to advance the stream.
        io::copy(&mut entry, &mut io::sink()).map_err(VfsError::Io)?;
    }

    Err(VfsError::NotFound(format!("entry at seq_idx {target_seq_idx} not found in archive")))
}

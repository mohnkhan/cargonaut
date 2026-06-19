// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! [`VfsRegistry`] — maps URI schemes (and scheme+authority pairs) to
//! [`VfsBackend`] instances, enabling scheme-agnostic pane dispatch.
//!
//! The registry owns the `LocalFs` singleton and any registered remote
//! backends (`SftpFs`, `FtpFs`). Archive backends (`ZipFs`, `TarFs`) are
//! **not** stored here; they are ephemeral and held directly by
//! `PaneState.backend`.

use crate::{VfsBackend, VfsPath};
use smol_str::SmolStr;
use std::collections::HashMap;
use std::sync::Arc;

/// Maps URI scheme strings (and optionally `scheme://authority` pairs) to
/// [`VfsBackend`] instances.
///
/// **Invariants**:
/// - `local()` always returns a valid backend; it is set at construction.
/// - `resolve` is deterministic for the same `path` and registry state.
/// - Archive backends (`zip://`, `tar://`) are **not** stored here; the
///   registry resolves them as `None` so the caller can handle them
///   directly via `PaneState.backend`.
pub struct VfsRegistry {
    local: Arc<dyn VfsBackend>,
    /// Key: `"{scheme}://{authority}"` (e.g. `"sftp://alice@host:22"`).
    remote_map: HashMap<SmolStr, Arc<dyn VfsBackend>>,
}

impl VfsRegistry {
    /// Create a new registry. `local` is the `LocalFs` backend, registered
    /// under the `"file"` scheme. It is always present and returned by
    /// [`Self::local`].
    pub fn new(local: Arc<dyn VfsBackend>) -> Self {
        Self {
            local,
            remote_map: HashMap::new(),
        }
    }

    /// Return the local-filesystem backend (`file://`).
    pub fn local(&self) -> Arc<dyn VfsBackend> {
        Arc::clone(&self.local)
    }

    /// Register a connection-scoped remote backend.
    ///
    /// `key` MUST be `"{scheme}://{authority}"`, e.g. `"sftp://alice@host:22"`.
    /// Overwrites any prior registration for the same key (reconnect scenario).
    pub fn register_remote(&mut self, key: impl Into<SmolStr>, backend: Arc<dyn VfsBackend>) {
        self.remote_map.insert(key.into(), backend);
    }

    /// Resolve the backend for `path`.
    ///
    /// Lookup order:
    /// 1. If `path.authority.is_some()`: check `remote_map["{scheme}://{authority}"]`
    /// 2. If `path.scheme == "file"`: return `local()`
    /// 3. Otherwise: `None` (caller must surface an appropriate error)
    pub fn resolve(&self, path: &VfsPath) -> Option<Arc<dyn VfsBackend>> {
        if let Some(auth) = &path.authority {
            let key = SmolStr::new(format!("{}://{}", path.scheme, auth));
            if let Some(b) = self.remote_map.get(&key) {
                return Some(Arc::clone(b));
            }
        }
        if path.scheme == "file" {
            return Some(self.local());
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{LocalFs, VfsCaps};

    fn local() -> Arc<dyn VfsBackend> {
        Arc::new(LocalFs::new())
    }

    #[test]
    fn resolve_file_returns_local() {
        let reg = VfsRegistry::new(local());
        let path = VfsPath::parse("file:///tmp").unwrap();
        let backend = reg.resolve(&path).expect("file:// must resolve");
        assert_eq!(backend.scheme(), "file");
    }

    #[test]
    fn resolve_unknown_scheme_returns_none() {
        let reg = VfsRegistry::new(local());
        let path = VfsPath::parse("s3://mybucket/key").unwrap();
        assert!(reg.resolve(&path).is_none());
    }

    #[test]
    fn register_and_resolve_remote() {
        let mut reg = VfsRegistry::new(local());
        let sftp_mock: Arc<dyn VfsBackend> = local(); // use LocalFs as a stub type
        reg.register_remote("sftp://alice@host:22", Arc::clone(&sftp_mock));
        let path = VfsPath {
            scheme: smol_str::SmolStr::new("sftp"),
            authority: Some(smol_str::SmolStr::new("alice@host:22")),
            segments: smallvec::smallvec![],
        };
        let resolved = reg.resolve(&path).expect("registered remote must resolve");
        // Both are Arc<dyn VfsBackend> — verify they share the same scheme
        // (our stub is LocalFs with scheme "file"; the important thing is
        // the correct Arc was returned, not the file backend).
        assert!(Arc::ptr_eq(&resolved, &sftp_mock));
    }

    #[test]
    fn re_register_overwrites() {
        let mut reg = VfsRegistry::new(local());
        let first: Arc<dyn VfsBackend> = local();
        let second: Arc<dyn VfsBackend> = local();
        reg.register_remote("sftp://host:22", Arc::clone(&first));
        reg.register_remote("sftp://host:22", Arc::clone(&second));
        let path = VfsPath {
            scheme: smol_str::SmolStr::new("sftp"),
            authority: Some(smol_str::SmolStr::new("host:22")),
            segments: smallvec::smallvec![],
        };
        let resolved = reg.resolve(&path).unwrap();
        assert!(
            Arc::ptr_eq(&resolved, &second),
            "should return the overwritten (second) backend"
        );
    }

    #[test]
    fn local_accessor_returns_local_backend() {
        let reg = VfsRegistry::new(local());
        let b = reg.local();
        assert!(b.caps().contains(VfsCaps::SEEKABLE));
    }
}

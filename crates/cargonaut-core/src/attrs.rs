// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `attrs` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// Feature 044 — cap on entries enumerated by a recursive attribute walk, so an
/// arbitrarily large tree cannot wedge the UI (FR-005). Matches the
/// `recursive_dir_size` bound.
pub(crate) const RECURSE_NODE_CAP: usize = 200_000;

/// Feature 044 — status line for a recursive attribute op: [`attr_status`] plus
/// a "(truncated)" suffix when the bounded walk hit its cap.
pub(crate) fn recursive_status(
    op: &str,
    ok: usize,
    failures: &[String],
    truncated: bool,
) -> String {
    let mut s = attr_status(op, ok, failures);
    if truncated {
        s.push_str(" (truncated)");
    }
    s
}

/// Feature 043 — status line for a batch attribute op: how many succeeded and,
/// if any failed, which (partial failures are surfaced, not rolled back).
pub(crate) fn attr_status(op: &str, ok: usize, failures: &[String]) -> String {
    if failures.is_empty() {
        format!("{op}: {ok} item(s)")
    } else {
        format!(
            "{op}: {ok} ok, {} failed ({})",
            failures.len(),
            failures.join("; ")
        )
    }
}

impl App {
    /// Change the permissions of the current selection (tagged files, else the
    /// focused entry, never the `..` row). `spec` is an octal or symbolic mode
    /// ([`cargonaut_vfs::ModeSpec`]); invalid input ⇒ [`AppError::BadAttr`] with
    /// no change. Symbolic specs are applied to each file's current bits.
    /// Per-file failures are reported in the status without rolling back the
    /// successes (FR-010); the active pane is refreshed (FR-008).
    pub async fn chmod_selection(&mut self, spec: &str) -> Result<Vec<Event>, AppError> {
        let mode_spec = cargonaut_vfs::ModeSpec::parse(spec)
            .map_err(|e| AppError::BadAttr(format!("invalid mode {spec:?} ({e:?})")))?;
        let id = self.active;
        let names = self.selection_or_focused(id);
        if names.is_empty() {
            return Ok(vec![Event::Status("No files selected".into())]);
        }
        let cwd = self.pane(id).cwd.clone();
        let mut ok = 0usize;
        let mut failures = Vec::new();
        for name in &names {
            let target = cwd.join(name);
            let current = match self.registry.local().stat(&target).await {
                Ok(m) => m.mode.map(|fm| fm.bits).unwrap_or(0),
                Err(e) => {
                    failures.push(format!("{name}: {e}"));
                    continue;
                }
            };
            match self
                .registry
                .local()
                .chmod(&target, mode_spec.apply(current))
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(attr_status("chmod", ok, &failures)));
        Ok(evs)
    }

    /// Change ownership of the current selection. `owner` is `user`, `:group`,
    /// or `user:group` (each side a name or numeric id; omitted side unchanged).
    /// Invalid/unknown owner ⇒ [`AppError::BadAttr`] with no change. Per-file
    /// failures (e.g. permission denied) are reported without rollback (FR-010);
    /// the pane is refreshed (FR-008).
    pub async fn chown_selection(&mut self, owner: &str) -> Result<Vec<Event>, AppError> {
        let (uid, gid) = cargonaut_vfs::parse_owner(owner)
            .map_err(|e| AppError::BadAttr(format!("invalid owner {owner:?} ({e:?})")))?;
        let id = self.active;
        let names = self.selection_or_focused(id);
        if names.is_empty() {
            return Ok(vec![Event::Status("No files selected".into())]);
        }
        let cwd = self.pane(id).cwd.clone();
        let mut ok = 0usize;
        let mut failures = Vec::new();
        for name in &names {
            let target = cwd.join(name);
            match self.registry.local().chown(&target, uid, gid).await {
                Ok(()) => ok += 1,
                Err(e) => failures.push(format!("{name}: {e}")),
            }
        }
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(attr_status("chown", ok, &failures)));
        Ok(evs)
    }

    /// Enumerate every entry under `roots` for a recursive attribute op. Each
    /// root is included; directory roots are walked breadth-first, descending
    /// **only** into real directories (`VfsKind::Dir`) — `VfsKind::Symlink` dirs
    /// are leaves, so links are never followed out of the subtree (FR-006).
    /// Bounded by [`RECURSE_NODE_CAP`]; returns paths shallow→deep plus whether
    /// the cap truncated the walk (FR-005).
    pub(crate) async fn collect_subtree(&self, roots: &[VfsPath]) -> (Vec<VfsPath>, bool) {
        self.collect_subtree_capped(roots, RECURSE_NODE_CAP).await
    }

    /// [`collect_subtree`](Self::collect_subtree) with an explicit cap (test seam).
    pub(crate) async fn collect_subtree_capped(
        &self,
        roots: &[VfsPath],
        cap: usize,
    ) -> (Vec<VfsPath>, bool) {
        use std::collections::VecDeque;
        let mut out: Vec<VfsPath> = Vec::new();
        let mut queue: VecDeque<VfsPath> = VecDeque::new();
        let mut truncated = false;
        for r in roots {
            out.push(r.clone());
            if let Ok(m) = self.registry.local().stat(r).await {
                if matches!(m.kind, cargonaut_vfs::VfsKind::Dir) {
                    queue.push_back(r.clone());
                }
            }
        }
        while let Some(dir) = queue.pop_front() {
            let listing = match self.registry.local().list(&dir, Sort::NameAsc).await {
                Ok(l) => l,
                Err(_) => continue, // unreadable dir: skip its subtree, keep going
            };
            for e in listing.entries {
                if out.len() >= cap {
                    truncated = true;
                    break;
                }
                let child = dir.join(e.name.as_str());
                out.push(child.clone());
                // Descend only into real directories — never symlinks (FR-006).
                if matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir) {
                    queue.push_back(child);
                }
            }
            if truncated {
                break;
            }
        }
        (out, truncated)
    }

    /// Recursively change permissions of the selection's subtree(s). `spec` is
    /// octal or symbolic ([`cargonaut_vfs::ModeSpec`]); invalid ⇒
    /// [`AppError::BadAttr`] with no walk. Symbolic specs are applied per entry
    /// relative to its current bits. Entries are changed **deepest-first** so a
    /// restrictive mode can't lock the walk out of a child (FR-011); symlink
    /// entries are skipped (never modified through a link, FR-006); per-entry
    /// failures are aggregated (FR-007); a truncated walk is noted (FR-005).
    pub async fn chmod_recursive(&mut self, spec: &str) -> Result<Vec<Event>, AppError> {
        let mode_spec = cargonaut_vfs::ModeSpec::parse(spec)
            .map_err(|e| AppError::BadAttr(format!("invalid mode {spec:?} ({e:?})")))?;
        let (mut paths, truncated) = match self.attr_roots() {
            Some(roots) => self.collect_subtree(&roots).await,
            None => return Ok(vec![Event::Status("No files selected".into())]),
        };
        paths.reverse(); // deepest-first (collect is shallow→deep)
        let mut ok = 0usize;
        let mut failures = Vec::new();
        for p in &paths {
            let meta = match self.registry.local().stat(p).await {
                Ok(m) => m,
                Err(e) => {
                    failures.push(format!("{}: {e}", p.display()));
                    continue;
                }
            };
            if matches!(meta.kind, cargonaut_vfs::VfsKind::Symlink { .. }) {
                continue; // never chmod through a symlink (FR-006)
            }
            let current = meta.mode.map(|fm| fm.bits).unwrap_or(0);
            match self
                .registry
                .local()
                .chmod(p, mode_spec.apply(current))
                .await
            {
                Ok(()) => ok += 1,
                Err(e) => failures.push(format!("{}: {e}", p.display())),
            }
        }
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(recursive_status(
            "chmod -R", ok, &failures, truncated,
        )));
        Ok(evs)
    }

    /// Recursively change ownership of the selection's subtree(s). `owner` is
    /// `user[:group]` (name or numeric); invalid/unknown ⇒ [`AppError::BadAttr`]
    /// with no walk. Deepest-first; symlink entries skipped (FR-006); per-entry
    /// failures aggregated (FR-007); truncation noted (FR-005).
    pub async fn chown_recursive(&mut self, owner: &str) -> Result<Vec<Event>, AppError> {
        let (uid, gid) = cargonaut_vfs::parse_owner(owner)
            .map_err(|e| AppError::BadAttr(format!("invalid owner {owner:?} ({e:?})")))?;
        let (mut paths, truncated) = match self.attr_roots() {
            Some(roots) => self.collect_subtree(&roots).await,
            None => return Ok(vec![Event::Status("No files selected".into())]),
        };
        paths.reverse(); // deepest-first
        let mut ok = 0usize;
        let mut failures = Vec::new();
        for p in &paths {
            match self.registry.local().stat(p).await {
                Ok(m) if matches!(m.kind, cargonaut_vfs::VfsKind::Symlink { .. }) => continue,
                Ok(_) => {}
                Err(e) => {
                    failures.push(format!("{}: {e}", p.display()));
                    continue;
                }
            }
            match self.registry.local().chown(p, uid, gid).await {
                Ok(()) => ok += 1,
                Err(e) => failures.push(format!("{}: {e}", p.display())),
            }
        }
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(recursive_status(
            "chown -R", ok, &failures, truncated,
        )));
        Ok(evs)
    }

    /// The recursive-op root paths from the current selection (tagged, else
    /// focused; excludes `..`), or `None` if nothing is selected.
    pub(crate) fn attr_roots(&self) -> Option<Vec<VfsPath>> {
        let id = self.active;
        let names = self.selection_or_focused(id);
        if names.is_empty() {
            return None;
        }
        let cwd = self.pane(id).cwd.clone();
        Some(names.iter().map(|n| cwd.join(n)).collect())
    }

    /// Create a symbolic link named `link_name` in the active pane's directory,
    /// pointing at the focused entry (a relative link to the sibling). Blank
    /// name ⇒ [`AppError::BadAttr`]; an existing name or OS error is reported.
    pub async fn create_symlink(&mut self, link_name: &str) -> Result<Vec<Event>, AppError> {
        let (target_name, cwd) = self.link_source()?;
        let link_name = link_name.trim();
        if link_name.is_empty() {
            return Err(AppError::BadAttr("link name must not be blank".into()));
        }
        let link = cwd.join(link_name);
        self.registry.local().symlink(&target_name, &link).await?;
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(format!("Linked {link_name} → {target_name}")));
        Ok(evs)
    }

    /// Create a hard link named `link_name` in the active pane's directory,
    /// referring to the focused entry. Blank name ⇒ [`AppError::BadAttr`];
    /// OS rejection (directory / cross-filesystem) is reported.
    pub async fn create_hard_link(&mut self, link_name: &str) -> Result<Vec<Event>, AppError> {
        let (target_name, cwd) = self.link_source()?;
        let link_name = link_name.trim();
        if link_name.is_empty() {
            return Err(AppError::BadAttr("link name must not be blank".into()));
        }
        let src = cwd.join(&target_name);
        let link = cwd.join(link_name);
        self.registry.local().hard_link(&src, &link).await?;
        let mut evs = self.refresh_active_pane().await?;
        evs.push(Event::Status(format!(
            "Hard-linked {link_name} → {target_name}"
        )));
        Ok(evs)
    }

    /// The focused real entry's name + the active pane cwd, for link creation.
    /// Errors if nothing is focused (e.g. cursor on the `..` row).
    pub(crate) fn link_source(&self) -> Result<(String, VfsPath), AppError> {
        let p = self.active_pane_state();
        let name = p
            .focused_entry_index()
            .and_then(|i| p.listing.entries.get(i))
            .map(|e| e.name.to_string())
            .ok_or_else(|| AppError::BadAttr("no file focused to link".into()))?;
        Ok((name, p.cwd.clone()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn collect_subtree_enumerates_depth_first_to_last() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir_all(td_l.path().join("a/b/c")).unwrap();
        fs::write(td_l.path().join("a/b/c/deep.txt"), b"x")
            .await
            .unwrap();
        let app = make_app(&td_l, &td_r).await;
        let root = app.pane(PaneId::Left).cwd.join("a");
        let (paths, truncated) = app.collect_subtree(&[root]).await;
        assert!(!truncated);
        let joined = paths
            .iter()
            .map(|p| p.display())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("a/b/c/deep.txt"),
            "deep entry missing:\n{joined}"
        );
        assert!(joined.contains("/a/b"), "intermediate dir missing");
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn collect_subtree_does_not_follow_symlinked_dir() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        fs::write(outside.path().join("secret.txt"), b"x")
            .await
            .unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        std::os::unix::fs::symlink(outside.path(), td_l.path().join("a/link")).unwrap();
        let app = make_app(&td_l, &td_r).await;
        let root = app.pane(PaneId::Left).cwd.join("a");
        let (paths, _) = app.collect_subtree(&[root]).await;
        let joined = paths
            .iter()
            .map(|p| p.display())
            .collect::<Vec<_>>()
            .join("\n");
        assert!(
            joined.contains("/a/link"),
            "the link entry itself should be listed"
        );
        assert!(
            !joined.contains("secret.txt"),
            "must NOT descend into a symlinked dir:\n{joined}"
        );
    }

    #[tokio::test]
    async fn collect_subtree_file_root_is_only_itself() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        let root = app.pane(PaneId::Left).cwd.join("f");
        let (paths, _) = app.collect_subtree(&[root]).await;
        assert_eq!(paths.len(), 1);
    }

    #[tokio::test]
    async fn collect_subtree_capped_truncates() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        for n in 0..10 {
            fs::write(td_l.path().join(format!("a/f{n}")), b"x")
                .await
                .unwrap();
        }
        let app = make_app(&td_l, &td_r).await;
        let root = app.pane(PaneId::Left).cwd.join("a");
        let (paths, truncated) = app.collect_subtree_capped(&[root], 3).await;
        assert!(
            truncated,
            "a tree larger than the cap must report truncation"
        );
        assert!(paths.len() <= 4); // root + up to cap children before stopping
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chown_recursive_noop_to_current_owner_at_depth() {
        use std::os::unix::fs::MetadataExt;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir_all(td_l.path().join("a/b")).unwrap();
        fs::write(td_l.path().join("a/b/deep"), b"x").await.unwrap();
        let md = std::fs::metadata(td_l.path().join("a/b/deep")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.chown_recursive(&format!("{}:{}", md.uid(), md.gid()))
            .await
            .unwrap();
        let md2 = std::fs::metadata(td_l.path().join("a/b/deep")).unwrap();
        assert_eq!((md2.uid(), md2.gid()), (md.uid(), md.gid()));
    }

    #[tokio::test]
    async fn chown_recursive_unknown_owner_does_not_walk() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(matches!(
            app.chown_recursive("no_such_user_xyzzy_42").await,
            Err(AppError::BadAttr(_))
        ));
    }

    #[tokio::test]
    async fn chown_recursive_empty_selection_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.active_pane_mut().cursor = 0; // on the `..` row, nothing tagged
        let evs = app.chown_recursive("0:0").await.unwrap();
        assert!(format!("{evs:?}").contains("No files selected"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_recursive_applies_at_depth() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir_all(td_l.path().join("a/b/c")).unwrap();
        fs::write(td_l.path().join("a/b/c/deep.txt"), b"x")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await; // focused = "a"
        app.chmod_recursive("700").await.unwrap();
        assert_eq!(mode_of(&td_l.path().join("a")), 0o700);
        assert_eq!(mode_of(&td_l.path().join("a/b")), 0o700);
        assert_eq!(mode_of(&td_l.path().join("a/b/c/deep.txt")), 0o700);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_recursive_symbolic_is_per_entry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        let f = td_l.path().join("a/f");
        fs::write(&f, b"x").await.unwrap();
        std::fs::set_permissions(&f, std::os::unix::fs::PermissionsExt::from_mode(0o600)).unwrap();
        std::fs::set_permissions(
            td_l.path().join("a"),
            std::os::unix::fs::PermissionsExt::from_mode(0o755),
        )
        .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.chmod_recursive("g+r").await.unwrap();
        // each entry changed relative to its own mode
        assert_eq!(mode_of(&f), 0o640);
        assert_eq!(mode_of(&td_l.path().join("a")), 0o755);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_recursive_deepest_first_no_lockout() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir_all(td_l.path().join("a/b")).unwrap();
        fs::write(td_l.path().join("a/b/leaf"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Strip all bits: a top-down apply would lose `x` on `a` and fail to
        // reach the leaf. Deepest-first must still change it (FR-011).
        app.chmod_recursive("000").await.unwrap();
        // Restore traverse on the ancestor dirs so the test can stat the leaf
        // (this does NOT touch the leaf's own bits).
        for p in ["a", "a/b"] {
            std::fs::set_permissions(
                td_l.path().join(p),
                std::os::unix::fs::PermissionsExt::from_mode(0o755),
            )
            .unwrap();
        }
        // If deepest-first worked, the leaf was reached and is now 000; a
        // top-down apply would have locked out and left it unchanged.
        assert_eq!(mode_of(&td_l.path().join("a/b/leaf")), 0o000);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_recursive_does_not_follow_symlink() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let secret = outside.path().join("secret");
        fs::write(&secret, b"x").await.unwrap();
        std::fs::set_permissions(&secret, std::os::unix::fs::PermissionsExt::from_mode(0o644))
            .unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        std::os::unix::fs::symlink(outside.path(), td_l.path().join("a/link")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.chmod_recursive("700").await.unwrap();
        assert_eq!(mode_of(&secret), 0o644, "must not chmod through a symlink");
    }

    #[tokio::test]
    async fn chmod_recursive_invalid_does_not_walk() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::create_dir(td_l.path().join("a")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(matches!(
            app.chmod_recursive("nope").await,
            Err(AppError::BadAttr(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_recursive_file_only_is_shallow() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await; // focused = "f"
        app.chmod_recursive("700").await.unwrap();
        assert_eq!(mode_of(&td_l.path().join("f")), 0o700);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_selection_sets_focused_file() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("only.txt"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Default cursor sits on the first real entry (Feature 040).
        app.chmod_selection("755").await.unwrap();
        assert_eq!(mode_of(&td_l.path().join("only.txt")), 0o755);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_selection_symbolic_and_multi_file() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b"] {
            fs::write(td_l.path().join(n), b"x").await.unwrap();
            std::fs::set_permissions(
                td_l.path().join(n),
                std::os::unix::fs::PermissionsExt::from_mode(0o644),
            )
            .unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        let (ia, ib) = (entry_index(&app, "a"), entry_index(&app, "b"));
        app.active_pane_mut().selected.insert(ia);
        app.active_pane_mut().selected.insert(ib);
        app.chmod_selection("u+x").await.unwrap();
        assert_eq!(mode_of(&td_l.path().join("a")), 0o744);
        assert_eq!(mode_of(&td_l.path().join("b")), 0o744);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_selection_invalid_changes_nothing() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        std::fs::set_permissions(
            td_l.path().join("f"),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let res = app.chmod_selection("xyz").await;
        assert!(matches!(res, Err(AppError::BadAttr(_))));
        assert_eq!(mode_of(&td_l.path().join("f")), 0o644);
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_selection_partial_failure_reports_and_continues() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b"] {
            fs::write(td_l.path().join(n), b"x").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        let (ia, ib) = (entry_index(&app, "a"), entry_index(&app, "b"));
        app.active_pane_mut().selected.insert(ia);
        app.active_pane_mut().selected.insert(ib);
        // Remove "b" from disk so its chmod fails while "a" succeeds.
        std::fs::remove_file(td_l.path().join("b")).unwrap();
        let evs = app.chmod_selection("700").await.unwrap();
        assert_eq!(mode_of(&td_l.path().join("a")), 0o700);
        let status = format!("{evs:?}");
        assert!(status.contains('b') || status.to_lowercase().contains("fail"));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chmod_selection_on_parent_row_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        std::fs::set_permissions(
            td_l.path().join("f"),
            std::os::unix::fs::PermissionsExt::from_mode(0o644),
        )
        .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Put the cursor on the synthetic `..` row (index 0); no tags.
        app.active_pane_mut().cursor = 0;
        app.chmod_selection("700").await.unwrap();
        assert_eq!(
            mode_of(&td_l.path().join("f")),
            0o644,
            "..-row chmod must no-op"
        );
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chown_selection_noop_to_current_owner_ok() {
        use std::os::unix::fs::MetadataExt;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let md = std::fs::metadata(td_l.path().join("f")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Numeric uid:gid equal to the current owner — always permitted.
        app.chown_selection(&format!("{}:{}", md.uid(), md.gid()))
            .await
            .unwrap();
        let md2 = std::fs::metadata(td_l.path().join("f")).unwrap();
        assert_eq!((md2.uid(), md2.gid()), (md.uid(), md.gid()));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn chown_selection_group_only_numeric_ok() {
        use std::os::unix::fs::MetadataExt;
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let md = std::fs::metadata(td_l.path().join("f")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.chown_selection(&format!(":{}", md.gid()))
            .await
            .unwrap();
        assert_eq!(
            std::fs::metadata(td_l.path().join("f")).unwrap().gid(),
            md.gid()
        );
    }

    #[tokio::test]
    async fn chown_selection_unknown_user_is_bad_attr() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(matches!(
            app.chown_selection("no_such_user_xyzzy_42").await,
            Err(AppError::BadAttr(_))
        ));
    }

    #[tokio::test]
    async fn chown_selection_empty_is_bad_attr() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("f"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(matches!(
            app.chown_selection("   ").await,
            Err(AppError::BadAttr(_))
        ));
    }

    #[cfg(unix)]
    #[tokio::test]
    async fn create_symlink_points_at_focused_entry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("src"), b"hello").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await; // focused = "src"
        app.create_symlink("ln").await.unwrap();
        let link = td_l.path().join("ln");
        assert!(std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink());
        assert_eq!(std::fs::read(&link).unwrap(), b"hello");
        assert!(app
            .pane(PaneId::Left)
            .listing
            .entries
            .iter()
            .any(|e| e.name.as_str() == "ln"));
    }

    #[tokio::test]
    async fn create_symlink_existing_name_is_refused() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("src"), b"x").await.unwrap();
        fs::write(td_l.path().join("taken"), b"y").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await; // focused = "src" (sorts first)
        let res = app.create_symlink("taken").await;
        assert!(res.is_err(), "must refuse an existing name");
        assert_eq!(std::fs::read(td_l.path().join("taken")).unwrap(), b"y");
    }

    #[tokio::test]
    async fn create_hard_link_shares_content() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("src"), b"shared").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.create_hard_link("h").await.unwrap();
        assert_eq!(std::fs::read(td_l.path().join("h")).unwrap(), b"shared");
    }

    #[tokio::test]
    async fn create_hard_link_to_directory_errors() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("d")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await; // focused = "d"
        let res = app.create_hard_link("h").await;
        assert!(
            res.is_err(),
            "hard-linking a directory must error, not panic"
        );
    }

    #[tokio::test]
    async fn create_symlink_blank_name_is_bad_attr() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("src"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(matches!(
            app.create_symlink("  ").await,
            Err(AppError::BadAttr(_))
        ));
    }
}

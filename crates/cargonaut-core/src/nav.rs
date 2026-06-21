// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `nav` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// Cycle the sort *key* (FR-021): name → ext → size → mtime → name.
/// Reverse direction is a separate toggle.
pub(crate) fn next_sort_key(s: Sort) -> Sort {
    match s {
        Sort::NameAsc | Sort::NameDesc => Sort::ExtAsc,
        Sort::ExtAsc => Sort::SizeDesc,
        Sort::SizeDesc => Sort::MtimeDesc,
        Sort::MtimeDesc => Sort::NameAsc,
    }
}

/// Human-facing label for a sort order (surfaced in the status bar).
pub(crate) fn sort_label(s: Sort) -> &'static str {
    match s {
        Sort::NameAsc => "name",
        Sort::NameDesc => "name (reverse)",
        Sort::ExtAsc => "extension",
        Sort::SizeDesc => "size",
        Sort::MtimeDesc => "modified",
    }
}

pub(crate) fn parse_path(s: &str) -> Result<VfsPath, AppError> {
    if let Some(rest) = s.strip_prefix("file://") {
        return VfsPath::parse(&format!("file://{}", rest))
            .map_err(|e| AppError::BadPath(e.to_string()));
    }
    if !s.starts_with('/') {
        return Err(AppError::BadPath(format!(
            "{s:?} must be absolute or a file:// URI"
        )));
    }
    VfsPath::parse(&format!("file://{}", s)).map_err(|e| AppError::BadPath(e.to_string()))
}

impl App {
    /// Re-list the active pane's cwd with its current sort; clamp cursor.
    pub(crate) async fn relist_active(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let (cwd, sort) = {
            let p = self.pane(id);
            (p.cwd.clone(), p.sort)
        };
        let listing = self.registry.local().list(&cwd, sort).await?;
        let p = self.pane_mut(id);
        p.listing = listing;
        // Feature 040: clamp within the virtual row range (`..` + entries).
        let rows = p.row_count();
        p.cursor = if rows == 0 { 0 } else { p.cursor.min(rows - 1) };
        Ok(vec![Event::PaneUpdated(id)])
    }

    /// Reload the active pane's listing from disk. Cursor + selection are
    /// preserved if the cursor is still in bounds; otherwise clamped.
    /// Navigate a pane to `path` using an explicitly-supplied `backend`.
    ///
    /// Called by the UI layer when mounting an archive backend (zip://, tar://)
    /// or connecting to a remote backend (sftp://, ftp://) before navigating.
    pub async fn navigate_into(
        &mut self,
        id: PaneId,
        path: VfsPath,
        backend: Arc<dyn VfsBackend>,
    ) -> Result<Vec<Event>, AppError> {
        self.navigate_to(id, path, backend).await
    }

    /// Re-list the active pane's directory and clamp the cursor.
    pub async fn refresh_active_pane(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let cwd = self.pane(id).cwd.clone();
        let listing = self.registry.local().list(&cwd, Sort::NameAsc).await?;
        let p = self.active_pane_mut();
        p.listing = listing;
        p.selected.clear();
        // Feature 040: clamp within the virtual row range (`..` + entries).
        let rows = p.row_count();
        p.cursor = if rows == 0 { 0 } else { p.cursor.min(rows - 1) };
        Ok(vec![Event::PaneUpdated(id)])
    }

    /// Names of entries the user "means" by their current selection:
    /// the tagged set if non-empty, else the focused entry alone.
    pub(crate) fn selection_or_focused(&self, id: PaneId) -> Vec<String> {
        let p = self.pane(id);
        if !p.selected.is_empty() {
            p.selected
                .iter()
                .filter_map(|i| p.listing.entries.get(*i))
                .map(|e| e.name.to_string())
                .collect()
        } else {
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .map(|e| vec![e.name.to_string()])
                .unwrap_or_default()
        }
    }

    pub(crate) async fn descend_into_focused(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let target = {
            let p = self.pane(id);
            let entry = p
                .focused_entry_index()
                .and_then(|i| p.listing.entries.get(i));
            entry.map(|e| (e.name.to_string(), e.meta.kind.clone()))
        };
        let Some((name, kind)) = target else {
            return Ok(vec![]);
        };
        if !matches!(kind, cargonaut_vfs::VfsKind::Dir) {
            // Descend on a file is a no-op for now (T1.21 will open via $EDITOR / openers).
            return Ok(vec![Event::Status(format!("{name} is not a directory"))]);
        }
        let new_cwd = self.pane(id).cwd.join(&name);
        self.navigate_to(id, new_cwd, self.registry.local()).await
    }

    pub(crate) async fn sync_other_panel_path(&mut self) -> Result<Vec<Event>, AppError> {
        let active = self.active;
        let other = active.other();
        let other_cwd = self.pane(other).cwd.clone();
        self.navigate_to(active, other_cwd, self.registry.local())
            .await
    }

    pub(crate) async fn show_focused_in_other_panel(&mut self) -> Result<Vec<Event>, AppError> {
        let active = self.active;
        let other = active.other();
        let target = {
            let p = self.pane(active);
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .and_then(|e| match &e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => Some(p.cwd.join(e.name.as_str())),
                    _ => None,
                })
        };
        let Some(target) = target else {
            return Ok(vec![Event::Status(
                "Focused entry isn't a directory".into(),
            )]);
        };
        // navigate_to acts on `other`, not `active` — FR-014: focus stays put.
        self.navigate_to(other, target, self.registry.local()).await
    }

    pub(crate) async fn ascend_to_parent(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;

        // FR-023: if we are at the root of a non-local backend (empty segments),
        // pop out to the local parent directory of the archive / remote root.
        let at_nonlocal_root = {
            let p = self.pane(id);
            p.cwd.segments.is_empty() && p.backend.scheme() != "file"
        };
        if at_nonlocal_root {
            let decoded = self.pane(id).cwd.decode_authority();
            if let Some(host_path_str) = decoded {
                let archive = std::path::Path::new(&host_path_str);
                if let Some(local_parent) = archive.parent() {
                    let url = format!("file://{}", local_parent.display());
                    if let Ok(local_vfs) = VfsPath::parse(&url) {
                        return self.navigate_to(id, local_vfs, self.registry.local()).await;
                    }
                }
            }
            return Ok(vec![Event::Status("Already at root".into())]);
        }

        let Some(parent) = self.pane(id).cwd.parent() else {
            return Ok(vec![Event::Status("Already at root".into())]);
        };
        self.navigate_to(id, parent, self.registry.local()).await
    }

    /// FR-011 history-aware navigation. Pushes the OLD cwd onto the
    /// pane's back-history (bounded by `Config::ui.history.directory_depth`)
    /// and clears the forward-history. Called by every non-history nav
    /// entry point (descend, ascend, sync, show-in-other).
    ///
    /// `backend` is the VFS backend that owns `new_cwd`. Pass
    /// `self.registry.local()` for local-filesystem navigation; pass the
    /// appropriate archive / remote backend for Feature 057 navigation.
    pub(crate) async fn navigate_to(
        &mut self,
        id: PaneId,
        new_cwd: VfsPath,
        backend: Arc<dyn VfsBackend>,
    ) -> Result<Vec<Event>, AppError> {
        let listing = backend.list(&new_cwd, Sort::NameAsc).await?;
        let depth = self.config.ui.history.directory_depth as usize;
        let p = self.pane_mut(id);
        let old_cwd = std::mem::replace(&mut p.cwd, new_cwd);
        if depth > 0 {
            p.dir_history_back.push(old_cwd);
            while p.dir_history_back.len() > depth {
                p.dir_history_back.remove(0);
            }
        }
        p.dir_history_fwd.clear();
        p.listing = listing;
        p.backend = backend;
        p.cursor = p.default_cursor(); // Feature 040: first real entry, past `..`
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    /// Feature 038 (FR-012/R-003): resolve quick-cd input text to a
    /// `VfsPath` relative to the active pane's cwd.
    ///
    /// - text containing `://` is parsed as a full URI;
    /// - text starting with `/` is an absolute path under the active
    ///   pane's scheme/authority (rooted at the active backend);
    /// - otherwise it is relative to the active pane's cwd.
    ///
    /// `.` segments are skipped, `..` pops one segment (saturating at
    /// root), empty segments (e.g. a trailing `/`) are ignored. The path
    /// is not checked for existence here — that happens in `navigate_to`.
    pub(crate) fn resolve_cd_target(&self, text: &str) -> Result<VfsPath, AppError> {
        if text.contains("://") {
            return VfsPath::parse(text).map_err(|e| AppError::BadPath(e.to_string()));
        }
        let active_cwd = &self.active_pane_state().cwd;
        let (mut path, rest) = if let Some(stripped) = text.strip_prefix('/') {
            // Absolute: walk to the root of the active backend, keeping
            // its scheme + authority, then apply the typed segments.
            let mut root = active_cwd.clone();
            while let Some(parent) = root.parent() {
                root = parent;
            }
            (root, stripped)
        } else {
            (active_cwd.clone(), text)
        };
        for seg in rest.split('/') {
            match seg {
                "" | "." => {}
                ".." => {
                    if let Some(parent) = path.parent() {
                        path = parent;
                    }
                }
                s => path = path.join(s),
            }
        }
        Ok(path)
    }

    /// Feature 038 (FR-004/005/006/012/013): accept a quick-cd path for
    /// the active pane. Resolves `path_text` relative to the active cwd
    /// and navigates via the normal [`Self::navigate_to`] path (which
    /// lists the target first, so a non-existent / non-directory /
    /// permission-denied target returns `Err` with App state unchanged).
    ///
    /// Empty / whitespace-only input is a no-op (`Ok` with no events).
    pub async fn quick_cd(&mut self, path_text: &str) -> Result<Vec<Event>, AppError> {
        let trimmed = path_text.trim();
        if trimmed.is_empty() {
            return Ok(vec![]);
        }
        let target = self.resolve_cd_target(trimmed)?;
        let id = self.active;
        self.navigate_to(id, target, self.registry.local()).await
    }

    /// Feature 033 (FR-003/004/005/006/009): set or clear the active pane's
    /// name filter from raw prompt text.
    ///
    /// - Empty / whitespace-only `pattern` clears the filter (no error),
    ///   restoring the full listing (FR-005).
    /// - A non-empty pattern is compiled into a case-insensitive glob
    ///   ([`PaneFilter::compile`]; metacharacter-free patterns match as a
    ///   substring) and applied to the active pane (FR-003).
    /// - A pattern that fails to compile returns [`AppError::BadFilter`]
    ///   and leaves all pane state unchanged (FR-006).
    ///
    /// On any successful set or clear the active pane's cursor is reset to
    /// the top of the (now-)visible list (FR-004). Only the active pane is
    /// touched (FR-009).
    pub fn set_filter(&mut self, pattern: &str) -> Result<Vec<Event>, AppError> {
        let trimmed = pattern.trim();
        let (filter, status) = if trimmed.is_empty() {
            (None, "Panel filter cleared".to_string())
        } else {
            // Compile BEFORE mutating so an invalid pattern is atomic.
            let pf = PaneFilter::compile(trimmed)?;
            let status = format!("Filter: {}", pf.pattern());
            (Some(pf), status)
        };
        let id = self.active;
        let p = self.active_pane_mut();
        p.filter = filter;
        p.cursor = p.default_cursor(); // Feature 040: first match, past `..`
        Ok(vec![Event::PaneUpdated(id), Event::Status(status)])
    }

    /// Feature 038 (FR-007/008/009): compute directory completion
    /// candidates for the active pane from `partial` quick-cd input.
    ///
    /// The final path segment is the prefix to complete; earlier segments
    /// name the directory to list (resolved relative to the active cwd).
    /// Returns full path strings (URI form), ordered recent-visited
    /// matches first (most-recent first), then filesystem children in
    /// backend sort order, de-duplicated. Only directories are returned;
    /// an empty result means "nothing to complete". Read-only.
    pub async fn complete_cd(&self, partial: &str) -> Vec<String> {
        let (dir_text, last) = match partial.rfind('/') {
            Some(i) => (&partial[..=i], &partial[i + 1..]),
            None => ("", partial),
        };
        let dir = if dir_text.is_empty() {
            self.active_pane_state().cwd.clone()
        } else {
            match self.resolve_cd_target(dir_text) {
                Ok(p) => p,
                Err(_) => return Vec::new(),
            }
        };

        // Recent-visited matches first (most-recent at the end of the
        // history vec, so iterate in reverse): a recent dir matches when
        // it lives directly under `dir` and its final segment shares the
        // prefix.
        let active = self.active_pane_state();
        let mut out: Vec<String> = Vec::new();
        for hist in active.dir_history_back.iter().rev() {
            if hist.parent().as_ref() == Some(&dir) {
                if let Some(name) = hist.segments.last() {
                    if name.as_str().starts_with(last) {
                        let s = hist.display();
                        if !out.contains(&s) {
                            out.push(s);
                        }
                    }
                }
            }
        }

        // Then filesystem children that are directories and match.
        if let Ok(listing) = self.registry.local().list(&dir, Sort::NameAsc).await {
            for e in listing.entries {
                if matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir)
                    && e.name.as_str().starts_with(last)
                {
                    let s = dir.join(e.name.as_str()).display();
                    if !out.contains(&s) {
                        out.push(s);
                    }
                }
            }
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn descend_into_subdir_then_ascend_back() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        fs::write(td_l.path().join("sub/x"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let parent_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 1);
        app.dispatch(Command::Ascend).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, parent_cwd);
    }

    #[tokio::test]
    async fn cycle_sort_key_reorders_listing() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Names ascending: a, b, z; sizes: a=3, b=1, z=2.
        fs::write(td_l.path().join("a"), b"xxx").await.unwrap();
        fs::write(td_l.path().join("b"), b"x").await.unwrap();
        fs::write(td_l.path().join("z"), b"xx").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.pane(PaneId::Left).sort, Sort::NameAsc);
        // name -> ext
        app.dispatch(Command::CycleSortKey).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::ExtAsc);
        // ext -> size (desc): largest first = a(3), z(2), b(1)
        app.dispatch(Command::CycleSortKey).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::SizeDesc);
        assert_eq!(app.pane(PaneId::Left).listing.entries[0].name.as_str(), "a");
    }

    #[tokio::test]
    async fn toggle_sort_reverse_flips_name_order() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::ToggleSortReverse).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).sort, Sort::NameDesc);
        assert_eq!(app.pane(PaneId::Left).listing.entries[0].name.as_str(), "c");
    }

    #[tokio::test]
    async fn set_filter_applies_glob_and_resets_cursor() {
        // FR-003, FR-003a, FR-004.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a.rs"), b"").await.unwrap();
        fs::write(td_l.path().join("b.rs"), b"").await.unwrap();
        fs::write(td_l.path().join("c.md"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        let p = app.active_pane_state();
        assert_eq!(p.visible_indices().len(), 2);
        assert_eq!(p.cursor, 1); // first real match, past the `..` row
        assert!(p.filter.is_some());
    }

    #[tokio::test]
    async fn set_filter_bare_word_is_substring() {
        // FR-003a.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("cargo.toml"), b"")
            .await
            .unwrap();
        fs::write(td_l.path().join("Cargo.lock"), b"")
            .await
            .unwrap();
        fs::write(td_l.path().join("readme.md"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("car").unwrap(); // case-insensitive substring
        assert_eq!(app.active_pane_state().visible_indices().len(), 2);
    }

    #[tokio::test]
    async fn set_filter_only_affects_active_pane() {
        // FR-009.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a.rs"), b"").await.unwrap();
        fs::write(td_r.path().join("x.rs"), b"").await.unwrap();
        fs::write(td_r.path().join("y.md"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        // Right (inactive) pane is unfiltered: both entries still visible.
        assert!(app.pane(PaneId::Right).filter.is_none());
        assert_eq!(app.pane(PaneId::Right).visible_indices().len(), 2);
    }

    #[tokio::test]
    async fn set_filter_persists_across_navigation() {
        // FR-003c: filter survives a directory change.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        fs::write(td_l.path().join("sub/keep.rs"), b"")
            .await
            .unwrap();
        fs::write(td_l.path().join("sub/skip.md"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        app.quick_cd("sub").await.unwrap();
        let p = app.active_pane_state();
        assert!(p.filter.is_some(), "filter must persist across navigation");
        assert_eq!(p.visible_indices().len(), 1); // only keep.rs
    }

    #[tokio::test]
    async fn set_filter_empty_clears_existing_filter() {
        // FR-005 (set then clear via empty input).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a.rs"), b"").await.unwrap();
        fs::write(td_l.path().join("b.md"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        assert_eq!(app.active_pane_state().visible_indices().len(), 1);
        app.set_filter("").unwrap();
        let p = app.active_pane_state();
        assert!(p.filter.is_none());
        assert_eq!(p.visible_indices().len(), 2); // full listing restored
        assert_eq!(p.cursor, 1); // first real entry, past the `..` row
    }

    #[tokio::test]
    async fn set_filter_whitespace_clears_and_noop_when_none() {
        // FR-005: whitespace-only == empty; clearing when none is a safe no-op.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert!(app.active_pane_state().filter.is_none());
        app.set_filter("   ").unwrap(); // no-op clear, no error
        assert!(app.active_pane_state().filter.is_none());
    }

    #[tokio::test]
    async fn set_filter_invalid_pattern_leaves_pane_unchanged() {
        // FR-006, SC-003: invalid glob errors atomically.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a.rs"), b"").await.unwrap();
        fs::write(td_l.path().join("b.md"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        let before = app.active_pane_state().visible_indices();
        let err = app.set_filter("[").unwrap_err();
        assert!(matches!(err, AppError::BadFilter(_)));
        let p = app.active_pane_state();
        // Prior filter intact, listing unchanged.
        assert_eq!(p.filter.as_ref().map(|f| f.pattern()), Some("*.rs"));
        assert_eq!(p.visible_indices(), before);
    }

    #[tokio::test]
    async fn toggle_panel_filter_dispatch_is_noop_in_core() {
        // Feature 033: the TUI intercepts Alt-! to open the dialog; core
        // dispatch is a no-op (R-007).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("*.rs").unwrap();
        let events = app.dispatch(Command::TogglePanelFilter).await.unwrap();
        assert!(events.is_empty());
        // Filter untouched by the no-op dispatch.
        assert!(app.active_pane_state().filter.is_some());
    }

    #[tokio::test]
    async fn sync_other_panel_path_copies_other_pane_cwd_into_active() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_r.path().join("over-there"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Active = Left initially. Sync into left from right.
        app.dispatch(Command::SyncOtherPanelPath).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, app.pane(PaneId::Right).cwd);
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 1);
    }

    #[tokio::test]
    async fn show_focused_in_other_panel_navigates_other_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        tokio::fs::write(td_l.path().join("sub/x"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Cursor on "sub" — but it's the only entry, so cursor=0 hits it.
        app.dispatch(Command::ShowFocusedInOtherPanel)
            .await
            .unwrap();
        // Right pane (other) now shows sub/'s contents.
        assert!(app.pane(PaneId::Right).cwd.display().ends_with("/sub"));
        assert_eq!(app.pane(PaneId::Right).listing.entries.len(), 1);
        // Active pane unchanged per FR-014.
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn show_focused_in_other_panel_is_noop_on_file() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_l.path().join("file"), b"")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app
            .dispatch(Command::ShowFocusedInOtherPanel)
            .await
            .unwrap();
        // Status event but no PaneUpdated.
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
        assert!(!events.iter().any(|e| matches!(e, Event::PaneUpdated(_))));
    }

    #[tokio::test]
    async fn quick_cd_popup_dispatch_is_noop_in_core() {
        // Feature 038: the UI intercepts Alt-c to open the dialog; reaching
        // core's dispatch is a no-op (no status stub anymore).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::QuickCdPopup).await.unwrap();
        assert!(events.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_absolute_path_navigates_active_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let target = format!("{}/sub", td_l.path().to_str().unwrap());
        app.quick_cd(&target).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
        // Inactive pane untouched (FR-013).
        assert!(app
            .pane(PaneId::Right)
            .cwd
            .display()
            .ends_with(td_r.path().file_name().unwrap().to_str().unwrap()));
    }

    #[tokio::test]
    async fn quick_cd_relative_path_resolves_against_cwd() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.quick_cd("sub").await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
    }

    #[tokio::test]
    async fn quick_cd_dotdot_ascends() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();
        app.quick_cd("sub").await.unwrap();
        app.quick_cd("..").await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, base);
    }

    #[tokio::test]
    async fn quick_cd_trailing_slash_ignored() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.quick_cd("sub/").await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/sub"));
    }

    #[tokio::test]
    async fn quick_cd_records_history() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("sub")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();
        app.quick_cd("sub").await.unwrap();
        assert_eq!(app.pane(PaneId::Left).dir_history_back, vec![base]);
    }

    #[tokio::test]
    async fn quick_cd_empty_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let evs = app.quick_cd("   ").await.unwrap();
        assert!(evs.is_empty());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_nonexistent_errors_without_navigating() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let res = app.quick_cd("/no/such/place/xyz123").await;
        assert!(res.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());
    }

    #[tokio::test]
    async fn quick_cd_file_target_errors_without_navigating() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("afile"), b"x").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.pane(PaneId::Left).cwd.clone();
        let res = app.quick_cd("afile").await;
        assert!(res.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, before);
    }

    #[tokio::test]
    async fn complete_cd_unique_prefix_single_candidate() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        fs::create_dir(td_l.path().join("docs")).await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("sr").await;
        assert_eq!(c.len(), 1);
        assert!(c[0].ends_with("/src"));
    }

    #[tokio::test]
    async fn complete_cd_multiple_matches_in_sort_order() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for d in ["app", "apple", "apply"] {
            fs::create_dir(td_l.path().join(d)).await.unwrap();
        }
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("app").await;
        assert_eq!(c.len(), 3);
        assert!(c[0].ends_with("/app"));
        assert!(c[1].ends_with("/apple"));
        assert!(c[2].ends_with("/apply"));
    }

    #[tokio::test]
    async fn complete_cd_excludes_files() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("data")).await.unwrap();
        fs::write(td_l.path().join("database"), b"x").await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        let c = app.complete_cd("dat").await;
        assert_eq!(c.len(), 1);
        assert!(c[0].ends_with("/data"));
    }

    #[tokio::test]
    async fn complete_cd_recent_dir_ordered_first() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("alpha")).await.unwrap();
        fs::create_dir(td_l.path().join("apex")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Visit alpha then ascend, so dir_history_back contains [base, alpha].
        app.quick_cd("alpha").await.unwrap();
        app.quick_cd("..").await.unwrap();
        let c = app.complete_cd("a").await;
        // alpha is recent → first; apex is filesystem-only → after; deduped.
        assert_eq!(c.len(), 2, "got {c:?}");
        assert!(c[0].ends_with("/alpha"), "recent dir must lead: {c:?}");
        assert!(c[1].ends_with("/apex"), "{c:?}");
    }

    #[tokio::test]
    async fn complete_cd_no_match_is_empty() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert!(app.complete_cd("zzz").await.is_empty());
    }

    /// SC-006 injected-input gate (T1.25 origin): drive the full quick-cd
    /// flow against the engine — complete → accept (success), a cancel
    /// path (read-only, zero side effects), and error-recovery (bad path
    /// rejected without mutation, then a valid accept succeeds).
    #[tokio::test]
    async fn quick_cd_end_to_end_complete_accept_cancel_and_recover() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("src")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let base = app.pane(PaneId::Left).cwd.clone();

        // --- Cancel path: completing is read-only; not accepting leaves
        // both panes untouched (SC-004).
        let candidates = app.complete_cd("sr").await;
        assert_eq!(candidates.len(), 1);
        assert!(candidates[0].ends_with("/src"));
        assert_eq!(
            app.pane(PaneId::Left).cwd,
            base,
            "complete_cd mutated state"
        );
        assert!(app.pane(PaneId::Left).dir_history_back.is_empty());

        // --- Error-recovery: accept a bad path → Err, no nav; then a
        // valid accept → Ok (SC-005 then SC-001).
        let bad = app.quick_cd("/no/such/dir/zzz").await;
        assert!(bad.is_err());
        assert_eq!(app.pane(PaneId::Left).cwd, base, "bad accept navigated");

        // --- Accept the completed candidate → active pane moves (SC-001),
        // previous cwd recorded (FR-005), other pane unchanged (FR-013).
        let right_before = app.pane(PaneId::Right).cwd.clone();
        app.quick_cd(&candidates[0]).await.unwrap();
        assert!(app.pane(PaneId::Left).cwd.display().ends_with("/src"));
        assert_eq!(app.pane(PaneId::Left).dir_history_back, vec![base]);
        assert_eq!(app.pane(PaneId::Right).cwd, right_before);
    }

    #[tokio::test]
    async fn ascend_from_zip_root_returns_to_local_parent() {
        use cargonaut_vfs::ZipFs;
        use std::io::Write;

        // Create a valid empty zip inside a temp directory.
        let td_archive = TempDir::new().unwrap();
        let zip_path = td_archive.path().join("archive.zip");
        {
            let mut f = std::fs::File::create(&zip_path).unwrap();
            // Minimal EOCD record — 0 entries, valid zip.
            f.write_all(&[
                0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ])
            .unwrap();
        }

        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;

        // Mount the zip archive in the active (left) pane.
        let zip_fs = ZipFs::open(zip_path.clone()).unwrap();
        let encoded_auth: String = zip_path
            .to_str()
            .unwrap()
            .chars()
            .flat_map(|c| match c {
                '%' => "%25".chars().collect::<Vec<_>>(),
                '/' => "%2F".chars().collect(),
                other => vec![other],
            })
            .collect();
        let zip_url = format!("zip://{encoded_auth}/");
        let zip_vfs = VfsPath::parse(&zip_url).unwrap();
        app.navigate_into(PaneId::Left, zip_vfs, std::sync::Arc::new(zip_fs))
            .await
            .unwrap();

        // Confirm we are at the zip:// root (empty segments).
        assert_eq!(app.pane(PaneId::Left).cwd.scheme.as_str(), "zip");
        assert!(app.pane(PaneId::Left).cwd.segments.is_empty());

        // Ascend — must pop out of the zip and land in the local parent dir.
        app.dispatch(Command::Ascend).await.unwrap();

        let pane = app.pane(PaneId::Left);
        assert_eq!(
            pane.cwd.scheme.as_str(),
            "file",
            "Ascend from zip root must restore file:// backend; cwd={}",
            pane.cwd.display()
        );
        // The parent directory of the archive is td_archive.path().
        let expected_parent = td_archive.path().to_str().unwrap();
        assert!(
            pane.cwd.display().ends_with(expected_parent)
                || pane.cwd.display().contains(expected_parent),
            "cwd after Ascend must be the archive's parent dir {expected_parent:?}; got {}",
            pane.cwd.display()
        );
    }
}

// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `hotlist` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    /// Read-only view of the saved bookmarks (UI snapshot source).
    pub fn bookmarks(&self) -> &[cargonaut_config::Bookmark] {
        &self.hotlist.bookmarks
    }

    /// Add the **active pane's current directory** as a new bookmark under
    /// `name` (and optional `group`), then persist. A blank name is rejected
    /// ([`AppError::BadBookmark`], FR-011) and nothing is saved. Duplicate
    /// names are allowed to coexist.
    pub fn add_bookmark(
        &mut self,
        name: &str,
        group: Option<&str>,
    ) -> Result<Vec<Event>, AppError> {
        let name = name.trim();
        if name.is_empty() {
            return Err(AppError::BadBookmark("name must not be blank".into()));
        }
        let path = self.active_pane_state().cwd.display();
        self.hotlist.add(cargonaut_config::Bookmark {
            name: name.to_string(),
            path,
            group: group
                .map(|g| g.trim().to_string())
                .filter(|g| !g.is_empty()),
        });
        self.persist_hotlist();
        Ok(vec![Event::Status(format!("Bookmarked: {name}"))])
    }

    /// Remove the bookmark at `index` and persist. Out-of-range ⇒
    /// [`AppError::BadBookmark`] with no change.
    pub fn remove_bookmark(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        if index >= self.hotlist.bookmarks.len() {
            return Err(AppError::BadBookmark(format!(
                "no bookmark at index {index}"
            )));
        }
        let removed = self.hotlist.bookmarks[index].name.clone();
        self.hotlist.remove(index);
        self.persist_hotlist();
        Ok(vec![Event::Status(format!("Removed bookmark: {removed}"))])
    }

    /// Navigate the active pane to the bookmark at `index`, reusing
    /// [`Self::quick_cd`] (so a missing/invalid target is reported without
    /// mutating pane state — FR-008 — and directory history is recorded).
    /// Out-of-range ⇒ [`AppError::BadBookmark`].
    pub async fn jump_to_bookmark(&mut self, index: usize) -> Result<Vec<Event>, AppError> {
        let path = self
            .hotlist
            .bookmarks
            .get(index)
            .ok_or_else(|| AppError::BadBookmark(format!("no bookmark at index {index}")))?
            .path
            .clone();
        self.quick_cd(&path).await
    }

    /// Best-effort persist of the hotlist; a write failure is logged, not fatal.
    pub(crate) fn persist_hotlist(&self) {
        if let Err(e) = self.hotlist.save(&self.hotlist_path) {
            tracing::warn!("could not save hotlist to {:?}: {e}", self.hotlist_path);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn add_bookmark_uses_active_cwd_and_persists() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let hl_file = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.hotlist_path = hl_file.path().join("hotlist.toml");

        app.add_bookmark("proj", Some("work")).unwrap();
        assert_eq!(app.bookmarks().len(), 1);
        assert_eq!(app.bookmarks()[0].name, "proj");
        assert_eq!(app.bookmarks()[0].group.as_deref(), Some("work"));
        // path is the active (left) pane's cwd.
        assert!(app.bookmarks()[0]
            .path
            .contains(td_l.path().to_str().unwrap()));
        // persisted to disk.
        let on_disk = std::fs::read_to_string(&app.hotlist_path).unwrap();
        assert!(on_disk.contains("proj"), "file: {on_disk}");
    }

    #[tokio::test]
    async fn add_bookmark_rejects_blank_name() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let res = app.add_bookmark("   ", None);
        assert!(matches!(res, Err(AppError::BadBookmark(_))));
        assert_eq!(app.bookmarks().len(), 0);
    }

    #[tokio::test]
    async fn add_bookmark_allows_duplicate_names() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let hl_file = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.hotlist_path = hl_file.path().join("hotlist.toml");
        app.add_bookmark("dup", None).unwrap();
        app.add_bookmark("dup", None).unwrap();
        assert_eq!(app.bookmarks().len(), 2);
    }

    #[tokio::test]
    async fn bookmarks_persist_and_reload(/* SC-002 */) {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let hl_file = TempDir::new().unwrap();
        let path = hl_file.path().join("hotlist.toml");
        let mut app = make_app(&td_l, &td_r).await;
        app.hotlist_path = path.clone();
        app.add_bookmark("proj", Some("work")).unwrap();
        app.add_bookmark("scratch", None).unwrap();

        // Reload from the same file (simulates a fresh session's App::new load).
        let reloaded = cargonaut_config::Hotlist::load(&path);
        assert_eq!(reloaded.bookmarks, app.bookmarks());
        assert_eq!(reloaded.bookmarks[0].name, "proj");
        assert_eq!(reloaded.bookmarks[0].group.as_deref(), Some("work"));
        assert_eq!(reloaded.bookmarks[1].group, None);
    }

    #[tokio::test]
    async fn remove_bookmark_drops_and_persists(/* SC-005 */) {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let hl_file = TempDir::new().unwrap();
        let path = hl_file.path().join("hotlist.toml");
        let mut app = make_app(&td_l, &td_r).await;
        app.hotlist_path = path.clone();
        app.add_bookmark("a", None).unwrap();
        app.add_bookmark("b", None).unwrap();

        app.remove_bookmark(0).unwrap();
        assert_eq!(app.bookmarks().len(), 1);
        assert_eq!(app.bookmarks()[0].name, "b");
        // gone on reload too.
        assert_eq!(cargonaut_config::Hotlist::load(&path).bookmarks.len(), 1);
        // out-of-range is a clean error, no panic.
        assert!(matches!(
            app.remove_bookmark(9),
            Err(AppError::BadBookmark(_))
        ));
    }

    #[tokio::test]
    async fn jump_to_missing_target_is_graceful_and_retains_bookmark(/* FR-008/SC-004 */) {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let before = app.active_pane_state().cwd.clone();
        app.hotlist.add(cargonaut_config::Bookmark {
            name: "gone".into(),
            path: "file:///no/such/cargonaut/dir/xyz".into(),
            group: None,
        });
        let res = app.jump_to_bookmark(0).await;
        assert!(res.is_err(), "jumping to a missing dir must error");
        // panes unchanged, bookmark retained.
        assert_eq!(app.active_pane_state().cwd, before);
        assert_eq!(app.bookmarks().len(), 1);
    }

    #[tokio::test]
    async fn jump_to_bookmark_navigates_active_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let sub = td_l.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.hotlist.add(cargonaut_config::Bookmark {
            name: "s".into(),
            path: format!("file://{}", sub.to_str().unwrap()),
            group: None,
        });
        app.jump_to_bookmark(0).await.unwrap();
        assert!(app.active_pane_state().cwd.display().ends_with("sub"));
    }
}

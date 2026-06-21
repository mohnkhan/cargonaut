// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `compare` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    /// Feature 049 — Compare both panels' visible listings and additively tag
    /// all differing entries (FR-001 through FR-004, FR-009, FR-010).
    pub(crate) fn compare_directories(&mut self) -> Result<Vec<Event>, AppError> {
        // Both panes must be local-filesystem directories.
        let left_scheme = self.pane(PaneId::Left).cwd.scheme.clone();
        let right_scheme = self.pane(PaneId::Right).cwd.scheme.clone();
        if left_scheme.as_str() != "file" || right_scheme.as_str() != "file" {
            return Ok(vec![Event::Status(
                "Compare requires both panels to show local (file://) directories".into(),
            )]);
        }

        // Detect same-directory case.
        let left_cwd = self.pane(PaneId::Left).cwd.clone();
        let right_cwd = self.pane(PaneId::Right).cwd.clone();
        if left_cwd == right_cwd {
            return Ok(vec![Event::Status(
                "Both panels point to the same directory — compare would mark nothing".into(),
            )]);
        }

        // Snapshot visible indices and entry data from both panes.
        // We work with plain vecs to avoid borrow-checker conflicts on `self.pane_mut`.
        let left_entries: Vec<(usize, String, u64, bool)> = {
            let p = self.pane(PaneId::Left);
            p.visible_indices()
                .into_iter()
                .filter_map(|idx| {
                    let e = p.listing.entries.get(idx)?;
                    let is_dir = matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir);
                    Some((idx, e.name.to_string(), e.meta.size, is_dir))
                })
                .collect()
        };
        let right_entries: Vec<(usize, String, u64, bool)> = {
            let p = self.pane(PaneId::Right);
            p.visible_indices()
                .into_iter()
                .filter_map(|idx| {
                    let e = p.listing.entries.get(idx)?;
                    let is_dir = matches!(e.meta.kind, cargonaut_vfs::VfsKind::Dir);
                    Some((idx, e.name.to_string(), e.meta.size, is_dir))
                })
                .collect()
        };

        let total_visible = left_entries.len() + right_entries.len();
        let mut events: Vec<Event> = Vec::new();

        // FR-009: progress indicator for >1,000 visible entries.
        if total_visible > 1_000 {
            events.push(Event::Status("Comparing\u{2026}".into()));
        }

        // Build name → (index, size, is_dir) maps for O(n) lookup.
        let left_map: HashMap<&str, (usize, u64, bool)> = left_entries
            .iter()
            .map(|(idx, name, size, is_dir)| (name.as_str(), (*idx, *size, *is_dir)))
            .collect();
        let right_map: HashMap<&str, (usize, u64, bool)> = right_entries
            .iter()
            .map(|(idx, name, size, is_dir)| (name.as_str(), (*idx, *size, *is_dir)))
            .collect();

        // Walk left pane: left-only or differing entries.
        let mut left_tags: Vec<usize> = Vec::new();
        let mut right_tags: Vec<usize> = Vec::new();

        for (l_idx, name, l_size, l_is_dir) in &left_entries {
            if let Some((r_idx, r_size, r_is_dir)) = right_map.get(name.as_str()) {
                // Present on both sides.
                if *l_is_dir && *r_is_dir {
                    // Dirs compared by name-presence only (FR-010); same name = identical.
                    continue;
                }
                if l_size != r_size {
                    // size-differ
                    left_tags.push(*l_idx);
                    right_tags.push(*r_idx);
                } else {
                    // Same size — check content hash.
                    let l_path_str = self.pane(PaneId::Left).cwd.join(name).display();
                    let r_path_str = self.pane(PaneId::Right).cwd.join(name).display();
                    let l_local = l_path_str
                        .strip_prefix("file://")
                        .unwrap_or(&l_path_str)
                        .to_string();
                    let r_local = r_path_str
                        .strip_prefix("file://")
                        .unwrap_or(&r_path_str)
                        .to_string();
                    let lh = crc32_partial(std::path::Path::new(&l_local), *l_size);
                    let rh = crc32_partial(std::path::Path::new(&r_local), *r_size);
                    if lh != rh {
                        // hash-differ or unreadable
                        left_tags.push(*l_idx);
                        right_tags.push(*r_idx);
                    }
                    // identical → no tags
                }
            } else {
                // left-only
                left_tags.push(*l_idx);
            }
        }

        // Walk right pane: right-only entries.
        for (r_idx, name, _, _) in &right_entries {
            if !left_map.contains_key(name.as_str()) {
                right_tags.push(*r_idx);
            }
        }

        // Additively insert tags (never clear existing selections — FR-004).
        let differ_count = left_tags.len() + right_tags.len();
        for idx in left_tags {
            self.pane_mut(PaneId::Left).selected.insert(idx);
        }
        for idx in right_tags {
            self.pane_mut(PaneId::Right).selected.insert(idx);
        }

        events.push(Event::PaneUpdated(PaneId::Left));
        events.push(Event::PaneUpdated(PaneId::Right));
        if differ_count == 0 {
            events.push(Event::Status("All visible entries are identical".into()));
        } else {
            events.push(Event::Status(format!("{differ_count} entries differ")));
        }
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn compare_left_only_tags_left_pane_only() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("only_left.txt"), b"x").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::CompareDirectories).await.unwrap();
        let statuses: Vec<_> = events
            .iter()
            .filter_map(|e| {
                if let Event::Status(s) = e {
                    Some(s.clone())
                } else {
                    None
                }
            })
            .collect();
        let has_status = statuses
            .iter()
            .any(|s| s.contains("differ") || s.contains("differ") || s.contains("1"));
        assert!(has_status, "expected status message; got: {statuses:?}");
        let left_sel = &app.pane(PaneId::Left).selected;
        assert!(
            !left_sel.is_empty(),
            "left-only file must be tagged in left pane"
        );
        let right_sel = &app.pane(PaneId::Right).selected;
        assert!(
            right_sel.is_empty(),
            "right pane should have no tags for a left-only file"
        );
    }

    #[tokio::test]
    async fn compare_right_only_tags_right_pane_only() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_r.path().join("only_right.txt"), b"x").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        app.dispatch(Command::CompareDirectories).await.unwrap();
        assert!(app.pane(PaneId::Left).selected.is_empty());
        assert!(!app.pane(PaneId::Right).selected.is_empty());
    }

    #[tokio::test]
    async fn compare_size_differ_tags_both_panes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("f.txt"), b"aaa").unwrap();
        std::fs::write(td_r.path().join("f.txt"), b"bb").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        app.dispatch(Command::CompareDirectories).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).selected.is_empty(),
            "left pane must be tagged for size-differ"
        );
        assert!(
            !app.pane(PaneId::Right).selected.is_empty(),
            "right pane must be tagged for size-differ"
        );
    }

    #[tokio::test]
    async fn compare_hash_differ_tags_both_panes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Same size (3 bytes) but different content → hash-differ
        std::fs::write(td_l.path().join("f.txt"), b"aaa").unwrap();
        std::fs::write(td_r.path().join("f.txt"), b"bbb").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        app.dispatch(Command::CompareDirectories).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).selected.is_empty(),
            "left pane must be tagged for hash-differ"
        );
        assert!(
            !app.pane(PaneId::Right).selected.is_empty(),
            "right pane must be tagged for hash-differ"
        );
    }

    #[tokio::test]
    async fn compare_identical_entries_not_tagged() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("same.txt"), b"identical content").unwrap();
        std::fs::write(td_r.path().join("same.txt"), b"identical content").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        app.dispatch(Command::CompareDirectories).await.unwrap();
        assert!(
            app.pane(PaneId::Left).selected.is_empty(),
            "identical file must NOT be tagged in left"
        );
        assert!(
            app.pane(PaneId::Right).selected.is_empty(),
            "identical file must NOT be tagged in right"
        );
    }

    #[tokio::test]
    async fn compare_same_path_both_panels_returns_status_no_tags() {
        let td = TempDir::new().unwrap();
        // Both panes pointed at same directory
        let mut app = App::new(
            cargonaut_config::Config::default(),
            td.path().to_str().unwrap(),
            td.path().to_str().unwrap(),
        )
        .await
        .unwrap();
        let events = app.dispatch(Command::CompareDirectories).await.unwrap();
        let has_same_dir_status = events.iter().any(|e| {
            if let Event::Status(s) = e {
                s.contains("same directory")
                    || s.contains("same path")
                    || s.contains("mark nothing")
            } else {
                false
            }
        });
        assert!(
            has_same_dir_status,
            "must return a status warning when both panes are the same dir; got {events:?}"
        );
        assert!(app.pane(PaneId::Left).selected.is_empty());
        assert!(app.pane(PaneId::Right).selected.is_empty());
    }

    #[tokio::test]
    async fn compare_additive_does_not_clear_existing_selection() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"x").unwrap();
        std::fs::write(td_l.path().join("only.txt"), b"y").unwrap();
        let mut app = make_compare_app(&td_l, &td_r).await;
        // Manually tag entry 0 (the first visible entry) in left pane
        let first_idx = {
            let p = app.pane(PaneId::Left);
            p.visible_indices().into_iter().next().unwrap_or(0)
        };
        // Use SelectionToggle to pre-tag it
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).selected.is_empty(),
            "pre-condition: left pane has a tagged entry"
        );
        let pre_selected = app.pane(PaneId::Left).selected.clone();
        // Now compare — should be additive, not clear existing tags
        app.dispatch(Command::CompareDirectories).await.unwrap();
        for idx in &pre_selected {
            assert!(
                app.pane(PaneId::Left).selected.contains(idx),
                "pre-existing tag at index {first_idx} was cleared by compare — must be additive"
            );
        }
    }

    #[tokio::test]
    async fn compare_large_dir_emits_status_comparing_first() {
        // >1000 visible entries should emit Status("Comparing…") as the first event.
        // We use a small threshold test — the impl must check visible count, not file count.
        // For efficiency in tests, we just verify the dispatch returns Ok (full 1000-file bench
        // is in T010). This test checks the event ordering contract.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Create 1001 files in left only to ensure >1000 visible entries total
        for i in 0..=1000usize {
            std::fs::write(td_l.path().join(format!("f{i:04}.txt")), b"x").unwrap();
        }
        let mut app = make_compare_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::CompareDirectories).await.unwrap();
        // First event should be Status("Comparing…")
        if let Some(Event::Status(s)) = events.first() {
            assert!(
                s.contains("Comparing"),
                "first event for >1000 entries must be Status(\"Comparing…\"); got {s:?}"
            );
        } else {
            panic!("expected first event to be Status(\"Comparing…\") for >1000 visible entries; got {:?}", events.first());
        }
    }
}

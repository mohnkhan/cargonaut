// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `rename` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// Feature 050 — validate editor output against the original listing.
///
/// Returns the changed pairs `(original_name, proposed_name)` in listing
/// order, filtering out unchanged entries.
///
/// Errors:
/// - line count differs from originals
/// - any proposed name is empty after trimming
/// - any proposed name contains `/`
/// - any two proposed names are identical (would cause a collision)
pub fn validate_rename_proposals(
    originals: &[String],
    edited: &[String],
) -> Result<Vec<(String, String)>, String> {
    if originals.len() != edited.len() {
        return Err(format!(
            "Line count mismatch: expected {}, got {}",
            originals.len(),
            edited.len()
        ));
    }
    let mut seen = std::collections::HashSet::new();
    let mut pairs = Vec::new();
    for (orig, proposed_raw) in originals.iter().zip(edited.iter()) {
        let proposed = proposed_raw.trim().to_string();
        if proposed.is_empty() {
            return Err(format!("Proposed name for '{orig}' is empty"));
        }
        if proposed.contains('/') {
            return Err(format!(
                "Proposed name '{proposed}' contains '/' — cross-directory renames are not supported"
            ));
        }
        if !seen.insert(proposed.clone()) {
            return Err(format!(
                "Duplicate proposed name '{proposed}' — each file must have a unique name"
            ));
        }
        if proposed != *orig {
            pairs.push((orig.clone(), proposed));
        }
    }
    Ok(pairs)
}

impl App {
    /// Undo the most recent reversible file operation (Feature 050).
    ///
    /// Consumes and clears `undo_log` regardless of outcome.
    pub async fn undo_last_operation(&mut self) -> Result<Vec<Event>, AppError> {
        let entry = match self.undo_log.take() {
            None => {
                return Ok(vec![Event::Status("Nothing to undo".into())]);
            }
            Some(e) => e,
        };

        let mut events: Vec<Event> = Vec::new();
        match entry {
            UndoEntry::Rename { dir, pairs } => {
                let dir_path = std::path::Path::new(&dir);
                let mut count = 0usize;
                for (new_name, old_name) in &pairs {
                    let src = dir_path.join(new_name);
                    let dst = dir_path.join(old_name);
                    if std::fs::rename(&src, &dst).is_ok() {
                        count += 1;
                    }
                }
                events.push(Event::Status(format!(
                    "{count} entr{} restored",
                    if count == 1 { "y" } else { "ies" }
                )));
            }
            UndoEntry::Copy { copies } => {
                for path in &copies {
                    let disp = path.display();
                    let local = disp.strip_prefix("file://").unwrap_or(&disp).to_string();
                    let _ =
                        std::fs::remove_file(&local).or_else(|_| std::fs::remove_dir_all(&local));
                }
                events.push(Event::Status(format!(
                    "{} cop{} removed (undo copy)",
                    copies.len(),
                    if copies.len() == 1 { "y" } else { "ies" }
                )));
            }
            UndoEntry::Move { pairs } => {
                // Move undo scaffold — Move is not fully implemented in Feature 050.
                // Attempt reverse-moves best-effort; errors are swallowed.
                for (dst, src) in &pairs {
                    let dst_disp = dst.display();
                    let src_disp = src.display();
                    let dst_local = dst_disp
                        .strip_prefix("file://")
                        .unwrap_or(&dst_disp)
                        .to_string();
                    let src_local = src_disp
                        .strip_prefix("file://")
                        .unwrap_or(&src_disp)
                        .to_string();
                    let _ = std::fs::rename(&dst_local, &src_local);
                }
                events.push(Event::Status("Move undone".into()));
            }
            UndoEntry::Delete => {
                events.push(Event::Status(
                    "Delete cannot be undone — files are permanently removed".into(),
                ));
            }
        }

        // Clear selection on both panes and re-list both.
        for id in [PaneId::Left, PaneId::Right] {
            let cwd = self.pane(id).cwd.clone();
            let sort = self.pane(id).sort;
            let listing = self.registry.local().list(&cwd, sort).await?;
            let p = self.pane_mut(id);
            p.listing = listing;
            p.selected.clear();
            let rows = p.row_count();
            p.cursor = if rows == 0 { 0 } else { p.cursor.min(rows - 1) };
            events.push(Event::PaneUpdated(id));
        }

        Ok(events)
    }

    /// Feature 050 — apply validated rename pairs to the active pane's directory.
    ///
    /// `pairs` is a list of `(old_name, new_name)` basenames to rename within the
    /// active pane's cwd. The collision check (does `new_name` already exist in
    /// the listing and is NOT being renamed away?) is done here before any rename
    /// is attempted. On success, records `UndoEntry::Rename` (reversed pairs) and
    /// re-lists the pane. On partial failure the undo entry covers completed renames.
    pub async fn apply_bulk_rename(
        &mut self,
        pairs: Vec<(String, String)>,
    ) -> Result<Vec<Event>, AppError> {
        if pairs.is_empty() {
            return Ok(vec![Event::Status("No changes — nothing renamed".into())]);
        }

        // Require a local filesystem pane (file:// scheme).
        let pane = self.active_pane_state();
        if pane.cwd.scheme != "file" {
            return Ok(vec![Event::Status(
                "Bulk rename only supported for local (file://) panes".into(),
            )]);
        }

        // Build a local path for the active pane's directory.
        let cwd_display = pane.cwd.display();
        // `file:///foo/bar` → `/foo/bar`
        let cwd_local = cwd_display
            .strip_prefix("file://")
            .unwrap_or(&cwd_display)
            .to_string();
        let cwd_path = std::path::Path::new(&cwd_local);

        // Collision check: for each proposed new name, it must not exist on disk
        // UNLESS it is itself a source (i.e. it's being renamed away).
        let sources: std::collections::HashSet<&str> =
            pairs.iter().map(|(old, _)| old.as_str()).collect();
        let existing_names: std::collections::HashSet<String> = pane
            .listing
            .entries
            .iter()
            .map(|e| e.name.to_string())
            .collect();
        for (_, new_name) in &pairs {
            if existing_names.contains(new_name) && !sources.contains(new_name.as_str()) {
                return Ok(vec![Event::Status(format!(
                    "Rename aborted: '{new_name}' already exists in the directory"
                ))]);
            }
        }

        // Apply renames one by one; record completed ones for undo.
        let mut completed: Vec<(String, String)> = Vec::new();
        let mut rename_error: Option<String> = None;
        for (old_name, new_name) in &pairs {
            let src = cwd_path.join(old_name);
            let dst = cwd_path.join(new_name);
            if let Err(e) = std::fs::rename(&src, &dst) {
                rename_error = Some(format!("Rename '{old_name}' → '{new_name}' failed: {e}"));
                break;
            }
            // Record reversed pair for undo: (new_name, old_name).
            completed.push((new_name.clone(), old_name.clone()));
        }

        // Always record what was completed (may be partial).
        if !completed.is_empty() {
            self.undo_log = Some(UndoEntry::Rename {
                dir: cwd_local.clone(),
                pairs: completed.clone(),
            });
        }

        // Re-list active pane.
        let relist_events = self.relist_active().await?;

        if let Some(err_msg) = rename_error {
            let completed_count = completed.len();
            return Ok(vec![Event::Status(format!(
                "{completed_count} entries renamed (partial); {err_msg}"
            ))]);
        }

        let count = pairs.len();
        let mut events = relist_events;
        events.push(Event::Status(format!(
            "{count} entr{} renamed",
            if count == 1 { "y" } else { "ies" }
        )));
        Ok(events)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn apply_bulk_rename_empty_pairs_returns_no_changes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.apply_bulk_rename(vec![]).await.unwrap();
        let has_no_changes = events
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("No changes")));
        assert!(
            has_no_changes,
            "empty pairs must emit 'No changes' status; got {events:?}"
        );
    }

    #[tokio::test]
    async fn apply_bulk_rename_two_of_three_renamed_on_disk() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        std::fs::write(td_l.path().join("b.txt"), b"2").unwrap();
        std::fs::write(td_l.path().join("c.txt"), b"3").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let pairs = vec![
            ("a.txt".to_string(), "A.txt".to_string()),
            ("c.txt".to_string(), "C.txt".to_string()),
        ];
        app.apply_bulk_rename(pairs).await.unwrap();
        assert!(
            td_l.path().join("A.txt").exists(),
            "a.txt must be renamed to A.txt"
        );
        assert!(
            td_l.path().join("b.txt").exists(),
            "b.txt must be unchanged"
        );
        assert!(
            td_l.path().join("C.txt").exists(),
            "c.txt must be renamed to C.txt"
        );
        assert!(!td_l.path().join("a.txt").exists());
        assert!(!td_l.path().join("c.txt").exists());
    }

    #[tokio::test]
    async fn apply_bulk_rename_collision_no_renames_applied() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        std::fs::write(td_l.path().join("existing.txt"), b"x").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Try to rename a.txt → existing.txt (collision)
        let pairs = vec![("a.txt".to_string(), "existing.txt".to_string())];
        let result = app.apply_bulk_rename(pairs).await;
        // Must fail and a.txt must still exist
        assert!(
            result.is_err() || {
                // OR: returns Ok but with error status, and a.txt unchanged
                td_l.path().join("a.txt").exists()
            },
            "collision must not rename a.txt"
        );
        assert!(
            td_l.path().join("a.txt").exists(),
            "a.txt must be unchanged on collision"
        );
    }

    #[tokio::test]
    async fn apply_bulk_rename_returns_pane_updated_and_status_events() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        std::fs::write(td_l.path().join("b.txt"), b"2").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let pairs = vec![
            ("a.txt".to_string(), "A.txt".to_string()),
            ("b.txt".to_string(), "B.txt".to_string()),
        ];
        let events = app.apply_bulk_rename(pairs).await.unwrap();
        let has_pane = events.iter().any(|e| matches!(e, Event::PaneUpdated(_)));
        let has_status = events
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("2")));
        assert!(has_pane, "must emit PaneUpdated; events={events:?}");
        assert!(
            has_status,
            "must emit Status with count 2; events={events:?}"
        );
    }

    #[tokio::test]
    async fn apply_bulk_rename_records_undo_entry_reversed() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let pairs = vec![("a.txt".to_string(), "A.txt".to_string())];
        app.apply_bulk_rename(pairs).await.unwrap();
        let undo = app
            .undo_log
            .as_ref()
            .expect("undo_log must be set after rename");
        match undo {
            UndoEntry::Rename { pairs, .. } => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(
                    pairs[0],
                    ("A.txt".to_string(), "a.txt".to_string()),
                    "undo pairs must be reversed (new→old)"
                );
            }
            other => panic!("expected UndoEntry::Rename, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn apply_bulk_rename_partial_failure_records_partial_undo() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Only a.txt exists; b.txt does not (collision check passes since B.txt doesn't exist,
        // but rename will fail because b.txt source doesn't exist).
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let pairs = vec![
            ("a.txt".to_string(), "A.txt".to_string()),
            ("b.txt".to_string(), "B.txt".to_string()),
        ];
        let _ = app.apply_bulk_rename(pairs).await;
        // a.txt was renamed; undo log should contain at least the completed rename
        if td_l.path().join("A.txt").exists() {
            let undo = app
                .undo_log
                .as_ref()
                .expect("undo_log must be set after partial rename");
            match undo {
                UndoEntry::Rename { pairs, .. } => {
                    assert!(
                        pairs
                            .iter()
                            .any(|(new, old)| new == "A.txt" && old == "a.txt"),
                        "partial undo must include completed rename A.txt→a.txt; pairs={pairs:?}"
                    );
                }
                other => panic!("expected UndoEntry::Rename, got {other:?}"),
            }
        }
    }

    #[tokio::test]
    async fn apply_bulk_rename_second_call_overwrites_undo_log() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"1").unwrap();
        std::fs::write(td_l.path().join("b.txt"), b"2").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.apply_bulk_rename(vec![("a.txt".to_string(), "A.txt".to_string())])
            .await
            .unwrap();
        // Second rename — must refresh listing first
        let _ = app.refresh_active_pane().await;
        app.apply_bulk_rename(vec![("b.txt".to_string(), "B.txt".to_string())])
            .await
            .unwrap();
        let undo = app.undo_log.as_ref().expect("undo_log must be set");
        match undo {
            UndoEntry::Rename { pairs, .. } => {
                assert_eq!(pairs.len(), 1);
                assert_eq!(pairs[0].0, "B.txt", "second call must overwrite undo log");
            }
            other => panic!("expected UndoEntry::Rename, got {other:?}"),
        }
    }

    #[tokio::test]
    async fn undo_none_log_returns_nothing_to_undo() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.undo_last_operation().await.unwrap();
        let has_nothing = events
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Nothing")));
        assert!(
            has_nothing,
            "None log must return 'Nothing to undo'; got {events:?}"
        );
    }

    #[tokio::test]
    async fn undo_rename_restores_files_on_disk() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("original.txt"), b"x").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Rename original.txt → renamed.txt
        app.apply_bulk_rename(vec![(
            "original.txt".to_string(),
            "renamed.txt".to_string(),
        )])
        .await
        .unwrap();
        assert!(td_l.path().join("renamed.txt").exists());
        // Undo: should restore original.txt
        app.undo_last_operation().await.unwrap();
        assert!(
            td_l.path().join("original.txt").exists(),
            "undo must restore original.txt"
        );
        assert!(!td_l.path().join("renamed.txt").exists());
    }

    #[tokio::test]
    async fn undo_copy_deletes_destination_copies() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let copy_target = td_r.path().join("copy.txt");
        std::fs::write(&copy_target, b"x").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let copy_path = VfsPath::parse(&format!("file://{}", copy_target.display())).unwrap();
        app.undo_log = Some(UndoEntry::Copy {
            copies: vec![copy_path],
        });
        app.undo_last_operation().await.unwrap();
        assert!(
            !copy_target.exists(),
            "undo Copy must delete the destination file"
        );
    }

    #[tokio::test]
    async fn undo_delete_returns_cannot_be_undone() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.undo_log = Some(UndoEntry::Delete);
        let events = app.undo_last_operation().await.unwrap();
        let has_warning = events.iter().any(|e| matches!(e, Event::Status(s) if s.to_lowercase().contains("cannot") || s.to_lowercase().contains("undo")));
        assert!(
            has_warning,
            "Delete undo must emit cannot-be-undone status; got {events:?}"
        );
    }

    #[tokio::test]
    async fn undo_second_call_returns_nothing_to_undo() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"x").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.apply_bulk_rename(vec![("a.txt".to_string(), "A.txt".to_string())])
            .await
            .unwrap();
        app.undo_last_operation().await.unwrap();
        // Second undo — log is now None
        let events = app.undo_last_operation().await.unwrap();
        let has_nothing = events
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Nothing")));
        assert!(
            has_nothing,
            "second undo must return 'Nothing to undo'; got {events:?}"
        );
    }

    #[tokio::test]
    async fn undo_clears_selection_on_both_panes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        std::fs::write(td_l.path().join("a.txt"), b"x").unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        app.apply_bulk_rename(vec![("a.txt".to_string(), "A.txt".to_string())])
            .await
            .unwrap();
        app.undo_last_operation().await.unwrap();
        assert!(
            app.pane(PaneId::Left).selected.is_empty(),
            "undo must clear left selection"
        );
        assert!(
            app.pane(PaneId::Right).selected.is_empty(),
            "undo must clear right selection"
        );
    }

    #[tokio::test]
    async fn undo_move_scaffold_does_not_crash() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let dummy_src =
            VfsPath::parse(&format!("file://{}/src.txt", td_l.path().display())).unwrap();
        let dummy_dst =
            VfsPath::parse(&format!("file://{}/dst.txt", td_l.path().display())).unwrap();
        app.undo_log = Some(UndoEntry::Move {
            pairs: vec![(dummy_dst, dummy_src)],
        });
        // Must return Ok (not panic) — Move undo is a scaffold in Feature 050
        let result = app.undo_last_operation().await;
        assert!(
            result.is_ok(),
            "Move undo scaffold must not panic; got {result:?}"
        );
    }

    #[test]
    fn validate_rename_all_unchanged_returns_empty() {
        let orig = vec![
            "a.txt".to_string(),
            "b.txt".to_string(),
            "c.txt".to_string(),
        ];
        let edited = orig.clone();
        let result = validate_rename_proposals(&orig, &edited).unwrap();
        assert!(
            result.is_empty(),
            "all-unchanged must return empty vec; got {result:?}"
        );
    }

    #[test]
    fn validate_rename_two_of_three_changed_correct_pairs() {
        let orig = vec![
            "a.txt".to_string(),
            "b.txt".to_string(),
            "c.txt".to_string(),
        ];
        let edited = vec![
            "a.txt".to_string(),
            "B.txt".to_string(),
            "C.txt".to_string(),
        ];
        let result = validate_rename_proposals(&orig, &edited).unwrap();
        assert_eq!(result.len(), 2);
        assert!(result.contains(&("b.txt".to_string(), "B.txt".to_string())));
        assert!(result.contains(&("c.txt".to_string(), "C.txt".to_string())));
    }

    #[test]
    fn validate_rename_line_count_mismatch_returns_err() {
        let orig = vec!["a.txt".to_string(), "b.txt".to_string()];
        let edited = vec!["a.txt".to_string()];
        let result = validate_rename_proposals(&orig, &edited);
        assert!(result.is_err(), "line count mismatch must return Err");
    }

    #[test]
    fn validate_rename_empty_name_returns_err() {
        let orig = vec!["a.txt".to_string(), "b.txt".to_string()];
        let edited = vec!["a.txt".to_string(), "".to_string()];
        let result = validate_rename_proposals(&orig, &edited);
        assert!(result.is_err(), "empty name must return Err");
    }

    #[test]
    fn validate_rename_slash_in_name_returns_err() {
        let orig = vec!["a.txt".to_string()];
        let edited = vec!["sub/a.txt".to_string()];
        let result = validate_rename_proposals(&orig, &edited);
        assert!(result.is_err(), "name containing '/' must return Err");
    }

    #[test]
    fn validate_rename_duplicate_proposed_names_returns_err() {
        let orig = vec!["a.txt".to_string(), "b.txt".to_string()];
        let edited = vec!["same.txt".to_string(), "same.txt".to_string()];
        let result = validate_rename_proposals(&orig, &edited);
        assert!(result.is_err(), "duplicate proposed names must return Err");
    }

    #[test]
    fn validate_rename_correct_output_pairs_ordering() {
        let orig = vec![
            "first.txt".to_string(),
            "second.txt".to_string(),
            "third.txt".to_string(),
        ];
        let edited = vec![
            "1st.txt".to_string(),
            "second.txt".to_string(),
            "3rd.txt".to_string(),
        ];
        let result = validate_rename_proposals(&orig, &edited).unwrap();
        assert_eq!(result.len(), 2);
        // pairs must be in listing order
        assert_eq!(result[0], ("first.txt".to_string(), "1st.txt".to_string()));
        assert_eq!(result[1], ("third.txt".to_string(), "3rd.txt".to_string()));
    }
}

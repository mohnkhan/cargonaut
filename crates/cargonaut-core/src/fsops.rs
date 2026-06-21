// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `fsops` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    /// FR-024 — create a directory in the active pane's cwd; refresh.
    pub(crate) async fn mkdir(&mut self, name: &str) -> Result<Vec<Event>, AppError> {
        let name = name.trim();
        if name.is_empty() || name.contains('/') {
            return Ok(vec![Event::Status(format!(
                "Invalid directory name {name:?}"
            ))]);
        }
        let target = self.active_pane_state().cwd.join(name);
        match self.registry.local().mkdir(&target, false).await {
            Ok(()) => {
                let mut evs = self.refresh_active_pane().await?;
                evs.push(Event::Status(format!("Created {name}")));
                Ok(evs)
            }
            Err(e) => Ok(vec![Event::Status(format!("mkdir failed: {e}"))]),
        }
    }

    /// FR-025 — tag (or untag) visible entries whose name matches `pat`.
    pub(crate) fn select_by_pattern(&mut self, pat: &str, add: bool) -> Vec<Event> {
        let id = self.active;
        let visible = self.pane(id).visible_indices();
        let mut matched = 0usize;
        let p = self.pane_mut(id);
        for i in visible {
            let name = p.listing.entries[i].name.to_string();
            if glob_match(pat, &name) {
                matched += 1;
                if add {
                    p.selected.insert(i);
                } else {
                    p.selected.remove(&i);
                }
            }
        }
        let verb = if add { "Tagged" } else { "Untagged" };
        vec![
            Event::PaneUpdated(id),
            Event::Status(format!(
                "{verb} {matched} entr{}",
                if matched == 1 { "y" } else { "ies" }
            )),
        ]
    }

    /// FR-023 — recursive size of the focused directory (bounded walk).
    pub(crate) async fn recursive_dir_size(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let target = {
            let p = self.pane(id);
            p.focused_entry_index()
                .and_then(|i| p.listing.entries.get(i))
                .and_then(|e| match &e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => {
                        Some((e.name.to_string(), p.cwd.join(e.name.as_str())))
                    }
                    _ => None,
                })
        };
        let Some((name, path)) = target else {
            return Ok(vec![Event::Status("Not a directory".into())]);
        };
        // Bounded breadth-first walk; cap node count so a huge tree can't
        // wedge the UI (FR-023).
        const NODE_CAP: usize = 200_000;
        let mut total: u64 = 0;
        let mut nodes = 0usize;
        let mut stack = vec![path];
        let mut truncated = false;
        while let Some(dir) = stack.pop() {
            let listing = match self.registry.local().list(&dir, Sort::NameAsc).await {
                Ok(l) => l,
                Err(_) => continue,
            };
            for e in listing.entries {
                nodes += 1;
                if nodes > NODE_CAP {
                    truncated = true;
                    break;
                }
                match e.meta.kind {
                    cargonaut_vfs::VfsKind::Dir => stack.push(dir.join(e.name.as_str())),
                    _ => total += e.meta.size,
                }
            }
            if truncated {
                break;
            }
        }
        let suffix = if truncated { " (truncated)" } else { "" };
        Ok(vec![Event::Status(format!(
            "{name}: {total} bytes{suffix}"
        ))])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn mkdir_creates_directory_and_refreshes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::Mkdir("newdir".into())).await.unwrap();
        assert!(td_l.path().join("newdir").is_dir());
        assert!(app
            .pane(PaneId::Left)
            .listing
            .entries
            .iter()
            .any(|e| e.name.as_str() == "newdir"));
    }

    #[tokio::test]
    async fn mkdir_rejects_invalid_name_without_crash() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let evs = app.dispatch(Command::Mkdir("a/b".into())).await.unwrap();
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Invalid"))));
        assert!(!td_l.path().join("a").exists());
    }

    #[tokio::test]
    async fn select_by_pattern_tags_matches_and_unselect_removes() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["one.rs", "two.rs", "note.txt"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert_eq!(app.pane(PaneId::Left).selected.len(), 2);
        app.dispatch(Command::UnselectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert_eq!(app.pane(PaneId::Left).selected.len(), 0);
    }

    #[tokio::test]
    async fn select_by_pattern_zero_match_reports_zero() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("x.txt"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let evs = app
            .dispatch(Command::SelectByPattern("*.rs".into()))
            .await
            .unwrap();
        assert!(evs
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Tagged 0"))));
    }

    #[tokio::test]
    async fn recursive_dir_size_sums_tree() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::create_dir(td_l.path().join("d")).await.unwrap();
        fs::write(td_l.path().join("d/a"), b"hello").await.unwrap(); // 5
        fs::create_dir(td_l.path().join("d/sub")).await.unwrap();
        fs::write(td_l.path().join("d/sub/b"), b"hi").await.unwrap(); // 2
        let mut app = make_app(&td_l, &td_r).await;
        // Cursor on "d" (only dir; NameAsc → first entry).
        let evs = app.dispatch(Command::RecursiveDirSize).await.unwrap();
        assert!(
            evs.iter()
                .any(|e| matches!(e, Event::Status(s) if s.contains("7 bytes"))),
            "expected 7 bytes total, got {evs:?}"
        );
    }
}

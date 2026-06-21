// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `history` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    pub(crate) async fn history_prev_dir(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let prev = self.pane_mut(id).dir_history_back.pop();
        let Some(prev) = prev else {
            return Ok(vec![Event::Status("No prior directory".into())]);
        };
        let listing = self.registry.local().list(&prev, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        let cur = std::mem::replace(&mut p.cwd, prev);
        p.dir_history_fwd.push(cur);
        p.listing = listing;
        p.cursor = p.default_cursor(); // Feature 040
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }

    pub(crate) async fn history_next_dir(&mut self) -> Result<Vec<Event>, AppError> {
        let id = self.active;
        let next = self.pane_mut(id).dir_history_fwd.pop();
        let Some(next) = next else {
            return Ok(vec![Event::Status("No forward directory".into())]);
        };
        let listing = self.registry.local().list(&next, Sort::NameAsc).await?;
        let p = self.pane_mut(id);
        let cur = std::mem::replace(&mut p.cwd, next);
        p.dir_history_back.push(cur);
        p.listing = listing;
        p.cursor = p.default_cursor(); // Feature 040
        p.selected.clear();
        Ok(vec![Event::PaneUpdated(id)])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn descend_pushes_back_history_clears_forward() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let initial_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        let p = app.pane(PaneId::Left);
        assert_eq!(p.dir_history_back, vec![initial_cwd]);
        assert!(p.dir_history_fwd.is_empty());
    }

    #[tokio::test]
    async fn history_prev_dir_pops_back_pushes_to_forward() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let initial_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::Descend).await.unwrap();
        let sub_cwd = app.pane(PaneId::Left).cwd.clone();

        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        let p = app.pane(PaneId::Left);
        assert_eq!(p.cwd, initial_cwd);
        assert!(p.dir_history_back.is_empty());
        assert_eq!(p.dir_history_fwd, vec![sub_cwd]);
    }

    #[tokio::test]
    async fn history_next_dir_returns_after_prev() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("sub"))
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::Descend).await.unwrap();
        let sub_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        app.dispatch(Command::HistoryNextDir).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cwd, sub_cwd);
        assert!(app.pane(PaneId::Left).dir_history_fwd.is_empty());
    }

    #[tokio::test]
    async fn history_prev_dir_with_empty_history_is_noop_with_status() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::HistoryPrevDir).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::Status(_))));
    }

    #[tokio::test]
    async fn descend_after_prev_drops_forward_history() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::create_dir(td_l.path().join("a")).await.unwrap();
        tokio::fs::create_dir(td_l.path().join("b")).await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Descend into "a", then back, then descend into "b" — forward
        // should be cleared.
        // (Cursor starts at 0 = "a" since NameAsc.)
        app.dispatch(Command::Descend).await.unwrap();
        app.dispatch(Command::HistoryPrevDir).await.unwrap();
        assert!(!app.pane(PaneId::Left).dir_history_fwd.is_empty());
        // Move cursor to "b" then descend.
        app.dispatch(Command::CursorDown).await.unwrap();
        app.dispatch(Command::Descend).await.unwrap();
        assert!(app.pane(PaneId::Left).dir_history_fwd.is_empty());
    }
}

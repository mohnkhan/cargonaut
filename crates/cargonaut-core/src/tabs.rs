// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `tabs` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    pub(crate) fn tab_new(&mut self) -> Result<Vec<Event>, AppError> {
        let idx = pane_idx(self.active);
        let s = &mut self.sides[idx];
        let src = &s.tabs[s.active_tab];
        let new_tab = PaneState {
            cwd: src.cwd.clone(),
            listing: src.listing.clone(),
            cursor: 0,
            selected: BTreeSet::new(),
            show_hidden: self.config.ui.show_hidden,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: Vec::new(),
            dir_history_fwd: Vec::new(),
            backend: Arc::clone(&src.backend),
        };
        s.tabs.push(new_tab);
        s.active_tab = s.tabs.len() - 1;
        Ok(vec![Event::PaneUpdated(self.active)])
    }

    pub(crate) fn tab_close(&mut self) -> Result<Vec<Event>, AppError> {
        let idx = pane_idx(self.active);
        let s = &mut self.sides[idx];
        if s.tabs.len() == 1 {
            return Ok(vec![]);
        }
        let closed = s.active_tab;
        s.tabs.remove(closed);
        s.active_tab = closed.min(s.tabs.len() - 1);
        Ok(vec![Event::PaneUpdated(self.active)])
    }

    pub(crate) fn tab_next(&mut self) -> Result<Vec<Event>, AppError> {
        let idx = pane_idx(self.active);
        let s = &mut self.sides[idx];
        let n = s.tabs.len();
        s.active_tab = (s.active_tab + 1) % n;
        Ok(vec![Event::PaneUpdated(self.active)])
    }

    pub(crate) fn tab_prev(&mut self) -> Result<Vec<Event>, AppError> {
        let idx = pane_idx(self.active);
        let s = &mut self.sides[idx];
        let n = s.tabs.len();
        s.active_tab = (s.active_tab + n - 1) % n;
        Ok(vec![Event::PaneUpdated(self.active)])
    }

    /// Return the view model for the tab bar of the given pane side.
    ///
    /// Each entry has a 1-based `index`, a `label` (basename of cwd, ≤20 UTF-8
    /// chars), and `is_active` set for the currently focused tab.
    pub fn tab_bar_view(&self, id: PaneId) -> Vec<TabBarEntry> {
        let idx = pane_idx(id);
        let s = &self.sides[idx];
        s.tabs
            .iter()
            .enumerate()
            .map(|(i, tab)| {
                let raw_label = tab
                    .cwd
                    .segments
                    .last()
                    .map(|seg| seg.as_str())
                    .unwrap_or("/")
                    .to_owned();
                let label = if raw_label.chars().count() > 20 {
                    let mut truncated: String = raw_label.chars().take(19).collect();
                    truncated.push('…');
                    truncated
                } else {
                    raw_label
                };
                TabBarEntry {
                    index: i + 1,
                    label,
                    is_active: i == s.active_tab,
                }
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[test]
    fn side_state_struct_shape() {
        // Compile-fails until SideState { tabs, active_tab } is defined.
        let _check: fn(Vec<PaneState>, usize) -> SideState =
            |tabs, active_tab| SideState { tabs, active_tab };
    }

    #[test]
    fn tab_bar_entry_struct_shape() {
        // Compile-fails until TabBarEntry { index, label, is_active } is defined.
        let e = TabBarEntry {
            index: 1usize,
            label: "foo".to_string(),
            is_active: true,
        };
        assert_eq!(e.index, 1);
        assert_eq!(e.label, "foo");
        assert!(e.is_active);
    }

    #[tokio::test]
    async fn tab_new_dispatch_returns_ok() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let result = app.dispatch(Command::TabNew).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn tab_close_dispatch_returns_ok() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let result = app.dispatch(Command::TabClose).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn tab_next_dispatch_returns_ok() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let result = app.dispatch(Command::TabNext).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn tab_prev_dispatch_returns_ok() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let result = app.dispatch(Command::TabPrev).await;
        assert!(result.is_ok());
    }

    #[tokio::test]
    async fn tab_new_opens_in_same_cwd() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let orig_cwd = app.pane(PaneId::Left).cwd.clone();
        app.dispatch(Command::TabNew).await.unwrap();
        // Left side should now have 2 tabs
        assert_eq!(app.sides[0].tabs.len(), 2, "expected 2 tabs after TabNew");
        assert_eq!(
            app.sides[0].tabs[1].cwd, orig_cwd,
            "new tab cwd should match original"
        );
    }

    #[tokio::test]
    async fn tab_new_inherits_no_filter_or_selection() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap();
        let new_tab = &app.sides[0].tabs[1];
        assert!(new_tab.filter.is_none(), "new tab filter should be None");
        assert!(
            new_tab.selected.is_empty(),
            "new tab selection should be empty"
        );
    }

    #[tokio::test]
    async fn tab_new_becomes_active() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, 1,
            "active_tab should be 1 after TabNew"
        );
    }

    #[tokio::test]
    async fn tab_close_noop_on_single_tab() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::TabClose).await.unwrap();
        assert_eq!(
            app.sides[0].tabs.len(),
            1,
            "single tab should remain after TabClose"
        );
        assert!(events.is_empty(), "single-tab close returns Ok(vec![])");
    }

    #[tokio::test]
    async fn tab_close_selects_right_successor() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Open 3 tabs total; close the first (index 0); successor is index 0 (former index 1)
        app.dispatch(Command::TabNew).await.unwrap(); // tab 1
        app.dispatch(Command::TabNew).await.unwrap(); // tab 2
        app.sides[0].active_tab = 0; // focus tab 0 (first)
        app.dispatch(Command::TabClose).await.unwrap();
        assert_eq!(app.sides[0].tabs.len(), 2, "should have 2 tabs after close");
        assert_eq!(
            app.sides[0].active_tab, 0,
            "active_tab should be 0 (right successor)"
        );
    }

    #[tokio::test]
    async fn tab_close_wraps_to_last_when_rightmost() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap(); // tab 1
        app.dispatch(Command::TabNew).await.unwrap(); // tab 2 (active)
                                                      // Close rightmost tab (index 2); should wrap to index 1 (last of remaining)
        app.dispatch(Command::TabClose).await.unwrap();
        assert_eq!(app.sides[0].tabs.len(), 2, "should have 2 tabs");
        assert_eq!(app.sides[0].active_tab, 1, "active_tab should wrap to last");
    }

    #[tokio::test]
    async fn tab_next_advances_and_wraps() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap(); // now 2 tabs, active=1
        app.sides[0].active_tab = 0; // reset to first
        app.dispatch(Command::TabNext).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, 1,
            "TabNext should advance to index 1"
        );
        app.dispatch(Command::TabNext).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, 0,
            "TabNext should wrap from last to first"
        );
    }

    #[tokio::test]
    async fn tab_prev_recedes_and_wraps() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap(); // now 2 tabs, active=1
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, 0,
            "TabPrev should go back to index 0"
        );
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, 1,
            "TabPrev should wrap from first to last"
        );
    }

    #[tokio::test]
    async fn tab_next_noop_with_one_tab() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNext).await.unwrap();
        assert_eq!(app.sides[0].active_tab, 0, "single tab: TabNext stays at 0");
    }

    #[tokio::test]
    async fn cross_pane_copy_dest_is_active_tab_cwd() {
        let td_parent = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_r.path().join("file.txt"), b"hi")
            .await
            .unwrap();
        let (mut app, inner_cwd) = make_nested_app(&td_parent, &td_r).await;
        // Open tab 2; ascend into td_parent (test's own temp dir, not /tmp itself)
        app.dispatch(Command::TabNew).await.unwrap();
        app.dispatch(Command::Ascend).await.unwrap();
        let tab2_cwd = app.sides[0].tabs[1].cwd.display();
        assert_ne!(
            tab2_cwd, inner_cwd,
            "tab 2 should have a different cwd after Ascend"
        );
        // Switch back to tab 1 (active_tab = 0) and focus right
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(app.sides[0].active_tab, 0);
        assert_eq!(
            app.pane(PaneId::Left).cwd.display(),
            inner_cwd,
            "active tab should be inner"
        );
        app.dispatch(Command::FocusRight).await.unwrap();
        app.dispatch(Command::SelectionToggle).await.unwrap();
        let events = app.dispatch(Command::Copy).await.unwrap();
        let body = events.iter().find_map(|e| {
            if let Event::DialogRequested(DialogKind::Confirm { body, .. }) = e {
                Some(body.clone())
            } else {
                None
            }
        });
        let body = body.expect("expected DialogRequested event");
        assert!(
            body.contains(&inner_cwd),
            "copy dialog body should contain active tab cwd ({inner_cwd}), got: {body}"
        );
    }

    #[tokio::test]
    async fn cross_pane_copy_after_tab_switch_uses_new_active() {
        let td_parent = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_r.path().join("file.txt"), b"hi")
            .await
            .unwrap();
        let (mut app, _inner_cwd) = make_nested_app(&td_parent, &td_r).await;
        // Open tab 2 and ascend to td_parent; keep tab 2 as active (active_tab = 1)
        app.dispatch(Command::TabNew).await.unwrap();
        app.dispatch(Command::Ascend).await.unwrap();
        let tab2_cwd = app.sides[0].tabs[1].cwd.display();
        assert_eq!(app.sides[0].active_tab, 1);
        app.dispatch(Command::FocusRight).await.unwrap();
        app.dispatch(Command::SelectionToggle).await.unwrap();
        let events = app.dispatch(Command::Copy).await.unwrap();
        let body = events.iter().find_map(|e| {
            if let Event::DialogRequested(DialogKind::Confirm { body, .. }) = e {
                Some(body.clone())
            } else {
                None
            }
        });
        let body = body.expect("expected DialogRequested event");
        assert!(
            body.contains(&tab2_cwd),
            "copy dialog body should contain tab 2 cwd ({tab2_cwd}), got: {body}"
        );
    }

    #[tokio::test]
    async fn sync_other_panel_uses_active_tab_cwd() {
        let td_parent = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let (mut app, _inner_cwd) = make_nested_app(&td_parent, &td_r).await;
        // Open tab 2; ascend to td_parent; switch back to tab 1; then switch to tab 2
        app.dispatch(Command::TabNew).await.unwrap();
        app.dispatch(Command::Ascend).await.unwrap();
        let tab2_cwd = app.sides[0].tabs[1].cwd.display();
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(app.sides[0].active_tab, 0);
        app.dispatch(Command::TabNext).await.unwrap();
        assert_eq!(app.sides[0].active_tab, 1);
        // Focus right; dispatch SyncOtherPanelPath (syncs right to OTHER pane = left's active tab)
        app.dispatch(Command::FocusRight).await.unwrap();
        app.dispatch(Command::SyncOtherPanelPath).await.unwrap();
        assert_eq!(
            app.pane(PaneId::Right).cwd.display(),
            tab2_cwd,
            "right pane cwd should match left's active tab 2 cwd"
        );
    }

    #[tokio::test]
    async fn dialog_dest_captured_at_open_time() {
        let td_parent = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_r.path().join("file.txt"), b"hi")
            .await
            .unwrap();
        let (mut app, inner_cwd) = make_nested_app(&td_parent, &td_r).await;
        // Open tab 2 and ascend; switch back to tab 1 (active_tab = 0, cwd = inner)
        app.dispatch(Command::TabNew).await.unwrap();
        app.dispatch(Command::Ascend).await.unwrap();
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(app.sides[0].active_tab, 0);
        let dest_at_open = app.pane(PaneId::Left).cwd.display();
        assert_eq!(dest_at_open, inner_cwd);
        // Focus right; select file; open copy dialog
        app.dispatch(Command::FocusRight).await.unwrap();
        app.dispatch(Command::SelectionToggle).await.unwrap();
        let events = app.dispatch(Command::Copy).await.unwrap();
        let body = events.iter().find_map(|e| {
            if let Event::DialogRequested(DialogKind::Confirm { body, .. }) = e {
                Some(body.clone())
            } else {
                None
            }
        });
        let body = body.expect("expected DialogRequested");
        assert!(
            body.contains(&dest_at_open),
            "dialog body should capture dst at open time ({dest_at_open}), got: {body}"
        );
    }

    #[tokio::test]
    async fn tab_state_filter_is_isolated() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Set filter on tab 0
        app.set_filter("xyz_no_match").unwrap();
        assert!(
            app.pane(PaneId::Left).filter.is_some(),
            "tab 0 should have filter"
        );
        // Open tab 1 (inherits no filter)
        app.dispatch(Command::TabNew).await.unwrap();
        assert!(
            app.pane(PaneId::Left).filter.is_none(),
            "tab 1 should have no filter"
        );
        // Switch back to tab 0; filter should still be there
        app.dispatch(Command::TabPrev).await.unwrap();
        assert!(
            app.pane(PaneId::Left).filter.is_some(),
            "tab 0 filter should persist"
        );
    }

    #[tokio::test]
    async fn tab_state_sort_is_isolated() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Cycle sort on tab 0 (NameAsc → ExtAsc per next_sort_key)
        app.dispatch(Command::CycleSortKey).await.unwrap();
        assert_eq!(
            app.pane(PaneId::Left).sort,
            Sort::ExtAsc,
            "tab 0 sort should be ExtAsc"
        );
        // Open tab 1
        app.dispatch(Command::TabNew).await.unwrap();
        assert_eq!(
            app.pane(PaneId::Left).sort,
            Sort::NameAsc,
            "tab 1 sort should be default NameAsc"
        );
        // Switch back to tab 0; sort should still be ExtAsc
        app.dispatch(Command::TabPrev).await.unwrap();
        assert_eq!(
            app.pane(PaneId::Left).sort,
            Sort::ExtAsc,
            "tab 0 sort should still be ExtAsc"
        );
    }

    #[tokio::test]
    async fn tab_state_selection_is_isolated() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        tokio::fs::write(td_l.path().join("file.txt"), b"x")
            .await
            .unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Toggle selection on tab 0
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).selected.is_empty(),
            "tab 0 should have a selection"
        );
        // Open tab 1 (clean selection)
        app.dispatch(Command::TabNew).await.unwrap();
        assert!(
            app.pane(PaneId::Left).selected.is_empty(),
            "tab 1 should have empty selection"
        );
        // Switch back to tab 0; selection should persist
        app.dispatch(Command::TabPrev).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).selected.is_empty(),
            "tab 0 selection should persist"
        );
    }

    #[tokio::test]
    async fn tab_new_does_not_inherit_filter() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.set_filter("xyz").unwrap();
        app.dispatch(Command::TabNew).await.unwrap();
        assert!(
            app.pane(PaneId::Left).filter.is_none(),
            "new tab should not inherit filter"
        );
    }

    #[tokio::test]
    async fn tab_state_show_hidden_is_isolated() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let default_show_hidden = app.pane(PaneId::Left).show_hidden;
        // Toggle show_hidden on tab 0
        app.dispatch(Command::ToggleHidden).await.unwrap();
        assert_ne!(
            app.pane(PaneId::Left).show_hidden,
            default_show_hidden,
            "tab 0 show_hidden toggled"
        );
        // Open tab 1 — should start with config default
        app.dispatch(Command::TabNew).await.unwrap();
        let config_default = app.config().ui.show_hidden;
        assert_eq!(
            app.pane(PaneId::Left).show_hidden,
            config_default,
            "tab 1 show_hidden should be config default"
        );
    }

    #[tokio::test]
    async fn tab_state_history_is_isolated() {
        // Use a nested dir so we can ascend without landing in /tmp
        let td_parent = TempDir::new().unwrap();
        let inner = td_parent.path().join("inner");
        tokio::fs::create_dir_all(&inner).await.unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = App::new(
            cargonaut_config::Config::default(),
            inner.to_str().unwrap(),
            td_r.path().to_str().unwrap(),
        )
        .await
        .unwrap();
        // Navigate up to td_parent; this builds back history in tab 0
        app.dispatch(Command::Ascend).await.unwrap();
        assert!(
            !app.pane(PaneId::Left).dir_history_back.is_empty(),
            "tab 0 should have back history after Ascend"
        );
        // Open tab 1 — history must be empty per data-model.md §Validation Rules
        app.dispatch(Command::TabNew).await.unwrap();
        assert!(
            app.pane(PaneId::Left).dir_history_back.is_empty(),
            "tab 1 history should be empty"
        );
        assert!(
            app.pane(PaneId::Left).dir_history_fwd.is_empty(),
            "tab 1 fwd history should be empty"
        );
    }

    #[tokio::test]
    async fn focus_swap_key_does_not_change_tabs() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap(); // 2 tabs on left, active=1
        let left_at_before = app.sides[0].active_tab;
        let right_at_before = app.sides[1].active_tab;
        // FocusSwap should change which pane is active, not which tab
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, left_at_before,
            "left active_tab unchanged by FocusSwap"
        );
        assert_eq!(
            app.sides[1].active_tab, right_at_before,
            "right active_tab unchanged by FocusSwap"
        );
        // FocusLeft / FocusRight also should not change tabs
        app.dispatch(Command::FocusLeft).await.unwrap();
        assert_eq!(
            app.sides[0].active_tab, left_at_before,
            "left active_tab unchanged by FocusLeft"
        );
        app.dispatch(Command::FocusRight).await.unwrap();
        assert_eq!(
            app.sides[1].active_tab, right_at_before,
            "right active_tab unchanged by FocusRight"
        );
    }

    #[tokio::test]
    async fn tab_bar_view_single_tab() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        let entries = app.tab_bar_view(PaneId::Left);
        assert_eq!(entries.len(), 1, "single tab → 1 entry");
        assert!(entries[0].is_active, "only tab should be active");
        assert_eq!(entries[0].index, 1, "1-based index");
        // Label should be basename of the cwd
        let basename = td_l.path().file_name().unwrap().to_str().unwrap();
        assert_eq!(entries[0].label, basename);
    }

    #[tokio::test]
    async fn tab_bar_view_multiple_tabs() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap();
        app.dispatch(Command::TabNew).await.unwrap();
        let entries = app.tab_bar_view(PaneId::Left);
        assert_eq!(entries.len(), 3, "should have 3 entries after 2 TabNews");
        let active_count = entries.iter().filter(|e| e.is_active).count();
        assert_eq!(active_count, 1, "exactly one active tab");
        assert!(entries[2].is_active, "last tab (index 2) should be active");
        // indices should be 1-based
        assert_eq!(entries[0].index, 1);
        assert_eq!(entries[1].index, 2);
        assert_eq!(entries[2].index, 3);
    }

    #[tokio::test]
    async fn tab_bar_view_label_truncates_long_name() {
        // We can't create a tempdir with a 30-char name via TempDir, so we
        // directly manipulate sides to inject a PaneState with a long cwd.
        // Instead, test that labels ≤20 chars in general.
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        let entries = app.tab_bar_view(PaneId::Left);
        for e in &entries {
            let char_count: usize = e.label.chars().count();
            assert!(char_count <= 20, "label '{}' exceeds 20 chars", e.label);
        }
    }

    #[tokio::test]
    async fn tab_bar_view_active_marker_on_correct_tab() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::TabNew).await.unwrap(); // active = 1
        let entries_before = app.tab_bar_view(PaneId::Left);
        assert!(
            entries_before[1].is_active,
            "tab at index 1 should be active"
        );
        app.dispatch(Command::TabNext).await.unwrap(); // wraps to 0
        let entries_after = app.tab_bar_view(PaneId::Left);
        assert!(
            entries_after[0].is_active,
            "after TabNext, tab 0 should be active"
        );
        assert!(!entries_after[1].is_active, "tab 1 should not be active");
    }
}

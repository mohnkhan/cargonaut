// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `app` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

impl App {
    /// Build an App with the two given starting paths. Paths may be
    /// absolute (`/home/me`) or `file://` URIs; relative paths are
    /// rejected. Lists both directories synchronously so the caller
    /// learns about NotFound up front.
    pub async fn new(
        config: cargonaut_config::Config,
        left: &str,
        right: &str,
    ) -> Result<Self, AppError> {
        let local_fs: Arc<dyn VfsBackend> = Arc::new(LocalFs::new());
        let registry = Arc::new(VfsRegistry::new(Arc::clone(&local_fs)));
        let left_p = parse_path(left)?;
        let right_p = parse_path(right)?;
        let left_listing = local_fs.list(&left_p, Sort::NameAsc).await?;
        let right_listing = local_fs.list(&right_p, Sort::NameAsc).await?;

        let show_hidden = config.ui.show_hidden;

        let mut left_tab = PaneState {
            cwd: left_p,
            listing: left_listing,
            cursor: 0,
            selected: BTreeSet::new(),
            show_hidden,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: Vec::new(),
            dir_history_fwd: Vec::new(),
            backend: Arc::clone(&local_fs),
        };
        let mut right_tab = PaneState {
            cwd: right_p,
            listing: right_listing,
            cursor: 0,
            selected: BTreeSet::new(),
            show_hidden,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: Vec::new(),
            dir_history_fwd: Vec::new(),
            backend: Arc::clone(&local_fs),
        };
        // Feature 040 (FR-014): start the cursor on the first real entry, past
        // the synthetic `..` row in a non-root directory.
        left_tab.cursor = left_tab.default_cursor();
        right_tab.cursor = right_tab.default_cursor();

        let sides = [
            SideState {
                tabs: vec![left_tab],
                active_tab: 0,
            },
            SideState {
                tabs: vec![right_tab],
                active_tab: 0,
            },
        ];

        // Feature 042 — load the persisted hotlist (best-effort: a missing or
        // malformed state file degrades to an empty list, never blocks launch).
        let hotlist_path = cargonaut_config::default_hotlist_path();
        let hotlist = cargonaut_config::Hotlist::load(&hotlist_path);

        Ok(Self {
            config,
            sides,
            active: PaneId::Left,
            registry,
            transfers: HashMap::new(),
            transfer_order: Vec::new(),
            paused: HashSet::new(),
            pending_resumes: Vec::new(),
            status: String::new(),
            split: SplitOrient::Horizontal,
            view_mode: ViewMode::Full,
            hotlist,
            hotlist_path,
            undo_log: None,
        })
    }

    /// Feature 057 — the VFS registry (scheme dispatch for all backends).
    pub fn registry(&self) -> Arc<VfsRegistry> {
        Arc::clone(&self.registry)
    }

    /// The current global listing/preview view mode (FR-022).
    pub fn view_mode(&self) -> ViewMode {
        self.view_mode
    }

    /// FR-026 — a UI-friendly projection of the most recent in-flight
    /// transfer, or `None` if nothing is running. Lets the UI render a
    /// progress dialog without depending on the transfer crate's types.
    pub fn active_progress(&self) -> Option<ProgressView> {
        for id in self.transfer_order.iter().rev() {
            if let Some(job) = self.transfers.get(id) {
                if let TransferState::Running {
                    bytes_done,
                    bytes_total,
                    eta_secs,
                    throughput_mibs,
                } = transfer_state_snapshot(job)
                {
                    return Some(ProgressView {
                        bytes_done,
                        bytes_total,
                        eta_secs,
                        throughput_mibs,
                    });
                }
            }
        }
        None
    }

    /// Current split orientation. UI reads this to lay out the two panes.
    pub fn split_orient(&self) -> SplitOrient {
        self.split
    }

    /// Read-only access to the App's config.
    pub fn config(&self) -> &cargonaut_config::Config {
        &self.config
    }

    /// Which pane currently has focus.
    pub fn active_pane(&self) -> PaneId {
        self.active
    }

    /// Read-only access to a specific pane's active tab.
    pub fn pane(&self, id: PaneId) -> &PaneState {
        let idx = pane_idx(id);
        let s = &self.sides[idx];
        &s.tabs[s.active_tab]
    }

    /// Read-only access to the active pane's active tab.
    pub fn active_pane_state(&self) -> &PaneState {
        self.pane(self.active)
    }

    /// Current status-bar message.
    pub fn status(&self) -> &str {
        &self.status
    }

    /// Apply a command. Returns the events the UI should react to.
    /// Many commands are state-only (no events beyond `PaneUpdated`);
    /// destructive ones request dialogs; `Copy`/`Move` spawn transfers.
    pub async fn dispatch(&mut self, cmd: Command) -> Result<Vec<Event>, AppError> {
        use Command::*;
        match cmd {
            CursorDown => {
                let p = self.active_pane_mut();
                // Feature 040: clamp to the last virtual row (`..` + entries).
                let rows = p.row_count();
                if rows > 0 {
                    p.cursor = (p.cursor + 1).min(rows - 1);
                }
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            CursorUp => {
                let p = self.active_pane_mut();
                p.cursor = p.cursor.saturating_sub(1);
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            CursorTo(n) => {
                let p = self.active_pane_mut();
                let rows = p.row_count();
                p.cursor = if rows == 0 { 0 } else { n.min(rows - 1) };
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            // Feature 040 (FR-003): activating the `..` row ascends; otherwise
            // descend into the focused real entry.
            Descend => {
                if self.active_pane_state().on_parent_row() {
                    self.ascend_to_parent().await
                } else {
                    self.descend_into_focused().await
                }
            }
            Ascend => self.ascend_to_parent().await,
            FocusSwap => {
                self.active = self.active.other();
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            FocusLeft => {
                self.active = PaneId::Left;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            FocusRight => {
                self.active = PaneId::Right;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            SelectionToggle => {
                let p = self.active_pane_mut();
                if let Some(idx) = p.focused_entry_index() {
                    if !p.selected.insert(idx) {
                        p.selected.remove(&idx);
                    }
                }
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            SelectionInvert => {
                let p = self.active_pane_mut();
                let all_visible: BTreeSet<usize> = p.visible_indices().into_iter().collect();
                let new_sel: BTreeSet<usize> =
                    all_visible.difference(&p.selected).copied().collect();
                p.selected = new_sel;
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            ToggleHidden => {
                let p = self.active_pane_mut();
                p.show_hidden = !p.show_hidden;
                p.cursor = p.default_cursor(); // Feature 040
                Ok(vec![Event::PaneUpdated(self.active)])
            }
            TogglePanelFilter => {
                // FR-013 (Feature 033): the filter prompt is a UI-side modal.
                // The TUI intercepts this command to open the dialog (mirroring
                // `QuickCdPopup`); set/clear is performed by [`App::set_filter`].
                // Dispatching directly into core is therefore a no-op.
                Ok(vec![])
            }
            SyncOtherPanelPath => self.sync_other_panel_path().await,
            ShowFocusedInOtherPanel => self.show_focused_in_other_panel().await,
            ToggleSplitOrientation => {
                self.split = self.split.toggle();
                Ok(vec![
                    Event::PaneUpdated(PaneId::Left),
                    Event::PaneUpdated(PaneId::Right),
                ])
            }
            HistoryPrevDir => self.history_prev_dir().await,
            HistoryNextDir => self.history_next_dir().await,
            // Feature 038: opened UI-side (the TUI builds the dialog and
            // calls `complete_cd`/`quick_cd`). A direct dispatch is a no-op.
            QuickCdPopup => Ok(vec![]),
            // Feature 039: the tasks panel is opened UI-side (the TUI builds
            // the modal from `job_views()` and routes per-row actions to
            // `cancel_transfer`/`pause_transfer`/`resume_paused`). A direct
            // dispatch is a no-op, like `QuickCdPopup`/`TogglePanelFilter`.
            ShowTasksPanel => Ok(vec![]),
            Copy => self.request_copy_confirmation(),
            Move => self.request_move_confirmation(),
            Delete => self.request_delete_confirmation(),
            CancelCurrentTransfer => {
                if let Some(id) = self.transfer_order.last().copied() {
                    Ok(self.cancel_transfer(id))
                } else {
                    Ok(vec![Event::Status("No active transfer to cancel".into())])
                }
            }
            CycleSortKey => {
                let p = self.active_pane_mut();
                p.sort = next_sort_key(p.sort);
                let label = sort_label(p.sort);
                let mut evs = self.relist_active().await?;
                evs.push(Event::Status(format!("Sort: {label}")));
                Ok(evs)
            }
            ToggleSortReverse => {
                let p = self.active_pane_mut();
                p.sort = match p.sort {
                    Sort::NameAsc => Sort::NameDesc,
                    Sort::NameDesc => Sort::NameAsc,
                    other => other,
                };
                let label = sort_label(p.sort);
                let mut evs = self.relist_active().await?;
                evs.push(Event::Status(format!("Sort: {label}")));
                Ok(evs)
            }
            CycleListingMode => {
                self.view_mode = self.view_mode.next();
                Ok(vec![Event::Status(format!("View: {:?}", self.view_mode))])
            }
            RecursiveDirSize => self.recursive_dir_size().await,
            Mkdir(name) => self.mkdir(&name).await,
            SelectByPattern(pat) => Ok(self.select_by_pattern(&pat, true)),
            UnselectByPattern(pat) => Ok(self.select_by_pattern(&pat, false)),
            Chown(owner) => self.chown_selection(&owner).await,
            ChmodRecursive(spec) => self.chmod_recursive(&spec).await,
            ChownRecursive(owner) => self.chown_recursive(&owner).await,
            CompareDirectories => self.compare_directories(),
            BulkRenameApply(pairs) => self.apply_bulk_rename(pairs).await,
            UndoLastOp => self.undo_last_operation().await,
            Quit => Ok(vec![Event::QuitRequested]),
            TabNew => self.tab_new(),
            TabClose => self.tab_close(),
            TabNext => self.tab_next(),
            TabPrev => self.tab_prev(),
        }
    }

    pub(crate) fn active_pane_mut(&mut self) -> &mut PaneState {
        let idx = pane_idx(self.active);
        let s = &mut self.sides[idx];
        let at = s.active_tab;
        &mut s.tabs[at]
    }

    pub(crate) fn pane_mut(&mut self, id: PaneId) -> &mut PaneState {
        let idx = pane_idx(id);
        let s = &mut self.sides[idx];
        let at = s.active_tab;
        &mut s.tabs[at]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[tokio::test]
    async fn new_loads_both_pane_listings() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"").await.unwrap();
        fs::write(td_l.path().join("b"), b"").await.unwrap();
        let app = make_app(&td_l, &td_r).await;
        assert_eq!(app.pane(PaneId::Left).listing.entries.len(), 2);
        assert_eq!(app.pane(PaneId::Right).listing.entries.len(), 0);
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn new_rejects_relative_path() {
        let config = cargonaut_config::Config::default();
        let res = App::new(config, "relative/path", "/tmp").await;
        assert!(matches!(res, Err(AppError::BadPath(_))));
    }

    #[tokio::test]
    async fn cursor_down_advances_within_visible_subset() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        // Feature 040: temp dirs are non-root, so row 0 is the synthetic `..`
        // and the cursor starts on the first real entry (virtual index 1).
        assert_eq!(app.pane(PaneId::Left).cursor, 1);
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 3);
        // Clamp at the last virtual row (`..` + 3 entries → row_count 4 → max 3).
        app.dispatch(Command::CursorDown).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 3);
    }

    #[tokio::test]
    async fn cursor_to_sets_absolute_position_and_clamps() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::CursorTo(2)).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 2);
        // Out-of-range clamps to the last virtual row (`..` + 3 entries → 3).
        app.dispatch(Command::CursorTo(99)).await.unwrap();
        assert_eq!(app.pane(PaneId::Left).cursor, 3);
    }

    #[tokio::test]
    async fn cursor_to_survives_resync_via_pane_state() {
        // A clicked cursor must be authoritative: reading pane state back
        // reflects it (the UI's PaneView::sync_from copies state.cursor).
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        for n in ["a", "b", "c", "d"] {
            fs::write(td_l.path().join(n), b"").await.unwrap();
        }
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::CursorTo(3)).await.unwrap();
        assert_eq!(app.active_pane_state().cursor, 3);
    }

    #[tokio::test]
    async fn focus_swap_toggles_active_pane() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.active_pane(), PaneId::Left);
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(app.active_pane(), PaneId::Right);
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(app.active_pane(), PaneId::Left);
    }

    #[tokio::test]
    async fn cycle_listing_mode_rotates_view() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.view_mode(), ViewMode::Full);
        app.dispatch(Command::CycleListingMode).await.unwrap();
        assert_eq!(app.view_mode(), ViewMode::QuickView);
        app.dispatch(Command::CycleListingMode).await.unwrap();
        assert_eq!(app.view_mode(), ViewMode::Brief);
    }

    #[tokio::test]
    async fn quit_emits_quit_requested() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let events = app.dispatch(Command::Quit).await.unwrap();
        assert!(events.iter().any(|e| matches!(e, Event::QuitRequested)));
    }

    #[tokio::test]
    async fn toggle_hidden_resets_cursor() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join(".hidden"), b"").await.unwrap();
        fs::write(td_l.path().join("visible"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Initially show_hidden=false (per Config default); cursor at 0 sees "visible".
        let initial = app.active_pane_state().focused_entry_index();
        assert!(initial.is_some());
        app.dispatch(Command::ToggleHidden).await.unwrap();
        assert!(app.pane(PaneId::Left).show_hidden);
        // Cursor reset to the first real entry (virtual index 1, past `..`);
        // both files now visible.
        assert_eq!(app.pane(PaneId::Left).cursor, 1);
        assert_eq!(app.pane(PaneId::Left).visible_indices().len(), 2);
    }

    #[tokio::test]
    async fn toggle_split_orientation_cycles_horizontal_vertical() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        assert_eq!(app.split_orient(), SplitOrient::Horizontal);
        app.dispatch(Command::ToggleSplitOrientation).await.unwrap();
        assert_eq!(app.split_orient(), SplitOrient::Vertical);
        app.dispatch(Command::ToggleSplitOrientation).await.unwrap();
        assert_eq!(app.split_orient(), SplitOrient::Horizontal);
    }

    #[tokio::test]
    async fn pane_accessor_returns_starting_cwd() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        // Public API unchanged: pane(PaneId) → &PaneState
        let left: &PaneState = app.pane(PaneId::Left);
        assert!(
            left.cwd
                .display()
                .ends_with(td_l.path().file_name().unwrap().to_str().unwrap()),
            "left cwd should be td_l: {}",
            left.cwd.display()
        );
        let right: &PaneState = app.pane(PaneId::Right);
        assert!(
            right
                .cwd
                .display()
                .ends_with(td_r.path().file_name().unwrap().to_str().unwrap()),
            "right cwd should be td_r: {}",
            right.cwd.display()
        );
    }

    #[tokio::test]
    async fn active_pane_state_returns_active_side() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        // Default active = Left; active_pane_state() returns Left's PaneState
        assert_eq!(app.active_pane(), PaneId::Left);
        let _state: &PaneState = app.active_pane_state();
        assert_eq!(app.active_pane_state().cwd, app.pane(PaneId::Left).cwd);
        // Focus swap → active = Right
        app.dispatch(Command::FocusSwap).await.unwrap();
        assert_eq!(app.active_pane(), PaneId::Right);
        assert_eq!(app.active_pane_state().cwd, app.pane(PaneId::Right).cwd);
    }

    #[tokio::test]
    async fn app_registry_returns_arc_vfs_registry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        // registry() must exist and its local() must be the file backend.
        let reg = app.registry();
        assert_eq!(reg.local().scheme(), "file");
    }

    #[tokio::test]
    async fn pane_backend_is_local_fs_on_startup() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        // Both panes must start with the local (file://) backend.
        for id in [PaneId::Left, PaneId::Right] {
            let backend = &app.pane(id).backend;
            assert_eq!(backend.scheme(), "file");
            assert!(
                backend.caps().contains(VfsCaps::SEEKABLE),
                "{id:?} backend must be seekable (LocalFs)"
            );
        }
    }
}

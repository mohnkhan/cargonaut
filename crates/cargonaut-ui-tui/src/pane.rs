// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Pane view widget — renders a [`cargonaut_vfs::DirListing`] with cursor
//! + selection + virtual scrolling.
//!
//! Wraps ratatui's `List` + `ListState` so virtual scrolling is free
//! (ratatui handles the viewport math given a selected index). Adds:
//!
//! - **Selection tracking** as a `BTreeSet<usize>` of indices into
//!   `listing.entries`. Cursor row gets a `*`/`·` prefix when selected.
//! - **Hidden-file masking** (FR-015 `Alt-.` toggle). Indices in
//!   [`PaneView::selected`] are stable across toggles — the prefix
//!   filter just changes what's visible, not what's tagged.
//! - **String-substring filter** (placeholder for the FR-013 glob
//!   filter — T1.26 will swap in `globset`).
//! - Cursor movement via [`PaneView::cursor_down`] / [`PaneView::cursor_up`]
//!   that respects the visible (filtered + hidden-masked) subset.

use crate::theme::Theme;
use cargonaut_vfs::{DirListing, VfsKind, VfsPath};
use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState, StatefulWidget};
use std::collections::BTreeSet;

/// How a pane lays out each row (FR-022). `Brief` shows names only;
/// `Full` adds size, mtime, and permission columns.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PaneLayout {
    /// Names only (compact).
    Brief,
    /// Name + size + mtime + permissions.
    Full,
}

/// One pane's view state: the directory + cursor + selection + display
/// toggles. The `App` owns one per pane.
#[derive(Debug)]
pub struct PaneView {
    /// Directory currently being viewed.
    pub cwd: VfsPath,
    /// Sorted listing snapshot for `cwd`.
    pub listing: DirListing,
    /// Indices into `listing.entries` that the user has tagged.
    pub selected: BTreeSet<usize>,
    /// Show Unix dotfiles in the listing.
    pub show_hidden: bool,
    /// Substring filter — entries whose name doesn't contain this string
    /// are hidden. `None` = no filter. T1.26 swaps this for a real glob.
    pub filter: Option<String>,
    /// Cursor position within the **visible** subset (filtered + hidden-masked).
    list_state: ListState,
}

impl PaneView {
    /// Build a new view rooted at `cwd` with the given pre-sorted `listing`.
    /// Cursor starts on the first visible entry (or unset if empty).
    pub fn new(cwd: VfsPath, listing: DirListing) -> Self {
        let mut list_state = ListState::default();
        if !listing.entries.is_empty() {
            list_state.select(Some(0));
        }
        Self {
            cwd,
            listing,
            selected: BTreeSet::new(),
            show_hidden: false,
            filter: None,
            list_state,
        }
    }

    /// Sync this view from the App's authoritative [`PaneState`]. Copies
    /// `cwd`, `listing`, `selected`, `show_hidden`, `filter`, and
    /// translates the App's absolute cursor index into a visible-relative
    /// position for ratatui's `ListState`. Called once per frame by the
    /// binary's event loop so the rendered view never drifts from App state.
    pub fn sync_from(&mut self, state: &cargonaut_core::PaneState) {
        self.cwd = state.cwd.clone();
        self.listing = state.listing.clone();
        self.selected = state.selected.clone();
        self.show_hidden = state.show_hidden;
        self.filter = state.filter.clone();
        // Translate visible-relative cursor (state.cursor) to ListState.
        let visible_len = self.visible_indices().len();
        if visible_len == 0 {
            self.list_state.select(None);
        } else {
            let clamped = state.cursor.min(visible_len - 1);
            self.list_state.select(Some(clamped));
        }
    }

    /// Replace the listing (e.g. after a `cd`). Cursor + selection reset
    /// because absolute indices don't carry across directories.
    pub fn set_listing(&mut self, listing: DirListing) {
        self.listing = listing;
        self.selected.clear();
        if self.listing.entries.is_empty() {
            self.list_state.select(None);
        } else {
            self.list_state.select(Some(0));
        }
    }

    /// Indices into `self.listing.entries` that pass the visibility
    /// filters (`show_hidden` + `filter`), in listing order.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.listing
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if !self.show_hidden && e.meta.is_hidden {
                    return None;
                }
                if let Some(pat) = &self.filter {
                    if !e.name.as_str().contains(pat.as_str()) {
                        return None;
                    }
                }
                Some(i)
            })
            .collect()
    }

    /// Move cursor down one visible entry. Clamped at the last visible
    /// entry (no wrap).
    pub fn cursor_down(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.list_state.select(None);
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0);
        let next = (cur + 1).min(visible.len().saturating_sub(1));
        self.list_state.select(Some(next));
    }

    /// Move cursor up one visible entry. Clamped at the first visible
    /// entry (no wrap).
    pub fn cursor_up(&mut self) {
        let visible = self.visible_indices();
        if visible.is_empty() {
            self.list_state.select(None);
            return;
        }
        let cur = self.list_state.selected().unwrap_or(0);
        let prev = cur.saturating_sub(1);
        self.list_state.select(Some(prev));
    }

    /// Toggle selection on the entry currently under the cursor. No-op if
    /// no visible entry is focused.
    pub fn toggle_selection(&mut self) {
        if let Some(idx) = self.focused_entry_index() {
            if !self.selected.insert(idx) {
                // Was already there; remove.
                self.selected.remove(&idx);
            }
        }
    }

    /// The visible-subset index at the top of the rendered viewport
    /// (ratatui's scroll offset). Used by mouse hit-testing (US3) to map a
    /// clicked screen row to an absolute visible index.
    pub fn viewport_top(&self) -> usize {
        self.list_state.offset()
    }

    /// Index into `listing.entries` of the cursor's current entry, or
    /// `None` if no visible entry is focused.
    pub fn focused_entry_index(&self) -> Option<usize> {
        let visible = self.visible_indices();
        let cur = self.list_state.selected()?;
        visible.get(cur).copied()
    }

    /// Render the pane to a ratatui buffer. Uses ratatui's `List` +
    /// `StatefulWidget` so virtual scrolling falls out of the box —
    /// `area.height` controls how many entries fit; entries outside the
    /// viewport are skipped, and `list_state` tracks the scroll offset
    /// implicitly via the selected index.
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme, layout: PaneLayout) {
        let items: Vec<ListItem<'_>> = self
            .visible_indices()
            .into_iter()
            .map(|i| {
                let entry = &self.listing.entries[i];
                let marked = self.selected.contains(&i);
                let prefix = if marked { '*' } else { ' ' };
                let kind_suffix = match &entry.meta.kind {
                    VfsKind::Dir => "/",
                    VfsKind::Symlink { .. } => "@",
                    _ => "",
                };
                let line = match layout {
                    PaneLayout::Brief => {
                        format!("{prefix} {}{}", entry.name.as_str(), kind_suffix)
                    }
                    PaneLayout::Full => {
                        // US4 (FR-019): name + size + mtime + perms columns.
                        let size = if matches!(entry.meta.kind, VfsKind::Dir) {
                            String::from("   <DIR>")
                        } else {
                            format!("{:>8}", entry.meta.size)
                        };
                        let mtime = crate::chrome::format_mtime(entry.meta.mtime);
                        let perms = entry
                            .meta
                            .mode
                            .as_ref()
                            .map(|m| crate::chrome::perms_string(m.bits, &entry.meta.kind))
                            .unwrap_or_else(|| "----------".to_string());
                        format!(
                            "{prefix}{} {} {}  {}{}",
                            perms,
                            size,
                            mtime,
                            entry.name.as_str(),
                            kind_suffix
                        )
                    }
                };
                // US1 (FR-003): per-entry color keyed on kind / mode /
                // hidden / marked, on the theme's panel background.
                let style = theme.entry_style(
                    &entry.meta.kind,
                    entry.meta.mode.as_ref(),
                    entry.meta.is_hidden,
                    marked,
                );
                ListItem::new(Line::from(Span::styled(line, style)))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(theme.cursor_style())
            .highlight_symbol(" ");
        StatefulWidget::render(list, area, buf, &mut self.list_state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargonaut_vfs::{DirEntry, FileMode, Sort, VfsKind, VfsMetadata};
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use smol_str::SmolStr;
    use std::time::SystemTime;

    fn entry(name: &str, kind: VfsKind, size: u64, hidden: bool) -> DirEntry {
        DirEntry {
            name: SmolStr::new(name),
            meta: VfsMetadata {
                size,
                mtime: SystemTime::UNIX_EPOCH,
                mode: Some(FileMode {
                    bits: 0o644,
                    uid: None,
                    gid: None,
                }),
                kind,
                is_hidden: hidden,
            },
        }
    }

    fn listing(entries: Vec<DirEntry>) -> DirListing {
        DirListing {
            entries,
            sort: Sort::NameAsc,
        }
    }

    fn vfs_path() -> VfsPath {
        VfsPath::parse("file:///tmp").unwrap()
    }

    #[test]
    fn new_starts_with_cursor_on_first_entry() {
        let p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("a", VfsKind::File, 10, false),
                entry("b", VfsKind::File, 20, false),
            ]),
        );
        assert_eq!(p.list_state.selected(), Some(0));
        assert_eq!(p.focused_entry_index(), Some(0));
    }

    #[test]
    fn new_with_empty_listing_has_no_cursor() {
        let p = PaneView::new(vfs_path(), listing(vec![]));
        assert_eq!(p.list_state.selected(), None);
        assert_eq!(p.focused_entry_index(), None);
    }

    #[test]
    fn cursor_down_advances_then_clamps() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("a", VfsKind::File, 10, false),
                entry("b", VfsKind::File, 20, false),
            ]),
        );
        p.cursor_down();
        assert_eq!(p.focused_entry_index(), Some(1));
        // Already at last; further cursor_down clamps.
        p.cursor_down();
        assert_eq!(p.focused_entry_index(), Some(1));
    }

    #[test]
    fn cursor_up_recedes_then_clamps_at_zero() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("a", VfsKind::File, 10, false),
                entry("b", VfsKind::File, 20, false),
                entry("c", VfsKind::File, 30, false),
            ]),
        );
        p.cursor_down();
        p.cursor_down();
        assert_eq!(p.focused_entry_index(), Some(2));
        p.cursor_up();
        assert_eq!(p.focused_entry_index(), Some(1));
        p.cursor_up();
        assert_eq!(p.focused_entry_index(), Some(0));
        p.cursor_up();
        assert_eq!(p.focused_entry_index(), Some(0));
    }

    #[test]
    fn hidden_entries_excluded_unless_show_hidden() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry(".hidden", VfsKind::File, 10, true),
                entry("visible", VfsKind::File, 20, false),
            ]),
        );
        assert_eq!(p.visible_indices(), vec![1]);
        p.show_hidden = true;
        assert_eq!(p.visible_indices(), vec![0, 1]);
    }

    #[test]
    fn substring_filter_constrains_visibility() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("apple", VfsKind::File, 10, false),
                entry("banana", VfsKind::File, 20, false),
                entry("apricot", VfsKind::File, 30, false),
            ]),
        );
        p.filter = Some("ap".into());
        assert_eq!(p.visible_indices(), vec![0, 2]); // apple, apricot
    }

    #[test]
    fn toggle_selection_marks_focused_entry() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("a", VfsKind::File, 10, false),
                entry("b", VfsKind::File, 20, false),
            ]),
        );
        p.toggle_selection();
        assert!(p.selected.contains(&0));
        p.toggle_selection();
        assert!(!p.selected.contains(&0));
    }

    #[test]
    fn set_listing_resets_cursor_and_selection() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("a", VfsKind::File, 10, false),
                entry("b", VfsKind::File, 20, false),
            ]),
        );
        p.cursor_down();
        p.toggle_selection();
        assert!(p.selected.contains(&1));

        p.set_listing(listing(vec![entry("x", VfsKind::File, 0, false)]));
        assert!(p.selected.is_empty());
        assert_eq!(p.focused_entry_index(), Some(0));
    }

    #[test]
    fn render_to_test_backend_shows_entry_names() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("alpha", VfsKind::File, 100, false),
                entry("beta", VfsKind::Dir, 0, false),
            ]),
        );
        let backend = TestBackend::new(40, 5);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = f.size();
            // Brief layout keeps the name first so a 40-wide buffer shows it.
            p.render(area, f.buffer_mut(), &Theme::default(), PaneLayout::Brief);
        })
        .unwrap();

        let buf = term.backend().buffer();
        let rendered: String = buf
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect();
        assert!(rendered.contains("alpha"), "rendered = {rendered:?}");
        assert!(rendered.contains("beta"), "rendered = {rendered:?}");
        // Directory marker
        assert!(
            rendered.contains("beta/"),
            "Dir should get '/' suffix: {rendered:?}"
        );
    }

    #[test]
    fn render_with_large_listing_does_not_panic() {
        // Virtual scrolling: 10000 entries into a 5-row viewport.
        let entries: Vec<DirEntry> = (0..10000)
            .map(|i| entry(&format!("file-{i:05}"), VfsKind::File, i, false))
            .collect();
        let mut p = PaneView::new(vfs_path(), listing(entries));
        // Move cursor near the end to force ratatui to scroll the viewport.
        for _ in 0..5000 {
            p.cursor_down();
        }
        let backend = TestBackend::new(40, 5);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = f.size();
            p.render(area, f.buffer_mut(), &Theme::default(), PaneLayout::Full);
        })
        .unwrap();
        // No panic = pass; assert focused index is what we expected.
        assert_eq!(p.focused_entry_index(), Some(5000));
    }

    // T010 (US1): a directory row renders in the theme's directory color
    // and the cursor row carries the theme's cursor background.
    #[test]
    fn render_applies_theme_colors() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("adir", VfsKind::Dir, 0, false),
                entry("afile", VfsKind::File, 10, false),
            ]),
        );
        let theme = Theme::commander_dark();
        let backend = TestBackend::new(40, 5);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|f| {
            let area = f.size();
            p.render(area, f.buffer_mut(), &theme, PaneLayout::Full);
        })
        .unwrap();
        let buf = term.backend().buffer();
        // Row 0 is the cursor row (selected index 0) → cursor bg.
        let cursor_cell = buf.get(1, 0);
        assert_eq!(cursor_cell.style().bg, Some(theme.cursor_bg));
        // Row 1 ("afile") is a regular file → panel_fg, panel_bg.
        let file_cell = buf.get(1, 1);
        assert_eq!(file_cell.style().fg, Some(theme.panel_fg));
        assert_eq!(file_cell.style().bg, Some(theme.panel_bg));
        // The directory color is distinct from the regular-file color.
        assert_ne!(theme.dir_fg, theme.panel_fg);
    }

    #[test]
    fn cursor_down_with_filter_only_walks_visible() {
        let mut p = PaneView::new(
            vfs_path(),
            listing(vec![
                entry("apple", VfsKind::File, 0, false),
                entry("banana", VfsKind::File, 0, false),
                entry("apricot", VfsKind::File, 0, false),
            ]),
        );
        p.filter = Some("ap".into());
        // Visible: [apple(0), apricot(2)]
        // Cursor starts at 0 (apple)
        assert_eq!(p.focused_entry_index(), Some(0));
        p.cursor_down();
        assert_eq!(p.focused_entry_index(), Some(2)); // apricot, skipping banana
        p.cursor_down();
        // Clamped at last visible
        assert_eq!(p.focused_entry_index(), Some(2));
    }
}

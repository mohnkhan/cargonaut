// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 059 split: `pane` module of `cargonaut-core`.
//!
//! Moved verbatim from the former `lib.rs` god-file (move-only refactor).

#[allow(unused_imports)]
use crate::*;

/// Pane identifier (left/right today; tabs later — FR-202 in Phase 5).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum PaneId {
    /// Left pane.
    Left,
    /// Right pane.
    Right,
}

impl PaneId {
    /// The other pane (Left ↔ Right).
    pub fn other(self) -> Self {
        match self {
            PaneId::Left => PaneId::Right,
            PaneId::Right => PaneId::Left,
        }
    }
}

/// A compiled, applied name filter for one pane (FR-013).
///
/// Carries both the original `pattern` text — so the prompt can be
/// re-opened prefilled (FR-002) — and a compiled, case-insensitive
/// [`GlobMatcher`] used by [`PaneState::visible_indices`] each frame.
///
/// Compilation rule (see spec FR-003a): a pattern containing any glob
/// metacharacter (`* ? [ ] { }`) is matched as a glob against the full
/// entry name; a metacharacter-free pattern is matched as a substring,
/// i.e. wrapped as `*pattern*`. Matching is case-insensitive (FR-003b).
#[derive(Debug, Clone)]
pub struct PaneFilter {
    pattern: String,
    matcher: GlobMatcher,
}

impl PaneFilter {
    /// Compile raw prompt text into a filter.
    ///
    /// The caller guarantees `pattern` is non-empty after trimming (empty
    /// input takes the clear path and is never compiled). Returns
    /// [`AppError::BadFilter`] if the (possibly auto-wrapped) pattern is not
    /// a valid glob.
    pub fn compile(pattern: &str) -> Result<PaneFilter, AppError> {
        let trimmed = pattern.trim();
        let has_meta = trimmed.contains(['*', '?', '[', ']', '{', '}']);
        let glob_src = if has_meta {
            trimmed.to_string()
        } else {
            format!("*{trimmed}*")
        };
        let matcher = GlobBuilder::new(&glob_src)
            .case_insensitive(true)
            .build()
            .map_err(|e| AppError::BadFilter(e.to_string()))?
            .compile_matcher();
        Ok(PaneFilter {
            pattern: trimmed.to_string(),
            matcher,
        })
    }

    /// True when `name` matches this filter.
    pub fn is_match(&self, name: &str) -> bool {
        self.matcher.is_match(name)
    }

    /// The original pattern text, for re-opening the prompt prefilled.
    pub fn pattern(&self) -> &str {
        &self.pattern
    }
}

/// Pure state for one pane. Renderable by the UI (ui-tui's `PaneView`
/// builds itself from a `&PaneState` per frame) and mutated by the
/// `App::dispatch` state machine.
#[derive(Clone)]
pub struct PaneState {
    /// Directory currently being viewed.
    pub cwd: VfsPath,
    /// Sorted listing snapshot for `cwd`.
    pub listing: DirListing,
    /// Cursor position within the **visible** subset (filter + hidden-masked).
    pub cursor: usize,
    /// Selected indices into `listing.entries`.
    pub selected: BTreeSet<usize>,
    /// Show Unix dotfiles in the listing.
    pub show_hidden: bool,
    /// Active sort order for this pane's listing (FR-021).
    pub sort: Sort,
    /// Active name filter (FR-013). `None` = no filter. Persists across
    /// directory navigation until explicitly cleared (FR-003c).
    pub filter: Option<PaneFilter>,
    /// FR-011 back history: cwds visited before the current one, most
    /// recent at the end. Bounded by `Config::ui.history.directory_depth`.
    pub dir_history_back: Vec<VfsPath>,
    /// FR-011 forward history: only populated after [`Command::HistoryPrevDir`].
    /// Cleared on any non-history navigation (descend / ascend / sync).
    pub dir_history_fwd: Vec<VfsPath>,
    /// Feature 057 — the VFS backend serving this pane's cwd. `LocalFs` at
    /// startup; replaced with `ZipFs`/`TarFs`/`SftpFs`/`FtpFs` when the user
    /// navigates into an archive or remote server.
    pub backend: Arc<dyn VfsBackend>,
}

impl std::fmt::Debug for PaneState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PaneState")
            .field("cwd", &self.cwd)
            .field("cursor", &self.cursor)
            .field("show_hidden", &self.show_hidden)
            .field("sort", &self.sort)
            .field("backend_scheme", &self.backend.scheme())
            .finish_non_exhaustive()
    }
}

/// Feature 040 — what the pane cursor currently points at: the synthetic
/// `..` parent row, or a real entry (index into `listing.entries`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusedRow {
    /// The synthetic `..` parent row (present only in a non-root directory).
    Parent,
    /// A real listing entry at this index into `listing.entries`.
    Entry(usize),
}

impl PaneState {
    /// Indices that pass the visibility filters, in listing order.
    pub fn visible_indices(&self) -> Vec<usize> {
        self.listing
            .entries
            .iter()
            .enumerate()
            .filter_map(|(i, e)| {
                if !self.show_hidden && e.meta.is_hidden {
                    return None;
                }
                if let Some(pf) = &self.filter {
                    if !pf.is_match(e.name.as_str()) {
                        return None;
                    }
                }
                Some(i)
            })
            .collect()
    }

    /// Feature 040 (FR-001/002) — true when the directory has a parent, so a
    /// synthetic `..` row is shown as the first row.
    pub fn has_parent(&self) -> bool {
        self.cwd.parent().is_some()
    }

    /// Feature 040 — the virtual↔real index shift: 1 when a `..` row is
    /// present (non-root), 0 at a filesystem root.
    pub fn parent_offset(&self) -> usize {
        usize::from(self.has_parent())
    }

    /// Feature 040 — number of addressable cursor rows: the `..` row (when
    /// present) plus the visible real entries.
    pub fn row_count(&self) -> usize {
        self.parent_offset() + self.visible_indices().len()
    }

    /// Feature 040 — true when the cursor is on the synthetic `..` row.
    pub fn on_parent_row(&self) -> bool {
        self.has_parent() && self.cursor < self.parent_offset()
    }

    /// Feature 040 — what the cursor currently points at (parent row vs a
    /// real entry).
    pub fn focused_row(&self) -> FocusedRow {
        match self.focused_entry_index() {
            Some(i) => FocusedRow::Entry(i),
            None if self.on_parent_row() => FocusedRow::Parent,
            // Empty real listing with no parent: nothing focusable; treat as
            // Parent only when a parent row exists, else default to Parent-less
            // — callers gate on focused_entry_index for real-entry actions.
            None => FocusedRow::Parent,
        }
    }

    /// Absolute index in `listing.entries` of the cursor's current real
    /// entry, or `None` when the cursor is on the `..` row (Feature 040).
    pub fn focused_entry_index(&self) -> Option<usize> {
        let off = self.parent_offset();
        if self.cursor < off {
            return None; // on the synthetic `..` row
        }
        self.visible_indices().get(self.cursor - off).copied()
    }

    /// Feature 040 (FR-014) — the cursor position to use for a fresh listing:
    /// the first real entry (just past the `..` row), or the `..` row itself
    /// in an empty non-root directory, or 0 at a root.
    pub(crate) fn default_cursor(&self) -> usize {
        self.parent_offset().min(self.row_count().saturating_sub(1))
    }
}

/// View model for one tab entry in the tab bar widget. Produced by
/// [`App::tab_bar_view`]; consumed by the TUI renderer. Pure data — no I/O.
#[derive(Debug, Clone, PartialEq)]
pub struct TabBarEntry {
    /// 1-based display index shown in the `[N]` prefix.
    pub index: usize,
    /// Truncated basename of this tab's cwd (max 20 UTF-8 chars, hard cap).
    pub label: String,
    /// `true` when this tab is currently active (visible) on its side.
    pub is_active: bool,
}

/// FR-022 — the global listing/preview view mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViewMode {
    /// Names only (compact).
    Brief,
    /// Name + size + mtime + permissions.
    Full,
    /// The passive panel previews the active panel's highlighted file.
    QuickView,
}

impl ViewMode {
    /// Cycle Brief → Full → QuickView → Brief.
    pub fn next(self) -> Self {
        match self {
            ViewMode::Brief => ViewMode::Full,
            ViewMode::Full => ViewMode::QuickView,
            ViewMode::QuickView => ViewMode::Brief,
        }
    }
}

/// FR-015 split orientation for the two-pane layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitOrient {
    /// Side-by-side panes (default — the classic orthodox look).
    Horizontal,
    /// Stacked panes (left = top, right = bottom).
    Vertical,
}

impl SplitOrient {
    /// Cycle to the other orientation.
    pub fn toggle(self) -> Self {
        match self {
            SplitOrient::Horizontal => SplitOrient::Vertical,
            SplitOrient::Vertical => SplitOrient::Horizontal,
        }
    }
}

pub(crate) fn pane_idx(id: PaneId) -> usize {
    match id {
        PaneId::Left => 0,
        PaneId::Right => 1,
    }
}

/// Minimal shell-glob matcher supporting `*` (any run) and `?` (one char).
/// Dependency-free (avoids pulling regex/globset for FR-025).
pub fn glob_match(pattern: &str, name: &str) -> bool {
    let p: Vec<char> = pattern.chars().collect();
    let n: Vec<char> = name.chars().collect();
    // Iterative backtracking matcher.
    let (mut pi, mut ni) = (0usize, 0usize);
    let (mut star, mut mark): (Option<usize>, usize) = (None, 0);
    while ni < n.len() {
        if pi < p.len() && (p[pi] == '?' || p[pi] == n[ni]) {
            pi += 1;
            ni += 1;
        } else if pi < p.len() && p[pi] == '*' {
            star = Some(pi);
            mark = ni;
            pi += 1;
        } else if let Some(s) = star {
            pi = s + 1;
            mark += 1;
            ni = mark;
        } else {
            return false;
        }
    }
    while pi < p.len() && p[pi] == '*' {
        pi += 1;
    }
    pi == p.len()
}

#[cfg(test)]
mod tests {
    use super::*;
    #[allow(unused_imports)]
    use crate::test_support::*;

    #[test]
    fn root_pane_has_no_parent_row() {
        let p = PaneState {
            cwd: VfsPath::parse("file:///").unwrap(),
            listing: DirListing {
                entries: vec![],
                sort: Sort::NameAsc,
            },
            cursor: 0,
            selected: BTreeSet::new(),
            show_hidden: false,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: Vec::new(),
            dir_history_fwd: Vec::new(),
            backend: Arc::new(LocalFs::new()),
        };
        assert!(!p.has_parent());
        assert_eq!(p.parent_offset(), 0);
        assert!(!p.on_parent_row());
    }

    #[tokio::test]
    async fn non_root_pane_has_parent_row_and_offset() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = app_with_three(&td_l, &td_r).await;
        let p = app.active_pane_state();
        assert!(p.has_parent());
        assert_eq!(p.parent_offset(), 1);
        assert_eq!(p.row_count(), 1 + 3); // `..` + a,b,c
    }

    #[tokio::test]
    async fn default_cursor_is_first_real_entry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = app_with_three(&td_l, &td_r).await;
        let p = app.active_pane_state();
        assert_eq!(p.cursor, 1); // past `..`
        assert!(!p.on_parent_row());
        assert_eq!(p.focused_entry_index(), Some(0)); // first real entry
        assert_eq!(p.focused_row(), FocusedRow::Entry(0));
    }

    #[tokio::test]
    async fn cursor_up_from_first_entry_lands_on_parent_then_clamps() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with_three(&td_l, &td_r).await;
        app.dispatch(Command::CursorUp).await.unwrap();
        let p = app.active_pane_state();
        assert_eq!(p.cursor, 0);
        assert!(p.on_parent_row());
        assert_eq!(p.focused_entry_index(), None);
        assert_eq!(p.focused_row(), FocusedRow::Parent);
        // Up again stays on `..` (nothing above it).
        app.dispatch(Command::CursorUp).await.unwrap();
        assert_eq!(app.active_pane_state().cursor, 0);
    }

    #[tokio::test]
    async fn descend_on_parent_row_ascends() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Use a controlled nested dir so ascent returns to OUR temp dir, not
        // the shared /tmp (which other parallel tests churn — TOCTOU NotFound).
        std::fs::create_dir(td_l.path().join("sub")).unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        let parent = app.active_pane_state().cwd.clone(); // td_l
        app.dispatch(Command::Descend).await.unwrap(); // into "sub" (first entry)
        assert_ne!(app.active_pane_state().cwd, parent);
        // "sub" is an empty non-root dir → cursor rests on the `..` row.
        assert!(app.active_pane_state().on_parent_row());
        app.dispatch(Command::Descend).await.unwrap(); // `..` → ascend
        assert_eq!(app.active_pane_state().cwd, parent);
    }

    #[tokio::test]
    async fn selection_toggle_on_parent_row_is_noop() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with_three(&td_l, &td_r).await;
        app.dispatch(Command::CursorUp).await.unwrap(); // onto `..`
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(app.active_pane_state().selected.is_empty());
    }

    #[tokio::test]
    async fn selection_invert_and_pattern_exclude_parent_row() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with_three(&td_l, &td_r).await;
        app.dispatch(Command::SelectionInvert).await.unwrap();
        // All three real entries tagged; the `..` row is never a real index.
        assert_eq!(app.active_pane_state().selected.len(), 3);
        // A pattern that textually matches `..` still selects no parent row.
        app.dispatch(Command::UnselectByPattern("*".into()))
            .await
            .unwrap();
        app.dispatch(Command::SelectByPattern("..".into()))
            .await
            .unwrap();
        assert!(app.active_pane_state().selected.is_empty());
    }

    #[tokio::test]
    async fn copy_on_parent_row_with_no_selection_targets_nothing() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with_three(&td_l, &td_r).await;
        app.dispatch(Command::CursorUp).await.unwrap(); // onto `..`
        let events = app.dispatch(Command::Copy).await.unwrap();
        assert!(events
            .iter()
            .any(|e| matches!(e, Event::Status(s) if s.contains("Nothing"))));
    }

    #[tokio::test]
    async fn parent_row_present_regardless_of_filter() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let mut app = app_with_three(&td_l, &td_r).await;
        app.set_filter("zzz-no-match").unwrap(); // matches zero real entries
        let p = app.active_pane_state();
        assert!(p.has_parent());
        assert_eq!(p.row_count(), 1); // only the `..` row
        assert!(p.on_parent_row()); // cursor clamped onto `..`
    }

    #[tokio::test]
    async fn empty_non_root_dir_focuses_parent_row() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        // Right pane's temp dir is empty.
        let app = make_app(&td_l, &td_r).await;
        let p = app.pane(PaneId::Right);
        assert!(p.has_parent());
        assert_eq!(p.row_count(), 1);
        assert!(p.on_parent_row());
        assert_eq!(p.focused_entry_index(), None);
    }

    #[tokio::test]
    async fn selection_toggle_marks_focused_entry() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        fs::write(td_l.path().join("a"), b"").await.unwrap();
        let mut app = make_app(&td_l, &td_r).await;
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(app.pane(PaneId::Left).selected.contains(&0));
        app.dispatch(Command::SelectionToggle).await.unwrap();
        assert!(!app.pane(PaneId::Left).selected.contains(&0));
    }

    #[test]
    fn glob_match_basic() {
        assert!(glob_match("*.rs", "lib.rs"));
        assert!(glob_match("*.rs", ".rs"));
        assert!(!glob_match("*.rs", "lib.toml"));
        assert!(glob_match("a?c", "abc"));
        assert!(!glob_match("a?c", "ac"));
        assert!(glob_match("*", "anything"));
        assert!(glob_match("read*e", "readme"));
        assert!(glob_match("read*e", "readsome"));
        assert!(glob_match("exact", "exact"));
        assert!(!glob_match("exact", "exactly"));
    }

    #[test]
    fn pane_filter_glob_matches_extension() {
        let pf = PaneFilter::compile("*.rs").unwrap();
        assert!(pf.is_match("lib.rs"));
        assert!(!pf.is_match("readme.md"));
    }

    #[test]
    fn pane_filter_bare_word_matches_as_substring() {
        // No glob metacharacters → wrapped as `*rs*` (FR-003a).
        let pf = PaneFilter::compile("rs").unwrap();
        assert!(pf.is_match("lib.rs"));
        assert!(pf.is_match("parser.md"));
        assert!(!pf.is_match("readme.txt"));
    }

    #[test]
    fn pane_filter_is_case_insensitive() {
        // FR-003b.
        let pf = PaneFilter::compile("*.RS").unwrap();
        assert!(pf.is_match("lib.rs"));
    }

    #[test]
    fn pane_filter_invalid_pattern_errors() {
        // Unterminated character class → BadFilter (FR-006).
        let err = PaneFilter::compile("[").unwrap_err();
        assert!(matches!(err, AppError::BadFilter(_)));
    }

    #[test]
    fn pane_filter_pattern_accessor_returns_trimmed_original() {
        // FR-002: prefill text is the original (trimmed) pattern.
        let pf = PaneFilter::compile("  *.rs  ").unwrap();
        assert_eq!(pf.pattern(), "*.rs");
    }

    #[tokio::test]
    async fn pane_state_has_backend_field() {
        let td_l = TempDir::new().unwrap();
        let td_r = TempDir::new().unwrap();
        let app = make_app(&td_l, &td_r).await;
        // Access the backend field; must exist and return "file" scheme.
        let left_scheme = app.pane(PaneId::Left).backend.scheme();
        let right_scheme = app.pane(PaneId::Right).backend.scheme();
        assert_eq!(left_scheme, "file", "left pane backend must be local");
        assert_eq!(right_scheme, "file", "right pane backend must be local");
    }
}

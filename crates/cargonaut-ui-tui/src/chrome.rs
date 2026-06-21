// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Screen chrome (Feature 031, US2): the bottom function-key button bar,
//! the top pull-down menu bar, and the per-pane mini-status line.
//!
//! These are the recognizable structural elements of the reference
//! orthodox file manager and the primary discoverability surface — and
//! they double as the on-screen targets for mouse hit-testing (US3).
//!
//! The keymap remains the single source of truth (constitution §III):
//! every bar/menu entry dispatches an existing [`crate::keymap::Command`],
//! not a new vocabulary.

use crate::keymap::Command;
use crate::theme::Theme;
use cargonaut_core::PaneState;
use cargonaut_vfs::VfsKind;
use ratatui::buffer::Buffer;
use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Clear, List, ListItem, ListState, Paragraph, StatefulWidget, Widget,
};
use std::time::SystemTime;

// =====================================================================
// FunctionKeyBar
// =====================================================================

/// One labeled function-key button.
#[derive(Debug, Clone)]
pub struct FKey {
    /// The number shown (1..=10).
    pub n: u8,
    /// The short label (e.g. "Copy").
    pub label: &'static str,
    /// The command this button dispatches.
    pub command: Command,
}

/// The bottom function-key button bar (`1Help 2Menu … 10Quit`).
#[derive(Debug, Clone)]
pub struct FunctionKeyBar {
    keys: Vec<FKey>,
}

impl Default for FunctionKeyBar {
    fn default() -> Self {
        Self::new()
    }
}

impl FunctionKeyBar {
    /// The canonical reference-manager default bar.
    pub fn new() -> Self {
        let keys = vec![
            FKey {
                n: 1,
                label: "Help",
                command: Command::ShowHelp,
            },
            FKey {
                n: 2,
                label: "Menu",
                command: Command::ShowUserMenu,
            },
            FKey {
                n: 3,
                label: "View",
                command: Command::Preview,
            },
            FKey {
                n: 4,
                label: "Edit",
                command: Command::Edit,
            },
            FKey {
                n: 5,
                label: "Copy",
                command: Command::CopySelection,
            },
            FKey {
                n: 6,
                label: "RenMov",
                command: Command::MoveOrRenameSelection,
            },
            FKey {
                n: 7,
                label: "Mkdir",
                command: Command::Mkdir,
            },
            FKey {
                n: 8,
                label: "Delete",
                command: Command::DeleteSelection,
            },
            FKey {
                n: 9,
                label: "PullDn",
                command: Command::OpenMenuBar,
            },
            FKey {
                n: 10,
                label: "Quit",
                command: Command::Quit,
            },
        ];
        Self { keys }
    }

    /// The labels in order (mostly for tests).
    pub fn labels(&self) -> Vec<&'static str> {
        self.keys.iter().map(|k| k.label).collect()
    }

    /// Per-button rects for a given bar area (left→right, equal width).
    fn button_rects(&self, area: Rect) -> Vec<Rect> {
        let constraints: Vec<Constraint> = (0..self.keys.len())
            .map(|_| Constraint::Ratio(1, self.keys.len() as u32))
            .collect();
        Layout::default()
            .direction(Direction::Horizontal)
            .constraints(constraints)
            .split(area)
            .to_vec()
    }

    /// Hit-test: the command for a click at `(x, y)`, if it lands on a
    /// button in this bar (FR-017).
    pub fn command_at(&self, area: Rect, x: u16, y: u16) -> Option<Command> {
        if y < area.y || y >= area.y + area.height {
            return None;
        }
        let rects = self.button_rects(area);
        for (i, r) in rects.iter().enumerate() {
            if x >= r.x && x < r.x + r.width {
                return Some(self.keys[i].command);
            }
        }
        None
    }

    /// Render the bar with each button's number chipped in the theme's
    /// fkey colors. Labels truncate gracefully on narrow terminals.
    pub fn render(&self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        let rects = self.button_rects(area);
        for (i, r) in rects.iter().enumerate() {
            let k = &self.keys[i];
            let num = Span::styled(
                format!("{}", k.n),
                ratatui::style::Style::default()
                    .fg(theme.fkey_num_fg)
                    .bg(theme.fkey_num_bg),
            );
            let label = Span::styled(
                format!("{} ", k.label),
                ratatui::style::Style::default()
                    .fg(theme.fkey_label_fg)
                    .bg(theme.fkey_label_bg),
            );
            let para = Paragraph::new(Line::from(vec![num, label]))
                .style(ratatui::style::Style::default().bg(theme.fkey_label_bg));
            para.render(*r, buf);
        }
    }
}

// =====================================================================
// MenuBar
// =====================================================================

/// A single pull-down menu: a title and its items (each an existing
/// command).
#[derive(Debug, Clone)]
struct Menu {
    title: &'static str,
    items: Vec<(&'static str, Command)>,
}

/// The top pull-down menu bar. Closed by default; [`MenuBar::open`]
/// drops a menu down, navigable by keyboard or mouse.
#[derive(Debug, Clone)]
pub struct MenuBar {
    menus: Vec<Menu>,
    /// Index of the open menu, if any.
    open: Option<usize>,
    /// Selected item within the open menu.
    item_sel: usize,
}

impl Default for MenuBar {
    fn default() -> Self {
        Self::new()
    }
}

impl MenuBar {
    /// The default menu structure mapping to existing commands.
    pub fn new() -> Self {
        let menus = vec![
            Menu {
                title: "Left",
                items: vec![
                    ("Sort order", Command::CycleSortKey),
                    ("Listing mode", Command::CycleListingMode),
                ],
            },
            Menu {
                title: "File",
                items: vec![
                    ("View", Command::Preview),
                    ("Edit", Command::Edit),
                    ("Mkdir", Command::Mkdir),
                    ("Copy", Command::CopySelection),
                    ("Rename/Move", Command::MoveOrRenameSelection),
                    ("Delete", Command::DeleteSelection),
                    ("Chmod", Command::Chmod),
                    ("Chown", Command::Chown),
                    ("Chmod -R", Command::ChmodRecursive),
                    ("Chown -R", Command::ChownRecursive),
                    ("Symlink", Command::CreateSymlink),
                    ("Hardlink", Command::CreateHardLink),
                ],
            },
            Menu {
                title: "Command",
                items: vec![
                    ("User menu", Command::ShowUserMenu),
                    ("Directory size", Command::RecursiveDirSize),
                ],
            },
            Menu {
                title: "Options",
                items: vec![("Help", Command::ShowHelp), ("About", Command::ShowAbout)],
            },
            Menu {
                title: "Right",
                items: vec![
                    ("Sort order", Command::CycleSortKey),
                    ("Listing mode", Command::CycleListingMode),
                ],
            },
        ];
        Self {
            menus,
            open: None,
            item_sel: 0,
        }
    }

    /// Menu titles, in order.
    pub fn titles(&self) -> Vec<&'static str> {
        self.menus.iter().map(|m| m.title).collect()
    }

    /// Whether a menu is currently open.
    pub fn is_open(&self) -> bool {
        self.open.is_some()
    }

    /// Open a menu by index (toggles closed if already open on it).
    pub fn open(&mut self, idx: usize) {
        if idx < self.menus.len() {
            self.open = Some(idx);
            self.item_sel = 0;
        }
    }

    /// Open the first menu (F9 / PullDn entry point).
    pub fn open_first(&mut self) {
        self.open(0);
    }

    /// Close the menu.
    pub fn close(&mut self) {
        self.open = None;
        self.item_sel = 0;
    }

    /// Move selection down within the open menu.
    pub fn select_down(&mut self) {
        if let Some(i) = self.open {
            let len = self.menus[i].items.len();
            if len > 0 {
                self.item_sel = (self.item_sel + 1).min(len - 1);
            }
        }
    }

    /// Move selection up within the open menu.
    pub fn select_up(&mut self) {
        if self.open.is_some() {
            self.item_sel = self.item_sel.saturating_sub(1);
        }
    }

    /// Switch to the next/previous menu (left/right arrows).
    pub fn next_menu(&mut self) {
        if let Some(i) = self.open {
            self.open((i + 1) % self.menus.len());
        }
    }

    /// Switch to the previous menu.
    pub fn prev_menu(&mut self) {
        if let Some(i) = self.open {
            let n = self.menus.len();
            self.open((i + n - 1) % n);
        }
    }

    /// The command for the currently-selected item in the open menu.
    pub fn selected_command(&self) -> Option<Command> {
        let i = self.open?;
        self.menus[i].items.get(self.item_sel).map(|(_, c)| *c)
    }

    /// Per-title rects across the bar area (left→right, padded).
    fn title_rects(&self, area: Rect) -> Vec<Rect> {
        let mut rects = Vec::new();
        let mut x = area.x;
        for m in &self.menus {
            let w = (m.title.len() as u16) + 2;
            let w = w.min(area.x + area.width - x);
            rects.push(Rect {
                x,
                y: area.y,
                width: w,
                height: 1,
            });
            x = x.saturating_add(w);
            if x >= area.x + area.width {
                break;
            }
        }
        rects
    }

    /// Hit-test a click on the menu-bar titles; returns the menu index.
    pub fn title_at(&self, area: Rect, x: u16, y: u16) -> Option<usize> {
        if y != area.y {
            return None;
        }
        for (i, r) in self.title_rects(area).iter().enumerate() {
            if x >= r.x && x < r.x + r.width {
                return Some(i);
            }
        }
        None
    }

    /// The rectangle the open dropdown occupies for the given bar `area` and
    /// buffer area `buf`, or `None` if no menu is open (or it would be empty).
    ///
    /// This is the single source of dropdown geometry: [`MenuBar::render`],
    /// [`MenuBar::item_at`] and [`MenuBar::in_dropdown`] all derive from it so
    /// the clickable rows can never drift from the rendered rows (FR-002).
    fn dropdown_rect(&self, area: Rect, buf: Rect) -> Option<Rect> {
        let i = self.open?;
        let menu = &self.menus[i];
        let rects = self.title_rects(area);
        let title_x = rects.get(i).map(|r| r.x).unwrap_or(area.x);
        let width = menu.items.iter().map(|(l, _)| l.len()).max().unwrap_or(4) as u16 + 4;
        let y = area.y + 1;
        // Clamp to the buffer so a long menu (or short terminal) stays in bounds.
        let max_h = buf.height.saturating_sub(y);
        let height = (menu.items.len() as u16 + 2).min(max_h);
        let drop = Rect {
            x: title_x,
            y,
            width: width.min(buf.width.saturating_sub(title_x)),
            height,
        };
        if drop.height == 0 || drop.width == 0 {
            return None;
        }
        Some(drop)
    }

    /// Hit-test a point against the open dropdown's item rows. Returns the item
    /// index under `(x, y)`, or `None` for clicks on the border, outside the
    /// dropdown, on rows clipped by a short terminal, or when no menu is open.
    pub fn item_at(&self, area: Rect, buf: Rect, x: u16, y: u16) -> Option<usize> {
        let i = self.open?;
        let drop = self.dropdown_rect(area, buf)?;
        // Exclude the one-cell border on every side.
        if x <= drop.x || x >= drop.x + drop.width - 1 {
            return None;
        }
        if y <= drop.y || y >= drop.y + drop.height - 1 {
            return None;
        }
        let idx = (y - drop.y - 1) as usize;
        (idx < self.menus[i].items.len()).then_some(idx)
    }

    /// Whether `(x, y)` falls anywhere within the open dropdown's rectangle
    /// (border included). Lets a caller tell a click inside the frame but off
    /// the items (a no-op, FR-003) from a click fully outside (close +
    /// pass-through, FR-004). Returns `false` when no menu is open.
    pub fn in_dropdown(&self, area: Rect, buf: Rect, x: u16, y: u16) -> bool {
        self.dropdown_rect(area, buf)
            .is_some_and(|r| x >= r.x && x < r.x + r.width && y >= r.y && y < r.y + r.height)
    }

    /// Set the highlighted item directly (used by mouse click and hover).
    /// Clamps to the open menu's item range; a no-op if no menu is open.
    pub fn select(&mut self, idx: usize) {
        if let Some(i) = self.open {
            let len = self.menus[i].items.len();
            if len > 0 {
                self.item_sel = idx.min(len - 1);
            }
        }
    }

    /// Render the title bar (and the open dropdown, if any).
    pub fn render(&mut self, area: Rect, buf: &mut Buffer, theme: &Theme) {
        // Bar background.
        let bar_style = ratatui::style::Style::default()
            .fg(theme.menu_fg)
            .bg(theme.menu_bg);
        Paragraph::new("").style(bar_style).render(area, buf);
        let rects = self.title_rects(area);
        for (i, r) in rects.iter().enumerate() {
            let selected = self.open == Some(i);
            let style = if selected {
                ratatui::style::Style::default()
                    .fg(theme.menu_sel_fg)
                    .bg(theme.menu_sel_bg)
            } else {
                bar_style
            };
            Paragraph::new(format!(" {} ", self.menus[i].title))
                .style(style)
                .render(*r, buf);
        }

        // Dropdown overlay — geometry comes from `dropdown_rect` so the drawn
        // rows and the hit-test rows are guaranteed identical (FR-002).
        if let Some(i) = self.open {
            let Some(drop) = self.dropdown_rect(area, *buf.area()) else {
                return;
            };
            let menu = &self.menus[i];
            Clear.render(drop, buf);
            let items: Vec<ListItem<'_>> =
                menu.items.iter().map(|(l, _)| ListItem::new(*l)).collect();
            let block = Block::default().borders(Borders::ALL).style(
                ratatui::style::Style::default()
                    .fg(theme.menu_fg)
                    .bg(theme.menu_bg),
            );
            let list = List::new(items).block(block).highlight_style(
                ratatui::style::Style::default()
                    .fg(theme.menu_sel_fg)
                    .bg(theme.menu_sel_bg),
            );
            let mut state = ListState::default();
            state.select(Some(self.item_sel));
            StatefulWidget::render(list, drop, buf, &mut state);
        }
    }
}

// =====================================================================
// Mouse-capture indicator (Feature 041 US2 / FR-005)
// =====================================================================

/// The persistent menu-bar label for the current mouse-capture state.
///
/// `session_supported` is `config.ui.mouse` (false ⇒ disabled for the whole
/// session via `--no-mouse`/config); `captured` is the runtime
/// `UiState.mouse_enabled`. A disabled session always reads `[mouse:off]`
/// regardless of the runtime flag.
pub fn mouse_indicator(session_supported: bool, captured: bool) -> &'static str {
    match (session_supported, captured) {
        (false, _) => "[mouse:off]",
        (true, true) => "[mouse:on]",
        (true, false) => "[mouse:susp]",
    }
}

// =====================================================================
// Pane header title (FR-022)
// =====================================================================

/// Compute the pane block-border title string (FR-022).
///
/// For non-local backends (scheme != "file") the full VfsPath URI is shown
/// so the user always knows which archive or remote host they are browsing.
/// For local backends the last path segment (basename) is shown — it is
/// shorter and the full path is already visible in the status bar.
pub fn pane_header_title(pane: &PaneState) -> String {
    if pane.backend.scheme() != "file" {
        pane.cwd.display()
    } else {
        pane.cwd
            .segments
            .last()
            .map(|s| s.to_string())
            .unwrap_or_else(|| "/".to_string())
    }
}

// =====================================================================
// Mini-status (per-pane)
// =====================================================================

/// A one-line summary of the highlighted entry: name, size, permissions,
/// and modification time (FR-010). Empty when no entry is focused.
pub fn mini_status_line(state: &PaneState) -> String {
    let Some(idx) = state.focused_entry_index() else {
        return String::new();
    };
    let Some(e) = state.listing.entries.get(idx) else {
        return String::new();
    };
    let perms = e
        .meta
        .mode
        .as_ref()
        .map(|m| perms_string(m.bits, &e.meta.kind))
        .unwrap_or_else(|| "----------".to_string());
    let size = if matches!(e.meta.kind, VfsKind::Dir) {
        "<DIR>".to_string()
    } else {
        format!("{}", e.meta.size)
    };
    let mtime = format_mtime(e.meta.mtime);
    format!("{perms}  {size:>12}  {mtime}  {}", e.name.as_str())
}

/// Render a `rwxr-xr-x`-style permission string from the low 9 mode bits,
/// prefixed by the entry type character.
pub fn perms_string(bits: u32, kind: &VfsKind) -> String {
    let type_ch = match kind {
        VfsKind::Dir => 'd',
        VfsKind::Symlink { .. } => 'l',
        VfsKind::Other => '?',
        VfsKind::File => '-',
    };
    let mut s = String::with_capacity(10);
    s.push(type_ch);
    const FLAGS: [(u32, char); 9] = [
        (0o400, 'r'),
        (0o200, 'w'),
        (0o100, 'x'),
        (0o040, 'r'),
        (0o020, 'w'),
        (0o010, 'x'),
        (0o004, 'r'),
        (0o002, 'w'),
        (0o001, 'x'),
    ];
    for (mask, ch) in FLAGS {
        s.push(if bits & mask != 0 { ch } else { '-' });
    }
    s
}

/// Format a [`SystemTime`] as `YYYY-MM-DD HH:MM` (UTC), dependency-free.
/// Pre-epoch times render as `----------------`.
pub fn format_mtime(t: SystemTime) -> String {
    let secs = match t.duration_since(SystemTime::UNIX_EPOCH) {
        Ok(d) => d.as_secs(),
        Err(_) => return "----------------".to_string(),
    };
    let (y, mo, d, h, mi) = civil_from_unix(secs);
    format!("{y:04}-{mo:02}-{d:02} {h:02}:{mi:02}")
}

/// Convert unix seconds to (year, month, day, hour, minute) in UTC using
/// Howard Hinnant's civil-from-days algorithm.
fn civil_from_unix(secs: u64) -> (i64, u32, u32, u32, u32) {
    let days = (secs / 86_400) as i64;
    let rem = secs % 86_400;
    let hour = (rem / 3600) as u32;
    let minute = ((rem % 3600) / 60) as u32;

    // days since 1970-01-01 → civil date.
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097; // [0, 146096]
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365; // [0, 399]
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100); // [0, 365]
    let mp = (5 * doy + 2) / 153; // [0, 11]
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32; // [1, 31]
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32; // [1, 12]
    let y = if m <= 2 { y + 1 } else { y };
    (y, m, d, hour, minute)
}

#[cfg(test)]
mod tests {
    use super::*;
    use cargonaut_vfs::{
        DirEntry, DirListing, FileMode, LocalFs, Sort, VfsKind, VfsMetadata, VfsPath,
    };
    use ratatui::backend::TestBackend;
    use ratatui::Terminal;
    use smol_str::SmolStr;
    use std::collections::BTreeSet;
    use std::sync::Arc;
    use std::time::{Duration, SystemTime};

    fn render_to_string(w: u16, h: u16, f: impl FnOnce(Rect, &mut Buffer)) -> String {
        let backend = TestBackend::new(w, h);
        let mut term = Terminal::new(backend).unwrap();
        term.draw(|frame| f(frame.size(), frame.buffer_mut()))
            .unwrap();
        term.backend()
            .buffer()
            .content()
            .iter()
            .map(|c| c.symbol().chars().next().unwrap_or(' '))
            .collect()
    }

    // Feature 041 US2 (FR-005): the persistent capture indicator label.
    #[test]
    fn mouse_indicator_truth_table() {
        // session disabled wins regardless of captured.
        assert_eq!(mouse_indicator(false, false), "[mouse:off]");
        assert_eq!(mouse_indicator(false, true), "[mouse:off]");
        // supported session: captured vs. suspended.
        assert_eq!(mouse_indicator(true, true), "[mouse:on]");
        assert_eq!(mouse_indicator(true, false), "[mouse:susp]");
    }

    // T018: the 10 canonical labels render; hit-test maps a click to a cmd.
    #[test]
    fn fkey_bar_renders_all_labels() {
        let bar = FunctionKeyBar::new();
        assert_eq!(
            bar.labels(),
            vec![
                "Help", "Menu", "View", "Edit", "Copy", "RenMov", "Mkdir", "Delete", "PullDn",
                "Quit"
            ]
        );
        let theme = Theme::default();
        let rendered = render_to_string(120, 1, |area, buf| bar.render(area, buf, &theme));
        for lbl in ["Help", "Copy", "Mkdir", "Quit"] {
            assert!(rendered.contains(lbl), "missing {lbl}: {rendered:?}");
        }
    }

    #[test]
    fn fkey_bar_hit_test_returns_button_command() {
        let bar = FunctionKeyBar::new();
        let area = Rect {
            x: 0,
            y: 0,
            width: 100,
            height: 1,
        };
        // Button 7 (Mkdir) occupies the 7th of 10 equal slots (x≈60..70).
        let cmd = bar.command_at(area, 65, 0);
        assert!(matches!(cmd, Some(Command::Mkdir)), "got {cmd:?}");
        // A y outside the bar misses.
        assert!(bar.command_at(area, 65, 5).is_none());
    }

    // T021: narrow terminal degrades without panic.
    #[test]
    fn fkey_bar_narrow_terminal_does_not_panic() {
        let bar = FunctionKeyBar::new();
        let theme = Theme::default();
        let _ = render_to_string(20, 1, |area, buf| bar.render(area, buf, &theme));
    }

    // T019: menu titles render; opening yields items; select resolves cmd.
    #[test]
    fn menu_bar_open_navigate_select() {
        let mut mb = MenuBar::new();
        assert_eq!(
            mb.titles(),
            vec!["Left", "File", "Command", "Options", "Right"]
        );
        assert!(!mb.is_open());
        mb.open(1); // File
        assert!(mb.is_open());
        // First item is "View" → Preview.
        assert!(matches!(mb.selected_command(), Some(Command::Preview)));
        mb.select_down(); // Edit
        assert!(matches!(mb.selected_command(), Some(Command::Edit)));
        mb.close();
        assert!(!mb.is_open());
    }

    #[test]
    fn menu_bar_title_hit_test() {
        let mb = MenuBar::new();
        let area = Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 1,
        };
        // "Left" is the first title at x=0.
        assert_eq!(mb.title_at(area, 1, 0), Some(0));
        // y!=bar row misses.
        assert_eq!(mb.title_at(area, 1, 3), None);
    }

    #[test]
    fn menu_bar_renders_titles_and_dropdown() {
        let mut mb = MenuBar::new();
        mb.open(1);
        let theme = Theme::default();
        let rendered = render_to_string(80, 10, |_area, buf| {
            mb.render(
                Rect {
                    x: 0,
                    y: 0,
                    width: 80,
                    height: 1,
                },
                buf,
                &theme,
            );
        });
        assert!(rendered.contains("File"), "title missing: {rendered:?}");
    }

    // Feature 065 — mouse interaction with the open dropdown.

    // Bar area used by the 065 hit-test tests: full-width single-row menu bar.
    fn bar_area() -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: 1,
        }
    }

    fn buf_area(h: u16) -> Rect {
        Rect {
            x: 0,
            y: 0,
            width: 80,
            height: h,
        }
    }

    // T004: dropdown_rect must equal the rectangle render actually draws.
    // We assert the geometry contract used by both render and hit-testing.
    #[test]
    fn menu_bar_dropdown_rect_matches_render() {
        let mut mb = MenuBar::new();
        assert_eq!(mb.dropdown_rect(bar_area(), buf_area(24)), None); // closed
        mb.open(1); // File: 12 items, widest "Rename/Move" (11) → width 15.
        let drop = mb
            .dropdown_rect(bar_area(), buf_area(24))
            .expect("open menu has a dropdown rect");
        // File title sits after "Left" (width 6) → x = 6; y just below the bar.
        assert_eq!(drop.x, 6);
        assert_eq!(drop.y, 1);
        assert_eq!(drop.width, 15);
        assert_eq!(drop.height, 14); // 12 items + 2 border rows
    }

    // T006: item hit-test — first/last rows, border, outside, closed.
    #[test]
    fn menu_bar_item_hit_test() {
        let mut mb = MenuBar::new();
        assert_eq!(mb.item_at(bar_area(), buf_area(24), 8, 2), None); // closed
        mb.open(1); // File
        let (area, buf) = (bar_area(), buf_area(24));
        // First item row is drop.y + 1 = 2.
        assert_eq!(mb.item_at(area, buf, 8, 2), Some(0));
        // Last item (index 11) is row 13.
        assert_eq!(mb.item_at(area, buf, 8, 13), Some(11));
        // Top border row (y = 1) is not an item.
        assert_eq!(mb.item_at(area, buf, 8, 1), None);
        // Bottom border row (y = drop.y + height - 1 = 14) is not an item.
        assert_eq!(mb.item_at(area, buf, 8, 14), None);
        // Left border column (x = drop.x = 6) is not an item.
        assert_eq!(mb.item_at(area, buf, 6, 2), None);
        // A point fully outside the dropdown.
        assert_eq!(mb.item_at(area, buf, 0, 2), None);
    }

    // T007: short terminal clips trailing items; clipped rows are not clickable.
    #[test]
    fn menu_bar_item_hit_test_clamped() {
        let mut mb = MenuBar::new();
        mb.open(1); // File (12 items)
        let (area, buf) = (bar_area(), buf_area(6)); // height 6 → dropdown clamped
        let drop = mb.dropdown_rect(area, buf).unwrap();
        assert_eq!(drop.height, 5); // (6 - y=1) clamp
        // Last visible item row is drop.y + height - 2 = 4 → index 2.
        assert_eq!(mb.item_at(area, buf, 8, 4), Some(2));
        // A row that was clipped away (y = 5 = bottom border) returns None.
        assert_eq!(mb.item_at(area, buf, 8, 5), None);
        // And anything below the dropdown.
        assert_eq!(mb.item_at(area, buf, 8, 10), None);
    }

    // T009: in_dropdown distinguishes inside-frame (incl. border) from outside.
    #[test]
    fn menu_bar_in_dropdown() {
        let mut mb = MenuBar::new();
        let (area, buf) = (bar_area(), buf_area(24));
        assert!(!mb.in_dropdown(area, buf, 6, 1)); // closed → false
        mb.open(1);
        assert!(mb.in_dropdown(area, buf, 6, 1)); // top-left border corner
        assert!(mb.in_dropdown(area, buf, 8, 2)); // an item row
        assert!(mb.in_dropdown(area, buf, 20, 14)); // bottom-right border
        assert!(!mb.in_dropdown(area, buf, 21, 2)); // one past the right edge
        assert!(!mb.in_dropdown(area, buf, 8, 15)); // one past the bottom edge
        assert!(!mb.in_dropdown(area, buf, 5, 2)); // left of the dropdown
    }

    // T011: select sets the highlighted item, clamps, no-ops when closed.
    #[test]
    fn menu_bar_select_sets_item() {
        let mut mb = MenuBar::new();
        mb.select(3); // closed → no panic, no effect
        assert!(mb.selected_command().is_none());
        mb.open(1); // File
        mb.select(3); // index 3 → "Copy"
        assert!(matches!(mb.selected_command(), Some(Command::CopySelection)));
        mb.select(999); // clamps to last item "Hardlink"
        assert!(matches!(mb.selected_command(), Some(Command::CreateHardLink)));
    }

    // T020: mini-status shows name/size/perms/mtime for the focused entry.
    #[test]
    fn mini_status_shows_entry_details() {
        let entry = DirEntry {
            name: SmolStr::new("readme.md"),
            meta: VfsMetadata {
                size: 4096,
                mtime: SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000),
                mode: Some(FileMode {
                    bits: 0o644,
                    uid: None,
                    gid: None,
                }),
                kind: VfsKind::File,
                is_hidden: false,
            },
        };
        let state = PaneState {
            cwd: VfsPath::parse("file:///tmp").unwrap(),
            listing: DirListing {
                entries: vec![entry],
                sort: Sort::NameAsc,
            },
            // Feature 040: `/tmp` is non-root, so row 0 is the `..` row; the
            // real entry is at virtual cursor 1.
            cursor: 1,
            selected: BTreeSet::new(),
            show_hidden: false,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: Vec::new(),
            dir_history_fwd: Vec::new(),
            backend: Arc::new(LocalFs::new()),
        };
        let line = mini_status_line(&state);
        assert!(line.contains("readme.md"), "name missing: {line}");
        assert!(line.contains("4096"), "size missing: {line}");
        assert!(line.contains("-rw-r--r--"), "perms missing: {line}");
        assert!(line.contains("2023-11-"), "mtime missing: {line}");
    }

    #[test]
    fn format_mtime_known_epoch() {
        // 1_700_000_000 = 2023-11-14 22:13 UTC.
        let t = SystemTime::UNIX_EPOCH + Duration::from_secs(1_700_000_000);
        assert_eq!(format_mtime(t), "2023-11-14 22:13");
    }

    #[test]
    fn perms_string_directory_and_file() {
        assert_eq!(perms_string(0o755, &VfsKind::Dir), "drwxr-xr-x");
        assert_eq!(perms_string(0o644, &VfsKind::File), "-rw-r--r--");
    }

    // T016a [US1] (red): pane_header_title for zip:// backend shows full URI.
    // Fails because pane_header_title does not exist yet.
    #[test]
    fn pane_header_title_zip_shows_full_display_string() {
        use cargonaut_vfs::ZipFs;
        use std::io::Write;
        // Minimal valid ZIP (just an EOCD record — 0 entries).
        let tf = tempfile::NamedTempFile::new().unwrap();
        {
            let mut f = tf.reopen().unwrap();
            f.write_all(&[
                0x50, 0x4b, 0x05, 0x06, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
                0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
            ])
            .unwrap();
        }
        let zip_fs = ZipFs::open(tf.path().to_path_buf()).unwrap();
        let cwd = VfsPath::parse("zip:///tmp%2Ftest.zip").unwrap();
        let state = PaneState {
            cwd: cwd.clone(),
            listing: DirListing {
                entries: vec![],
                sort: Sort::NameAsc,
            },
            cursor: 0,
            selected: BTreeSet::new(),
            show_hidden: false,
            sort: Sort::NameAsc,
            filter: None,
            dir_history_back: vec![],
            dir_history_fwd: vec![],
            backend: Arc::new(zip_fs),
        };
        let title = pane_header_title(&state);
        assert!(
            title.contains(&cwd.display()),
            "zip pane header must contain full URI; got: {title:?}"
        );
    }
}

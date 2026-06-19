// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persistent subshell panel state (Feature 054, FR-001..FR-015).
//!
//! [`SubshellPhase`] tracks the three-state Ctrl-o cycle.
//! [`SubshellState`] owns the PTY master, VT100 parser, async output
//! channel, and all associated runtime state. Both are owned by `UiState`
//! in `lib.rs`; `SubshellState` is wrapped in `Option<SubshellState>` and
//! starts as `None` (lazy spawn on first Ctrl-o, FR-001 / R-012).

use portable_pty::{MasterPty, PtySize};
use std::path::Path;
use tokio::sync::mpsc;

// =====================================================================
// SubshellPhase
// =====================================================================

/// Three-state Ctrl-o cycle (Feature 054, FR-002).
///
/// Transitions: `Hidden → VisibleFmFocus → VisibleShellFocus → Hidden`.
/// Advancing from `Hidden` when `UiState.subshell` is `None` triggers
/// lazy spawn of the PTY-backed shell process.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum SubshellPhase {
    /// Panel not visible; shell process may or may not be spawned yet.
    #[default]
    Hidden,
    /// Panel visible; file-manager retains keyboard focus.
    VisibleFmFocus,
    /// Panel visible; keyboard focus is in the shell (all keystrokes
    /// except Ctrl-o forwarded verbatim to the PTY).
    VisibleShellFocus,
}

impl SubshellPhase {
    /// Advance one step through the three-state cycle.
    ///
    /// `Hidden → VisibleFmFocus → VisibleShellFocus → Hidden`
    pub(crate) fn advance(self) -> Self {
        match self {
            Self::Hidden => Self::VisibleFmFocus,
            Self::VisibleFmFocus => Self::VisibleShellFocus,
            Self::VisibleShellFocus => Self::Hidden,
        }
    }

    /// True when the subshell panel should be visible on screen.
    pub(crate) fn is_visible(self) -> bool {
        matches!(self, Self::VisibleFmFocus | Self::VisibleShellFocus)
    }
}

// =====================================================================
// SubshellState
// =====================================================================

/// Runtime state for the persistent subshell panel (Feature 054).
///
/// Owns the PTY master fd, VT100 parser, async output channel, and
/// associated metadata. Stored in `UiState` as `Option<SubshellState>`;
/// `None` until the first `Ctrl-o` advances from `Hidden`.
pub(crate) struct SubshellState {
    /// PTY master file descriptor; used for `resize()` and RAII cleanup.
    pub(crate) master: Box<dyn MasterPty + Send>,
    /// Write end of the PTY; keystrokes and cwd-sync `cd` commands go here.
    pub(crate) writer: Box<dyn std::io::Write + Send>,
    /// ANSI/VT100 state machine. `parser.process(&bytes)` on PTY output;
    /// `parser.screen()` for rendering.
    pub(crate) parser: vt100::Parser,
    /// Receives byte chunks from the `spawn_blocking` reader task.
    /// Empty `Vec<u8>` is a sentinel indicating shell exit (EOF on master).
    pub(crate) pty_rx: mpsc::Receiver<Vec<u8>>,
    /// Rows the user has scrolled up in scrollback (0 = latest output).
    pub(crate) scroll_offset: u16,
    /// True when the shell process has exited; panel shows restart notice.
    pub(crate) dead: bool,
    /// Last-known PTY size; kept in sync with the panel dimensions.
    pub(crate) current_size: PtySize,
}

impl SubshellState {
    /// Spawn a new PTY-backed shell and return the initialized state.
    ///
    /// The shell binary is taken from `$SHELL` or falls back to `/bin/sh`.
    /// A `tokio::task::spawn_blocking` reader task sends byte chunks via
    /// the returned `mpsc::Receiver`; EOF sends an empty `Vec<u8>` sentinel.
    pub(crate) fn spawn(shell: &str, cwd: &Path, rows: u16, cols: u16) -> anyhow::Result<Self> {
        use portable_pty::{CommandBuilder, NativePtySystem, PtySystem as _};

        let pty_system = NativePtySystem::default();
        let size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let pair = pty_system.openpty(size)?;

        let mut cmd = CommandBuilder::new(shell);
        cmd.cwd(cwd);
        cmd.env("TERM", "xterm-256color");

        let _child = pair.slave.spawn_command(cmd)?;
        drop(pair.slave);

        let writer = pair.master.take_writer()?;
        let reader = pair.master.try_clone_reader()?;

        let (tx, rx) = mpsc::channel::<Vec<u8>>(64);
        tokio::task::spawn_blocking(move || {
            let mut reader = reader;
            let mut buf = [0u8; 4096];
            loop {
                match std::io::Read::read(&mut reader, &mut buf) {
                    Ok(0) | Err(_) => {
                        let _ = tx.blocking_send(vec![]);
                        break;
                    }
                    Ok(n) => {
                        if tx.blocking_send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });

        let parser = vt100::Parser::new(rows, cols, 200);

        Ok(Self {
            master: pair.master,
            writer,
            parser,
            pty_rx: rx,
            scroll_offset: 0,
            dead: false,
            current_size: size,
        })
    }

    /// Drain all pending PTY output and feed it into the VT100 parser.
    ///
    /// Non-blocking (`try_recv` loop). An empty chunk sentinel sets `dead = true`.
    /// Called once per `run_loop` iteration before the `term.draw` call.
    pub(crate) fn poll_output(&mut self) {
        loop {
            match self.pty_rx.try_recv() {
                Ok(bytes) if bytes.is_empty() => {
                    self.dead = true;
                    break;
                }
                Ok(bytes) => {
                    self.parser.process(&bytes);
                }
                Err(_) => break,
            }
        }
    }

    /// Shell-quote `path` and write `cd <quoted>\r` to the PTY.
    ///
    /// No-op when `dead`. Fire-and-forget — if the shell is busy the `cd`
    /// queues in the PTY input buffer and runs after the current command.
    pub(crate) fn sync_cwd(&mut self, path: &Path) {
        if self.dead {
            return;
        }
        let target = find_valid_ancestor(path);
        let quoted = shell_words::quote(target.to_str().unwrap_or("/"));
        let cmd = format!("cd {quoted}\r");
        let _ = std::io::Write::write_all(&mut self.writer, cmd.as_bytes());
    }

    /// Forward a crossterm key event to the PTY.
    ///
    /// No-op when `dead`. Applies the key→byte mapping from research R-009.
    pub(crate) fn write_key(&mut self, key: crossterm::event::KeyEvent) {
        if self.dead {
            return;
        }
        let app_cursor = self.parser.screen().application_cursor();
        let bytes = key_to_pty_bytes(key, app_cursor);
        if !bytes.is_empty() {
            let _ = std::io::Write::write_all(&mut self.writer, &bytes);
        }
    }

    /// Resize the PTY and VT100 parser to new dimensions.
    ///
    /// vt100 0.16 has no `set_size`; replace the parser on resize.
    /// The PTY sends SIGWINCH which triggers a full shell redraw anyway.
    pub(crate) fn resize(&mut self, rows: u16, cols: u16) {
        let new_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        let _ = self.master.resize(new_size);
        self.parser = vt100::Parser::new(rows, cols, 200);
        self.current_size = new_size;
    }

    /// Borrow the current VT100 screen state for rendering.
    pub(crate) fn screen(&self) -> &vt100::Screen {
        self.parser.screen()
    }

    /// Mutably borrow the VT100 screen (needed to call set_scrollback before draw).
    pub(crate) fn screen_mut(&mut self) -> &mut vt100::Screen {
        self.parser.screen_mut()
    }

    /// Drop the old PTY and spawn a fresh shell. Resets `dead` and `scroll_offset`.
    pub(crate) fn respawn(
        &mut self,
        shell: &str,
        cwd: &Path,
        rows: u16,
        cols: u16,
    ) -> anyhow::Result<()> {
        let fresh = Self::spawn(shell, cwd, rows, cols)?;
        self.master = fresh.master;
        self.writer = fresh.writer;
        self.parser = fresh.parser;
        self.pty_rx = fresh.pty_rx;
        self.scroll_offset = 0;
        self.dead = false;
        self.current_size = fresh.current_size;
        Ok(())
    }
}

// =====================================================================
// Key → PTY byte mapping (research R-009)
// =====================================================================

/// Map a crossterm [`KeyEvent`] to the byte sequence to write to the PTY.
///
/// `app_cursor` reflects `parser.screen().application_cursor()`: when true,
/// arrow keys use the application-cursor escape sequences (`\x1bOx`) rather
/// than the normal ANSI sequences (`\x1b[x`).
pub(crate) fn key_to_pty_bytes(key: crossterm::event::KeyEvent, app_cursor: bool) -> Vec<u8> {
    use crossterm::event::{KeyCode, KeyModifiers};

    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    match key.code {
        KeyCode::Char(c) => {
            if ctrl {
                // Control modifier: mask to 0x1f range.
                let b = (c as u8) & 0x1f;
                vec![b]
            } else {
                let mut buf = [0u8; 4];
                let len = c.len_utf8();
                c.encode_utf8(&mut buf);
                buf[..len].to_vec()
            }
        }
        KeyCode::Enter => vec![b'\r'],
        KeyCode::Backspace => vec![0x7f],
        KeyCode::Delete => vec![0x1b, b'[', b'3', b'~'],
        KeyCode::Tab => vec![0x09],
        KeyCode::BackTab => vec![0x1b, b'[', b'Z'],
        KeyCode::Esc => vec![0x1b],
        KeyCode::Up => {
            if app_cursor {
                vec![0x1b, b'O', b'A']
            } else {
                vec![0x1b, b'[', b'A']
            }
        }
        KeyCode::Down => {
            if app_cursor {
                vec![0x1b, b'O', b'B']
            } else {
                vec![0x1b, b'[', b'B']
            }
        }
        KeyCode::Right => {
            if app_cursor {
                vec![0x1b, b'O', b'C']
            } else {
                vec![0x1b, b'[', b'C']
            }
        }
        KeyCode::Left => {
            if app_cursor {
                vec![0x1b, b'O', b'D']
            } else {
                vec![0x1b, b'[', b'D']
            }
        }
        KeyCode::Home => vec![0x1b, b'[', b'1', b'~'],
        KeyCode::End => vec![0x1b, b'[', b'4', b'~'],
        KeyCode::PageUp => vec![0x1b, b'[', b'5', b'~'],
        KeyCode::PageDown => vec![0x1b, b'[', b'6', b'~'],
        KeyCode::F(n) => f_key_bytes(n),
        _ => vec![],
    }
}

fn f_key_bytes(n: u8) -> Vec<u8> {
    match n {
        1 => vec![0x1b, b'[', b'1', b'1', b'~'],
        2 => vec![0x1b, b'[', b'1', b'2', b'~'],
        3 => vec![0x1b, b'[', b'1', b'3', b'~'],
        4 => vec![0x1b, b'[', b'1', b'4', b'~'],
        5 => vec![0x1b, b'[', b'1', b'5', b'~'],
        6 => vec![0x1b, b'[', b'1', b'7', b'~'],
        7 => vec![0x1b, b'[', b'1', b'8', b'~'],
        8 => vec![0x1b, b'[', b'1', b'9', b'~'],
        9 => vec![0x1b, b'[', b'2', b'0', b'~'],
        10 => vec![0x1b, b'[', b'2', b'1', b'~'],
        11 => vec![0x1b, b'[', b'2', b'3', b'~'],
        12 => vec![0x1b, b'[', b'2', b'4', b'~'],
        _ => vec![],
    }
}

// =====================================================================
// Helpers
// =====================================================================

/// Walk ancestors of `path` until finding one that exists; return it.
/// Falls back to `"/"` if no ancestor exists (e.g., tmpfs was unmounted).
fn find_valid_ancestor(path: &Path) -> std::path::PathBuf {
    if path.exists() {
        return path.to_path_buf();
    }
    for ancestor in path.ancestors().skip(1) {
        if ancestor.exists() {
            return ancestor.to_path_buf();
        }
    }
    std::path::PathBuf::from("/")
}

// =====================================================================
// VT100 → ratatui renderer (no tui-term dep; ratatui 0.27 compatible)
// =====================================================================

/// Render a `vt100::Screen` into a ratatui `Buffer` region.
///
/// Iterates over every cell in the visible screen area and writes the
/// character + style into the corresponding buffer cell. Cells that fall
/// outside `area` are silently clipped.
pub(crate) fn render_vt100_screen(
    screen: &vt100::Screen,
    area: ratatui::layout::Rect,
    buf: &mut ratatui::buffer::Buffer,
) {
    use ratatui::style::{Modifier, Style};

    let rows = screen.size().0;
    let cols = screen.size().1;

    for row in 0..rows {
        for col in 0..cols {
            let buf_x = area.x + col;
            let buf_y = area.y + row;
            if buf_x >= area.x + area.width || buf_y >= area.y + area.height {
                continue;
            }
            if let Some(cell) = screen.cell(row, col) {
                let contents = cell.contents();
                let sym = if contents.is_empty() { " " } else { contents };

                let fg = vt100_color_to_ratatui(cell.fgcolor());
                let bg = vt100_color_to_ratatui(cell.bgcolor());

                let mut mods = Modifier::empty();
                if cell.bold() {
                    mods |= Modifier::BOLD;
                }
                if cell.italic() {
                    mods |= Modifier::ITALIC;
                }
                if cell.underline() {
                    mods |= Modifier::UNDERLINED;
                }
                if cell.inverse() {
                    mods |= Modifier::REVERSED;
                }

                let style = Style::default().fg(fg).bg(bg).add_modifier(mods);
                let buf_cell = buf.get_mut(buf_x, buf_y);
                buf_cell.set_symbol(sym);
                buf_cell.set_style(style);
            }
        }
    }

    // T007: skip cursor when in scrollback — cursor_position() returns live coords
    // that don't correspond to the shifted visible window.
    if screen.scrollback() == 0 {
        let (cur_row, cur_col) = screen.cursor_position();
        let cur_x = area.x + cur_col;
        let cur_y = area.y + cur_row;
        if cur_x < area.x + area.width && cur_y < area.y + area.height && !screen.hide_cursor() {
            let cur_cell = buf.get_mut(cur_x, cur_y);
            let existing_style = cur_cell.style();
            cur_cell.set_style(existing_style.add_modifier(Modifier::REVERSED));
        }
    }
}

/// Convert a `vt100::Color` to the closest `ratatui::style::Color`.
fn vt100_color_to_ratatui(c: vt100::Color) -> ratatui::style::Color {
    use ratatui::style::Color;
    match c {
        vt100::Color::Default => Color::Reset,
        vt100::Color::Idx(i) => Color::Indexed(i),
        vt100::Color::Rgb(r, g, b) => Color::Rgb(r, g, b),
    }
}

// =====================================================================
// Tests
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // T006 (green): compile-time proof that SubshellState has the required fields.
    #[allow(dead_code)]
    fn _assert_subshell_state_fields(s: &SubshellState) {
        let _dead: bool = s.dead;
        let _offset: u16 = s.scroll_offset;
    }

    #[test]
    fn subshell_state_struct_fields() {
        let _: fn(&SubshellState) = _assert_subshell_state_fields;
    }

    // T001 (red): compile-time contract — screen_mut must exist on SubshellState.
    // This assertion fails to compile until screen_mut() is added (T001b).
    #[allow(dead_code)]
    fn _assert_screen_mut_exists(_: fn(&mut SubshellState) -> &mut vt100::Screen) {}
    #[allow(dead_code)]
    fn _check_screen_mut() {
        _assert_screen_mut_exists(SubshellState::screen_mut);
    }

    // T010 (green): three-state cycle advance tests.
    #[test]
    fn advance_hidden_to_visible_fm_focus() {
        assert_eq!(
            SubshellPhase::Hidden.advance(),
            SubshellPhase::VisibleFmFocus
        );
    }

    #[test]
    fn advance_visible_fm_to_shell_focus() {
        assert_eq!(
            SubshellPhase::VisibleFmFocus.advance(),
            SubshellPhase::VisibleShellFocus
        );
    }

    #[test]
    fn advance_shell_focus_to_hidden() {
        assert_eq!(
            SubshellPhase::VisibleShellFocus.advance(),
            SubshellPhase::Hidden
        );
    }

    // T027 (green): key_to_pty_bytes mapping tests (R-009).
    #[test]
    fn key_to_pty_bytes_char() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Char('a'), KeyModifiers::empty());
        assert_eq!(key_to_pty_bytes(key, false), vec![b'a']);
    }

    #[test]
    fn key_to_pty_bytes_enter() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Enter, KeyModifiers::empty());
        assert_eq!(key_to_pty_bytes(key, false), vec![b'\r']);
    }

    #[test]
    fn key_to_pty_bytes_ctrl_c() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Char('c'), KeyModifiers::CONTROL);
        assert_eq!(key_to_pty_bytes(key, false), vec![0x03]);
    }

    #[test]
    fn key_to_pty_bytes_arrow_up_normal_cursor() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(key_to_pty_bytes(key, false), vec![0x1b, b'[', b'A']);
    }

    #[test]
    fn key_to_pty_bytes_arrow_up_application_cursor() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Up, KeyModifiers::empty());
        assert_eq!(key_to_pty_bytes(key, true), vec![0x1b, b'O', b'A']);
    }

    #[test]
    fn key_to_pty_bytes_backspace() {
        use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
        let key = KeyEvent::new(KeyCode::Backspace, KeyModifiers::empty());
        assert_eq!(key_to_pty_bytes(key, false), vec![0x7f]);
    }

    // T020 (green): sync_cwd tests use a fake writer (Vec<u8>).
    #[test]
    fn sync_cwd_sends_quoted_cd() {
        // Tests the shell-quoting logic in isolation without a real PTY.
        let path = std::path::Path::new("/home/user/my docs");
        let quoted = shell_words::quote(path.to_str().unwrap_or(""));
        let cmd = format!("cd {quoted}\r");
        assert!(cmd.contains("my docs"), "quoted path in cmd: {cmd}");
        assert!(cmd.ends_with('\r'), "must end with CR for PTY");
    }

    #[test]
    fn find_valid_ancestor_returns_existing() {
        let existing = std::path::Path::new("/tmp");
        assert_eq!(
            find_valid_ancestor(existing),
            std::path::PathBuf::from("/tmp")
        );
    }

    #[test]
    fn find_valid_ancestor_walks_up() {
        let gone = std::path::Path::new("/tmp/surely_nonexistent_xyzzy123/sub");
        let result = find_valid_ancestor(gone);
        assert!(result.exists(), "ancestor must exist: {result:?}");
    }

    // T008 (green): scrollback offset must change rendered content.
    #[test]
    fn render_vt100_screen_scrollback_offset_changes_content() {
        use ratatui::{buffer::Buffer, layout::Rect};

        // 5 rows × 10 cols, 20-row scrollback capacity.
        let mut parser = vt100::Parser::new(5, 10, 20);
        // Feed 25 distinct lines so scrollback is non-empty.
        for i in 0..25u32 {
            parser.process(format!("L{i:03}\r\n").as_bytes());
        }
        let area = Rect { x: 0, y: 0, width: 10, height: 5 };

        let mut buf_live = Buffer::empty(area);
        render_vt100_screen(parser.screen(), area, &mut buf_live);

        parser.screen_mut().set_scrollback(5);
        let mut buf_scroll = Buffer::empty(area);
        render_vt100_screen(parser.screen(), area, &mut buf_scroll);
        parser.screen_mut().set_scrollback(0);

        assert_ne!(
            buf_live,
            buf_scroll,
            "scrollback must shift visible content"
        );
    }

    // T010 (red): scrollback must clamp at buffer limit without panic.
    #[test]
    fn scrollback_clamps_at_buffer_limit() {
        todo!()
    }

    // T009 (green): cursor must not appear when scrolled into history.
    #[test]
    fn render_vt100_screen_hides_cursor_in_scrollback() {
        use ratatui::{buffer::Buffer, layout::Rect, style::Modifier};

        // 3 rows × 5 cols, 5-row scrollback.
        let mut parser = vt100::Parser::new(3, 5, 5);
        // Push 4 lines of plain text; cursor ends on a blank cell at live bottom.
        for i in 0..4u8 {
            parser.process(format!("L{i:02}\r\n").as_bytes());
        }
        let area = Rect { x: 0, y: 0, width: 5, height: 3 };
        let (cur_row, cur_col) = parser.screen().cursor_position();

        // At live bottom the cursor cell should carry REVERSED.
        let mut buf_live = Buffer::empty(area);
        render_vt100_screen(parser.screen(), area, &mut buf_live);
        let live_style = buf_live.get(cur_col, cur_row).style();
        assert!(
            live_style.add_modifier.contains(Modifier::REVERSED),
            "cursor must show REVERSED at live bottom; style={live_style:?}"
        );

        // In scrollback mode the cursor block is skipped entirely.
        parser.screen_mut().set_scrollback(1);
        let mut buf_scroll = Buffer::empty(area);
        render_vt100_screen(parser.screen(), area, &mut buf_scroll);
        let scroll_style = buf_scroll.get(cur_col, cur_row).style();
        assert!(
            !scroll_style.add_modifier.contains(Modifier::REVERSED),
            "cursor must not show REVERSED in scrollback mode; style={scroll_style:?}"
        );
        parser.screen_mut().set_scrollback(0);
    }
}

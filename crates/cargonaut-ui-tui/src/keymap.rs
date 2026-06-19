// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Keymap parser + runtime lookup.
//!
//! Loads `design/contracts/keymap.toml` (or a user override at
//! `~/.config/cargonaut/keymap.toml`) into a [`Keymap`] indexed by
//! `(Mode, KeyChord)` → [`Command`]. The dispatcher (T1.19 `App`) calls
//! [`Keymap::lookup`] on every `crossterm::event::KeyEvent` and runs the
//! returned [`Command`] (or no-ops on `None`).
//!
//! Schema mirror of `design/contracts/keymap.toml`:
//!
//! ```toml
//! [[binding]]
//! mode = "pane"
//! key = "F5"
//! action = "copy-selection"
//! ```

use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ============================================================
// Mode / Command
// ============================================================

/// Which input mode a binding applies in. The dispatcher tracks the
/// active mode and looks up bindings in (active_mode | Global).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Mode {
    /// Always-active bindings (F1, F10, Tab, Alt-1/2, …).
    Global,
    /// File pane has focus.
    Pane,
    /// A modal dialog is open.
    Dialog,
    /// Search prompt is open.
    Search,
    /// Subshell is active (e.g. Ctrl-O drop-down terminal — Phase 4).
    Subshell,
    /// Previewer pane has focus (FR-209).
    Preview,
}

/// Every recognized action a binding can resolve to. Strings in the TOML
/// `action = "…"` field use kebab-case and are mapped to these variants
/// via `#[serde(rename_all = "kebab-case")]`.
///
/// Unknown actions in the TOML cause [`Keymap::load`] to error — bindings
/// to actions the engine doesn't recognize are silently broken otherwise.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Command {
    // Global
    /// Show the help dialog.
    ShowHelp,
    /// Quit cargonaut.
    Quit,
    /// Swap focus between the two panes.
    FocusSwapPane,
    /// Focus the left pane.
    FocusLeftPane,
    /// Focus the right pane.
    FocusRightPane,
    /// Reload config + themes from disk.
    ReloadConfigAndThemes,
    // Pane navigation
    /// Move cursor down one entry.
    CursorDown,
    /// Move cursor up one entry.
    CursorUp,
    /// Descend into the focused directory (or open the focused file).
    DescendOrOpen,
    /// Ascend to the parent directory.
    AscendParent,
    /// `cd ~`.
    CdHome,
    /// `cd /`.
    CdRoot,
    /// Open the command-line prompt (`:`).
    OpenCmdline,
    // Selection
    /// Toggle selection on the focused entry (Insert).
    SelectionToggle,
    /// Invert the entire selection (`*`).
    SelectionInvert,
    /// Open the glob-pattern dialog to add to selection (`+`).
    SelectionAddByPattern,
    /// Open the glob-pattern dialog to remove from selection (`-`).
    SelectionRemoveByPattern,
    // File operations
    /// Copy the selection to the other pane (F5).
    CopySelection,
    /// Move/rename the selection (F6).
    MoveOrRenameSelection,
    /// Create a new directory (F7).
    Mkdir,
    /// Delete the selection (F8).
    DeleteSelection,
    /// Cancel the current operation (Ctrl-c).
    CancelCurrentOperation,
    // Sorting
    /// Cycle to the next sort key (Ctrl-s).
    CycleSortKey,
    // Preview + editor
    /// Open the previewer (F3).
    Preview,
    /// Spawn `$EDITOR` (F4).
    Edit,
    // Power features
    /// Open the bookmarks menu (Ctrl-b).
    BookmarksMenu,
    /// Open a new tab (Ctrl-t).
    NewTab,
    /// Close the current tab (Ctrl-w).
    CloseTab,
    /// Open the filter prompt for the current dir (Ctrl-f).
    FilterCurrentDir,
    /// Undo the last destructive operation (Ctrl-z).
    UndoLastOp,
    /// Drop down a subshell (Ctrl-o).
    OpenSubshell,
    // Search mode
    /// Close the search prompt.
    CloseSearch,
    /// Navigate to the focused search result.
    SearchGoToResult,
    // Dialog mode
    /// Dismiss the active dialog.
    DialogCancel,
    /// Confirm the active dialog (Enter).
    DialogConfirm,
    // FR-011 history
    /// Open the directory-history popup (Alt-Shift-h).
    ShowDirectoryHistory,
    /// Open the command-history popup (Alt-h).
    ShowCommandHistory,
    /// Step to the previous directory in the per-pane history (Alt-y).
    HistoryPrevDir,
    /// Step to the next directory in the per-pane history (Alt-u).
    HistoryNextDir,
    // FR-012 / FR-013 / FR-014 panel ergonomics
    /// Open the quick-cd popup (Alt-c).
    QuickCdPopup,
    /// Toggle the panel filter prompt (Alt-!).
    TogglePanelFilter,
    /// Copy other pane's path to focused pane (Alt-i).
    SyncOtherPanelPath,
    /// Show the focused entry's dir in the other pane (Alt-o).
    ShowFocusedInOtherPanel,
    // FR-015 niceties
    /// Toggle hidden-file visibility per-pane (Alt-.).
    ToggleHidden,
    /// Toggle split orientation vertical ↔ horizontal (Alt-,).
    ToggleSplitOrientation,
    /// Compute recursive size for focused/tagged entries (Ctrl-Space).
    RecursiveDirSize,
    // FR-016 jobs panel
    /// Open the in-flight transfers panel (F12).
    ShowTasksPanel,
    // FR-204+ power features
    /// External panelize: run cmd, present stdout as a pane (Ctrl-x !).
    ExternalPanelize,
    /// Open the F2 user menu.
    ShowUserMenu,
    /// Open the F9 menu bar.
    OpenMenuBar,
    /// Cycle per-pane listing mode (Alt-t).
    CycleListingMode,
    /// Compare directories dialog (Ctrl-x d).
    CompareDirectories,
    /// Side-by-side diff of two tagged files (Ctrl-x Ctrl-d).
    DiffTwoTaggedFiles,
    /// Bulk-rename the tagged files via `$EDITOR` (Ctrl-x r).
    BulkRenameViaEditor,
    // FR-209 previewer hex view + search
    /// Toggle the previewer's hex view (Ctrl-x X).
    ToggleHexView,
    /// Forward in-previewer search (`/`).
    PreviewSearchForward,
    /// Backward in-previewer search (`?`).
    PreviewSearchBackward,
    /// Next match in the previewer (`n`).
    PreviewSearchNext,
    /// Previous match in the previewer (`N`).
    PreviewSearchPrev,
    // Feature 051 — built-in file viewer (FR-031)
    /// Open the goto prompt in the file viewer (`g`).
    ViewerGoto,
    /// Jump to the last line or hex row in the file viewer (`G`).
    ViewerEnd,
    /// Toggle word-wrap in text-mode viewer (`w`).
    ViewerWrap,
    /// Close the file viewer (`q`).
    ViewerQuit,
    // FR-210 fuzzy filter
    /// Open the fuzzy filter prompt (`<`).
    OpenFuzzyFilter,
    // Feature 041 (FR-013 follow-up, #38)
    /// Toggle runtime mouse capture on/off (Alt-m).
    ToggleMouseCapture,
    // Feature 043 (file attributes, #46)
    /// Change permissions of the selection (C-x c).
    Chmod,
    /// Change ownership of the selection (C-x o).
    Chown,
    /// Create a symbolic link to the focused entry (C-x s).
    CreateSymlink,
    /// Create a hard link to the focused entry (C-x l).
    CreateHardLink,
    // Feature 044 (recursive attributes, #65)
    /// Recursively change permissions of a directory subtree (C-x C).
    ChmodRecursive,
    /// Recursively change ownership of a directory subtree (C-x O).
    ChownRecursive,
}

// ============================================================
// KeyChord
// ============================================================

/// A parsed key with its modifier set. Built by [`parse_key_chord`] from
/// the TOML `key = "…"` string; matched against `crossterm` `KeyEvent`s at
/// dispatch time.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyChord {
    /// The non-modifier portion (character, function key, or named special).
    pub code: KeyCode,
    /// The modifier mask (Ctrl, Alt, Shift).
    pub modifiers: KeyModifiers,
}

/// A whitespace-separated sequence of [`KeyChord`]s, e.g. `C-x !`.
/// Length-1 sequences are the common case (single chord); length 2+
/// drive the multi-key state machine in the dispatcher (T1.19).
pub type KeySequence = Vec<KeyChord>;

/// Parse a keymap.toml-format key string into a [`KeySequence`].
/// Splits on whitespace and parses each segment via [`parse_key_chord`].
pub fn parse_key_sequence(s: &str) -> Result<KeySequence, KeymapError> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.is_empty() {
        return Err(KeymapError::BadKey(s.into()));
    }
    parts.into_iter().map(parse_key_chord).collect()
}

/// Result of looking up an in-flight chord sequence.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SeqLookup {
    /// Sequence exactly matches a binding — dispatch this command.
    Command(Command),
    /// Sequence is a strict prefix of one or more bindings — caller
    /// should wait for the next chord rather than treating this as
    /// unbound.
    Pending,
    /// Neither a match nor a prefix — the sequence is unbound.
    NoMatch,
}

/// Parse a keymap.toml-format key string into a [`KeyChord`].
///
/// Supports:
/// - Function keys `F1`..`F12`.
/// - Named specials: `Tab`, `Enter`, `Esc`, `Backspace`, `Up`, `Down`,
///   `Left`, `Right`, `Insert`, `Delete`, `Home`, `End`, `PageUp`,
///   `PageDown`, `Space`.
/// - Single characters (literal or shifted).
/// - Modifier-prefixed forms: `C-x` (Ctrl), `M-x` (Alt/Meta), `S-x`
///   (Shift). Combinable: `C-S-x`.
pub fn parse_key_chord(s: &str) -> Result<KeyChord, KeymapError> {
    let mut modifiers = KeyModifiers::empty();
    let mut rest = s;
    loop {
        if let Some(tail) = rest.strip_prefix("C-") {
            modifiers |= KeyModifiers::CONTROL;
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("M-") {
            modifiers |= KeyModifiers::ALT;
            rest = tail;
        } else if let Some(tail) = rest.strip_prefix("S-") {
            modifiers |= KeyModifiers::SHIFT;
            rest = tail;
        } else {
            break;
        }
    }
    let code = match rest {
        "" => return Err(KeymapError::BadKey(s.into())),
        "Tab" => KeyCode::Tab,
        "Enter" => KeyCode::Enter,
        "Esc" => KeyCode::Esc,
        "Backspace" => KeyCode::Backspace,
        "Up" => KeyCode::Up,
        "Down" => KeyCode::Down,
        "Left" => KeyCode::Left,
        "Right" => KeyCode::Right,
        "Insert" => KeyCode::Insert,
        "Delete" => KeyCode::Delete,
        "Home" => KeyCode::Home,
        "End" => KeyCode::End,
        "PageUp" => KeyCode::PageUp,
        "PageDown" => KeyCode::PageDown,
        "Space" => KeyCode::Char(' '),
        f if f.starts_with('F') && f.len() <= 3 => {
            let n: u8 = f[1..].parse().map_err(|_| KeymapError::BadKey(s.into()))?;
            if !(1..=12).contains(&n) {
                return Err(KeymapError::BadKey(s.into()));
            }
            KeyCode::F(n)
        }
        other if other.chars().count() == 1 => {
            let c = other.chars().next().unwrap();
            KeyCode::Char(c)
        }
        _ => return Err(KeymapError::BadKey(s.into())),
    };
    Ok(KeyChord { code, modifiers })
}

// ============================================================
// Binding / Keymap
// ============================================================

/// One raw binding row from `keymap.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct RawBinding {
    mode: Mode,
    key: String,
    action: Command,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct KeymapDoc {
    binding: Vec<RawBinding>,
}

/// In-memory keymap. Built from TOML via [`Keymap::load`]; queried at
/// dispatch time via [`Keymap::lookup`] (single chord) or
/// [`Keymap::lookup_sequence`] (multi-chord, e.g. `C-x !`).
///
/// A binding `(mode, sequence) -> command` overrides earlier bindings
/// with the same `(mode, sequence)`. Per-user override files merge by
/// replacing matching keys via [`Keymap::merge`].
#[derive(Debug, Clone, Default)]
pub struct Keymap {
    bindings: HashMap<(Mode, KeySequence), Command>,
}

impl Keymap {
    /// Parse a TOML string into a [`Keymap`].
    pub fn load(toml_text: &str) -> Result<Self, KeymapError> {
        let doc: KeymapDoc =
            toml::from_str(toml_text).map_err(|e| KeymapError::Parse(e.to_string()))?;
        let mut bindings = HashMap::with_capacity(doc.binding.len());
        for b in doc.binding {
            let seq = parse_key_sequence(&b.key)?;
            bindings.insert((b.mode, seq), b.action);
        }
        Ok(Self { bindings })
    }

    /// Resolve a single `KeyEvent` in the given mode to a [`Command`].
    /// Convenience for single-key bindings (the common case) — looks up
    /// `mode` first, falls back to [`Mode::Global`]. Returns `None` if
    /// the key is unbound *or* if the binding is multi-chord (use
    /// [`Self::lookup_sequence`] for those).
    pub fn lookup(&self, mode: Mode, event: KeyEvent) -> Option<Command> {
        let seq = vec![KeyChord {
            code: event.code,
            modifiers: event.modifiers,
        }];
        match self.lookup_sequence(mode, &seq) {
            SeqLookup::Command(c) => Some(c),
            SeqLookup::Pending | SeqLookup::NoMatch => None,
        }
    }

    /// Resolve a chord sequence in the given mode. Returns:
    /// - `Command(c)` if `seq` exactly matches a binding.
    /// - `Pending` if `seq` is a strict prefix of some binding (caller
    ///   waits for the next chord).
    /// - `NoMatch` if neither.
    ///
    /// Searches `mode`-scoped bindings first; falls back to
    /// [`Mode::Global`].
    pub fn lookup_sequence(&self, mode: Mode, seq: &[KeyChord]) -> SeqLookup {
        if seq.is_empty() {
            return SeqLookup::NoMatch;
        }
        let key = seq.to_vec();
        // Exact-match check, mode then Global.
        if let Some(cmd) = self.bindings.get(&(mode, key.clone())) {
            return SeqLookup::Command(*cmd);
        }
        if let Some(cmd) = self.bindings.get(&(Mode::Global, key.clone())) {
            return SeqLookup::Command(*cmd);
        }
        // Prefix check — is anything strictly longer that starts with seq?
        let has_continuation = self.bindings.keys().any(|(m, ks)| {
            (*m == mode || *m == Mode::Global) && ks.len() > seq.len() && ks.starts_with(seq)
        });
        if has_continuation {
            SeqLookup::Pending
        } else {
            SeqLookup::NoMatch
        }
    }

    /// Merge `other`'s bindings on top of `self` — overlapping keys win
    /// for `other`. Used to layer a user override on top of the bundled
    /// default.
    pub fn merge(&mut self, other: Keymap) {
        self.bindings.extend(other.bindings);
    }

    /// Number of bindings currently in the map (mostly for tests + debug).
    pub fn len(&self) -> usize {
        self.bindings.len()
    }

    /// True if no bindings are loaded.
    pub fn is_empty(&self) -> bool {
        self.bindings.is_empty()
    }
}

// ============================================================
// Errors
// ============================================================

/// Errors from loading or parsing a keymap.
#[derive(Debug, thiserror::Error)]
pub enum KeymapError {
    /// TOML parse failure (malformed file, wrong field types, unknown
    /// mode/action value).
    #[error("parse: {0}")]
    Parse(String),

    /// `key = "…"` couldn't be parsed into a [`KeyChord`].
    #[error("bad key {0:?}")]
    BadKey(String),
}

// ============================================================
// Tests
// ============================================================

#[cfg(test)]
mod tests {
    use super::*;

    /// The canonical bundled keymap, embedded at compile time so the test
    /// runs without depending on the working directory.
    const DEFAULT_KEYMAP_TOML: &str = include_str!("../../../design/contracts/keymap.toml");

    #[test]
    fn parses_minimal_toml() {
        let km = Keymap::load(
            r#"
[[binding]]
mode = "pane"
key = "j"
action = "cursor-down"
"#,
        )
        .unwrap();
        assert_eq!(km.len(), 1);
    }

    #[test]
    fn parses_full_default_keymap_without_error() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).expect("default keymap must parse");
        assert!(km.len() >= 35, "expected ≥35 bindings, got {}", km.len());
    }

    #[test]
    fn parses_function_keys() {
        let c = parse_key_chord("F10").unwrap();
        assert_eq!(c.code, KeyCode::F(10));
        assert!(c.modifiers.is_empty());
    }

    #[test]
    fn parses_named_specials() {
        for (s, expected) in [
            ("Tab", KeyCode::Tab),
            ("Enter", KeyCode::Enter),
            ("Esc", KeyCode::Esc),
            ("Backspace", KeyCode::Backspace),
            ("Up", KeyCode::Up),
            ("PageDown", KeyCode::PageDown),
            ("Space", KeyCode::Char(' ')),
        ] {
            let c = parse_key_chord(s).unwrap();
            assert_eq!(c.code, expected, "key {s}");
            assert!(c.modifiers.is_empty(), "key {s}");
        }
    }

    #[test]
    fn parses_modifier_prefixes() {
        let c = parse_key_chord("C-x").unwrap();
        assert_eq!(c.code, KeyCode::Char('x'));
        assert_eq!(c.modifiers, KeyModifiers::CONTROL);

        let c = parse_key_chord("M-1").unwrap();
        assert_eq!(c.code, KeyCode::Char('1'));
        assert_eq!(c.modifiers, KeyModifiers::ALT);

        let c = parse_key_chord("C-S-a").unwrap();
        assert_eq!(c.code, KeyCode::Char('a'));
        assert_eq!(c.modifiers, KeyModifiers::CONTROL | KeyModifiers::SHIFT);
    }

    #[test]
    fn parse_key_chord_rejects_garbage() {
        assert!(parse_key_chord("").is_err());
        assert!(parse_key_chord("F13").is_err());
        assert!(parse_key_chord("F0").is_err());
        assert!(parse_key_chord("ZZZ").is_err());
    }

    #[test]
    fn unknown_action_errors() {
        let res = Keymap::load(
            r#"
[[binding]]
mode = "pane"
key = "j"
action = "this-action-does-not-exist"
"#,
        );
        assert!(res.is_err());
    }

    #[test]
    fn lookup_global_f10_resolves_to_quit_regardless_of_mode() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let ev = KeyEvent::new(KeyCode::F(10), KeyModifiers::empty());
        for mode in [Mode::Pane, Mode::Dialog, Mode::Search] {
            assert_eq!(
                km.lookup(mode, ev),
                Some(Command::Quit),
                "F10 must fall through to Global from mode {mode:?}"
            );
        }
    }

    #[test]
    fn lookup_resolves_alt_1_to_focus_left_pane() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let ev = KeyEvent::new(KeyCode::Char('1'), KeyModifiers::ALT);
        assert_eq!(km.lookup(Mode::Pane, ev), Some(Command::FocusLeftPane));
    }

    #[test]
    fn lookup_resolves_alt_m_to_toggle_mouse_capture() {
        // FR-001: M-m toggles mouse capture. Bound in `global` mode, so it must
        // resolve from any active mode (like F10/F12). FR-009: no other binding
        // claims M-m — asserting the resolved command *is* ToggleMouseCapture
        // (and not something else) in Pane mode proves there's no collision.
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let ev = KeyEvent::new(KeyCode::Char('m'), KeyModifiers::ALT);
        for mode in [Mode::Pane, Mode::Preview, Mode::Search] {
            assert_eq!(
                km.lookup(mode, ev),
                Some(Command::ToggleMouseCapture),
                "M-m must resolve to ToggleMouseCapture from mode {mode:?}"
            );
        }
    }

    #[test]
    fn lookup_resolves_pane_j_to_cursor_down() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let ev = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        assert_eq!(km.lookup(Mode::Pane, ev), Some(Command::CursorDown));
    }

    #[test]
    fn lookup_returns_none_for_unbound_key() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let ev = KeyEvent::new(
            KeyCode::Char('Q'),
            KeyModifiers::CONTROL | KeyModifiers::ALT,
        );
        assert_eq!(km.lookup(Mode::Pane, ev), None);
    }

    #[test]
    fn parses_multi_chord_sequence() {
        let seq = parse_key_sequence("C-x !").unwrap();
        assert_eq!(seq.len(), 2);
        assert_eq!(seq[0].code, KeyCode::Char('x'));
        assert_eq!(seq[0].modifiers, KeyModifiers::CONTROL);
        assert_eq!(seq[1].code, KeyCode::Char('!'));
        assert!(seq[1].modifiers.is_empty());
    }

    #[test]
    fn lookup_sequence_returns_pending_for_prefix() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        // C-x alone should be a prefix of C-x ! / C-x d / C-x r / C-x X / C-x C-d
        let c_x = KeyChord {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        };
        assert_eq!(
            km.lookup_sequence(Mode::Pane, &[c_x]),
            SeqLookup::Pending,
            "C-x must be a Pending prefix of the multi-chord bindings"
        );
    }

    #[test]
    fn lookup_sequence_returns_command_for_full_match() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let c_x = KeyChord {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        };
        let bang = KeyChord {
            code: KeyCode::Char('!'),
            modifiers: KeyModifiers::empty(),
        };
        assert_eq!(
            km.lookup_sequence(Mode::Pane, &[c_x, bang]),
            SeqLookup::Command(Command::ExternalPanelize),
            "C-x ! must resolve to external-panelize"
        );
    }

    // Feature 043: the orthodox attribute chords C-x c/o/s/l.
    #[test]
    fn attribute_chords_resolve() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let c_x = KeyChord {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        };
        let plain = |c| KeyChord {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        };
        for (ch, cmd) in [
            ('c', Command::Chmod),
            ('o', Command::Chown),
            ('s', Command::CreateSymlink),
            ('l', Command::CreateHardLink),
        ] {
            assert_eq!(
                km.lookup_sequence(Mode::Pane, &[c_x, plain(ch)]),
                SeqLookup::Command(cmd),
                "C-x {ch} must resolve to {cmd:?}"
            );
        }
    }

    // Feature 044: recursive attribute chords C-x C / C-x O (case-sensitive).
    #[test]
    fn recursive_attribute_chords_resolve() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let c_x = KeyChord {
            code: KeyCode::Char('x'),
            modifiers: KeyModifiers::CONTROL,
        };
        let upper = |c| KeyChord {
            code: KeyCode::Char(c),
            modifiers: KeyModifiers::empty(),
        };
        for (ch, cmd) in [
            ('C', Command::ChmodRecursive),
            ('O', Command::ChownRecursive),
        ] {
            assert_eq!(
                km.lookup_sequence(Mode::Pane, &[c_x, upper(ch)]),
                SeqLookup::Command(cmd),
                "C-x {ch} must resolve to {cmd:?}"
            );
        }
        // …and the lowercase shallow chords still resolve distinctly.
        assert_eq!(
            km.lookup_sequence(
                Mode::Pane,
                &[
                    c_x,
                    KeyChord {
                        code: KeyCode::Char('c'),
                        modifiers: KeyModifiers::empty()
                    }
                ]
            ),
            SeqLookup::Command(Command::Chmod)
        );
    }

    #[test]
    fn lookup_sequence_no_match_for_unbound_prefix() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).unwrap();
        let bogus = KeyChord {
            code: KeyCode::Char('Q'),
            modifiers: KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT,
        };
        assert_eq!(km.lookup_sequence(Mode::Pane, &[bogus]), SeqLookup::NoMatch);
    }

    // Feature 052 T002 (red): M-? resolves to FindFilePopup in Pane mode,
    // and no other binding resolves M-? (no collision).
    #[test]
    fn lookup_alt_question_resolves_to_find_file_popup() {
        let km = Keymap::load(DEFAULT_KEYMAP_TOML).expect("default keymap must parse");
        let ev = KeyEvent::new(KeyCode::Char('?'), KeyModifiers::ALT);
        assert_eq!(
            km.lookup(Mode::Pane, ev),
            Some(Command::FindFilePopup),
            "M-? must resolve to FindFilePopup in Pane mode"
        );
        // Non-collision: M-? must not resolve from any other mode.
        for mode in [Mode::Global, Mode::Dialog, Mode::Search, Mode::Preview] {
            let result = km.lookup(mode, ev);
            assert!(
                result != Some(Command::FindFilePopup) || mode == Mode::Pane,
                "M-? must not resolve to FindFilePopup in mode {mode:?}"
            );
        }
    }

    #[test]
    fn merge_replaces_existing_binding() {
        let mut base = Keymap::load(
            r#"
[[binding]]
mode = "pane"
key = "j"
action = "cursor-down"
"#,
        )
        .unwrap();
        let overlay = Keymap::load(
            r#"
[[binding]]
mode = "pane"
key = "j"
action = "cursor-up"
"#,
        )
        .unwrap();
        base.merge(overlay);
        let ev = KeyEvent::new(KeyCode::Char('j'), KeyModifiers::empty());
        assert_eq!(base.lookup(Mode::Pane, ev), Some(Command::CursorUp));
    }
}

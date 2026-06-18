# Data Model: Internal File Viewer F3

**Feature**: 051-f3-file-viewer | **Date**: 2026-06-19

All entities live in `crates/cargonaut-ui-tui/src/dialog.rs` (UI layer only). None are persisted across sessions.

---

## ViewMode

```
enum ViewMode {
    Text,
    Hex,
}
```

Governs: rendering layout, scrolling granularity (line vs 16-byte row), search encoding (str vs bytes), goto prompt format (line number vs byte offset).

**Transitions**:
- Default: `Text` if file is valid UTF-8 (first 4096 bytes), otherwise `Hex`.
- `Ctrl-x X`: toggle between `Text` and `Hex`. Scroll position resets to top on each toggle. Active search state is cleared.

---

## ViewBuffer

Holds the file content accessible to the viewer. Two variants depending on file size.

```
enum ViewBuffer {
    /// Files ≤ STREAMING_THRESHOLD_BYTES (10 MiB) — fully pre-loaded.
    Loaded {
        lines: Vec<String>,     // For Text mode — ANSI-stripped, one entry per file line
        bytes: Vec<u8>,         // For Hex mode — raw bytes
    },

    /// Files > STREAMING_THRESHOLD_BYTES — streamed on demand.
    Streaming {
        path: PathBuf,                        // Path for re-opening
        /// Compact line index: (line_number, byte_offset) every 1000 lines.
        chunk_index: Vec<(usize, u64)>,
        /// Sliding window of ANSI-stripped lines around the current position.
        lines: VecDeque<String>,
        /// File line number of lines[0].
        window_start_line: usize,
        /// Total line count (approximate for status bar; set from chunk_index + tail scan).
        total_lines: usize,
        /// Total file bytes (set on open).
        total_bytes: u64,
        /// Byte offset of the next line not yet in `lines`.
        reader_offset: u64,
    },
}
```

**Constraints**:
- `lines` in Streaming variant: max 2000 entries (≈400 KB at 200 bytes/line avg).
- ANSI stripping (FR-032): applied to every line before storing in `lines` or `Loaded::lines`.
- `STREAMING_THRESHOLD_BYTES: usize = 10 * 1024 * 1024` — module-level constant.

---

## SearchState

Active search within the viewer. Optional; `None` when no search is in progress.

```
struct SearchState {
    /// The literal search pattern (case-sensitive).
    pattern: String,
    /// Direction of the last search.
    direction: SearchDirection,
    /// Line index (in the current buffer window) of the last found match.
    last_match_line: Option<usize>,
    /// Byte offset within the matched line where the pattern starts.
    last_match_col: Option<usize>,
}

enum SearchDirection {
    Forward,   // `/` — searches downward
    Backward,  // `?` — searches upward
}
```

**Constraints**:
- Pattern is literal string; no regex (FR-019).
- When the viewer switches ViewMode, SearchState is set to `None` (FR-022).
- In hex mode, pattern bytes are searched in the raw byte buffer; matches are reported by row and column offset.
- Partial coverage (FR-033): the status annotation is computed from `buffer_covered_bytes` vs `total_bytes` when `ViewBuffer::Streaming`.

---

## ViewerPrompt

Inline prompt shown at the bottom of the viewer (search input or goto input). `None` when the viewer is in normal navigation mode.

```
enum ViewerPrompt {
    Search {
        /// Text typed so far.
        buffer: String,
        /// Direction for this search.
        direction: SearchDirection,
    },
    Goto {
        /// Text typed so far (decimal line number or 0x-prefixed hex offset).
        buffer: String,
    },
}
```

**Constraints**:
- Only one prompt can be active at a time.
- Esc closes the prompt without acting (FR-021, FR-026).
- Enter submits the prompt; an empty submission clears the active search or is a no-op for goto.

---

## FileViewerDialog

The top-level widget stored in `ActiveDialog::FileViewer { widget: FileViewerDialog }`.

```
struct FileViewerDialog {
    /// Path of the open file (for title bar and streaming re-open).
    path: PathBuf,
    /// Current display mode.
    mode: ViewMode,
    /// Content buffer.
    buffer: ViewBuffer,
    /// Current scroll position — top-of-view line index (text) or row index (hex).
    scroll_offset: usize,
    /// Active search. None if no search has been run.
    search: Option<SearchState>,
    /// Active inline prompt. None in normal navigation mode.
    prompt: Option<ViewerPrompt>,
    /// Word-wrap enabled (text mode only; no effect in hex).
    word_wrap: bool,
    /// Status text shown in the overlay header (e.g. "Line 42/350" or "Pattern not found").
    status: String,
}
```

**Lifecycle**:
1. **Open** (`Command::Preview` or `Command::DescendOrOpen` on a file):
   - Read first 4096 bytes → determine ViewMode.
   - If file ≤ `STREAMING_THRESHOLD_BYTES`: load all lines/bytes in `spawn_blocking`.
   - Else: build chunk index in `spawn_blocking`, load first 2000 lines into VecDeque.
   - Set `active_dialog = Some(ActiveDialog::FileViewer { widget })`.
2. **Navigate** (scroll, search, goto):
   - Handled by `FileViewerDialog::handle_key`; returns `FileViewerAction`.
   - Streaming scroll triggers `spawn_blocking` reads as needed.
3. **Close** (`q`, `Esc`, or `Command::ViewerQuit`):
   - Set `active_dialog = None`.
   - Pane cursor, selection, and filter are unchanged (they are owned by the App/PaneState, not the dialog).

---

## ActiveDialog::FileViewer (extension to existing enum)

```
// In lib.rs, added to the existing ActiveDialog enum:
FileViewer {
    widget: dialog::FileViewerDialog,
},
```

This follows the exact pattern of `ActiveDialog::UserMenu`, `ActiveDialog::TasksPanel`, etc.

---

## FileViewerAction (handle_key return type)

```
enum FileViewerAction {
    /// Close the viewer.
    Close,
    /// Key consumed; viewer stays open. May include a pending I/O request.
    Swallow,
    /// Viewer needs more data loaded (streaming scroll). Contains the byte
    /// offset to read from and how many lines to fetch.
    NeedsData { offset: u64, line_count: usize },
}
```

`NeedsData` is handled in `lib.rs` by spawning a blocking task and updating the buffer before re-rendering.

---

## Constants

```rust
// In dialog.rs (viewer module):
const STREAMING_THRESHOLD_BYTES: usize = 10 * 1024 * 1024; // 10 MiB
const WINDOW_MAX_LINES: usize = 2000;                        // sliding window cap
const CHUNK_INDEX_INTERVAL: usize = 1000;                    // lines between index entries
const HEX_ROW_WIDTH: usize = 16;                             // bytes per hex row
const BINARY_DETECT_BYTES: usize = 4096;                     // bytes sampled for UTF-8 check
```

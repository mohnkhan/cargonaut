# Tasks: Internal File Viewer F3 (Text + Hex + Search)

**Input**: Design documents from `specs/051-f3-file-viewer/`

**Prerequisites**: plan.md ✓, spec.md ✓, research.md ✓, data-model.md ✓, contracts/viewer-keymap.md ✓, quickstart.md ✓

**TDD Note**: Constitution §II mandates test-first. Every FR-### task gets a failing test committed before the green implementation. Commit labels follow the project convention: `T0NN (red): …` → `T0NN (green): …`.

**Organization**: Tasks are grouped by user story (US1–US5) to enable independent implementation and testing.

## Format: `[ID] [P?] [Story] Description`

- **[P]**: Can run in parallel (different files, no in-flight dependencies)
- **[Story]**: Which user story this task belongs to

---

## Phase 1: Setup (Shared Infrastructure)

**Purpose**: Wire the new dependency and define all keymap artifacts before any functional code lands. Zero functional code in this phase — just declarations that make the rest compile.

- [ ] T001 Add `strip-ansi-escapes = "0.2"` to `[dependencies]` in `crates/cargonaut-ui-tui/Cargo.toml`
- [ ] T003 Add four `mode = "preview"` bindings (`g`→`viewer-goto`, `G`→`viewer-end`, `w`→`viewer-wrap`, `q`→`viewer-quit`) to `design/contracts/keymap.toml`; verify `make ci-local` passes the keymap-parse test (`parses_full_default_keymap_without_error`) — **must be committed before T002** (Constitution §III: keymap.toml bindings land before Command enum variants)
- [ ] T002 Add four new `Command` variants (`ViewerGoto`, `ViewerEnd`, `ViewerWrap`, `ViewerQuit`) with `kebab-case` serde names to the `Command` enum in `crates/cargonaut-ui-tui/src/keymap.rs` — **depends on T003 committed**

**Checkpoint**: `cargo check --workspace` compiles clean; default keymap test still passes.

---

## Phase 2: Foundational (Data Structures + Core Scaffolding)

**Purpose**: Define all entities from data-model.md in a single commit so every later phase can build on stable types. No I/O, no rendering — pure structure.

- [ ] T004 Write failing unit tests for `ViewMode`, `SearchState`, `ViewerPrompt`, and `FileViewerAction` construction and transitions in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T004 (red)`)
- [ ] T005 Implement `ViewMode`, `ViewBuffer` (Loaded variant only, Streaming stub), `SearchState`, `ViewerPrompt`, `FileViewerAction`, and `FileViewerDialog` struct with all constants (`STREAMING_THRESHOLD_BYTES`, `WINDOW_MAX_LINES`, `CHUNK_INDEX_INTERVAL`, `HEX_ROW_WIDTH`, `BINARY_DETECT_BYTES`) in `crates/cargonaut-ui-tui/src/dialog.rs` — **add `///` doc comments to every public type, variant, field, and associated function** (Constitution §I: `#![warn(missing_docs)]` is active on this crate; clippy `-D warnings` will fail CI without them) (green commit — `T005 (green)`)
- [ ] T006 Add `ActiveDialog::FileViewer { widget: dialog::FileViewerDialog }` variant to the `ActiveDialog` enum in `crates/cargonaut-ui-tui/src/lib.rs`; add `FileViewerDialog`, `FileViewerAction`, and `ViewMode` to the `pub use dialog::` re-export list (all three types are pattern-matched in `lib.rs` and must be importable at the crate root)

**Checkpoint**: `cargo test --workspace` passes all existing tests; new unit tests from T004 pass.

---

## Phase 3: User Story 1 — Open a file in text mode and scroll (Priority: P1) 🎯 MVP

**Goal**: F3 on a text file opens a full-screen overlay with line numbers, scrollable via arrow keys / Page Up / Page Down / Home, closable with `q` or Esc. No shell-out.

**Independent Test**: Quickstart Scenario 1 — `seq 1 500` file, F3, scroll, `q`; pane cursor unchanged.

### Tests for User Story 1 (constitution §II — TDD required)

- [ ] T007 [US1] Write failing tests for `FileViewerDialog` text-mode construction: `new_text`, `scroll_down`, `scroll_up`, `page_down`, `page_up`, `home_key`, `total_lines`, `status_line` format (`Line N/TOTAL`) in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T007 (red)`)
- [ ] T008 [US1] Write failing render test: `FileViewerDialog::render` into a `TestBackend` shows the title `F3 View — <name>  [text]`, line numbers in left margin, and status bar for a 5-line buffer in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T008 (red)`)

### Implementation for User Story 1

- [ ] T009 [US1] Implement `FileViewerDialog::new_text(path, lines, wrap)` constructor and text-mode accessors (`total_lines`, `scroll_offset`, `current_status_text`) in `crates/cargonaut-ui-tui/src/dialog.rs`; handle the empty-file edge case: when `lines.is_empty()`, set `status = "(empty file)"` and render that message in the content area rather than panicking on index 0 (spec Edge Cases) (green commit — `T009 (green)`)
- [ ] T010 [US1] Implement navigation methods (`scroll_down`, `scroll_up`, `page_down(height)`, `page_up(height)`, `home_key`, `end_key`) for text mode in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T011 [US1] Implement `FileViewerDialog::render(&self, f, area, theme)` for text mode: draw `Block` with title format `F3 View — <filename>  [text]`, line-numbered content via `ratatui::text::Line` spans, and status bar row at bottom in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T011 (green)`)
- [ ] T012 [US1] Implement `FileViewerDialog::handle_key(code) -> FileViewerAction` for normal navigation (Up/Down/PageUp/PageDown/Home/End/Esc/q) in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T013 [US1] Implement `open_file_viewer(path, app)` async helper in `crates/cargonaut-ui-tui/src/lib.rs`: (a) resolve symlinks via `std::fs::canonicalize` before opening, retaining the symlink's display name (not the resolved path) for the title bar (spec Edge Cases); (b) read first `BINARY_DETECT_BYTES` bytes via `spawn_blocking`, determine `ViewMode`; (c) load lines with ANSI stripping via `strip_ansi_escapes::strip_str`; (d) construct `FileViewerDialog`
- [ ] T014 [US1] Wire `Command::Preview` in `dispatch_ui_command` in `crates/cargonaut-ui-tui/src/lib.rs`: **replace** the existing `queue_external(app, ui, status, ExternalTool::Pager)` call entirely — do not add a branch alongside it; the new arm: (a) if `active_dialog.is_some()`, swallow keypress and return early (FR-004); (b) if focused entry is a directory, set status "Not a file" and return; (c) call `open_file_viewer(path)`, set `active_dialog = ActiveDialog::FileViewer { widget }`, and set **`*mode = Mode::Preview`** so keymap lookups in the viewer arm resolve against the preview binding table (C1 fix)
- [ ] T015 [US1] Wire `ActiveDialog::FileViewer` in `handle_key` in `crates/cargonaut-ui-tui/src/lib.rs` with the following architecture (C2 fix — the dialog block returns before chord accumulation at line 836, so keymap lookup must happen *inside* this arm):
  - (a) **Chord accumulation**: push `key` into `chord_buf` (same slice as the normal path uses)
  - (b) **Keymap lookup**: call `keymap.lookup_sequence(Mode::Preview, &chord_buf)` and match:
    - `SeqLookup::Found(Command::ViewerQuit | Command::ToggleHexView | Command::ViewerGoto | …)` → dispatch the resolved Command to the widget; clear `chord_buf`
    - `SeqLookup::Partial` → return `Ok(true)` (chord in progress, keep accumulating)
    - `SeqLookup::NoMatch` → clear `chord_buf`, fall through to raw-key handling below
  - (c) **Raw navigation keys** (Up/Down/PgUp/PgDn/Home/End/Esc/Char) → call `widget.handle_key(key.code)` to get `FileViewerAction`
  - (d) **FileViewerAction dispatch** (exhaustive match — C3 fix: include NeedsData stub now):
    - `FileViewerAction::Close` → set `active_dialog = None`, set **`*mode = Mode::Pane`** (C1 fix), clear `chord_buf`
    - `FileViewerAction::Swallow` → return `Ok(true)`
    - `FileViewerAction::NeedsData { .. }` → `{ /* streaming I/O wired in T042 */ }` (stub keeps CI green between Phase 3 and Phase 7)
- [ ] T016 [US1] Wire `ActiveDialog::FileViewer` in `draw_frame` in `crates/cargonaut-ui-tui/src/lib.rs`: render the widget full-screen (`area`) via `widget.render(f, area, theme)`

**Checkpoint**: F3 on any text file opens the overlay, scrolls, and closes cleanly. `make ci-local` green.

---

## Phase 4: User Story 2 — View binary content in hex mode (Priority: P1)

**Goal**: F3 on a binary file auto-detects non-UTF-8 and opens in hex mode. Classic 16-byte-per-row hex+ASCII layout per the keymap contract. `Ctrl-x X` toggles between modes.

**Independent Test**: Quickstart Scenario 2 — F3 on the `cargonaut` binary; title shows `[hex]`; first row matches ELF magic `7f 45 4c 46`.

### Tests for User Story 2

- [ ] T017 [US2] Write failing tests for binary detection (`is_utf8_file_bytes` returns `false` for `[0x7f, b'E', b'L', b'F', ...]`), hex rendering (`render_hex_row` output matches the format contract: `00000000  7f 45 4c 46 …  |.ELF…|`), and `total_hex_rows` computation in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T017 (red)`)
- [ ] T018 [US2] Write failing test for `ToggleHexView`: construct a text-mode dialog, toggle → hex, toggle again → text; assert `ViewMode` and scroll reset in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T018 (red)`)

### Implementation for User Story 2

- [ ] T019 [US2] Implement `fn is_valid_utf8_sample(bytes: &[u8]) -> bool` and integrate binary detection into `open_file_viewer` in `crates/cargonaut-ui-tui/src/lib.rs` (green commit — `T019 (green)`)
- [ ] T020 [US2] Implement `FileViewerDialog::new_hex(path, bytes)` constructor and `render_hex_row(offset, data: &[u8]) -> String` helper matching the contract (`00000000  HH HH …  |ASCII|`) in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T021 [US2] Implement hex-mode rendering branch in `FileViewerDialog::render`: iterate rows from `scroll_offset`, format each via `render_hex_row`, show `Offset 0x<HEX> / <BYTES>` status per contract in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T021 (green)`)
- [ ] T022 [US2] Implement hex-mode scroll (row-granularity, 16-byte stride) and `total_hex_rows` in `FileViewerDialog` in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T023 [US2] Handle `Command::ToggleHexView` in `handle_key` for `ActiveDialog::FileViewer` in `crates/cargonaut-ui-tui/src/lib.rs`: call `widget.toggle_mode()`, reset scroll and search state (green commit — `T023 (green)`)

**Checkpoint**: F3 on the cargonaut binary shows `[hex]` mode with ELF magic in first row; `Ctrl-x X` toggles mode; `make ci-local` green.

---

## Phase 5: User Story 3 — Incremental search (Priority: P2)

**Goal**: `/` opens a forward search prompt; `?` backward. Enter jumps to the first match with highlighting; `n`/`N` advance. No match → status "Pattern not found". Esc clears prompt and highlights.

**Independent Test**: Quickstart Scenario 3 — open the 500-line file, press `/`, type `Line 42`, Enter; view jumps to line 42 and the text is highlighted.

### Tests for User Story 3

- [ ] T024 [US3] Write failing tests for: `search_forward` returns first matching line index, `search_backward` returns last matching line before cursor, wrap-around returns `None` when no match, status format matches contract (`/error  match 3  Line 128`), and search cleared on mode toggle in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T024 (red)`)
- [ ] T025 [P] [US3] Write failing tests for `ViewerPrompt::Search` state machine: `/` opens prompt, Esc closes without search, Enter with empty text clears highlights, Backspace edits buffer in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T025 (red)`)

### Implementation for User Story 3

- [ ] T026 [US3] Implement `FileViewerDialog::search_forward(pattern: &str) -> Option<(usize, usize)>` and `search_backward(pattern: &str) -> Option<(usize, usize)>` (literal `str::contains`, line + col index) in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T026 (green)`)
- [ ] T027 [US3] Implement `ViewerPrompt::Search` handling in `FileViewerDialog::handle_key`: `/` and `?` set `prompt = Some(ViewerPrompt::Search{...})`, Char accumulates buffer, Backspace pops, Enter runs search and sets `SearchState`, Esc clears in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T028 [US3] Implement match highlighting in `FileViewerDialog::render` (text mode): for every visible line in `scroll_offset..scroll_offset+viewport_height`, scan the line text against `search.pattern` using `str::match_indices(pattern)` and apply `Style::reversed()` spans at each occurrence's byte range — **all visible matches on all visible lines must be highlighted**, not only the cursor-position match in `search.last_match_line` (FR-018 requires ALL visible matches highlighted) in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T028 (green)`)
- [ ] T029 [US3] Implement inline search prompt rendering at the bottom of the overlay in `FileViewerDialog::render`: when `prompt.is_some()`, **replace the status bar row entirely** with the prompt text (`Search: _` or `Go to line: _`) — do not display both prompt and normal status simultaneously; on prompt dismiss (Esc or Enter), restore the normal status bar text (keymap contract: distinct status formats for normal vs. search-active) in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T030 [US3] Wire `Command::PreviewSearchNext` (`n`) and `Command::PreviewSearchPrev` (`N`) in `handle_key` for `ActiveDialog::FileViewer` in `crates/cargonaut-ui-tui/src/lib.rs`: call `widget.advance_search(dir)` to jump to next/prev match (green commit — `T030 (green)`)

**Checkpoint**: Open 500-line file, `/Line 42`+Enter jumps and highlights; `n` cycles; `N` reverses; `?` searches backward; Esc clears. `make ci-local` green.

---

## Phase 6: User Story 4 — Goto a specific position (Priority: P2)

**Goal**: `g` opens a goto prompt; Enter jumps to the line (text) or byte offset (hex); out-of-range input is clamped. `G` jumps to the last line/row.

**Independent Test**: Quickstart Scenario 4 — 500-line file; `g`→`250`→Enter → status `Line 250/500`; `g`→`999`→Enter → clamped to 500; `G` → line 500.

### Tests for User Story 4

- [ ] T031 [US4] Write failing tests for `goto_line(n)`: clamps to [1, last_line], sets scroll_offset to n-1; `goto_offset(offset)` in hex mode clamps to [0, last_row]; `parse_goto_input` handles decimal and `0x`-prefixed hex in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T031 (red)`)
- [ ] T032 [P] [US4] Write failing test for `ViewerPrompt::Goto` state machine: `g` opens prompt, Esc closes without scrolling, Enter with `"250"` calls `goto_line(250)` in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T032 (red)`)

### Implementation for User Story 4

- [ ] T033 [US4] Implement `goto_line(n: usize)`, `goto_offset(offset: u64)`, `goto_end()`, and `fn parse_goto_input(s: &str) -> Option<u64>` (decimal + `0x`-prefix) in `FileViewerDialog` in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T033 (green)`)
- [ ] T034 [US4] Implement `ViewerPrompt::Goto` handling in `FileViewerDialog::handle_key`: `g` sets `prompt = Some(Goto{buffer: "".into()})`, Char/Backspace edit buffer, Enter calls `parse_goto_input`→`goto_line`/`goto_offset`, Esc clears prompt in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T035 [US4] Implement goto prompt rendering (bottom status row) in `FileViewerDialog::render`: `Go to line: _` (text) or `Go to offset: _` (hex) per contract in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T035 (green)`)
- [ ] T036 [US4] Extend the `SeqLookup::Found(cmd)` dispatch arm inside `ActiveDialog::FileViewer` in `handle_key` (established by T015) to handle `Command::ViewerGoto`, `Command::ViewerEnd`, `Command::ViewerWrap`, and `Command::ViewerQuit`: call the appropriate `widget` methods (`widget.open_goto_prompt()`, `widget.goto_end()`, `widget.toggle_wrap()`, `widget.close()`) — these Commands arrive via the keymap lookup path (C2 fix), **not** via raw `key.code`, so do **not** add a separate `widget.handle_key(key.code)` delegation for them in `crates/cargonaut-ui-tui/src/lib.rs`

**Checkpoint**: `g`/`G`/`Home` navigation all work in text + hex mode; clamping verified; `make ci-local` green.

---

## Phase 7: User Story 5 — Streaming, Enter-on-file, Word-wrap (Priority: P2)

**Goal**: Files > 10 MiB stream via chunk index + sliding VecDeque. `Enter` on a file entry opens the viewer (same as F3). `w` toggles word-wrap in text mode. Search on streaming files annotates coverage in the status bar.

**Independent Test**: Quickstart Scenario 5 — 15 MiB file opens quickly; scrolling continues without crash; `g` jumps near end; RSS < 128 MiB. Scenario 6 — Enter on file opens viewer; Enter on directory navigates.

### Tests for User Story 5

- [ ] T037 [US5] Write failing tests for `build_chunk_index`: given a 3000-line in-memory buffer (written to a temp file), the index has 3 entries at lines 0, 1000, 2000; `load_window_from_file` reads from a given entry and returns the correct lines in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T037 (red)`)
- [ ] T038 [P] [US5] Write failing tests for `word_wrap_toggle`: `toggle_wrap()` flips `word_wrap`; status includes `wrap: on` or `wrap: off` in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T038 (red)`)
- [ ] T039 [P] [US5] Write failing test for `partial_search_annotation`: when `buffer_end_line < total_lines`, status shows `(searched X MiB of Y MiB)` per FR-033 contract in `crates/cargonaut-ui-tui/src/dialog.rs` (red commit — `T039 (red)`)

### Implementation for User Story 5

- [ ] T040 [US5] Implement `fn build_chunk_index(path: &Path) -> Result<(Vec<(usize, u64)>, usize, u64)>` (chunk index, total_lines, total_bytes) running in `spawn_blocking` in `crates/cargonaut-ui-tui/src/lib.rs`
- [ ] T041 [US5] Implement `ViewBuffer::Streaming` variant construction and `load_window_from_chunk(path, chunk_entry, lines_needed)` function (seek → read → ANSI-strip → push to VecDeque) in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T041 (green)`)
- [ ] T042 [US5] Implement streaming forward-scroll in `FileViewerDialog::scroll_down` / `page_down`: when approaching window end, return `FileViewerAction::NeedsData{ offset, line_count }`; handle it in `lib.rs` by spawning blocking read and calling `widget.append_lines(lines)` — **also wire the full `NeedsData` arm** here to replace the Phase 3 stub from T015; in `lib.rs`, on `io::Error` from the blocking read (e.g., file deleted mid-view), call `widget.set_status("File no longer readable")` instead of propagating the error (spec Edge Cases: file-disappears scenario) in `crates/cargonaut-ui-tui/src/dialog.rs` and `crates/cargonaut-ui-tui/src/lib.rs`
- [ ] T043 [US5] Implement streaming backward-scroll past window start: find nearest chunk index entry, seek, re-read forward into VecDeque via `FileViewerAction::NeedsData` in `crates/cargonaut-ui-tui/src/dialog.rs`
- [ ] T044 [US5] Implement streaming goto: binary-search `chunk_index` for nearest entry ≤ target line, emit `FileViewerAction::NeedsData` for the window load in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T044 (green)`)
- [ ] T045 [US5] Add streaming annotation to `search_forward`/`search_backward` status: when `ViewBuffer::Streaming` and `buffer_end_line < total_lines`, append `(searched X MiB of Y MiB)` to the match status (green commit — `T045 (green)`)
- [ ] T046 [US5] Intercept `Command::DescendOrOpen` in `dispatch_ui_command` in `crates/cargonaut-ui-tui/src/lib.rs`: check focused entry `VfsKind`; if `File` or `Symlink` → call `open_file_viewer`; if `Dir` or `..` → pass through to `AppCommand::Descend` (green commit — `T046 (green)`)
- [ ] T047 [US5] Implement `word_wrap` toggle in `FileViewerDialog::render` (text mode): when `word_wrap` is true, use `ratatui::widgets::Wrap { trim: false }`; status bar reflects `wrap: on/off` per contract in `crates/cargonaut-ui-tui/src/dialog.rs` (green commit — `T047 (green)`)

**Checkpoint**: 15 MiB file opens < 150 ms, scrolls without crash, RSS < 128 MiB; Enter-on-file works; word-wrap toggles; `make ci-local` green.

---

## Phase 8: Polish & Cross-Cutting Concerns

**Purpose**: Performance gates, help-overlay update, documentation.

- [ ] T048 [P] Add `benches/viewer_open.rs` criterion bench: write a 1 MiB temp file, time `open_file_viewer` async via `tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap().block_on(open_file_viewer(...))` (criterion does not support async natively — wrap explicitly; do not rely on `#[tokio::test]` or `tokio_test`); assert p50 ≤ 150 ms (SC-001) in `crates/cargonaut-ui-tui/benches/viewer_open.rs`; add the `[[bench]]` entry to `crates/cargonaut-ui-tui/Cargo.toml`
- [ ] T049 [P] Extend `benches/keypress_latency.rs` with a viewer scenario: open viewer dialog, send a `Down` key event, time render — assert ≤ 16 ms (SC-002) in `crates/cargonaut-ui-tui/benches/keypress_latency.rs`
- [ ] T050 [P] Extend `benches/rss_headroom.rs` with a large-file streaming viewer scenario: create a **1 GiB sparse file** via `File::seek(SeekFrom::End(1_073_741_823))` + `file.write_all(&[0u8])` (zero disk writes, instant creation, exercises the streaming path at the SC-003 mandated size); open the viewer on it, scroll through ≥ 5 page-downs, assert RSS ≤ 64 MiB (SC-003 requires "1 GiB file open" — not just > 10 MiB) in `crates/cargonaut-ui-tui/benches/rss_headroom.rs`
- [ ] T051 Update `HELP_SECTIONS` in `crates/cargonaut-ui-tui/src/dialog.rs`: **rename the existing "Preview" section to "File Viewer"** (the prior section had only 5 rows; adding g/G/w/q/Up/Down/PgUp/PgDn/Home/End doubles the content and warrants a more descriptive label); add rows for all viewer keys: `g` (goto line/offset), `G` (jump to end), `w` (toggle word-wrap), `q` / `Esc` (close), Up/Down (scroll 1 line), PgUp/PgDn (scroll page), Home/End (first/last line), `Ctrl-x X` (toggle hex mode), `/` (search forward), `?` (search backward), `n`/`N` (next/prev match)
- [X] T052 Update `README.md`: increment test count in "At a Glance" metrics table (≥479 tests); add one-line entry to "Feature History" for feature 051
- [X] T053 Update `Learnings.md`: append a "Feature 051 — F3 Built-in File Viewer" section with ≥ 3 bullet points covering what was hard, root causes, and non-obvious decisions

**Checkpoint**: `make ci-local` fully green; all 3 bench targets run; binary size still ≤ 8 MiB; test count ≥ 479.

---

## Dependencies & Execution Order

### Phase Dependencies

- **Phase 1 (Setup)**: No dependencies — start immediately.
- **Phase 2 (Foundational)**: Depends on Phase 1 (T001 dep for Cargo.toml; T002 dep for Command enum).
- **Phase 3 (US1)**: Depends on Phase 2 — all data structures and ActiveDialog variant must exist.
- **Phase 4 (US2)**: Depends on Phase 3 (renderer infrastructure); can start as soon as T011/T016 green.
- **Phase 5 (US3)**: Depends on Phase 3 (text mode stable); can run after US1 checkpoint.
- **Phase 6 (US4)**: Depends on Phase 3; can run in parallel with Phase 5.
- **Phase 7 (US5)**: Depends on Phase 3 (pre-load path); T046 depends on Phase 3 dispatch wiring.
- **Phase 8 (Polish)**: Depends on all US phases complete.

### User Story Dependencies

- **US1 (P1)**: Foundation of all later phases — must ship first.
- **US2 (P1)**: Can proceed immediately after US1 checkpoint (shares render infrastructure).
- **US3 (P2)**: Can proceed after US1 checkpoint in parallel with US2.
- **US4 (P2)**: Can proceed after US1 checkpoint in parallel with US2 and US3.
- **US5 (P2)**: Streaming depends on US1 pre-load path; Enter-interception is pure UI-layer.

### Within Each Phase

- Write red tests → verify they fail → implement → verify green → commit.
- Models/structs before methods; methods before render; render before wiring in lib.rs.

### Parallel Opportunities

- T003 (keymap.toml) must be committed **before** T002 (keymap.rs) — Constitution §III.
- T008 (render test) runs in parallel with T007 (logic tests) — different test functions, same file, no conflict.
- T025 / T032 / T038 / T039 (prompt state-machine tests) can run in parallel with primary story tests.
- T048 / T049 / T050 (bench tasks) can all run in parallel.
- T051 (help sections) runs in parallel with bench tasks.

---

## Parallel Example: User Story 1

```bash
# Run both test groups concurrently (red phase):
Task T007: logic tests (scroll, status, total_lines)
Task T008: render test (TestBackend snapshot)

# Then implement both before wiring:
Task T009: constructor + accessors
Task T010: navigation methods
Task T011: render()

# Wire sequentially (order matters):
Task T012: lib.rs Preview dispatch
Task T013: lib.rs ActiveDialog::FileViewer key handler
Task T014: lib.rs draw_frame render call
```

---

## Implementation Strategy

### MVP First (User Stories 1 + 2 only)

1. Complete Phase 1: Setup (T001–T003)
2. Complete Phase 2: Foundational (T004–T006)
3. Complete Phase 3: US1 — text mode + scroll (T007–T016)
4. Complete Phase 4: US2 — hex mode (T017–T023)
5. **STOP and validate**: Quickstart Scenarios 1 + 2 pass. `make ci-local` green.
6. Merge if US3/US4/US5 can be deferred.

### Incremental Delivery

- US1 + US2 → MVP: viewer opens for any file (text or binary)
- + US3 → searchable viewer (most useful for log files)
- + US4 → goto (productivity for large files)
- + US5 → streaming + Enter shortcut + word-wrap (full polish)
- + Phase 8 → CI gates + docs (required before PR merge per CLAUDE.md)

---

## Notes

- `[P]` tasks touch different functions or different files — no merge conflicts.
- Each user story's checkpoint represents a deliverable that can be demoed or merged incrementally.
- Constitution §II: never skip the red commit. The failing test is the spec.
- Constitution §V: always run `make test` not bare `cargo test` to preserve tmpfs guard.
- SC-005: ≥30 new tests required. This task list targets ~40 new test functions across Phases 2–8.

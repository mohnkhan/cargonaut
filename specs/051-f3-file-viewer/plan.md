# Implementation Plan: Internal File Viewer F3 (Text + Hex + Search)

**Branch**: `051-f3-file-viewer` | **Date**: 2026-06-19 | **Spec**: [spec.md](spec.md)

**Input**: Feature specification from `specs/051-f3-file-viewer/spec.md`

## Summary

Replace the F3 external-pager shell-out (`$PAGER`) with a built-in TUI file viewer that runs entirely in-process. The viewer lives in `cargonaut-ui-tui` as a new `ActiveDialog::FileViewer` variant and a `FileViewerDialog` widget. It supports text mode (UTF-8 content with line numbers, ANSI stripping, word-wrap), hex mode (16-bytes-per-row hex+ASCII), incremental literal-string search with match highlighting, and goto line/offset prompts. Files ≤ 10 MiB are pre-loaded; larger files stream on demand with a bounded memory window. No changes to `cargonaut-core`, `cargonaut-config`, or `cargonaut-vfs`.

## Technical Context

**Language/Version**: Rust 1.76 (workspace MSR, see `Cargo.toml`)

**Primary Dependencies**:
- `ratatui = "0.27"` — TUI rendering (existing)
- `crossterm = "0.28"` — raw mode + event stream (existing)
- `tokio = "1.40"` — async runtime; `spawn_blocking` for file I/O (existing)
- `strip-ansi-escapes = "0.2"` — strip ANSI/CSI/OSC escape sequences from bytes **[NEW]**

**Storage**: Local filesystem only; `std::fs::File` + `tokio::task::spawn_blocking` for reads

**Testing**: `cargo test --workspace` (existing harness); `criterion` for perf benchmarks

**Target Platform**: Linux desktop (TUI in alternate screen)

**Performance Goals**:
- SC-001: ≤150 ms open for ≤1 MiB file
- SC-002: ≤16 ms keypress→repaint (existing NFR-002)
- SC-003: ≤64 MiB RSS with 1 GiB file open
- SC-004: ≤8 MiB stripped binary

**Constraints**:
- All new keymap bindings MUST land in `design/contracts/keymap.toml` under `mode = "preview"` first (constitution §III)
- No `unsafe` in any cargonaut crate without `// SAFETY:` comment + test
- No regex: search is literal string only (spec FR-019)
- No shell-out (spec FR-001)
- Streaming threshold: `STREAMING_THRESHOLD_BYTES: usize = 10 * 1024 * 1024` constant in the viewer module

**Scale/Scope**: Single-user TUI; files up to several GiB supported via streaming.

## Constitution Check

*GATE: Must pass before Phase 0 research. Re-check after Phase 1 design.*

| Principle | Status | Notes |
|-----------|--------|-------|
| I — Code quality (clippy -D warnings, docs, no unsafe) | ✅ PASS | `strip-ansi-escapes` has no unsafe surface we expose; all new pub items get doc comments |
| II — Test-first TDD (red → green commits) | ✅ PASS | Tasks are structured red-first; enforced in task commit conventions |
| III — UX consistency (dialog! macro, keymap.toml) | ✅ PASS | FileViewerDialog uses same `Block/Clear/Paragraph` pattern; new bindings in keymap.toml first |
| IV — Performance (SC-001..004) | ✅ PASS | SC-001 gated by viewer-open bench; SC-002 by existing keypress_latency harness; SC-003 by rss_headroom harness; SC-004 by check-binary-size.sh |
| V — SSD preservation (tmpfs) | ✅ PASS | CI exempt; dev uses `make build`/`make test` which enforce tmpfs guard |

**Post-design re-check**: All gates still pass. No violations introduced by the design.

## Project Structure

### Documentation (this feature)

```text
specs/051-f3-file-viewer/
├── plan.md              # This file
├── research.md          # Phase 0 output
├── data-model.md        # Phase 1 output
├── quickstart.md        # Phase 1 output
├── contracts/
│   └── viewer-keymap.md # Phase 1 output — preview-mode binding contracts
└── tasks.md             # Phase 2 output (/speckit-tasks command)
```

### Source Code (modified files only)

```text
crates/cargonaut-ui-tui/
├── Cargo.toml                     # +strip-ansi-escapes dep
└── src/
    ├── dialog.rs                  # +FileViewerDialog, FileViewerAction, ViewMode, SearchState, ViewerPrompt
    ├── keymap.rs                  # +Command::ViewerGoto, ViewerEnd, ViewerWrap, ViewerQuit
    └── lib.rs                     # +ActiveDialog::FileViewer; update Command::Preview + Command::DescendOrOpen dispatch

design/contracts/
└── keymap.toml                    # +g, G, w, q in mode = "preview"

crates/cargonaut-ui-tui/benches/
└── viewer_open.rs                 # +SC-001 gate (new bench)
```

No changes to `cargonaut-core`, `cargonaut-config`, `cargonaut-vfs`, or `cargonaut-bin`.

## Complexity Tracking

No constitution violations requiring justification.

---

## Phase 0: Research

See [research.md](research.md) — all NEEDS CLARIFICATION items resolved.

Key decisions:
- **ANSI stripping**: `strip-ansi-escapes = "0.2"` crate (see research.md R-001)
- **Streaming strategy**: pre-load ≤10 MiB; for larger files, use a byte-offset chunk index + sliding VecDeque window (see research.md R-002)
- **Search**: `str::contains()` over the loaded buffer; annotate partial coverage when streaming (R-003)
- **Enter-on-file interception**: UI layer only — check focused entry kind in `Command::DescendOrOpen` handler before dispatching to core (R-004)
- **Hex rendering**: fixed 16-byte rows; synchronous `File::seek` + `read_exact` per page, no pre-load (R-005)

---

## Phase 1: Design

See [data-model.md](data-model.md) for entity definitions and [contracts/viewer-keymap.md](contracts/viewer-keymap.md) for binding contracts.

### Key Design Decisions

#### 1. `FileViewerDialog` lives in `dialog.rs` — not a new file

Pattern established by all prior dialogs (`HelpOverlay`, `UserMenuDialog`, etc.). The viewer is a new struct with its own `handle_key` / `render` methods. Dialog is stored as `ActiveDialog::FileViewer { widget }` — the same mutual-exclusion guard (`active_dialog.is_some()`) that protects other dialogs from stacking already covers FR-004 (F3 swallowed when dialog open).

#### 2. `HelpOverlay` stays separate; `FileViewerDialog` goes in `ActiveDialog`

`HelpOverlay` has a long-standing carve-out in `UiState` (pre-dates the `ActiveDialog` enum). The file viewer uses `ActiveDialog` to benefit from the stacking guard. Both can coexist correctly: if `help_overlay` is Some, the code returns before reaching the `active_dialog` match, so the viewer is never input-active while the help is shown.

#### 3. File I/O via `tokio::task::spawn_blocking`

All blocking file reads happen in `spawn_blocking` tasks so the async event loop is not starved. On open: `spawn_blocking(|| load_or_index_file(path))`. On streaming scroll: `spawn_blocking(|| read_lines(path, offset, n))`. Since `handle_key` is already `async`, `.await` on spawn_blocking tasks is native.

#### 4. Streaming buffer: chunk index + sliding VecDeque

For files > 10 MiB in text mode:
- On open: scan the file in 64 KiB chunks, recording byte offset at every 1000th line → `chunk_index: Vec<(line_no: usize, byte_offset: u64)>`. For a 500 MiB file with 80-byte avg lines: 6.25M lines → ~6250 index entries → 100 KB. Within budget.
- `lines: VecDeque<String>` holds the current ≤2000-line window (≈400 KB at 200 bytes/line avg).
- `window_start_line: usize` tracks which file line is at index 0 of `lines`.
- Scroll forward: extend `lines` from the file reader; drop old entries from the front.
- Scroll backward within window: O(1). Scroll backward past window start: seek to nearest chunk-index entry, re-read forward.
- Goto: use chunk index to find the nearest entry before the target, seek, read forward.

#### 5. `Command::DescendOrOpen` — UI-layer file detection

In `dispatch_ui_command`, when `Command::DescendOrOpen` fires:
1. Peek at the focused entry's `VfsKind` from `app.active_pane_state()`.
2. If `VfsKind::File` (or symlink to file): open viewer instead.
3. If `VfsKind::Dir` (or `..` parent row): pass through to `AppCommand::Descend`.

No core changes needed. The existing comment in `descend_into_focused` ("T1.21 will open via $EDITOR / openers") is updated in the implementation.

#### 6. ANSI stripping applied at load time

When loading a file into text mode (either pre-load or streaming chunks), run each line through `strip_ansi_escapes::strip_str(&line)` before storing it in the buffer. Stored lines are always plain text, so render and search both work on clean content.

#### 7. Search: linear scan over the loaded window

For Loaded files: scan `lines[current_line..]` (or reverse for `?`). All lines already in memory.
For Streaming: scan the loaded window only; annotate with `(searched N of M bytes)`.
Match highlighting: record per-line match positions; apply `Style::reversed()` spans in `render`.

#### 8. New `Command` variants for viewer-only keys

The `Command` enum already has `ToggleHexView`, `PreviewSearchForward`, `PreviewSearchBackward`, `PreviewSearchNext`, `PreviewSearchPrev`. Add four more:
- `ViewerGoto` — opens goto prompt
- `ViewerEnd` — jump to last line/offset
- `ViewerWrap` — toggle word-wrap
- `ViewerQuit` — close viewer (equivalent to Esc)

These are `mode = "preview"` only — they are never dispatched from pane or dialog mode.

### User Story → Implementation Mapping

| User Story | FR(s) | Key Files | Effort |
|---|---|---|---|
| US1 — text mode + scroll | FR-001,6,7,8,9 | dialog.rs, lib.rs, keymap.toml | M |
| US2 — hex mode | FR-011,12,13,14 | dialog.rs | S |
| US3 — search | FR-015..022,033 | dialog.rs, lib.rs | M |
| US4 — goto | FR-023..026 | dialog.rs | S |
| US5 — streaming + Enter + wrap | FR-002,027,028,029,010 | dialog.rs, lib.rs | M |
| Infra — ANSI strip, keymap, bench | FR-030,31,32 + SC-001..005 | keymap.rs, keymap.toml, benches/ | S |

### Success Criterion Gates

| SC | Gate mechanism | When |
|----|---------------|------|
| SC-001 (150ms open) | `benches/viewer_open.rs` criterion bench | CI |
| SC-002 (16ms keypress) | existing `benches/keypress_latency.rs` — extend to cover viewer | CI |
| SC-003 (64 MiB RSS) | existing `benches/rss_headroom.rs` — add large-file viewer scenario | CI |
| SC-004 (≤8 MiB binary) | existing `scripts/check-binary-size.sh` | CI |
| SC-005 (≥30 new tests) | CI test count delta | PR review |

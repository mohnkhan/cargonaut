# Research: Internal File Viewer F3

**Feature**: 051-f3-file-viewer | **Date**: 2026-06-19

---

## R-001 — ANSI escape sequence stripping

**Decision**: Use `strip-ansi-escapes = "0.2.1"` crate.

**API**:
```
strip_ansi_escapes::strip(bytes: &[u8]) -> Vec<u8>   // returns raw bytes
strip_ansi_escapes::strip_str(s: impl AsRef<[u8]>) -> String  // returns String
```

Apply `strip_str` to each line after reading from file and before storing it in the `ViewBuffer`. Stripping happens at load time (not at render time) so search and render both operate on clean plain text. Since `strip_str` takes any bytes-compatible input, passing `&line_str` works directly.

**Rationale**: Single-function call, no state to manage, well-tested, covers all CSI/OSC/ESC sequences. The dependency footprint is minimal (`vte ^0.14` as the sole transitive dep).

**Alternatives considered**:
- `anstyle-parse` — lower-level, state-machine only, requires more boilerplate. Better if custom handling is needed; not needed here.
- Manual state machine — duplicates `vte`'s parser; not worth the maintenance burden.

---

## R-002 — Streaming strategy for large files (text mode)

**Decision**: Pre-load ≤10 MiB; for larger files, build a compact chunk index and use a sliding `VecDeque<String>` window.

**Chunk index**: On open for large files, scan the entire file in 64 KiB chunks, recording `(line_number, byte_offset)` at every 1000th line. Memory cost: `~8 bytes × (total_lines / 1000)`. For a 500 MiB file with 80-byte avg lines (~6.25M lines): 6250 index entries × 8 bytes ≈ 50 KB. Well within budget.

**Sliding window**: `lines: VecDeque<String>` holds the current ≤2000-line window (≈400 KB at 200 bytes/line avg). `window_start_line: usize` tracks the file line index at VecDeque position 0.

**Scroll mechanics**:
- Forward: if `cursor + viewport_height > lines.len()`, read the next batch from the file reader and push to the back; pop from the front if window exceeds max.
- Backward within window: just decrement `scroll_offset` — O(1).
- Backward past window start: use chunk index to find the nearest entry before `window_start_line`, seek file to that offset, re-read forward to `window_start_line`.
- Goto: use chunk index to find nearest entry ≤ target line, seek, read forward.

**All file I/O in `spawn_blocking`**: on open (chunk index build) and on each scroll that triggers a new page read. Since `handle_key` in `lib.rs` is `async`, `.await`ing spawn_blocking is natural and does not block the executor.

**Rationale**: Bounded memory, fast forward scrolling, acceptable backward performance. A full mmap approach would be cleaner but adds a new dependency (`memmap2`) and has edge cases (file truncated while mapped, non-seekable files).

**Alternatives considered**:
- Full pre-load always — fails SC-003 for large files.
- mmap via `memmap2` — elegant but new dep + edge cases; over-engineered for MVP.
- Line-only index (all newline offsets) — O(total_lines × 8 bytes) memory; 50 MB for 6.25M lines, exceeds budget.

---

## R-003 — Search implementation

**Decision**: Linear scan over the `ViewBuffer` window using `str::contains(pattern)` (literal match); `memchr::memmem` for byte-level hex search.

**Text mode**: For each forward search, iterate `lines[current_line..]`; for backward, iterate `lines[..current_line]` in reverse. Return the first matching line index.

**Hex mode**: Convert the loaded byte buffer to a flat `Vec<u8>` slice; use `memchr::memmem::find_iter` for efficient multi-byte pattern search (the `memchr` crate is already a transitive dep of `crossterm`).

**Partial coverage indicator (FR-033)**: Track `buffer_end_line` and `total_lines_hint`. When `buffer_end_line < total_lines_hint`, annotate search results: `"1 match (searched 10 MiB of 512 MiB)"`.

**Rationale**: `str::contains` is O(m×n) but sufficient for typical file sizes in the 10 MiB window. No regex needed (spec FR-019). `memchr::memmem` is O(m+n) for hex; already available transitively.

**Alternatives considered**:
- Regex via `regex` crate — adds significant binary size (~1 MiB); out of spec scope.
- Boyer-Moore — manual implementation; `memchr` is equivalent and already available.

---

## R-004 — Enter-on-file interception

**Decision**: Intercept in the UI layer only — no core changes.

In `dispatch_ui_command` in `lib.rs`, when `Command::DescendOrOpen` fires:
1. Read the focused entry kind: `app.active_pane_state().listing.entries[idx].meta.kind`.
2. If `VfsKind::File` (or the `..` parent row is not focused): open `ActiveDialog::FileViewer`.
3. If `VfsKind::Dir` (or parent row): pass through to `app.dispatch(AppCommand::Descend)`.

**Rationale**: Keeps the core unchanged, avoids introducing a new `AppCommand` variant or `Event` type. The existing comment in `descend_into_focused` notes "T1.21 will open via $EDITOR / openers" — this is the implementation of that intention, but in the UI layer.

**Alternatives considered**:
- New `AppEvent::OpenViewer(path)` emitted from core — cleaner separation of concerns but adds a new core API surface just for a UI concern.
- `AppCommand::OpenViewer` — same argument; over-engineered for a one-way UI decision.

---

## R-005 — Hex mode I/O

**Decision**: Synchronous `File::seek + read_exact` wrapped in `spawn_blocking`, no pre-load. 16 bytes per row × 24 rows = 384 bytes per frame.

**Hex mode always streams**: `File::seek(SeekFrom::Start(byte_offset))` then `read_exact(&mut buf[..16 * rows])`. No line index needed. Random access is O(1) via seek.

**Goto in hex**: directly compute `byte_offset = row_number × 16`; no index needed.

**Total lines for status bar**: `total_bytes / 16` (rounded up).

**Rationale**: Hex access is naturally random-access by byte offset. Seeking is O(1) on regular files. No index or window needed.

---

## R-006 — Binary detection

**Decision**: Sample the first 4096 bytes; if `std::str::from_utf8` fails, open in hex mode.

Implementation: read the first 4096 bytes in `spawn_blocking` during `open_viewer`. If they parse as valid UTF-8, text mode. Otherwise hex mode. This matches spec FR-006/011 and the Edge Cases section.

**Rationale**: 4096 bytes is large enough to detect non-UTF-8 headers in binaries (ELF magic, etc.) without reading the whole file. `std::str::from_utf8` is zero-copy on the sample slice.

---

## R-007 — `strip-ansi-escapes` vs binary size

The `strip-ansi-escapes` 0.2.1 + `vte` transitive dep adds approximately 15–30 KB to the release binary (estimated from typical Rust zero-cost abstraction + LTO). The current binary is ~2.72 MiB; the NFR-001 budget is 8 MiB. Ample headroom. No concern.

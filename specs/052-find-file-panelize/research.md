# Research: Find-File and Panelize

**Feature**: 052-find-file-panelize | **Date**: 2026-06-19

---

## R-001: Glob matching library

- **Decision**: Use `globset = { workspace = true }` (existing workspace dep at `"0.4"`). Consumed via `GlobBuilder::new(pattern)?.build()?.compile_matcher()`.
- **Rationale**: Already in the workspace (used in `cargonaut-core` for tag-by-wildcard). No new crate, no lock-file change, Constitution §I satisfied.
- **Alternatives considered**: `glob` crate (unmaintained), `fnmatch` (C FFI). Both rejected; `globset` is already present and well-maintained.
- **Note**: `globset` must be added to `cargonaut-ui-tui/Cargo.toml` as `globset = { workspace = true }` — it is currently only listed under `cargonaut-core`.

---

## R-002: Async directory walk for name-search mode

- **Decision**: `tokio::task::spawn_blocking` + synchronous BFS using `std::fs::read_dir` and `std::collections::VecDeque`. Results streamed via `tokio::sync::mpsc::unbounded_channel()`.
- **Rationale**: Mirrors the existing `collect_subtree_capped` pattern in `lib.rs` (lines 1546–1570). No new dependency. BFS is cancellable by dropping the sender end. The walk checks an `AtomicBool` abort flag each directory so `Esc` cancels within one directory-read latency (~1–10 ms for large dirs).
- **Alternatives considered**: `walkdir` crate (not in workspace, no compelling advantage for this use case); `tokio::fs::read_dir` with async recursion (complex stack, harder to cancel). Both rejected.
- **Abort mechanism**: The walk task holds an `Arc<AtomicBool>` (abort flag). `FindFileDialog::cancel()` sets the flag to `true`; the walk loop checks it at each entry and on each `read_dir` call.

---

## R-003: Content search (ripgrep) invocation

- **Decision**: `tokio::process::Command::new(rg_path).args([pattern, "--files-with-matches", "--no-messages", root_path_str]).stdout(Stdio::piped()).spawn()`. Read stdout line-by-line via `tokio::io::AsyncBufReadExt`. Send `FindEvent::Found(path)` per line, then `FindEvent::Done { truncated }` at end-of-stream.
- **Rationale**: `tokio::process::Command` (not `std::process::Command`) is mandated because it supports `kill_on_drop` for async-native cancellation — `child.kill().await` cleanly terminates the subprocess without a blocking thread. ripgrep handles binary-file filtering, gitignore-aware walks, and Unicode automatically. `--files-with-matches` gives file-level output (spec requirement). `--no-messages` suppresses binary-file notices. **`rg --files-with-matches` deduplicates by design** (one path per matched file); no additional dedup step is needed.
- **Abort mechanism**: Hold `tokio::process::Child`; `cancel()` calls `child.kill().await` and drops the handle. Non-zero rg exit (binary files, permission errors) is treated as end-of-stream: send `Done { truncated: false }` with accumulated results (never panics).
- **Ripgrep availability check**: At dialog open time, check `std::process::Command::new(rg_path).arg("--version").status()` succeeds. Cache as `FindFileDialog::content_available: bool`. If false, Tab to Content mode is a no-op.
- **Note (H3 resolution)**: An earlier draft of this document referenced `std::process::Command + spawn_blocking`. The authoritative implementation instruction is `tokio::process::Command` (tasks.md T013) — this entry has been updated to match.
- **Ripgrep availability check**: At dialog open time, check `std::process::Command::new(rg_path).arg("--version").status()` succeeds. Cache the result in `FindFileDialog::content_available: bool`. If false, Tab to Content mode is a no-op.

---

## R-004: Streaming results to the event loop

- **Decision**: `tokio::sync::mpsc::unbounded_channel::<FindEvent>()` where `FindEvent = Found(PathBuf) | Done { truncated: bool }`. The walk task holds the `Sender`; `FindFileDialog` holds the `Receiver` as `Option<mpsc::UnboundedReceiver<FindEvent>>`. Each 100ms tick, `run_loop` drains the receiver by calling `FindFileDialog::poll_results()` which calls `try_recv()` in a loop.
- **Rationale**: Fits the existing 100ms `tokio::time::interval` tick already used in `run_loop`. No new runtime primitives. Draining is O(new_results) per tick — cheap.
- **Alternatives considered**: `tokio::sync::watch` (no accumulation); `tokio::sync::broadcast` (unnecessary multi-consumer). Both rejected.

---

## R-005: Panelize — synthetic DirListing

- **Decision**: Construct a `DirListing { entries: Vec<DirEntry>, sort: Sort::None }` from the found `Vec<PathBuf>` by calling `std::fs::metadata(path)` for each and building `DirEntry { name, kind, metadata }`. Call `active_pane.set_listing(synthetic_listing)`. Store the label as `UiState.find_label: Option<String>` (the search pattern). Status bar draw reads `find_label` to render `[Find: <pattern>]`.
- **Rationale**: `PaneView::set_listing` is the existing API for replacing panel contents (used in every directory navigation). Using real `DirEntry` means tagging, copy, delete all work via existing code paths — no special-casing needed.
- **Returning from panelize**: When the user navigates (Enter on a directory, `..`, `Backspace`), the pane loads a real directory and `find_label` is cleared. This is automatic — `navigate_to` already sets a fresh listing.

---

## R-006: Key binding `Alt-?`

- **Decision**: `mode = "pane"`, `key = "M-?"`, `action = "find-file-popup"`. Verified unbound by grepping `keymap.toml`. `M-?` is `Alt+Shift+/` on most keyboard layouts (the `?` key).
- **Rationale**: Mnemonic ("what files?"), matches issue #41 spec input. No collision with existing bindings.
- **Note**: `M-?` (Alt-?) requires confirming the crossterm key-event representation. In crossterm, `Alt+?` is `KeyEvent { code: KeyCode::Char('?'), modifiers: Alt }`. The keymap parser already handles `M-<char>` for single printable chars (used for M-m, M-c, M-!, etc.).

---

## R-007: Dialog state machine phases

```
InputFocused → (Enter) → Walking → (Done) → ResultsFocused
                                          → (NoResults) → InputFocused + notice
                  (Esc) ↓           (Esc) ↓       ↓ (Esc)
                  Closed           Closed        Closed
ResultsFocused → (Enter, count≥1) → Panelize → Closed
```

- **InputFocused**: user is typing; cursor in input field.
- **Walking**: walk in progress; results accumulate; input frozen; walking indicator shown.
- **ResultsFocused**: walk done; cursor in result list; scroll navigation active.
- **NoResults notice**: stays on InputFocused with a message; user edits and presses Enter again.

---

## R-008: Constitution compliance

- **§I (Code Quality)**: No `unsafe`. All new public items (`FindFileDialog`, `SearchMode`, `FindEvent`, `Command::FindFilePopup`, `UiState.find_label`) carry doc comments. `clippy -D warnings` satisfied.
- **§II (Test-First)**: Pure decision functions (`content_available`, result-count guard) unit-testable. Dialog phase transitions testable via `TestBackend`. Walk tested with a temp-dir fixture. ripgrep check mocked via a fake binary path. All FRs map to red→green commits.
- **§III (UX Consistency)**: `FindFileDialog` in `dialog.rs`. Binding in `design/contracts/keymap.toml`. Theme-colored rendering (no hardcoded ANSI).
- **§IV (Performance)**: Walk is off-thread. Result drain is O(n) per 100ms tick — negligible. Frame budget unaffected.
- **§V (SSD)**: Build via `make`; no `cargo clean`.

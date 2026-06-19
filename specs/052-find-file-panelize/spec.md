# Feature Specification: Find-File and Panelize

**Feature Branch**: `052-find-file-panelize`

**Created**: 2026-06-19

**Status**: Draft

**Input**: User description: "Find-file popup (Alt-?): async directory walk with filename-glob and file-content (ripgrep) search modes, results shown in a scrollable overlay dialog with match count and path list, pressing Enter panelizes the result set into the active panel as a synthetic flat listing enabling bulk operations (tag, copy, delete) on the found files. Closes GitHub issue #41. config.search.ripgrep_path already exists in the config crate for ripgrep path configuration."

## Clarifications

### Session 2026-06-19

- Q: Which key opens the find-file dialog, and what are the in-dialog navigation keys? → A: `Alt-?` opens the dialog (mnemonic: "what files?", currently unbound); `Tab` switches between Name and Content search modes; `Enter` panelizes results into the active panel; `Esc` cancels without panelizing at any dialog phase (see FR-011); arrow keys / `PgUp` / `PgDn` scroll the result list; all consistent with the existing overlay conventions.
- Q: When content search (ripgrep) is unavailable (rg not on PATH and ripgrep_path not configured), what happens? → A: Content-search mode is visibly disabled — the Tab toggle greys it out and shows a one-line notice ("Content search unavailable: rg not found"); name-search still works. No crash.
- Q: What is the scope (root) of the search? → A: The active panel's current directory is the search root; the user cannot change the root in the dialog (a separate future feature). Subdirectories are always traversed recursively.
- Q: What triggers the search walk — pressing Enter or live-as-you-type? → A: Enter in the input field triggers the walk; results populate the list incrementally; a second Enter (while the result list is focused) panelizes the results. No debounce/live search.
- Q: What happens when Enter is pressed with 0 results — block or panelize empty? → A: Block — the dialog stays open and shows "No files found matching `<pattern>`"; the user can edit the pattern and retry. No empty synthetic listing is created.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Search by filename glob (Priority: P1)

A user is inside a deep directory tree and wants to locate all Rust source files matching `*.rs` without manually navigating subdirectories. They press `Alt-?`, type `*.rs` in the Name field, and press `Enter`. The dialog shows a scrollable list of matching file paths with a count header. They press `Enter` again to panelize the results — the active panel now shows a flat synthetic listing of only those files, ready for tagging and bulk copy.

**Why this priority**: Filename search is the most universal use case, requires no external tool (native directory walk), and delivers a complete, independently testable slice.

**Independent Test**: Open the dialog, enter a glob pattern, see results populate incrementally with a count, press `Enter` to panelize, confirm the panel lists exactly those files and normal panel operations (tag, copy, delete) work on them.

**Acceptance Scenarios**:

1. **Given** the active panel is on a directory, **When** the user presses `Alt-?`, **Then** a find-file overlay opens with a Name input field focused, a result list (empty), and a match count of 0.
2. **Given** the find-file dialog is open with a name pattern typed and the input field focused, **When** the user presses `Enter`, **Then** the walk starts and results populate the list incrementally (see FR-005 — batched per 100ms UI tick); a header shows `N matches` updating live. The input field is no longer focused (focus moves to the result list).
3. **Given** results are shown, **When** the user presses `Enter` on a highlighted result (or `Enter` at the panelize action), **Then** the dialog closes and the active panel switches to a synthetic flat listing of the matched paths, with a status bar showing `[Find: <pattern>]`.
4. **Given** the panelized find result is the active listing, **When** the user tags files and copies them, **Then** the operations apply to the actual file paths represented, identical to normal panel operations.
5. **Given** the user types a pattern with no matches, **When** the walk completes, **Then** the result list is empty and the count reads `0 matches`; pressing `Enter` does NOT panelize — the dialog stays open and displays "No files found matching `<pattern>`" so the user can refine and retry (see FR-008).

---

### User Story 2 — Search by file content via ripgrep (Priority: P2)

A user wants to find all files under the current directory containing the string `TODO` and bulk-delete them. They press `Alt-?`, press `Tab` to switch to Content mode, type `TODO`, and press `Enter`. Results show file paths containing matches (not individual match lines — file-level results). They press `Enter` to panelize, tag all with `*`, and delete.

**Why this priority**: Content search (ripgrep) is the second primary mode from the issue. It requires an external binary but delivers significantly higher value than name-only search for code/text workflows.

**Independent Test**: With `rg` on PATH, open the dialog in Content mode, type a pattern known to exist, confirm matching file paths appear, panelize, confirm bulk operations work.

**Acceptance Scenarios**:

1. **Given** the find-file dialog is open with `rg` available, **When** the user presses `Tab`, **Then** the mode switches to `Content` (visibly labeled) and the input hint changes to reflect content-search syntax.
2. **Given** Content mode is active and a pattern is entered, **When** the walk runs, **Then** results list unique file paths (one per file, regardless of how many lines match); no line-number detail shown at the file-level result list.
3. **Given** `rg` is not found (not on PATH, ripgrep_path not configured), **When** the user tries to switch to Content mode, **Then** the Tab toggle is visually dimmed and a status line explains "Content search unavailable: rg not found"; Name mode remains usable.
4. **Given** the user has panelized content-search results, **When** they perform bulk operations, **Then** behavior is identical to panelized name-search results.

---

### User Story 3 — Cancel and abort in-progress search (Priority: P3)

A user starts a search on a large directory tree that takes several seconds. They realize they typed the wrong pattern. They press `Esc` to cancel — the walk stops, the dialog closes, and the active panel is unchanged. (`Ctrl-C` triggers OS-level SIGINT and is not a dialog key; see FR-011.)

**Why this priority**: Long walks in deeply nested trees can take many seconds. Without cancellation, the user is stuck.

**Independent Test**: Start a search on a directory with many files, press `Esc` before completion, verify the panel is unchanged and the app is fully responsive.

**Acceptance Scenarios**:

1. **Given** a search is in progress, **When** the user presses `Esc`, **Then** the async walk is aborted within ≤300 ms, the dialog closes, and the active panel retains its previous listing.
2. **Given** a search was cancelled mid-walk, **When** the user immediately opens the find-file dialog again, **Then** the new dialog opens cleanly with no stale results or state from the previous walk.
3. **Given** the walk completes before the user presses `Esc`, **When** they press `Esc` on a completed results list, **Then** the dialog closes without panelizing (panel unchanged).

---

### Edge Cases

- What happens when the pattern is empty and the user presses Enter? The empty string is treated as the glob `"**"` (matching every file); a confirmation notice warns the user if the match count exceeds `config.search.max_results` (currently 5000). An empty pattern on an empty directory shows the standard "No files found" notice. (Rationale: `globset::Glob::new("")` panics; the implementation substitutes `"**"` before building the matcher.)
- What happens when match count exceeds `config.search.max_results`? The walk stops collecting new results at the limit; the count header shows `5000 matches (truncated)`; the user is not blocked from panelizing the capped set.
- What happens when the search root directory is unreadable? Results for that subtree are silently skipped; if the root itself is unreadable, a status message explains it. (This is captured as FR-018 below.)
- What happens when the user navigates away from the panelized listing (enters a subdirectory)? The synthetic listing is replaced by the real directory listing; pressing the back key returns to the previous real directory, not to the synthetic listing.
- What happens with very long file paths in the result list? Paths are truncated with `…` on the left (preserving the filename) to fit the available width.
- What happens with binary files in content search? `rg` excludes binary files by default; this behavior is honored without additional configuration.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The system MUST open a find-file overlay dialog when `Alt-?` is pressed, with the search root set to the active panel's current directory.
- **FR-002**: The dialog MUST support two search modes — Name (filename glob) and Content (ripgrep regex/literal) — selectable by pressing `Tab`.
- **FR-003**: Name-mode search MUST use `config.search.default_pattern_type` (`PatternType::Glob` by default; the `PatternType` enum in `cargonaut-config` has variants `Glob` and `Literal`) and traverse subdirectories recursively from the search root. When the user input is empty, the system substitutes `"**"` before building the glob matcher to avoid a panic on empty-string glob construction.
- **FR-004**: Content-mode search MUST invoke the configured `config.search.ripgrep_path` binary (default `rg`) and collect unique matching file paths.
- **FR-005**: The walk is triggered by pressing `Enter` in the input field (not live-as-you-type). Once triggered, it runs asynchronously; results MUST populate the result list incrementally — "incrementally" means batched per the 100ms UI tick (≤100ms latency from found-on-disk to visible-in-list), not frame-by-frame. A second `Enter` while the result list is focused panelizes (see FR-008).
- **FR-006**: The result list MUST display paths relative to the search root for readability (display-relative), while the underlying `PathBuf` values stored in `FindFileDialog.results` are absolute (store-absolute) to ensure correct file operations. The list is scrollable with arrow keys / `PgUp` / `PgDn`, with a live `N matches` header.
- **FR-007**: The system MUST stop collecting results when the count reaches `config.search.max_results` (5000) and label the header `N matches (truncated)`.
- **FR-008**: Pressing `Enter` while the result list is focused and contains ≥1 result MUST close the dialog and replace the active panel's listing with a synthetic flat listing of the matched paths (panelize action). If the result count is 0, `Enter` MUST NOT panelize; the dialog stays open and shows "No files found matching `<pattern>`".
- **FR-009**: The panelized listing MUST behave like a normal directory listing for all panel operations: cursor movement, tagging (`Space`, `+`, `-`, `*`), copy, move, delete, view (F3), edit (F4).
- **FR-010**: The status bar MUST display `[Find: <pattern>]` in the path segment of the **active pane's** status bar (replacing the directory path, not appending to it) while a panelized find-result listing is active. The passive pane's status bar is unaffected.
- **FR-011**: Pressing `Esc` at any point (while typing, while results display, while a walk is in progress) MUST cancel the walk (if running) and close the dialog without changing the active panel.
- **FR-012**: If Content mode is requested but the configured ripgrep binary is not found, the system MUST prevent switching to Content mode, display an explanatory notice, and keep Name mode usable.
- **FR-013**: The keymap binding for `Alt-?` MUST be defined in `design/contracts/keymap.toml` (single source of truth, Constitution §III).
- **FR-014**: The dialog overlay MUST use the shared `dialog!` macro / shared widget infrastructure (Constitution §III — no ad-hoc layouts).
- **FR-015**: All new public API items MUST carry doc comments (`#![warn(missing_docs)]`, Constitution §I).
- **FR-016**: The feature MUST introduce no new `unsafe` blocks (Constitution §I).
- **FR-017**: The feature MUST add no new crates to the workspace root `Cargo.toml`. Adding a crate that is already declared at the workspace level to a specific crate's `[dependencies]` table (e.g. `globset = { workspace = true }` in `cargonaut-ui-tui/Cargo.toml`) is permitted and does not violate this requirement.
- **FR-018**: If the search root directory is unreadable at walk-start, the system MUST display a status message in the dialog explaining the error (e.g. "Cannot read directory: <path>") and NOT start the walk. Unreadable subdirectories encountered during a walk are silently skipped (logged if debug logging is enabled).
- **FR-019**: The F1 help overlay MUST include an entry for `Alt-?` (`M-?`) mapping to "Find file (glob or ripgrep content search, then panelize)" in the Navigation or Search section, so the binding is discoverable (Constitution §III).

### Key Entities *(include if feature involves data)*

- **FindDialog**: Overlay state struct — holds the current mode (Name/Content), the input string, in-progress walk handle, collected results (`Vec<PathBuf>`), scroll offset, and a truncated flag.
- **SearchMode**: Enum — `Name` | `Content`. Governs which backend drives the walk.
- **SearchResult**: A resolved file path relative to the search root; collected incrementally from the walk task.
- **SyntheticListing**: The panelized output — a `Vec<PathBuf>` of absolute paths displayed as a flat directory listing in the active pane, tagged with the originating pattern for status display.
- **FindOutcome**: Enum returned by the dialog dispatch — `Panelize(Vec<PathBuf>, String)` | `Cancelled`.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: A user can locate all files matching a glob pattern within any directory depth and panelize them in under 5 seconds on a 10,000-file tree backed by tmpfs (warm cache, same hardware as the development host used for CI benchmarks).
- **SC-002**: The UI remains responsive (≤16 ms frame budget honored, NFR-002) during an active async walk — keystrokes and scrolling do not stall. Verification: the existing `benches/keypress-latency.rs` bench (which covers the full dispatch loop including dialog event handling) serves as the CI gate for this criterion; no additional bench is required.
- **SC-003**: Content-search results (via ripgrep) correctly match what `rg <pattern> --files-with-matches` would report on the same tree.
- **SC-004**: The panelized listing supports 100% of the panel operations that a real directory listing supports (tag, copy, move, delete, view, edit).
- **SC-005**: With `rg` absent, the app never crashes and Name-search remains fully functional.
- **SC-006**: Pressing `Esc` during an in-progress walk aborts within ≤300 ms (no lingering background task after dialog close).
- **SC-007**: The feature introduces no binary-size regression beyond 50 KiB stripped (NFR-001 ≤8 MiB total).
- **SC-008**: All existing tests pass (`cargo test --workspace`) with no regressions.

## Assumptions

- `tokio` is already a workspace dependency (used throughout `cargonaut-ui-tui`); async walk via `tokio::fs::read_dir` is available without adding a new crate.
- `tokio::process::Command` is available for spawning `rg` as a subprocess; no separate `ripgrep` Rust library is needed.
- The active panel's `current_dir()` is always a real filesystem path (not a VFS/SFTP path) for this feature; VFS-backed find-file is out of scope.
- The existing `pane.rs` listing infrastructure can be adapted to display a synthetic `Vec<PathBuf>` flat listing (similar to the existing synthetic `..` parent row pattern).
- Search results are file paths only — content-search results do not show individual match lines or line numbers in the find dialog (file-level granularity).
- The `Alt-?` key is currently unbound; confirmed by reviewing `design/contracts/keymap.toml`.

## Out of Scope

- **External panelize** (`Ctrl-x !` — run arbitrary command, feed stdout as listing): already registered as a separate `Command::ExternalPanelize` binding; this feature does not reimplement it.
- **VFS / remote directory search** (SFTP, archive-as-directory): tracked in GitHub issue #48.
- **In-dialog result previewer**: the find dialog does not preview file content; F3 on a panelized result opens the normal viewer.
- **Saved searches / search history**: no persistence; each dialog open starts fresh.
- **Line-level content results**: ripgrep is invoked with `--files-with-matches` (file paths only); line numbers and match context are not shown in the find dialog.
- **Interactive pattern builder / regex wizard**: plain text input only.
- **Search-root selection**: always the active panel's current directory; no picker.

# Research: Internal F4 Editor (Feature 056)

## R-001 — Reuse `FileViewerDialog` pattern or write a fresh widget?

**Decision**: Write `FileEditorDialog` as a new struct in `dialog.rs`, modelled structurally on `FileViewerDialog` but owning a mutable line buffer instead of read-only content.

**Rationale**: `FileViewerDialog` is read-only; it holds `ViewBuffer` (pre-loaded or streamed lines) and never mutates file content. Attempting to retrofit mutable editing on top of it would require invasive changes to every `handle_key` return type and all callers. A fresh widget with the same *shape* (struct in `dialog.rs`, `handle_key` → action enum, `render` method, full-screen) is the right-sized addition.

**Alternatives considered**: (a) Wrapping `FileViewerDialog` — rejected because its interior is read-only by design. (b) Writing a separate file in its own module — rejected; the existing precedent is all dialog widgets in `dialog.rs`, and the file is well-structured with clear section breaks.

---

## R-002 — How to handle the unsaved-changes prompt?

**Decision**: Add `UnsavedChangesDialog` as a new struct in `dialog.rs` with three choices: Save, Discard, Cancel. Do NOT repurpose `ConfirmDialog`.

**Rationale**: `ConfirmDialog` only has two focus states (Confirm / Cancel). The unsaved-changes flow requires three distinct actions. Adding a third focus state to `ConfirmDialog` would bloat its interface and break existing callers that only test for `Confirm`/`Cancel`. A purpose-built three-choice dialog is cleaner and adds no complexity to existing code.

**Alternatives considered**: Two successive `ConfirmDialog`s (first "save?" → then if No, "really discard?") — rejected as confusing UX and non-atomic behavior.

---

## R-003 — Input routing: reuse `Mode::Preview` or add `Mode::Editor`?

**Decision**: Add `Mode::Editor` to `keymap.rs`.

**Rationale**: `Mode::Preview` routes keys to the file viewer. If the editor reused `Mode::Preview`, keys would be dispatched through viewer-command checks first (e.g., `/` would open a search prompt, `q` would call `viewer-quit`). The editor needs a clean input surface where every printable key is character insertion. A separate mode is the correct factoring.

**Alternatives considered**: Override keybindings in `Mode::Preview` — rejected; would silently break the viewer's bindings if both are in the mode table simultaneously, and creates action-to-mode confusion.

---

## R-004 — Line buffer representation?

**Decision**: `Vec<String>` where each `String` is the line content without its line terminator.

**Rationale**: The standard editing model: lines are split on `\n` or `\r\n` at load time; the original terminator style is recorded as `LineEnding { Lf | Crlf }` and reapplied at save time. `Vec<String>` is zero-dependency, directly indexable by `cursor_line`, and trivially joined for save. For files ≤10 MiB, worst case is ~10M chars in memory — well within 64 MiB RSS budget (Constitution §IV SC-003).

**Alternatives considered**: Rope data structure (e.g., `ropey` crate) — rejected; adds a dependency for a feature whose file-size limit ensures O(n) operations are acceptable. No crate addition is preferable for a ≤10 MiB constraint.

---

## R-005 — How to detect binary files and apply the size limit?

**Decision**: Reuse `is_valid_utf8_sample()` (already in `lib.rs`) and `BINARY_DETECT_BYTES` / `STREAMING_THRESHOLD_BYTES` (already in `dialog.rs`). Binary detection: read up to `BINARY_DETECT_BYTES` (4 KiB); if `is_valid_utf8_sample()` returns false → decline. Size limit: if `file_size > STREAMING_THRESHOLD_BYTES` (10 MiB) → decline. No new constants needed.

**Rationale**: The viewer already defines these constants and uses the same detection logic. Reusing them ensures the editor and viewer behave consistently and avoids duplicating thresholds that could diverge.

---

## R-006 — F2 key in editor mode conflicts with F2 (User Menu)?

**Decision**: No conflict. `Mode::Pane` and `Mode::Editor` are distinct lookup contexts in the keymap. `F2 → show-user-menu` is bound in `Mode::Pane`; `F2 → save-file` will be bound in `Mode::Editor`. The keymap lookup already dispatches on the active mode, so the same physical key resolves to different commands depending on context.

**Rationale**: This is exactly the design intent of the mode system — identical keys can mean different things in pane vs. preview vs. dialog. The viewer already uses `q` as `viewer-quit` in `Mode::Preview` while `q` in `Mode::Pane` opens the filter prompt. Same pattern.

---

## R-007 — Which new `Command` variants are needed?

**Decision**: Add `Command::SaveFile` and `Command::EditorQuit` to the `Command` enum in `keymap.rs`.

**Rationale**: The keymap system requires every key action to be a `Command` variant. New editor-specific actions need new variants; they cannot be subsumed by existing viewer or dialog commands without conflating semantics.

---

## R-008 — Can `ExternalTool` and `queue_external` be deleted?

**Decision**: Yes. `ExternalTool` has exactly one variant (`Editor`) and `queue_external` is called only from `Command::Edit`. After Feature 056, `Command::Edit` will open the built-in `FileEditorDialog` instead of queuing an external program. Both `ExternalTool` and `queue_external` become dead code and should be deleted.

**Note**: `queue_bulk_rename` and `queue_diff` are independent functions — they are NOT removed.

---

## R-009 — Save operation: synchronous or async?

**Decision**: Synchronous `std::fs::write()` inside the editor's `handle_key` save branch (not spawned to a thread).

**Rationale**: Files are capped at 10 MiB. A 10 MiB write to a local SSD completes in <1 ms — well within the 16 ms frame budget. Spawning a blocking task for this would add latency and complexity (needing to handle "save in progress" state) with no practical benefit for the file-size range this feature handles.

**Alternatives considered**: `tokio::task::spawn_blocking` — rejected for the reasons above; acceptable if a future version lifts the size limit.

---

## R-010 — Pane refresh after editor close?

**Decision**: After `ActiveDialog::FileEditor` is dismissed (save or discard), call the existing `refresh_active_pane(app, ui)` function (the same mechanism used after `pending_external` returns) so the pane listing reflects any file-size or mtime change.

**Rationale**: The F3 viewer already refreshes the pane on close via the `FileViewerAction::Close` arm. The editor must do the same to satisfy FR-009.

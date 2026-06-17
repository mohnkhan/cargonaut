# Research: PTY Binary-Level Navigation Smoke Test

**Feature**: 045-pty-nav-smoke-test | **Date**: 2026-06-17

All Technical Context items resolved. No NEEDS CLARIFICATION after `/speckit-clarify`.

## R-001: Startup detection signal — function-key bar "Quit" label

- **Decision**: Poll PTY output for the string `"Quit"` as the TUI-ready signal before injecting any key sequence.
- **Rationale**: `chrome.rs` renders a bottom function-key bar (`1Help 2Menu … 10Quit`) on every frame. The label `"Quit"` appears as the last button label and is emitted on the very first rendered frame. It is stable across themes (it is a code literal in `chrome.rs`, not a configurable string) and will never appear in the temp directory names used by the test. The `wait_until` polling loop from the resume test (Feature 037) applies directly.
- **Alternatives considered**: Fixed `thread::sleep(700ms)` — matches the resume test's approach but is fragile on slow CI runners; polling is both faster on fast machines and more robust on slow ones. Polling for the temp directory name — unreliable because the directory name might not appear in the initial render if the path is very long and truncated.

## R-002: Cursor-position assertion via delta-buffer approach on the mini-status line

- **Decision**: Use a **delta-buffer** approach: record the buffer length before each key injection, then `wait_until` for new bytes (bytes after the snapshot index) that contain the expected entry name. The observable signal is the per-pane **mini-status line**, which renders `" {perms}  {size:>12}  {mtime}  {name}"` for the currently focused entry (`chrome.rs:461`).
- **Rationale**: The raw PTY output accumulates across the whole session. After the first Down-arrow moves the cursor to `aaa`, both `aaa` and `bbb` and `ccc` are already in the buffer from the initial listing render. If we used a full-buffer scan, every subsequent assertion would immediately pass (false positive). The delta approach isolates only new bytes written since the last action, so `aaa` in new bytes proves the cursor just moved to `aaa`. The mini-status line is the correct signal because it contains **only** the focused entry's name — not all entries — and is re-emitted on every keypress-triggered frame.
- **Key detail**: When the cursor is on the `..` parent row, `focused_entry_index()` returns `None` and the mini-status line is empty (`chrome.rs:441-443`). This means after launch (cursor starts on `..`), the mini-status shows nothing — the first Down-arrow moves to the first real entry and the mini-status emits its name.
- **Alternatives considered**: Scanning the full cumulative buffer — false positives (all names present from initial listing). Parsing ANSI escape sequences to reconstruct the screen buffer — high complexity, brittle across themes. Negative assertion (verify old highlight escapes disappear) — unreliable; escape-code formats vary by theme.

## R-003: Helper functions — shared `pty_harness` module within the test crate

- **Decision**: Extract the shared PTY helpers (`spawn`, `output_contains`, `wait_until`, `sigkill`, `delta_contains`) into a `tests/pty_harness.rs` module inside `cargonaut-bin` (declared `#[path = ...]` from each test file, or via a `tests/common/mod.rs` pattern). Both `local_navigation.rs` and `resume_sigkill.rs` import from it.
- **Rationale**: The `resume_sigkill.rs` test already defines `spawn`, `output_contains`, and `wait_until`. Duplicating them in `local_navigation.rs` would create drift risk. A shared `pty_harness` module avoids duplication and is the standard Rust integration-test pattern for common helpers (placing a `mod.rs` in `tests/common/` so Cargo does not treat it as a standalone test binary).
- **New helper needed**: `delta_contains(sink, prev_len, needle)` — takes the arc-mutex sink, a snapshot byte count, and a needle string; returns `true` if any new bytes (since `prev_len`) contain `needle`. Used for all cursor-position assertions.
- **Alternatives considered**: Duplicating helpers in each test file — rejected; maintenance burden. Extracting helpers to `cargonaut-ui-tui` as a test utility — wrong crate; PTY helpers are bin-level concerns.

## R-004: Temp directory layout — deterministic sort order via alphabetic names

- **Decision**: Test fixtures use entries named `aaa`, `bbb`, `ccc` as **directories** (not files), so that the descend test can also use them as descent targets without a separate fixture.
- **Rationale**: The pane listing is sorted; alphabetic names sort predictably. Using directories for all three entries means the same fixture serves all three test functions (`nav_cursor_arrow_keys` navigates among them, `nav_descend_enter` descends into one, `nav_ascend_backspace` ascends back out). The `..` parent row is always index 0; `aaa` is index 1, `bbb` index 2, `ccc` index 3.
- **Alternatives considered**: Mixed files and directories — forces separate fixtures per test function, adding setup complexity. Single-letter names (`a`, `b`, `c`) — slightly less distinctive in debug output; three-letter names (`aaa`) read clearly in failure messages.

## R-005: No CI changes required

- **Decision**: The existing CI workflow already sets `CARGONAUT_PTY_TESTS=1` for the `cargo test --workspace --lib --tests` step (Feature 037). No `.github/workflows/ci.yml` modifications are needed. The three new un-ignored test functions will be picked up automatically.
- **Rationale**: Confirmed by reading `.github/workflows/ci.yml`: `CARGONAUT_PTY_TESTS: "1"` is already in the env block for the unit-test job.

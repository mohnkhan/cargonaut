# Feature Specification: PTY Binary-Level Navigation Smoke Test

**Feature Branch**: `045-pty-nav-smoke-test`

**Created**: 2026-06-17

**Status**: Draft

**Input**: User description: "PTY end-to-end navigation smoke test — implement the currently-ignored bin-level PTY navigation test (T1.07, issue #30). The test should launch the real cargonaut binary in a PTY, send keyboard navigation sequences (arrow keys, Enter to descend into a directory, Backspace/Left to ascend), and assert that the TUI output reflects the expected cursor and directory state. It should be gated behind the CARGONAUT_PTY_TESTS=1 env var (same pattern as the existing PTY resume test from Feature 037) and run in CI when that env var is set. The test lives in the integration test suite (tests/ directory or the existing pty_tests module). Feature 028 deferred this; the PTY harness was laid down in Feature 037."

## Clarifications

### Session 2026-06-17

- Q: Should the three user stories be one combined test function or three separate functions? → A: **Three separate test functions** — `nav_cursor_arrow_keys`, `nav_descend_enter`, `nav_ascend_backspace` — for independent failure attribution in CI.
- Q: How should the test detect that the TUI is ready before sending the first key? → A: **Poll PTY output for a recognizable startup string** (deadline-bounded), consistent with FR-006; do not use a fixed sleep.
- Q: How should cursor-position assertions work for arrow-key navigation? → A: **Predictably-named temp entries** (lexicographically sorted unique names such as `aaa`, `bbb`, `ccc`) + raw PTY substring scan using `output_contains`; no ANSI parsing required.

## User Scenarios & Testing *(mandatory)*

### User Story 1 — Cursor moves up and down via arrow keys (Priority: P1)

A developer or CI environment launches the cargonaut binary against a directory that contains multiple entries. Arrow key presses navigate the cursor within the listing: the cursor position changes and the TUI output confirms which entry is highlighted.

**Why this priority**: Arrow-key navigation is the most fundamental interaction a file-manager binary can perform. Without a passing bin-level test for this, engine-layer unit tests are the only coverage — any wiring gap between the binary entrypoint and the TUI render loop goes undetected until a user reports it.

**Independent Test**: Spawn the binary with a temp directory containing several named files. Send a down-arrow key, assert the cursor has advanced; send another, assert it advanced again; send an up-arrow, assert it retreated.

**Acceptance Scenarios**:

1. **Given** a directory with ≥3 entries and the cursor on the first entry, **When** a down-arrow key is sent via the PTY, **Then** the TUI output reflects the cursor on the second entry.
2. **Given** the cursor is on the second entry, **When** another down-arrow is sent, **Then** the cursor advances to the third entry.
3. **Given** the cursor is not on the first entry, **When** an up-arrow key is sent, **Then** the cursor retreats by one entry.

---

### User Story 2 — Enter key descends into a subdirectory (Priority: P1)

After navigating to a subdirectory entry, pressing Enter causes the active pane to change its working directory to that subdirectory. The TUI output reflects the new path and the contents of the subdirectory.

**Why this priority**: Descend-on-Enter is core navigation behaviour. US1 (cursor movement) and US2 (descend) together constitute the minimal navigable binary — both are P1.

**Independent Test**: Spawn with a temp directory that contains a named subdirectory. Navigate the cursor to the subdirectory entry, send Enter, and assert the TUI output contains the subdirectory's name or path, confirming the pane changed directory.

**Acceptance Scenarios**:

1. **Given** the cursor is on a subdirectory entry, **When** Enter is sent via the PTY, **Then** the TUI output reflects the subdirectory as the new active directory (its contents are listed or its path is visible).
2. **Given** the cursor is on a regular file entry (not a directory), **When** Enter is sent, **Then** no directory change occurs (the pane remains in the current directory).

---

### User Story 3 — Backspace / Left key ascends to the parent directory (Priority: P2)

After descending into a subdirectory, pressing Backspace (or the Left arrow, if that also triggers ascent per the keymap) causes the pane to navigate to the parent directory. The TUI output reflects the parent path and the original listing is restored.

**Why this priority**: Ascent completes the navigation loop and exercises the `..` row logic added in Feature 040. It is P2 because US1+US2 alone are an independently verifiable slice.

**Independent Test**: Descend into a subdirectory via Enter (US2), then send Backspace and assert the TUI output returns to showing the parent directory's entries.

**Acceptance Scenarios**:

1. **Given** the active pane is inside a subdirectory, **When** Backspace is sent via the PTY, **Then** the TUI output reflects the parent directory as the new active directory.
2. **Given** the active pane is at the root of the supplied path (no navigable parent within the session), **When** Backspace is sent, **Then** the pane remains at the current directory without crashing.

---

### Edge Cases

- **PTY gate respects the env var**: when `CARGONAUT_PTY_TESTS` is absent or `0`, the test self-skips rather than failing, so ordinary `cargo test` stays fast.
- **Timing**: output assertions must tolerate TUI render latency; a polling loop with a deadline is used rather than fixed sleeps, to keep the test reliable on slow CI runners.
- **Non-TTY environment**: the test uses a real PTY (not a pipe) so crossterm renders correctly; on platforms without PTY support the test is unconditionally skipped.
- **Binary not found**: if the binary has not been built, the test fails with a clear message rather than a cryptic panic.
- **Empty directory**: a directory with no entries (other than `..`) causes the cursor to rest on `..`; arrow keys do not panic.
- **Quit key (F10)**: the test must cleanly exit the binary at the end of each scenario to avoid leaving zombie processes; if the binary does not exit within a deadline, the test kills it and marks itself failed.

## Requirements *(mandatory)*

### Functional Requirements

- **FR-001**: The test MUST gate execution behind the `CARGONAUT_PTY_TESTS=1` environment variable; when the variable is absent or not `1`, the test MUST self-skip (print a diagnostic and return) rather than fail.
- **FR-002**: The test MUST launch the actual compiled cargonaut binary (not a mock or subprocess wrapper) inside a PTY with a real terminal size, passing two temporary directory paths as arguments.
- **FR-003**: The test MUST verify that sending a down-arrow key sequence advances the cursor position in the TUI output, and that sending an up-arrow key sequence retreats it.
- **FR-004**: The test MUST verify that pressing Enter when the cursor is on a subdirectory causes the active pane to display the subdirectory's contents.
- **FR-005**: The test MUST verify that pressing Backspace when inside a subdirectory causes the active pane to return to the parent directory's contents.
- **FR-006**: All TUI-state assertions MUST use a polling loop with a configurable deadline (not `thread::sleep` with a fixed delay), so the test is reliable on slow CI runners. This includes startup detection: the test MUST poll for a recognizable startup string in the PTY output before injecting any key sequences.
- **FR-007**: The test MUST cleanly exit the binary (via F10 or equivalent quit key) at the end of the scenario; if the binary does not exit within a deadline, the test MUST kill it and report a failure.
- **FR-008**: The existing `#[ignore]`d `local_navigation_smoke` stub in `cargonaut-bin/tests/local_navigation.rs` MUST be replaced by three separate test functions — `nav_cursor_arrow_keys`, `nav_descend_enter`, and `nav_ascend_backspace` — one per user story. No combined single-function form is acceptable.
- **FR-009**: The test MUST be Unix-only (`#[cfg(unix)]`), consistent with the existing PTY resume gate.
- **FR-010**: CI MUST execute the test when `CARGONAUT_PTY_TESTS=1` is set; the existing CI workflow already sets this flag, so no CI config change is needed — the test just needs to exist and be un-ignored.

## Success Criteria *(mandatory)*

### Measurable Outcomes

- **SC-001**: Running `cargo test --workspace --tests` with `CARGONAUT_PTY_TESTS=1` completes without the navigation test failing or panicking.
- **SC-002**: The test reliably detects a cursor movement within 5 seconds of sending the key; it does not flake on three consecutive CI runs at baseline.
- **SC-003**: Running `cargo test --workspace --tests` *without* `CARGONAUT_PTY_TESTS=1` completes without any skip being counted as a failure — the test self-skips silently.
- **SC-004**: The previously-ignored stub (`local_navigation_smoke` with `#[ignore]`) is absent from the codebase; `cargo test -- --ignored` reports zero ignored tests in the `cargonaut-bin` crate.

## Assumptions

- `portable-pty` is already a workspace dependency (confirmed; it landed in Feature 037) — no new external crate is required.
- The CI workflow already sets `CARGONAUT_PTY_TESTS=1` during the test step, so no CI configuration changes are needed.
- The binary is available as `env!("CARGO_BIN_EXE_cargonaut")` in integration tests, consistent with how Feature 037's PTY test resolves the binary path.
- The keymap is stable: down-arrow = `\x1b[B`, up-arrow = `\x1b[A`, Enter = `\r`, Backspace = `\x7f`; these match the crossterm sequences already exercised by Feature 037.
- Observable TUI state can be detected by scanning raw PTY output for distinguishing strings (directory names, path segments, or entry labels) rather than by parsing ANSI sequences into a structured screen buffer. Temp entries are created with predictably sorted unique names (e.g. `aaa`, `bbb`, `ccc`) so each arrow-key step is verifiable by asserting the next name appears in the output.
- Two panes are displayed side-by-side; the test focuses on the left (active) pane for all assertions.
- Startup readiness is detected by polling the PTY output for a recognizable initial TUI string rather than a fixed sleep, consistent with the polling requirement that applies to all assertions (FR-006).

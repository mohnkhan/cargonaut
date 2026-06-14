# Implementation Checklist: Panel Filter Prompt Dialog

**Purpose**: Pre-merge gate — every box must be checked before the PR for #33 is marked ready.
**Created**: 2026-06-15
**Feature**: [spec.md](../spec.md) · [tasks.md](../tasks.md)

## Behavior (maps to FRs / acceptance scenarios)

- [x] Filter key (`Alt-!`) opens a modal prompt instead of clearing immediately (FR-001)
- [x] Prompt is prefilled with the active filter's pattern, empty when none (FR-002)
- [x] Non-empty valid pattern narrows the focused pane to matching names (FR-003)
- [x] Glob metacharacters work (`*.rs`); bare words match as substring `*word*` (FR-003a)
- [x] Matching is case-insensitive (`*.RS` matches `lib.rs`) (FR-003b)
- [x] Filter persists across directory navigation until cleared (FR-003c)
- [x] Cursor resets to the first visible entry after set or clear (FR-004)
- [x] Empty / whitespace submit clears the filter and restores the full listing (FR-005)
- [x] Invalid pattern keeps the prompt open with an inline error, pane unchanged (FR-006)
- [x] Editing after an error clears the error (FR-007)
- [x] Esc closes the prompt and leaves the filter exactly as before (FR-008)
- [x] Only the focused pane is affected; the other pane is untouched (FR-009)
- [x] The shared `PathInputDialog` is reused; no new widget added (FR-010)

## Tests (Constitution §II, SC-005)

- [x] `PaneFilter::compile` unit tests: glob, auto-substring, case-insensitive, invalid (T003)
- [x] `set_filter` set tests: narrowing, cursor reset, other-pane untouched, persistence (T008)
- [x] `set_filter` clear tests: clear active, whitespace, no-op when none (T013)
- [x] `set_filter` invalid test: `Err(BadFilter)`, pane byte-for-byte unchanged (T016)
- [x] End-to-end injected-input test: set → invalid/error → clear → cancel (T019)
- [~] Red-before-green git history: tests and implementation were committed
      together in this run (all FRs are test-covered and green), not as separate
      per-task red→green commits. Flagged for reviewer awareness (Constitution §II).

## Quality gates (Constitution §I / §IV / docs gate)

- [x] `cargo fmt --check` clean (T021)
- [x] `cargo clippy --workspace --all-targets -- -D warnings` clean (T021)
- [x] New public items (`PaneFilter`, `set_filter`, `AppError::BadFilter`) documented (T004)
- [x] No `unsafe` introduced
- [x] `scripts/check-binary-size.sh` ≤ 8 MiB with `globset` added (T022)
- [x] `make ci-local` green end to end (T021)
- [x] README At-a-Glance + Feature History updated; Learnings.md section added (T023)
- [x] ROADMAP.md #33 row resolved; issue #33 closed from the PR (T024)

## SSD preservation (Constitution §V)

- [x] `make tmpfs-status` shows `target/` linked to tmpfs; no `cargo clean` / `rm -rf target`
      used during this work

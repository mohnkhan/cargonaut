# Cargonaut Roadmap

This file is a stable index into the open-issue tracker, organised by leverage. The authoritative source is `gh issue list`; this file exists so the structure is visible without leaving GitHub's repo browser.

**Last updated**: 2026-06-15 (Feature 037 shipped resume-on-launch wiring + the binary-level SC-002 PTY gate, resolving #29; Tier 1 now clear).

Every item below has a corresponding GitHub issue with: a problem statement, a sketch of the proposed approach, an effort estimate, and a pointer to the spec / commit / file that originally deferred it. Tier numbers reflect leverage × effort tradeoff, not strict execution order.

---

## Tier 1 — highest leverage right now

Pick from here first. These are issues whose fix unblocks downstream work or whose investigation has already been substantially done.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| _(none — Tier 1 is clear)_ | | | |

> ✅ **Resolved**: [#29](https://github.com/mohnkhan/cargonaut/issues/29) (T1.08 — binary-level SIGKILL-resume PTY test, SC-002 gate) shipped in **Feature 037** (`037-resume-binary-wiring`). The scope grew once discovered: the resume-on-launch flow was never wired into the binary, so Feature 037 wired `scan_resumable`→`ResumePromptDialog`→`resume_transfer` end-to-end and added the gated PTY test (`CARGONAUT_PTY_TESTS=1`, enforced in CI). The PTY harness also lays groundwork for [#30](https://github.com/mohnkhan/cargonaut/issues/30).

> ✅ **Resolved**: [#31](https://github.com/mohnkhan/cargonaut/issues/31) (T1.25 — full quick-cd popup w/ tab-completion, FR-012) shipped in **Feature 038** (`038-quick-cd-popup`). Alt-c now opens an inline prompt (prefilled with the active pane's cwd) that Tab-completes directories against the pane's VFS + recent-dir history and navigates via the existing `navigate_to` path. It also delivers the shared, caller-driven `PathInputDialog` widget, which **unblocks [#32](https://github.com/mohnkhan/cargonaut/issues/32) and [#33](https://github.com/mohnkhan/cargonaut/issues/33)** (both were waiting on a reusable text-input dialog).

> ✅ **Resolved**: [#33](https://github.com/mohnkhan/cargonaut/issues/33) (FR-013 — panel filter prompt dialog, was clear-only) shipped in **Feature 033** (`033-panel-filter-prompt`). Alt-! now opens an inline prompt (prefilled with the active filter) reusing the shared `PathInputDialog`; on accept it compiles the pattern with `globset` (case-insensitive; metacharacter-free patterns match as `*word*` substrings) and applies it to the focused pane, empty submit clears, invalid patterns show an inline error and keep the prompt open. The filter became a compiled `PaneFilter` (`Option<String>` → `Option<PaneFilter>`) across core and the pane view. NB: the deferral's "globset plumbing present" note was inaccurate — only a substring placeholder existed.

> ✅ **Resolved**: [#37](https://github.com/mohnkhan/cargonaut/issues/37) (FR-020 — `..` parent entry as first row) shipped in **Feature 040** (`040-parent-row`). Every non-root pane shows a `..` row first; Enter/double-click on it ascends. Implemented as a core-owned virtual-row cursor (`PaneState.cursor` indexes `[..] ++ visible entries` via a `parent_offset`); `focused_entry_index()` returns `None` on the row so the existing selection/copy guards exclude it for free; `Descend` ascends on the parent row; the cursor defaults to the first real entry (clarified). No keymap change. The predicted ~10-test break stayed contained by pinning pane unit tests to a root cwd.

> ✅ **Resolved**: [#32](https://github.com/mohnkhan/cargonaut/issues/32) (T1.29 — tasks/jobs panel popup, FR-016/NFR-004) shipped in **Feature 039** (`039-tasks-jobs-panel`). F12 now opens a modal `TasksPanelDialog` over the App transfer registry listing each transfer's `source → destination` + live state/progress, with per-row cancel (`c`), pause (`p`), and resume (`r`). Pause reuses the cancellation token (leaving the checkpoint) + an `App.paused` marker so the job holds as `Paused` while siblings continue; resume re-arms via the existing `resume_transfer` checkpoint path (same id, fresh token). Core gained UI-agnostic `JobView`/`JobStatus` + `job_views()`/`cancel_transfer`/`pause_transfer`/`resume_paused`; no transfer-crate changes. The SC-003 three-job pause/resume scenario is a CI integration test.

---

## Tier 2 — near-term, well-scoped features

| Issue | Title | Effort | Origin |
|---|---|---|---|
| _(none — Tier 2 is clear; see Tier 3 for diagnostics/follow-ups)_ | | | |

---

## Tier 3 — diagnostics + follow-ups

Useful but not urgent. Pick up when the underlying use case materialises.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#38](https://github.com/mohnkhan/cargonaut/issues/38) | Mouse: in-session toggle key (FR-013) | XS | Feature 031 — `--no-mouse`/config disable already ship |
| [#42](https://github.com/mohnkhan/cargonaut/issues/42) | Directory hotlist / bookmarks | M | Feature 031 §Out of Scope |
| [#46](https://github.com/mohnkhan/cargonaut/issues/46) | File attributes: chmod/chown + sym/hardlink | M | Feature 031 §Out of Scope — needs new VFS ops |
| [#49](https://github.com/mohnkhan/cargonaut/issues/49) | External / user-authored theme (skin) files | S–M | Feature 031 — built-in themes ship; external loader deferred (clarified) |
| [#30](https://github.com/mohnkhan/cargonaut/issues/30) | T1.07 — PTY end-to-end navigation smoke test | 0.5 ew | Feature 028 — behavior covered by 154 lower-level tests; only the bin-level driver is `#[ignore]`d. |

---

## Tier 4 — long-term / multi-session features

Substantial multi-session work. Don't start without confirming the scope is still warranted.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#39](https://github.com/mohnkhan/cargonaut/issues/39) | Internal file viewer F3 (text + hex + search) | L | Feature 031 — F3 external-pager shell-out ships |
| [#40](https://github.com/mohnkhan/cargonaut/issues/40) | Internal full-screen editor F4 | XL | Feature 031 — F4 `$EDITOR` shell-out ships |
| [#41](https://github.com/mohnkhan/cargonaut/issues/41) | Find-file (name + content) + external panelize | L | Feature 031 §Out of Scope |
| [#43](https://github.com/mohnkhan/cargonaut/issues/43) | Compare directories + diff two tagged files | M | Feature 031 §Out of Scope |
| [#44](https://github.com/mohnkhan/cargonaut/issues/44) | Persistent subshell integration (Ctrl-o) | L | Feature 031 §Out of Scope |
| [#45](https://github.com/mohnkhan/cargonaut/issues/45) | Tabs: multiple panels per side | L | Feature 031 §Out of Scope |
| [#47](https://github.com/mohnkhan/cargonaut/issues/47) | Bulk rename via editor + undo of file ops | M | Feature 031 §Out of Scope |
| [#48](https://github.com/mohnkhan/cargonaut/issues/48) | VFS backends: archives + remote (SFTP/FTP/sh) | XL | Feature 031 §Out of Scope — already Phase 2+ roadmapped |
| [#50](https://github.com/mohnkhan/cargonaut/issues/50) | User menu (F2) + hypertext help content | M | Feature 031 — minimal F1 overlay + F2 placeholder ship |

---

## How rows land here

A row appears in this file **only when** a GitHub issue also exists tracking the same work. The rule: deferred work needs both. The issue carries the deep context (problem statement, approach, effort, pointer to where deferral was decided); the ROADMAP row carries the one-line "what tier, what's blocking it" view.

When an issue is closed, delete its ROADMAP row (or move to a `## Closed` section if you want the history visible — but the issue history is the authoritative record).

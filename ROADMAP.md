# Cargonaut Roadmap

This file is a stable index into the open-issue tracker, organised by leverage. The authoritative source is `gh issue list`; this file exists so the structure is visible without leaving GitHub's repo browser.

**Last updated**: 2026-06-21 (Feature 062 survivability follow-ups; #90 resolved).

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
| _(none — Tier 3 is clear)_ | | | |

> ✅ **Resolved**: [#90](https://github.com/mohnkhan/cargonaut/issues/90) (Feature 061 follow-ups) shipped in **Feature 062** (`062-survivability-followups`): input-handler recovery (catch_unwind on the input boundary, escalate after 3), transfer task-panic → `Failed` (non-downgrading, via `Arc<watch::Sender>`), a dedicated menu-reachable About modal, and the FR-009 unwrap audit (core hot paths were already unwrap-free; one fragile site hardened). Also fixed a recovery-semantics bug (keep the captured panic on fatal escalation so the report is written) and extended the gated PTY test to the input path.

> ✅ **Resolved**: [#84](https://github.com/mohnkhan/cargonaut/issues/84) (Docker-based SFTP integration test, SC-003/SC-004) shipped in **Feature 060** (`060-sftp-docker-integration-test`). A `ci-integration`-gated test (`crates/cargonaut-vfs/tests/sftp_integration.rs`) drives the real `SftpFs::connect` path against an `atmoz/sftp` fixture (`docker-compose.ci.yml`; `make ci-sftp-up`/`ci-sftp-down`): SC-003 asserts root-list latency ≤ 5 s; SC-004 transfers a 10 MiB file and logs throughput vs the 87.5 MB/s target while gating on a conservative non-flaky floor (single-stream SFTP is crypto-bound). A new `sftp-integration` CI job runs it and feeds the `ci` rollup.

> ✅ **Resolved**: [#86](https://github.com/mohnkhan/cargonaut/issues/86) (split the `cargonaut-core/src/lib.rs` god-file into cohesive submodules) shipped in **Feature 059** (`059-cargonaut-core-split`). The 6,246-line module became a 122-line root + 14 submodules (`pane`, `command`, `error`, `jobs`, `app`, `nav`, `history`, `fsops`, `attrs`, `compare`, `rename`, `hotlist`, `tabs`, `transfers`) + a `#[cfg(test)]` `test_support`, each with co-located tests. Move-only: public API byte-for-byte stable (rustdoc-JSON surface diff vs committed baseline), 192 core tests green, zero downstream edits; only internal helpers widened to `pub(crate)`.

> ✅ **Resolved**: [#79](https://github.com/mohnkhan/cargonaut/issues/79) (subshell scrollback rendering) shipped in **Feature 055** (`055-subshell-scrollback`). `scroll_offset` is now wired into `render_vt100_screen` via `Screen::set_scrollback()`; scroll direction inversion fixed; cursor hidden in scrollback mode; `scroll_offset` reset on resize.

---

## Tier 4 — long-term / multi-session features

Substantial multi-session work. Don't start without confirming the scope is still warranted.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| ~~[#39](https://github.com/mohnkhan/cargonaut/issues/39)~~ | ~~Internal file viewer F3 (text + hex + search)~~ | ~~L~~ | **Closed — Feature 051** |
| ~~[#40](https://github.com/mohnkhan/cargonaut/issues/40)~~ | ~~Internal full-screen editor F4~~ | ~~XL~~ | **Closed — Feature 056** |
| ~~[#41](https://github.com/mohnkhan/cargonaut/issues/41)~~ | ~~Find-file (name + content) + external panelize~~ | ~~L~~ | **Closed — Feature 052** |
| ~~[#43](https://github.com/mohnkhan/cargonaut/issues/43)~~ | ~~Compare directories + diff two tagged files~~ | ~~M~~ | **Closed — Feature 049** |
| ~~[#44](https://github.com/mohnkhan/cargonaut/issues/44)~~ | ~~Persistent subshell integration (Ctrl-o)~~ | ~~L~~ | **Closed — Feature 054** |
| ~~[#45](https://github.com/mohnkhan/cargonaut/issues/45)~~ | ~~Tabs: multiple panels per side~~ | ~~L~~ | **Closed — Feature 053** |
| ~~[#47](https://github.com/mohnkhan/cargonaut/issues/47)~~ | ~~Bulk rename via editor + undo of file ops~~ | ~~M~~ | **Closed — Feature 050** |
| ~~[#48](https://github.com/mohnkhan/cargonaut/issues/48)~~ | ~~VFS backends: archives + remote (SFTP/FTP/sh)~~ | ~~XL~~ | **Closed — Feature 057** |
| ~~[#50](https://github.com/mohnkhan/cargonaut/issues/50)~~ | ~~User menu (F2) + hypertext help content~~ | ~~M~~ | **Closed — Feature 047** |

---

## How rows land here

A row appears in this file **only when** a GitHub issue also exists tracking the same work. The rule: deferred work needs both. The issue carries the deep context (problem statement, approach, effort, pointer to where deferral was decided); the ROADMAP row carries the one-line "what tier, what's blocking it" view.

When an issue is closed, delete its ROADMAP row (or move to a `## Closed` section if you want the history visible — but the issue history is the authoritative record).

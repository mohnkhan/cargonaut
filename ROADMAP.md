# Cargonaut Roadmap

This file is a stable index into the open-issue tracker, organised by leverage. The authoritative source is `gh issue list`; this file exists so the structure is visible without leaving GitHub's repo browser.

**Last updated**: 2026-06-14 (Feature 031 visual & interactive parity layer shipped US1–US5; deferrals #37–#50 populated below).

Every item below has a corresponding GitHub issue with: a problem statement, a sketch of the proposed approach, an effort estimate, and a pointer to the spec / commit / file that originally deferred it. Tier numbers reflect leverage × effort tradeoff, not strict execution order.

---

## Tier 1 — highest leverage right now

Pick from here first. These are issues whose fix unblocks downstream work or whose investigation has already been substantially done.

_(empty — populate as features ship and deferrals accumulate)_

---

## Tier 2 — near-term, well-scoped features

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#37](https://github.com/mohnkhan/cargonaut/issues/37) | Panel: `..` parent entry as first row (FR-020) | S | Feature 031 — ascent works via key/menu/mouse; needs an index model separating the synthetic row from real entries |

---

## Tier 3 — diagnostics + follow-ups

Useful but not urgent. Pick up when the underlying use case materialises.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#38](https://github.com/mohnkhan/cargonaut/issues/38) | Mouse: in-session toggle key (FR-013) | XS | Feature 031 — `--no-mouse`/config disable already ship |
| [#42](https://github.com/mohnkhan/cargonaut/issues/42) | Directory hotlist / bookmarks | M | Feature 031 §Out of Scope |
| [#46](https://github.com/mohnkhan/cargonaut/issues/46) | File attributes: chmod/chown + sym/hardlink | M | Feature 031 §Out of Scope — needs new VFS ops |
| [#49](https://github.com/mohnkhan/cargonaut/issues/49) | External / user-authored theme (skin) files | S–M | Feature 031 — built-in themes ship; external loader deferred (clarified) |

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

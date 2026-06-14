# Cargonaut Roadmap

This file is a stable index into the open-issue tracker, organised by leverage. The authoritative source is `gh issue list`; this file exists so the structure is visible without leaving GitHub's repo browser.

**Last updated**: 2026-06-14 (Phase 1.1 polish follow-ups logged — T1.07/08/25/29 + FR-013 filter prompt, deferred at Phase 1 closure / Feature 028).

Every item below has a corresponding GitHub issue with: a problem statement, a sketch of the proposed approach, an effort estimate, and a pointer to the spec / commit / file that originally deferred it. Tier numbers reflect leverage × effort tradeoff, not strict execution order.

---

## Tier 1 — highest leverage right now

Pick from here first. These are issues whose fix unblocks downstream work or whose investigation has already been substantially done.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#29](https://github.com/mohnkhan/cargonaut/issues/29) | T1.08 — binary-level SIGKILL-resume PTY test (SC-002 gate) | 0.75 ew | Feature 028 — SC-002 currently proven only at transfer-crate level; binary-level gate is `#[ignore]`d. Closes a NON-NEGOTIABLE perf-gate hole (Constitution §IV). |

---

## Tier 2 — near-term, well-scoped features

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#31](https://github.com/mohnkhan/cargonaut/issues/31) | T1.25 — full quick-cd popup w/ tab-completion (FR-012) | 0.25–0.5 ew | Feature 028 — ships as status-bar placeholder; needs shared text-input dialog (also unblocks #32/#33). |
| [#32](https://github.com/mohnkhan/cargonaut/issues/32) | T1.29 — tasks/jobs panel popup (FR-016, NFR-004) | 1.0 ew | Feature 028 — ships as status-bar placeholder; registry data already in `App`, needs list dialog + pause/resume wiring. |

---

## Tier 3 — diagnostics + follow-ups

Useful but not urgent. Pick up when the underlying use case materialises.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| [#30](https://github.com/mohnkhan/cargonaut/issues/30) | T1.07 — PTY end-to-end navigation smoke test | 0.5 ew | Feature 028 — behavior covered by 154 lower-level tests; only the bin-level driver is `#[ignore]`d. |
| [#33](https://github.com/mohnkhan/cargonaut/issues/33) | FR-013 — panel filter prompt dialog (currently clear-only) | 0.25 ew | Feature 022 — `globset` plumbing present; prompt deferred pending shared input dialog (#31). |

---

## Tier 4 — long-term / multi-session features

Substantial multi-session work. Don't start without confirming the scope is still warranted.

| Issue | Title | Effort | Origin |
|---|---|---|---|
| _(empty)_ | | | |

---

## How rows land here

A row appears in this file **only when** a GitHub issue also exists tracking the same work. The rule: deferred work needs both. The issue carries the deep context (problem statement, approach, effort, pointer to where deferral was decided); the ROADMAP row carries the one-line "what tier, what's blocking it" view.

When an issue is closed, delete its ROADMAP row (or move to a `## Closed` section if you want the history visible — but the issue history is the authoritative record).

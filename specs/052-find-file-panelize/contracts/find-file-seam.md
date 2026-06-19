# Contract: Find-File Dialog Seam

**Feature**: 052-find-file-panelize | **Date**: 2026-06-19

This document defines the testable interface contracts for the find-file feature. Every numbered item here maps to at least one red→green test pair in `tasks.md`.

---

## §1 — Keymap binding

```toml
# design/contracts/keymap.toml
[[binding]]
mode = "pane"
key = "M-?"
action = "find-file-popup"  # FR-001 (issue #41)
```

**Contract**: `Keymap::load(DEFAULT_KEYMAP_TOML)` must succeed and `lookup(Mode::Pane, M-?)` must resolve to `Command::FindFilePopup`. No other binding resolves to `M-?`.

---

## §2 — Command variant

```rust
// crates/cargonaut-ui-tui/src/keymap.rs
pub enum Command {
    // ...existing variants...
    /// Open the find-file popup (Alt-?) — FR-001 (issue #41).
    FindFilePopup,
}
```

---

## §3 — SearchMode / DialogPhase truth tables

### 3a. Content-mode availability

| `content_available` | Tab pressed | Outcome |
|---|---|---|
| `true` | any | Mode toggles Name ↔ Content |
| `false` | Name → Content | No-op; notice set to "Content search unavailable: rg not found" |
| `false` | Content → Name | No-op (already Name) |

### 3b. Enter-key dispatch by phase and result count

| `phase` | `results.len()` | Enter pressed | Outcome |
|---|---|---|---|
| `InputFocused` | any | any | Start walk → `Walking` |
| `Walking` | any | any | Ignored (walk in progress) |
| `Walking` | 0 (Done received) | — | Transition → `NoResults` (not `ResultsFocused`) |
| `Walking` | ≥ 1 (Done received) | — | Transition → `ResultsFocused` |
| `ResultsFocused` | 0 | — | Unreachable (invariant; guarded by transition above) |
| `ResultsFocused` | ≥ 1 | pressed | `FindOutcome::Panelize { paths, pattern }` |
| `NoResults` | 0 | pressed | No-op; notice remains |

### 3c. Esc dispatch by phase

| `phase` | Esc pressed | Outcome |
|---|---|---|
| `InputFocused` | any | `FindOutcome::Cancelled` |
| `Walking` | any | cancel walk; `FindOutcome::Cancelled` |
| `ResultsFocused` | any | `FindOutcome::Cancelled` |
| `NoResults` | any | `FindOutcome::Cancelled` |

---

## §4 — Walk result streaming

- `poll_results()` MUST drain all pending `FindEvent` messages from `walk_rx` in a single call (loop until `try_recv()` returns `Empty`).
- After a `FindEvent::Done { truncated }` is received, `walk_rx` is set to `None` and `truncated` field is stored.
- `results.len()` MUST NOT exceed `config.search.max_results` (walk task stops sending `Found` after this count; sends `Done { truncated: true }`).

---

## §5 — Panelize contract

- `FindOutcome::Panelize { paths, pattern }` is only returned when `paths.len() ≥ 1`.
- After panelizing, the active pane's `listing.entries` contains exactly `paths.len()` entries (one per found path).
- `ui.find_label` equals `Some(pattern.clone())`.
- All standard pane operations function on the panelized listing identically to a real directory listing (FR-009): cursor movement, tag (`Space`, `+`, `-`, `*`), copy (F5), move (F6), delete (F8), view (F3), edit (F4).

---

## §6 — find_label lifecycle

| Event | `find_label` state |
|---|---|
| Dialog opens | unchanged |
| Esc cancel (any phase) | unchanged (label NOT cleared — panel is unchanged) |
| Panelize confirmed | `Some(pattern)` |
| `navigate_to(real_dir)` called | `None` (cleared) |
| Another `FindFilePopup` panelized | `Some(new_pattern)` |

---

## §7 — Abort timing

- After `cancel()` is called (Esc during walk), the walk task MUST stop sending new `Found` events within one directory-read cycle (≤300 ms on local FS per SC-006).
- The `abort_flag` (`Arc<AtomicBool>`) is checked by the walk loop at the start of each `read_dir` iteration and before collecting each `DirEntry`.

---

## §8 — Help text

The F1 help overlay MUST include an entry for `M-?` (Alt-?) → `Find file (find-file-popup)` in the Navigation or Search section. Test: help-overlay string contains both `M-?` and `Find`. → T020 (red), T021 (green)

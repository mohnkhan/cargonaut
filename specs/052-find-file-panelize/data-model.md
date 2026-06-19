# Data Model: Find-File and Panelize

**Feature**: 052-find-file-panelize | **Date**: 2026-06-19

---

## Entities

### SearchMode (enum)

Governs which backend drives the walk.

```
SearchMode
├── Name     — filename glob (std::fs + globset)
└── Content  — file-content search (ripgrep --files-with-matches)
```

**Validation**: Only `Content` is valid when `FindFileDialog.content_available = true`. `Tab` toggle is a no-op when `content_available = false`.

---

### DialogPhase (enum)

The state machine phase of the find-file dialog.

```
DialogPhase
├── InputFocused         — user is typing; no walk running
├── Walking              — walk task in progress; results streaming in
├── ResultsFocused       — walk complete, ≥1 result; cursor in result list
└── NoResults            — walk complete, 0 results; notice shown; input refocused
```

**Transitions**:
- `InputFocused` + Enter → `Walking`
- `Walking` + `FindEvent::Done { truncated: false }`, count ≥ 1 → `ResultsFocused`
- `Walking` + `FindEvent::Done { .. }`, count = 0 → `NoResults`
- `NoResults` + any keystroke → `InputFocused` (notice cleared)
- `ResultsFocused` + Enter → `Panelize` outcome (dialog closes)
- Any phase + Esc → dialog closes, walk aborted if running

---

### FindEvent (enum, channel message)

Messages sent from the walk task to the dialog via `mpsc::UnboundedSender`.

```
FindEvent
├── Found(PathBuf)           — one matching file path (absolute)
└── Done { truncated: bool } — walk complete; truncated=true if max_results hit
```

---

### FindFileDialog (struct)

All state for the overlay dialog. Lives in `dialog.rs`.

| Field | Type | Description |
|---|---|---|
| `mode` | `SearchMode` | Active search mode (Name or Content) |
| `input` | `String` | Raw user-typed pattern |
| `phase` | `DialogPhase` | Current state-machine phase |
| `results` | `Vec<PathBuf>` | Accumulated matching paths (relative to search root) |
| `scroll_offset` | `usize` | First visible result row index |
| `truncated` | `bool` | True when results hit `max_results` cap |
| `content_available` | `bool` | True when rg binary resolved at dialog open time |
| `notice` | `Option<String>` | Transient notice text ("No files found…", "Content search unavailable…") |
| `walk_rx` | `Option<mpsc::UnboundedReceiver<FindEvent>>` | Receiver; `None` when not walking |
| `abort_flag` | `Option<Arc<AtomicBool>>` | Shared abort flag for the walk task; `None` when idle |

**Methods** (behaviour contracts):
- `FindFileDialog::new(content_available) -> Self` — constructs with `InputFocused`, empty results, no walk.
- `FindFileDialog::handle_key(key, config) -> FindOutcome` — processes a single key; drives phase transitions; returns `FindOutcome`.
- `FindFileDialog::start_walk(root, config) -> ()` — transitions to `Walking`; spawns task; opens channel.
- `FindFileDialog::poll_results() -> ()` — called each 100ms tick; drains `walk_rx`; transitions to `ResultsFocused` / `NoResults` on `Done`.
- `FindFileDialog::cancel()` — sets abort_flag, drops walk_rx; transitions to `InputFocused`.
- `FindFileDialog::render(f, area, theme)` — draws the overlay.

---

### FindOutcome (enum)

Return value from `FindFileDialog::handle_key`.

```
FindOutcome
├── Consumed                 — key handled; no caller action needed
├── Panelize {               — user pressed Enter on ≥1 results; caller should panelize
│     paths: Vec<PathBuf>,
│     pattern: String,
│   }
└── Cancelled                — Esc pressed; caller should close dialog; panel unchanged
```

---

### SyntheticListing (runtime concept, not a new type)

Not a new struct — a `DirListing` constructed from found paths. Created by the event-loop panelize arm:

1. For each `PathBuf` in `paths`: call `std::fs::metadata(path)` → build `DirEntry { name, kind, metadata }`.
2. Wrap in `DirListing { entries, sort: Sort::None }`.
3. Call `active_pane.set_listing(synthetic_listing)`.
4. Set `ui.find_label = Some(pattern)`.

When `find_label` is `Some(s)`, the status bar renders `[Find: s]` instead of the current directory path.

---

### UiState additions

| Field | Type | Description |
|---|---|---|
| `find_label` | `Option<String>` | Present while a panelized find-result is the active panel's listing. Cleared on any real directory navigation. |

---

### ActiveDialog addition

New variant added to the `ActiveDialog` enum in `lib.rs`:

```rust
/// Feature 052 — Alt-? find-file popup with glob/ripgrep search and panelize.
FindFile {
    widget: dialog::FindFileDialog,
    /// Search root (active panel cwd at the time the dialog was opened).
    root: PathBuf,
}
```

---

## State Transitions (full lifecycle)

```
[Pane mode, user presses Alt-?]
        ↓
  check rg availability
        ↓
  ActiveDialog::FindFile opened, phase = InputFocused
        ↓
  user types pattern, presses Enter
        ↓
  phase = Walking; spawn_blocking walk task; open mpsc channel
        ↓
  each 100ms tick: drain walk_rx → append results, update count
        ↓
  FindEvent::Done received
     ├── count = 0 → phase = NoResults; notice shown
     └── count ≥ 1 → phase = ResultsFocused
              ↓
        user presses Enter
              ↓
        FindOutcome::Panelize { paths, pattern }
              ↓
        build DirListing from paths → active_pane.set_listing(...)
        ui.find_label = Some(pattern)
        active_dialog = None; mode = Pane
```

---

## Invariants

- `results.len() ≤ config.search.max_results` at all times.
- `walk_rx` and `abort_flag` are both `Some` iff `phase == Walking`.
- `scroll_offset < results.len()` always (clamped on results change).
- Panelize is only reachable from `ResultsFocused` phase, guaranteeing `results.len() ≥ 1`.
- `find_label` is cleared by `navigate_to` whenever the active pane loads a real directory listing.

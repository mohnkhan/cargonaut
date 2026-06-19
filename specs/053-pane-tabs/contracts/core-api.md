# Contract: cargonaut-core Public API Changes (Feature 053)

## Stable (unchanged) public API

The following public items are explicitly preserved by FR-009:

```rust
pub enum PaneId { Left, Right }
impl PaneId { pub fn other(self) -> Self }

impl App {
    pub async fn new(config: Config, left: &str, right: &str) -> Result<Self, AppError>
    pub fn active_pane(&self) -> PaneId
    pub fn pane(&self, id: PaneId) -> &PaneState        // now returns active tab
    pub fn active_pane_state(&self) -> &PaneState       // now returns active tab
}
```

No call site outside `cargonaut-core` requires modification. Behaviour is identical: "the visible state for this side".

---

## New public types

```rust
/// View model for one tab entry in the tab bar widget.
pub struct TabBarEntry {
    /// 1-based display index shown in the `[N]` prefix.
    pub index: usize,
    /// Truncated basename of this tab's cwd (max 20 UTF-8 chars).
    pub label: String,
    /// True when this tab is the currently-active (visible) tab.
    pub is_active: bool,
}
```

---

## New public methods on `App`

```rust
impl App {
    /// Produce the tab bar view model for `id`'s side (one entry per tab, in order).
    /// Pure — no I/O. Called once per frame per side.
    pub fn tab_bar_view(&self, id: PaneId) -> Vec<TabBarEntry>
}
```

---

## New `Command` variants

Added to `cargonaut_core::Command`:

```rust
pub enum Command {
    // ... existing variants ...
    /// Ctrl-t — open a new tab on the active side.
    TabNew,
    /// Ctrl-w — close the current tab (no-op if only one remains).
    TabClose,
    /// ] — cycle to the next tab on the active side (wrapping).
    TabNext,
    /// [ — cycle to the previous tab on the active side (wrapping).
    TabPrev,
}
```

---

## Dispatch contract

`App::dispatch(Command::TabNew)` → `Ok(vec![Event::PaneUpdated(active)])` (never errors)  
`App::dispatch(Command::TabClose)` → `Ok(vec![Event::PaneUpdated(active)])` or `Ok(vec![])` if only one tab  
`App::dispatch(Command::TabNext)` → `Ok(vec![Event::PaneUpdated(active)])` (no-op wraps with 1 tab)  
`App::dispatch(Command::TabPrev)` → `Ok(vec![Event::PaneUpdated(active)])` (no-op wraps with 1 tab)

All four are synchronous at the `App` level (no `await`). The TUI dispatches them directly into `App::dispatch`.

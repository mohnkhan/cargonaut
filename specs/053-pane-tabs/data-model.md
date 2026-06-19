# Data Model: Pane Tabs (Feature 053)

## Entities

### `SideState` (new — private to `cargonaut-core`)

A single pane side's complete tabbed state. Replaces the flat `PaneState` slot in `App.panes`.

```rust
// crates/cargonaut-core/src/lib.rs (private)
struct SideState {
    tabs: Vec<PaneState>,   // at least one entry; never empty
    active_tab: usize,      // index into tabs; invariant: < tabs.len()
}
```

**Invariants**:
- `tabs.len() >= 1` at all times (enforced by `tab_close` which is a no-op when `len == 1`)
- `active_tab < tabs.len()` at all times (maintained by `tab_new`, `tab_close`, `tab_next`, `tab_prev`)

**Fields**:
| Field | Type | Notes |
|---|---|---|
| `tabs` | `Vec<PaneState>` | Ordered list of directory tabs; index is 0-based |
| `active_tab` | `usize` | Currently visible tab index |

**Lifetime**: session-only; not persisted.

---

### `TabBarEntry` (new — public, in `cargonaut-core`)

Read-only view model for one tab in the tab bar widget. Built by `App::tab_bar_view(PaneId)` per frame; consumed by the TUI renderer.

```rust
// crates/cargonaut-core/src/lib.rs (public)
pub struct TabBarEntry {
    /// 1-based display index (for the `[N]` prefix).
    pub index: usize,
    /// Truncated basename of this tab's cwd (max 20 chars).
    pub label: String,
    /// True if this tab is currently active on its side.
    pub is_active: bool,
}
```

**Generation**: produced by `App::tab_bar_view(PaneId) -> Vec<TabBarEntry>`. Pure (no I/O). Called once per frame per side.

---

### `App` struct changes

**Before**:
```rust
pub struct App {
    panes: [PaneState; 2],   // private; [0]=left, [1]=right
    active: PaneId,
    ...
}
```

**After**:
```rust
pub struct App {
    sides: [SideState; 2],   // private; [0]=left, [1]=right
    active: PaneId,
    ...
}
```

The `pane_idx` helper function is unchanged. All existing private helpers (`active_pane_mut`, `pane_mut`) now dereference through `sides[idx].tabs[active_tab]`. The public surface (`pane`, `active_pane_state`) is identical.

---

### `Command` additions (cargonaut-core)

Four new variants added to the `Command` enum:

| Variant | Trigger | Behaviour |
|---|---|---|
| `TabNew` | `Ctrl-t` | Open a new tab on the active side in the same cwd |
| `TabClose` | `Ctrl-w` | Close the current tab (no-op if only one tab) |
| `TabNext` | `]` | Cycle to the next tab (wrapping) |
| `TabPrev` | `[` | Cycle to the previous tab (wrapping) |

---

### `Command` additions (cargonaut-ui-tui keymap)

Two new variants added to the `keymap::Command` enum:

| Variant | Action string | Key |
|---|---|---|
| `TabNext` | `"tab-next"` | `]` |
| `TabPrev` | `"tab-prev"` | `[` |

(`NewTab` → `"new-tab"` and `CloseTab` → `"close-tab"` already exist.)

---

## State Transitions

### Tab creation (`tab_new`)

```
Pre:  sides[idx] = SideState { tabs: [A, B, C*], active_tab: 2 }
Post: sides[idx] = SideState { tabs: [A, B, C, D*], active_tab: 3 }
```
where D is a fresh `PaneState` with `cwd = C.cwd`, `listing = C.listing.clone()`, and all other fields at defaults.

### Tab close (`tab_close`)

```
Case 1: closing tab 1 of [0, 1*, 2]
Pre:  active_tab = 1
Post: tabs = [0, 2], active_tab = 1  (former tab 2, now at index 1)

Case 2: closing last tab 2 of [0, 1, 2*]
Pre:  active_tab = 2
Post: tabs = [0, 1], active_tab = 1  (wraps to new last)

Case 3: only 1 tab — no-op
```

### Tab cycle

```
tab_next: active_tab = (active_tab + 1) % tabs.len()
tab_prev: active_tab = (active_tab + n - 1) % tabs.len()
```
where n = `tabs.len()`. With 1 tab: `(0 + 1) % 1 = 0`, `(0 + 0) % 1 = 0` — no visible change, correct.

---

## Validation Rules

- `SideState.tabs` must never be empty (enforced by `tab_close` guard)
- `SideState.active_tab` must always be a valid index (post-mutation bounds check)
- A new tab inherits only `cwd` and `listing` from source; `filter = None`, `selected = BTreeSet::new()`, `cursor = default_cursor()`, `show_hidden = config.ui.show_hidden`, `sort = Sort::NameAsc`, histories empty

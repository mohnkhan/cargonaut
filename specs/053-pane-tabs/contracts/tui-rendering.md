# Contract: TUI Rendering Changes (Feature 053)

## Tab bar widget

The tab bar is a **1-row ratatui `Line`** of `Span`s, rendered at the top of each pane column (above the existing list block).

### Label format per tab

```
[N]basename       (inactive tab — normal style)
[N*]basename      (active tab — highlighted style, e.g. bold or cursor_bg)
```

- `N`: 1-based tab index
- `basename`: `tab.cwd.segments.last()` truncated to min(20, available_width) chars
- Separator: 2 spaces between tabs

### Horizontal scroll (FR-005)

When the total rendered width of all labels exceeds `pane_width`, the bar scrolls so the active tab is always visible. Scroll is computed per-frame (stateless — no persistent offset).

### Style

| Element | Style |
|---|---|
| Inactive tab text | `theme.panel_fg` on `theme.panel_bg` |
| Active tab text | Bold OR `theme.cursor_style()` (TBD at impl) |
| Tab bar background | `theme.panel_bg` |

### Layout change in `draw_pane`

**Before (Feature 052)**:
```
┌─────────────────┐   ← col[0]: Min(2), list + border
│                 │
└─────────────────┘
 mini-status line     ← col[1]: Length(1)
```

**After (Feature 053)**:
```
 [1]src  [2*]docs     ← col[0]: Length(1), tab bar
┌─────────────────┐   ← col[1]: Min(2), list + border (inner rect returned for mouse hit-testing)
│                 │
└─────────────────┘
 mini-status line     ← col[2]: Length(1)
```

The inner rect returned by `draw_pane` changes from `col[0]`'s inner to `col[1]`'s inner. `FrameLayout.left`/`FrameLayout.right` are updated accordingly. Mouse hit-testing code (computing which row was clicked) must be re-verified against the new inner rect.

## `draw_frame` signature extension

```rust
fn draw_frame(
    f: &mut Frame,
    left: &mut PaneView,
    right: &mut PaneView,
    active: PaneId,
    mode: Mode,
    status: &str,
    dialog: Option<&mut ActiveDialog>,
    theme: &Theme,
    menu: &mut MenuBar,
    fkeybar: &FunctionKeyBar,
    ms_left: &str,
    ms_right: &str,
    help_overlay: Option<&HelpOverlay>,
    view_mode: cargonaut_core::ViewMode,
    qv_preview: &str,
    progress: Option<&str>,
    mouse_supported: bool,
    mouse_captured: bool,
    // NEW:
    tab_bar_left: &[cargonaut_core::TabBarEntry],
    tab_bar_right: &[cargonaut_core::TabBarEntry],
) -> FrameLayout
```

## `run_loop` additions

```rust
// Computed before draw closure (app is not in scope inside draw):
let tab_bar_left = app.tab_bar_view(PaneId::Left);
let tab_bar_right = app.tab_bar_view(PaneId::Right);
```

## Keymap additions (cargonaut-ui-tui)

Two new `Command` variants in `keymap.rs`:

```rust
pub enum Command {
    // ... existing ...
    /// Cycle to the next tab on the active side (`]`).
    TabNext,
    /// Cycle to the previous tab on the active side (`[`).
    TabPrev,
}
```

Mapped to core in `ui_command_to_core`:

```rust
U::NewTab    => AppCommand::TabNew,
U::CloseTab  => AppCommand::TabClose,
U::TabNext   => AppCommand::TabNext,
U::TabPrev   => AppCommand::TabPrev,
```

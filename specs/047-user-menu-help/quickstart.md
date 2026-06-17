# Quickstart & Validation Guide: Feature 047

**Feature**: User Menu (F2) + Scrollable Help (F1)
**Date**: 2026-06-18

---

## Prerequisites

```bash
# Ensure the tmpfs symlink is active (SSD preservation)
make tmpfs-status

# Build the project
make build
```

---

## Validation Scenarios

### VS-1: F1 help overlay opens and is scrollable

```bash
# Launch cargonaut
./target/debug/cargonaut

# In the TUI:
# 1. Press F1
# Expected: Full-screen modal overlay appears with a title "Help — Cargonaut"
#           and named sections (Navigation, File Operations, …)
# 2. Press Down arrow several times
# Expected: Content scrolls to reveal more bindings; scroll indicator updates
# 3. Press Page Down
# Expected: Scrolls a full page
# 4. Press Home
# Expected: Returns to top (scroll_offset = 0)
# 5. Press Esc (or F1 again)
# Expected: Overlay closes; cursor position, active pane, tags unchanged
```

### VS-2: F1 overlay swallows non-navigation keys

```bash
# In the TUI with F1 overlay open:
# Press 'j' (would normally move cursor)
# Expected: Overlay stays open; no cursor movement in the underlying pane
# Press Enter
# Expected: Overlay stays open; no directory descent
```

### VS-3: F2 with no menu.toml shows placeholder

```bash
# Ensure ~/.config/cargonaut/menu.toml does not exist
rm -f ~/.config/cargonaut/menu.toml

# In the TUI: press F2
# Expected: Modal menu opens showing one row:
#   "No actions defined — see docs for menu.toml"
# Press Esc → overlay closes; app continues normally
```

### VS-4: F2 with a valid menu.toml shows user actions

```bash
# Create a test menu.toml
mkdir -p ~/.config/cargonaut
cat > ~/.config/cargonaut/menu.toml << 'EOF'
[[actions]]
label   = "Echo path"
command = "echo {path}"
key     = "e"

[[actions]]
label   = "List directory"
command = "ls -la {path}"
only_if = "test -d {path}"
key     = "l"
EOF

# In the TUI: navigate to a file; press F2
# Expected: Menu shows "Echo path" (always visible) and "List directory" (only for dirs)
# Select "Echo path" with Enter
# Expected: echo runs; status bar shows "Done." after completion
```

### VS-5: {path} with special characters is safe

```bash
# Create a file with spaces and quotes
touch '/tmp/my file "test".txt'

# In the TUI: navigate to /tmp; highlight 'my file "test".txt'; press F2
# Select "Echo path"
# Expected: The echo output shows the full path correctly (no mangled quoting)
```

### VS-6: F2 with TOML parse error shows error

```bash
cat > ~/.config/cargonaut/menu.toml << 'EOF'
[[actions]
label = "broken"
EOF

# In the TUI: press F2
# Expected: Menu overlay opens with error message (not a crash);
#           the error includes a filename and line number
# Press Esc → app continues normally
```

### VS-7: F2 is blocked while another dialog is open

```bash
# In the TUI: press F7 (mkdir dialog opens)
# While the mkdir dialog is open, press F2
# Expected: F2 is ignored; mkdir dialog remains open; no crash
```

### VS-8: only_if condition hides/shows actions

```bash
cat > ~/.config/cargonaut/menu.toml << 'EOF'
[[actions]]
label   = "Dirs only"
command = "ls {path}"
only_if = "test -d {path}"

[[actions]]
label   = "Files only"
command = "wc -l {path}"
only_if = "test -f {path}"
EOF

# Highlight a directory → press F2 → "Dirs only" shows; "Files only" hidden
# Highlight a file → press F2 → "Files only" shows; "Dirs only" hidden
```

### VS-9: Single-char shortcut key executes immediately

```bash
# With the "Echo path" action having key = "e"
# In the TUI: press F2 to open menu; press 'e' without navigating
# Expected: "Echo path" action executes immediately
```

---

## Automated Test Suite Validation

```bash
# Run all tests (includes new unit tests for this feature)
make test

# Run only the UI crate tests
cargo test -p cargonaut-ui-tui

# Verify help coverage CI assertion (SC-002)
# The test `help_covers_all_keymap_bindings` in cargonaut-ui-tui
# parses keymap.toml and asserts every action appears in HELP_SECTIONS
cargo test -p cargonaut-ui-tui help_covers_all_keymap_bindings

# Verify binary size (SC-007)
make build
scripts/check-binary-size.sh
```

---

## References

- [menu.toml schema](contracts/menu-toml-schema.md)
- [data model](data-model.md)
- [spec](spec.md)
- [design/contracts/keymap.toml](../../design/contracts/keymap.toml)

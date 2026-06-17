# Quickstart: External Theme (Skin) Files

## Prerequisites

- `cargo build --release` completed (or `make build`)
- A working `cargonaut` binary at `target/release/cargonaut`

## Scenario 1 — Apply a custom skin (US1, SC-001)

```sh
# 1. Create the skin directory
mkdir -p ~/.config/cargonaut/themes

# 2. Write a minimal 3-color skin
cat > ~/.config/cargonaut/themes/my-theme.toml << 'EOF'
panel_bg  = "Red"
cursor_bg = "#00ff00"
exec_fg   = 201
EOF

# 3. Set the theme in config
mkdir -p ~/.config/cargonaut
echo '[ui]' >> ~/.config/cargonaut/config.toml
echo 'theme = "my-theme"' >> ~/.config/cargonaut/config.toml

# 4. Launch the app
./target/release/cargonaut ~/src ~/dst

# Expected: panel background is red, cursor is green, executables are
# palette color 201 (bright magenta). No status message shown.
```

## Scenario 2 — CLI override (US1 AC3)

```sh
./target/release/cargonaut --theme my-theme ~/src ~/dst
# Expected: same as Scenario 1, config file need not exist.
```

## Scenario 3 — Partial skin inherits default (US2)

```sh
cat > ~/.config/cargonaut/themes/green-cursor.toml << 'EOF'
cursor_bg = "Green"
cursor_fg = "Black"
EOF

./target/release/cargonaut --theme green-cursor ~/src ~/dst
# Expected: cursor bar is green/black; all other colors are commander-dark defaults.
```

## Scenario 4 — Invalid color falls back gracefully (US3)

```sh
cat > ~/.config/cargonaut/themes/broken.toml << 'EOF'
panel_bg = "Bleu"
EOF

./target/release/cargonaut --theme broken ~/src ~/dst
# Expected: app starts with commander-dark defaults.
# Status bar shows: Skin "broken": field panel_bg: unknown color "Bleu"
```

## Scenario 5 — Unknown field name (US3 AC2)

```sh
cat > ~/.config/cargonaut/themes/wrong-key.toml << 'EOF'
frobnicate = "Blue"
EOF

./target/release/cargonaut --theme wrong-key ~/src ~/dst
# Expected: falls back to commander-dark, status shows unknown field error.
```

## Scenario 6 — Missing skin file (US1 AC2)

```sh
./target/release/cargonaut --theme nonexistent ~/src ~/dst
# Expected: falls back to commander-dark.
# Status bar shows: Unknown theme "nonexistent" — using commander-dark
```

## Scenario 7 — All three color formats (US4)

```sh
cat > ~/.config/cargonaut/themes/all-formats.toml << 'EOF'
panel_bg  = "Blue"      # named
exec_fg   = 196         # 256-color index
cursor_bg = "#ff8800"   # RGB hex
EOF

./target/release/cargonaut --theme all-formats ~/src ~/dst
# Expected: panel = Blue, executables = color 196 (bright red),
# cursor = #ff8800 (orange). App starts without errors.
```

## Automated Test Equivalent

The unit tests in `crates/cargonaut-ui-tui/src/theme.rs` cover all scenarios
programmatically (no running binary required):

```sh
CARGONAUT_ALLOW_SSD_TARGET=1 cargo test -p cargonaut-ui-tui skin -- --nocapture
# or
make test  # runs the full workspace
```

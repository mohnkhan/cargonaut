# Quickstart Validation Guide: Feature 054 — Persistent Subshell (Ctrl-o)

**Branch**: `054-persistent-subshell`

---

## Prerequisites

```bash
# Ensure tmpfs is set up (SSD preservation)
make tmpfs-status    # must show symlink active

# Build in release mode (binary-size check later)
make build           # or: CARGONAUT_ALLOW_SSD_TARGET=1 cargo build

# Verify tests pass from the base (before any feature code)
make test
```

---

## Build Gate (after implementation)

```bash
# Full CI pipeline — must be green before pushing
make ci-local

# Binary size check (must be ≤ 8 MiB stripped)
scripts/check-binary-size.sh
```

---

## Scenario 1: Three-state Ctrl-o cycle (US1 / FR-002)

```bash
./target/debug/cargonaut ~/ /tmp
```

1. Press `Ctrl-o` **once** — subshell panel appears in the lower ~33% of the screen. File manager panes visible above. Arrow keys still move the panel cursor. Status: `VisibleFmFocus`.
2. Press `Ctrl-o` **again** — keyboard focus transfers to the shell. A shell prompt is visible. Arrow keys now move the shell cursor (not the panel). Status: `VisibleShellFocus`.
3. Type `pwd` + Enter — output appears inside the subshell panel; the directory matches the active file-manager panel.
4. Press `Ctrl-o` **again** — subshell panel hides. File manager returns to full height. Status: `Hidden`.
5. Press `Ctrl-o` — panel reappears with the same shell session (history intact; `pwd` still shows the last directory).

**Expected**: Each `Ctrl-o` advances exactly one state. No flicker. No layout jump. Shell session preserved across hide/show.

---

## Scenario 2: cwd-sync on panel navigation (US2 / FR-007)

```bash
./target/debug/cargonaut ~/ /tmp
```

1. Press `Ctrl-o` twice (enter shell-focus mode). Type `pwd` + Enter — note the directory.
2. Press `Ctrl-o` (hide panel). Navigate the left pane into `~/Documents` (press Enter on the directory row).
3. Press `Ctrl-o` (show panel) + `Ctrl-o` (enter shell). Type `pwd` + Enter.

**Expected**: Shell `pwd` output shows `~/Documents` (or its absolute equivalent).

4. Press `Ctrl-o` (return to FM). Press `Tab` to switch focus to the right pane (which shows `/tmp`).
5. Press `Ctrl-o` + `Ctrl-o`. Type `pwd` + Enter.

**Expected**: Shell `pwd` shows `/tmp`.

---

## Scenario 3: Tab-switch cwd-sync (US2 / FR-007 + F053)

```bash
./target/debug/cargonaut ~/ /tmp
```

1. Press `Ctrl-t` to open a second tab on the left pane; navigate it to `/var/log`.
2. Press `Ctrl-o` + `Ctrl-o` (enter shell). Type `pwd`.

**Expected**: `/var/log` (the active tab's cwd).

3. Press `Ctrl-o`. Press `[` to switch left pane back to tab 1 (pointing at `~/`). Press `Ctrl-o` + `Ctrl-o`. Type `pwd`.

**Expected**: `~` (the now-active tab's cwd).

---

## Scenario 4: Full VT100 emulation — cursor-addressing programs (FR-005 / SC-005)

1. Press `Ctrl-o` twice. Type `less /etc/hostname` + Enter.

**Expected**: `less` renders the file with full pager UI; `q` quits back to shell prompt.

2. Type `top` + Enter.

**Expected**: `top` renders its full-screen interactive display within the subshell panel; `q` exits.

3. Resize the terminal window while `top` is running.

**Expected**: `top` redraws for the new dimensions (SIGWINCH delivered; PTY and VT100 parser resized).

---

## Scenario 5: Shell exit and restart (FR-009)

1. Press `Ctrl-o` twice (enter shell). Type `exit` + Enter.

**Expected**: Panel shows "Shell exited — press Ctrl-o to restart" (or equivalent notice). No crash. `VisibleShellFocus` state returns to `Hidden` automatically.

2. Press `Ctrl-o`.

**Expected**: Fresh shell spawns in the active panel's current directory.

---

## Scenario 6: Modal blocks Ctrl-o (FR-012)

1. Press `F5` on a file to open the copy dialog.
2. While the dialog is showing, press `Ctrl-o`.

**Expected**: `Ctrl-o` does not toggle or modify the subshell state. Dialog remains active.

---

## Scenario 7: Minimum terminal size guard (FR-002 / R-011)

```bash
# Resize terminal to fewer than 8 usable rows (e.g., 7 rows total)
# Then:
```

1. Press `Ctrl-o`.

**Expected**: Panel does NOT open. Status bar shows "Terminal too small for subshell" (or equivalent).

---

## Automated Test Coverage

```bash
# Unit + integration tests (must all pass)
cargo test --workspace

# Specific Feature 054 tests
cargo test -p cargonaut-ui-tui subshell
cargo test -p cargonaut-config subshell_height_pct

# Keypress latency bench (NFR-002: must stay ≤ 16 ms)
cargo bench -p cargonaut-ui-tui --bench keypress_latency

# RSS headroom bench (NFR: must stay ≤ 64 MiB)
cargo bench -p cargonaut-ui-tui --bench rss_headroom
```

---

## Regression Check

```bash
# All existing features must work identically with and without the subshell visible
# Spot-check: copy a file with subshell panel open
# Expected: F5 copy dialog works; copy completes; panel stays visible throughout
```

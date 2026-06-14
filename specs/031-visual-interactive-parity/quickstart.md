# Quickstart & Validation: Visual & Interactive Parity Layer

How to build, run, and validate each user story. Implementation details live in tasks.md; this is the run/validation guide.

## Build & run (tmpfs-guarded — constitution §V)

```bash
make build            # cargo build behind check-tmpfs
make run              # or: ./target/debug/cargonaut <LEFT> <RIGHT>
cargo run -p cargonaut-bin -- ~/  /tmp
```

## Automated validation

```bash
make test             # cargo test --workspace (unit + integration + TestBackend render tests)
make ci-local         # full pipeline: clippy -D warnings → test → release build → docs-gate → binary-size
```

Targeted suites:

```bash
cargo test -p cargonaut-ui-tui theme::       # T-THEME-*
cargo test -p cargonaut-ui-tui mouse         # T-MOUSE-*
cargo test -p cargonaut-ui-tui chrome        # menu/fkey bar render + hit-test
cargo test -p cargonaut-core   sort          # sort cycle / reverse
cargo test -p cargonaut-core   pattern       # select/unselect by glob
cargo test -p cargonaut-core   mkdir         # mkdir round-trip (LocalFs + TempDir)
```

## Manual validation by user story

### US1 — Looks like the reference manager (P1)
1. `cargo run -p cargonaut-bin -- ~/ /tmp` → panels render with a blue background; directories/executables/symlinks/hidden each a distinct color; cursor row is a colored bar; tagged rows (Insert/Ctrl-T) are in the marked color.
2. `cargo run -p cargonaut-bin -- --theme monochrome ~/ /tmp` → palette changes.
3. `cargo run -p cargonaut-bin -- --theme nope ~/ /tmp` → starts on the default theme + a non-fatal notice (does not crash). **Validates SC-001/002/008, FR-001..007.**

### US2 — Chrome (P1)
1. Confirm a top menu bar and a bottom `1Help 2Menu 3View 4Edit 5Copy 6RenMov 7Mkdir 8Delete 9PullDn 10Quit` bar are visible.
2. Press F9 → a menu opens; select an item → its command runs.
3. Move the cursor → each panel's mini-status updates with name/size/mtime/perms.
4. Shrink the terminal → labels abbreviate, no panic. **Validates SC-001/004, FR-008..012.**

### US3 — Mouse (P1, default on)
1. Single-click a row in each panel → focus follows, cursor moves.
2. Double-click a directory → descend; double-click a file → opens via its action.
3. Scroll wheel → listing scrolls.
4. Click an F-key button and a menu title → actions fire.
5. `--no-mouse` (or `mouse=false` in config) → keyboard-only behavior, native text selection works. **Validates SC-003/004, FR-013..018.**

### US4 — Richer listing (P2)
1. Listing shows mtime + perms columns and a `..` first row; activate `..` → ascends; at `/` no ascent past root.
2. Press C-s repeatedly → sort order cycles; reverse toggle inverts.
3. Press M-t → cycles brief → full → quick-view; in quick-view the passive panel previews the highlighted text file; a binary file shows a placeholder.
4. Cursor on a directory, press C-Space → recursive size appears; UI stays responsive. **Validates SC-006, FR-019..023.**

### US5 — Operation parity (P2)
1. F7 → enter a name → directory created and listed; invalid name/permission error reported, no crash.
2. `+` `*.rs` → matching files tagged; `-` `*.rs` → untagged; a no-match pattern reports zero matches.
3. Copy a large file (F5) → a progress dialog shows current item, progress, throughput, ETA; cancel stops it; on completion the dialog dismisses and the target panel refreshes.
4. F3 on a text file → external pager opens and returns cleanly; F4 → `$EDITOR` opens, edits persist, panel refreshes. **Validates SC-005/007, FR-024..027, FR-030/031.**

## Regression gates (must stay green)

- All pre-existing unit + integration tests pass (SC-010).
- `keypress-latency` bench within NFR-002 budget; `check-binary-size.sh` ≤ 8 MiB (NFR-001).
- No existing keybinding changes behavior (FR-028).

## Deferral check (SC-009)

Before PR: every item in spec.md "Out of Scope" has a GitHub issue + a ROADMAP.md row (CLAUDE.md deferral policy).

# Quickstart: Internal F4 Editor Validation (Feature 056)

## Prerequisites

- `make tmpfs-status` shows `target/` is a tmpfs symlink (or CI=true)
- `cargo test --workspace` passes on main before switching to this branch
- A terminal at least 80×24

## Scenario 1 — Edit and Save (US1 core)

1. `make run` in one terminal
2. Navigate to a writable directory containing a plain-text file (e.g. `/tmp/test.txt` — create with `echo "hello" > /tmp/test.txt`)
3. Navigate the pane cursor to `test.txt`
4. Press **F4** → editor opens full-screen showing `hello` at line 1, col 1
5. Press **End** → cursor moves to end of line
6. Type ` world` → line reads `hello world`; modified indicator `*` appears in header
7. Press **F2** → file is saved; `*` clears
8. Press **F10** → editor closes; file manager pane is refreshed
9. In another terminal: `cat /tmp/test.txt` → output is `hello world`

**Pass criteria**: file content on disk matches what was typed; no crash; pane is active after close.

---

## Scenario 2 — Unsaved-changes guard (US2)

1. Open `/tmp/test.txt` in the editor (F4)
2. Type any character → `*` appears
3. Press **F10** → unsaved-changes dialog appears: "Save / Discard / Cancel"
4. Press **Tab** once → focus moves to Discard
5. Press **Enter** → editor closes without saving; file on disk is unchanged

Then repeat steps 1–3, press **Esc** → dialog dismisses; back to editing.

**Pass criteria**: `cat /tmp/test.txt` still shows the pre-edit content after Discard.

---

## Scenario 3 — Binary file decline (US3)

1. Navigate to a binary file (e.g. `ls -la /usr/bin/ls` to find it, navigate to `/usr/bin`)
2. Move cursor to `ls`
3. Press **F4** → editor does NOT open; status bar reads "Cannot edit: binary file"

**Pass criteria**: file manager is still active; no editor screen shown.

---

## Scenario 4 — Large file decline (US3)

1. `dd if=/dev/urandom of=/tmp/big.txt bs=1M count=11` (create an 11 MiB text-ish file)
   - Actually: `yes "line" | head -n 2200000 > /tmp/bigtext.txt` (~11 MiB)
2. Navigate pane to `/tmp/bigtext.txt`
3. Press **F4** → editor does NOT open; status bar reads "Cannot edit: file too large (>10 MiB)"

**Pass criteria**: no editor opened; file manager remains active.

---

## Automated Test Equivalents (CI)

The following unit tests in `crates/cargonaut-ui-tui/src/dialog.rs` cover the above scenarios automatically:

| Manual Scenario | Automated Test |
|-----------------|----------------|
| S1 type + save | `editor_insert_and_save_writes_correct_content` |
| S2 unsaved guard | `editor_unsaved_changes_guard_triggered_on_quit` |
| S3 binary decline | `open_file_editor_declines_binary` |
| S4 large file decline | `open_file_editor_declines_too_large` |

Run with: `cargo test -p cargonaut-ui-tui editor`

---

## Regression Gate

After implementation, run `make ci-local`. All of the following must pass:

- `cargo clippy --workspace --all-targets -- -D warnings`
- `cargo test --workspace`
- `cargo build --release`
- `docs-gate` (both `README.md` and `Learnings.md` updated)

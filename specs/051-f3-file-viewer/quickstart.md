# Quickstart: Internal File Viewer F3

**Feature**: 051-f3-file-viewer | **Date**: 2026-06-19

This guide validates the viewer end-to-end without reading implementation code. Run these scenarios after a successful `make build`.

---

## Prerequisites

```bash
make tmpfs-status     # Confirm target/ → tmpfs (required by constitution §V)
make build            # Build the release binary
./dist/cargonaut --version  # or: cargo run --release -- --version
```

---

## Scenario 1 — Open a text file in text mode (US1)

```bash
# Create a test file with known content and line count
seq 1 500 | sed 's/.*/Line &: The quick brown fox/' > /tmp/test-text.txt
wc -l /tmp/test-text.txt   # Should print 500
```

1. Launch `cargonaut` and navigate to `/tmp/`.
2. Highlight `test-text.txt`; press **F3**.
3. **Expected**: Full-screen overlay opens. Title bar shows `F3 View — test-text.txt  [text]`. Line numbers visible in left margin. Status shows `Line 1/500`.
4. Press **Down** 5 times → status shows `Line 6/500`.
5. Press **Page Down** → jumps ~viewport height; status updates.
6. Press **Home** → back to `Line 1/500`.
7. Press **q** → overlay closes; pane cursor still on `test-text.txt`.

---

## Scenario 2 — Hex mode on a binary file (US2)

```bash
ls -la /proc/self/exe   # Should resolve to the cargonaut binary
```

1. Navigate to the directory containing the `cargonaut` binary.
2. Highlight it; press **F3**.
3. **Expected**: Viewer opens in hex mode automatically (binary not valid UTF-8). Title shows `[hex]`. First row: `00000000  7f 45 4c 46 …  |.ELF…|`.
4. Press **Down** → offset advances by 16 bytes.
5. Press **Ctrl-x X** → switches to text mode (content will look garbled — that is expected for binary).
6. Press **Ctrl-x X** again → back to hex mode.
7. Press **G** → jumps to last row; offset near file end.
8. Press **Esc** → closes viewer.

---

## Scenario 3 — Search (US3)

```bash
# Use test-text.txt from Scenario 1
grep -n "quick" /tmp/test-text.txt   # Should show all 500 lines
grep -n "Line 42" /tmp/test-text.txt # Should show line 42 specifically
```

1. Open `test-text.txt` with F3.
2. Press **/** → search prompt appears at the bottom: `Search: _`.
3. Type `Line 42` → press **Enter**.
4. **Expected**: View jumps to line 42; `Line 42` is highlighted in the content. Status shows match info.
5. Press **n** → advances to line 142 (if the pattern appears again; in this case it won't, so "Pattern not found" repeats the message).
6. Press **?** → backward search prompt. Type `Line 4` → Enter → finds the nearest match above.
7. Press **Esc** → closes prompt; highlights cleared.

---

## Scenario 4 — Goto (US4)

1. Open `test-text.txt` with F3.
2. Press **g** → goto prompt: `Go to line: _`.
3. Type `250` → **Enter** → view jumps to line 250; status shows `Line 250/500`.
4. Type `g` again → type `999` → **Enter** → clamped to 500 (last line).
5. Press **G** → jumps to last line (500).
6. Press **Home** → back to line 1.

---

## Scenario 5 — Large file streaming (US5)

```bash
# Generate a file larger than 10 MiB
yes "$(seq 1 100 | tr '\n' ' ')" | head -c 15000000 > /tmp/test-large.txt
wc -c /tmp/test-large.txt   # Should be ~15 MB
wc -l /tmp/test-large.txt   # Line count
```

1. Launch cargonaut, navigate to `/tmp/`, press **F3** on `test-large.txt`.
2. **Expected**: Viewer opens quickly (within 150 ms visible), shows first screenful. Status shows line number and estimated total.
3. Press **Page Down** repeatedly; viewer continues loading content. No crash.
4. Press **g** → type a line number near the end of the file → **Enter**. Viewer scrolls there.
5. Press **/**, type a known pattern → **Enter**. If the file is streaming, status shows `(searched 10 MiB of 15 MiB)` or similar annotation.
6. Monitor memory: `ps aux | grep cargonaut`. RSS should stay well below 128 MiB.

---

## Scenario 6 — Enter-on-file shortcut

1. Navigate to a directory with text files.
2. Highlight a file; press **Enter** (not F3).
3. **Expected**: Same viewer opens as with F3.
4. Highlight a directory; press **Enter**. **Expected**: navigates into the directory (unchanged behavior).

---

## Scenario 7 — Word-wrap toggle

```bash
python3 -c "print('A' * 300)" > /tmp/long-line.txt   # One very long line
```

1. Open `long-line.txt` with F3.
2. **Expected**: Long line truncated at right edge (no-wrap default). Status shows `wrap: off`.
3. Press **w** → line wraps at terminal width. Status shows `wrap: on`.
4. Press **w** again → truncation restored.

---

## Scenario 8 — ANSI escape stripping

```bash
printf '\033[31mRed text\033[0m\nNormal text\n' > /tmp/ansi-test.txt
```

1. Open `ansi-test.txt` with F3.
2. **Expected**: First line shows `Red text` in plain text (not red, no escape codes visible). No terminal corruption.

---

## Scenario 9 — Edge cases

```bash
touch /tmp/empty-file.txt            # Empty file
echo -n "no newline at end" > /tmp/no-newline.txt
```

- F3 on empty file: shows `(empty file)` message, no crash.
- F3 on a directory: status bar shows "Not a file", viewer does not open.
- F3 while another dialog (e.g., F5 copy confirm) is open: keypress swallowed, viewer does not open.

---

## CI Gate Validation

```bash
make ci-local   # Runs clippy → test → build → check-pr-body → docs-gate
```

Specific benchmarks:
```bash
cargo bench --bench viewer_open        # SC-001: must show ≤150 ms p50
cargo bench --bench keypress_latency   # SC-002: viewer scenario must be ≤16 ms
cargo bench --bench rss_headroom       # SC-003: ≤64 MiB with large file open
```

Binary size check:
```bash
bash scripts/check-binary-size.sh     # SC-004: ≤8 MiB stripped
```

Test count delta:
```bash
cargo test --workspace 2>&1 | grep "test result" | awk '{sum += $4} END {print sum " tests"}'
# Should show ≥ (449 + 30) = 479 tests
```

# Quickstart — Validating Feature 061 (Survivability, Crash Safety & About)

Run from repo root. All builds go through `make` (Constitution §V tmpfs guard).
Confirm `make tmpfs-status` shows the link active first.

## Prerequisites

- Build: `make build` (debug) / `make build-release`.
- The crash/restore integration test is **gated**: it runs only with
  `CARGONAUT_PTY_TESTS=1` (CI sets this).
- Fault injection: set `CARGONAUT_PANIC_INJECT=<site>` (`startup|render|input|task`).

## 1. Unit-test the pure diagnostics (fast, no terminal)

```bash
cargo test -p cargonaut-core diag
```

Expected: the ring buffer drops oldest past 64; `format_crash_report` is
deterministic and contains version/platform/location/backtrace headings;
`prune_reports` keeps the newest 10; `unseen_report`/`mark_seen` fire the notice
exactly once; the SC-008 sentinel-secret test finds no secret in the report;
`about_lines()` contains version, author, copyright, and `MIT OR Apache-2.0`.

## 2. Terminal survives a fatal crash (SC-001, SC-002)

```bash
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test crash_safety
```

Expected: the test spawns the real binary in a PTY with
`CARGONAUT_PANIC_INJECT=render` (or `input`), drives one keystroke, and after the
process dies asserts:
- the PTY is back in cooked mode (no manual `reset` needed) — **SC-001**;
- a `crash-*.log` exists in a temp data dir containing the version, platform,
  panic location, and a backtrace — **SC-002**;
- the restored terminal shows a line naming the crash-report path (**FR-006**).

Manual smoke (optional):

```bash
CARGONAUT_PANIC_INJECT=render ./target/release/cargonaut
# press any key → app exits; your shell prompt is normal (not garbled);
# last line points to ~/.local/share/cargonaut/crash-<ts>.log
cat ~/.local/share/cargonaut/crash-*.log | tail -n +1   # inspect the report
```

## 3. Session survives a recoverable fault (SC-003, SC-004)

```bash
cargo test -p cargonaut-ui-tui recover
cargo test -p cargonaut-transfer task_panic_isolated
```

Expected: with a one-shot injected render/input panic, the run-loop test shows the
loop continues and the status line carries a "recovered from internal error"
message (**SC-003**); a panicking transfer task resolves to a `Failed` job while
the rest of the app stays usable (**SC-004**). No `crash-*.log` is written for
recovered faults.

## 4. Next-launch crash notice (SC-009)

```bash
cargo test -p cargonaut-core unseen_report_fires_once
```

Expected: with a crash report present and no marker, the first check returns the
report (notice shown, marker written); the second check returns `None` (no repeat).

## 5. About / version surface (SC-005)

```bash
./target/release/cargonaut --version            # long output incl. © + license
cargo test -p cargonaut-core about_lines
cargo test -p cargonaut-ui-tui about            # help section + About dialog
```

Manual: launch the app, press **F1** → "About" section shows version/author/
copyright/license; open the menu → "About" entry opens the About dialog with the
same details.

## 6. Binary-size gate (SC-007)

```bash
make build-release && bash scripts/check-binary-size.sh
```

Expected: stripped release binary ≤ 8 MiB after switching to `panic = "unwind"`.

## 7. Full local CI

```bash
make ci-local        # fmt + clippy -D warnings + tests + release build + gates
CARGONAUT_PTY_TESTS=1 cargo test --workspace --lib --tests   # incl. gated crash test
```

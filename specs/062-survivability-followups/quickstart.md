# Quickstart — Validating Feature 062

Builds via `make` (Constitution §V). Reuses Feature 061 seams.

## Input recovery (SC-001)
```bash
cargo test -p cargonaut-ui-tui recover          # run-loop test: injected input panic → loop continues, status set
```
Manual: `CARGONAUT_PANIC_INJECT=input ./target/release/cargonaut` then press keys
→ status shows "recovered from internal input error"; app stays interactive; after
3 it exits cleanly with a crash report.

## Transfer task → Failed (SC-002)
```bash
cargo test -p cargonaut-transfer task_panic     # panicking task → job state Failed; registry usable
```

## About view (SC-003)
```bash
cargo test -p cargonaut-ui-tui about_dialog     # open via Command::ShowAbout, content = about_lines(), Esc/Enter closes
```
Manual: launch → open the menu → "About" → modal shows version/author/copyright/
license; Esc closes.

## Regression + size (SC-004)
```bash
make build-release && bash scripts/check-binary-size.sh        # ≤ 8 MiB
CARGONAUT_PTY_TESTS=1 cargo test --workspace --lib --tests     # incl. Feature 061 crash test
make ci-local
```

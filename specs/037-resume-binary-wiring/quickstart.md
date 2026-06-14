# Quickstart: Resume-from-Interrupted-Transfer

## Run the everyday test suite (gated PTY test skipped)

```bash
make test            # cargo test --workspace --lib --tests — fast; SC-002 PTY test NOT run
```

The expensive end-to-end resume test is opt-in, so routine runs stay quick.

## Run the SC-002 end-to-end resume test (the gate)

```bash
# Unix only. Destination temp files land in $TMPDIR (tmpfs on the dev host).
CARGONAUT_PTY_TESTS=1 cargo test -p cargonaut --test resume_sigkill -- --nocapture
```

What it does, fully automated:
1. Builds the `cargonaut` binary (located via `CARGO_BIN_EXE_cargonaut`).
2. Creates a ~128 MiB deterministic source file in a temp dir; uses a second temp dir as the
   destination pane.
3. Spawns the binary under a PTY with a 1 MiB checkpoint interval and a throughput throttle
   (`CARGONAUT_TRANSFER_THROTTLE_MIBPS`) so the copy lasts several seconds.
4. Sends `F5` then confirms, starting the copy.
5. Polls the destination dir until a `.cargonaut-transfer-*.json` sidecar exists and the
   partial destination is non-empty but smaller than the source, then **SIGKILLs** the run.
6. Relaunches the binary against the same destination, detects the resume prompt, sends `r`.
7. Waits for completion (destination size == source size, sidecar removed).
8. Asserts `sha256(source) == sha256(destination)` and that the resumed run re-copied no more
   than one checkpoint interval beyond the pre-kill offset (SC-002).

The test self-skips (passes trivially) when `CARGONAUT_PTY_TESTS` is unset.

## CI

`.github/workflows/ci.yml` sets `CARGONAUT_PTY_TESTS=1` on the `cargo test` step, so the gate
runs on every PR. CI is exempt from the tmpfs guard (`$CI=true`).

## Manual smoke (no test harness)

```bash
# 1. Make a large file in dir A; use dir B as the other pane.
mkdir -p /tmp/A /tmp/B && head -c 1G </dev/urandom >/tmp/A/big.bin

# 2. Launch with a small checkpoint interval via a throwaway config (or defaults) and throttle.
CARGONAUT_TRANSFER_THROTTLE_MIBPS=32 cargo run -p cargonaut -- /tmp/A /tmp/B
#    Focus the left pane, press F5, confirm — the copy starts.

# 3. In another terminal, kill it mid-copy:
pkill -KILL cargonaut

# 4. Relaunch against the same dirs:
cargo run -p cargonaut -- /tmp/A /tmp/B
#    A resume prompt appears listing the unfinished transfer. Press r.

# 5. After completion, hashes must match:
sha256sum /tmp/A/big.bin /tmp/B/big.bin
```

Try `s` (start over — recopies from scratch) and `c`/Esc (skip — the prompt reappears on the
next launch) to exercise the other two choices.

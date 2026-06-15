# Quickstart: Panel Filter Prompt Dialog

How to exercise issue #33 once implemented.

## Build & run

```bash
make build           # tmpfs-guarded debug build
make run             # or: ./target/debug/cargonaut <dir> <dir>
```

## Manual walkthrough

1. **Set a glob filter**
   - Focus a pane on a directory with mixed files (e.g. the repo root).
   - Press `Alt-!`. A "Filter" prompt opens, empty.
   - Type `*.rs` and press `Enter`.
   - Expect: only `.rs` entries remain; cursor sits on the first one. The other pane is
     unchanged.

2. **Bare-word (substring) filter**
   - Press `Alt-!`, type `car`, press `Enter`.
   - Expect: every entry whose name contains `car` (case-insensitive) is shown — e.g.
     `cargonaut`, `Cargo.toml`.

3. **Edit an existing filter**
   - With a filter active, press `Alt-!`.
   - Expect: the prompt opens **prefilled** with the current pattern. Edit and re-submit.

4. **Clear the filter**
   - Press `Alt-!`, delete the text to empty, press `Enter`.
   - Expect: the full listing returns; status shows the filter cleared.

5. **Invalid pattern**
   - Press `Alt-!`, type `[` (an unterminated class), press `Enter`.
   - Expect: the prompt stays open with an inline error; the listing is unchanged. Edit the
     text and the error clears.

6. **Cancel**
   - Press `Alt-!`, type anything, press `Esc`.
   - Expect: the prompt closes; the pane's filter is exactly what it was before.

7. **Persistence across navigation**
   - With a filter active, descend into a subdirectory and back.
   - Expect: the filter is still applied (it persists until cleared).

## Automated checks

```bash
make test            # full workspace test suite (set / clear / invalid / persistence)
make ci-local        # fmt + clippy -D warnings + test + release build + docs/binary gates
scripts/check-binary-size.sh   # NFR-001 ≤8 MiB gate (globset is the only new dep)
```

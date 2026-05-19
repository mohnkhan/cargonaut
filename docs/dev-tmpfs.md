# Tmpfs build redirection — protect your SSD

> **TL;DR**: `make tmpfs-setup` → `target/` becomes a symlink into a per-checkout subdirectory of `/tmp/cargonaut/`, so incremental `cargo build` writes hit RAM instead of the SSD. Reversible with `make tmpfs-teardown`. **Mandatory for single-user dev boxes per CLAUDE.md.** No-op on CI.

## Why this exists

This is a single-user dev machine where the SSD has finite write-life. A normal cargo dev cycle hammers it:

| Path | Size | Write pattern |
|---|---|---|
| `target/` | ~300 MB–2 GB depending on workspace | Cargo rewrites large chunks on every incremental build |

`cargo build` doesn't just append — it rewrites incremental metadata, recompiles dependent crates, and re-links binaries on every change. Over months of heavy iteration that's tens of GB written per day per project. Redirecting `target/` to a tmpfs (RAM-backed) filesystem is a meaningful win on SSD lifetime.

The pattern is borrowed from the sibling MyOS2026 project where it cut SSD writes by ~3 GB/day during heavy iteration.

## What it does

```text
Before                        After  `make tmpfs-setup`
──────                        ───────────────────────────
target/   (real, on SSD)      target/  → /tmp/cargonaut/<hash>/target/
```

The `<hash>` is a 12-char SHA-256 prefix of the absolute repo root path, so two checkouts of cargonaut get separate tmpfs subdirectories and never fight over each other's build artifacts.

Existing build, test, and CI tooling works unchanged — they reference `target/` by relative path, and the symlink is transparent to all standard tools (Cargo, rust-analyzer, IDE indexers, rsync).

## What it does NOT touch

- **`crates/` and `src/`** — that's source, on disk under git
- **`Cargo.toml` / `Cargo.lock`** — tracked by git
- **anything else tracked by git** — only fully-gitignored output trees move
- **CI runners** — `tmpfs-setup` short-circuits when `$CI=true`

## Three commands

```sh
make tmpfs-setup       # one-time per checkout: create the tmpfs subdir, migrate
                       # any existing target/ contents in, replace with a symlink.
                       # Idempotent — re-running is safe.
make tmpfs-status      # show current state: which paths are linked, where, how
                       # much disk usage in tmpfs, free /tmp space
make tmpfs-teardown    # remove the symlink; recreate empty real directory
                       # (build artifacts in tmpfs are kept for fast re-setup)
make tmpfs-teardown WIPE=1   # same, plus rm -rf the tmpfs subdirectory
```

## Interaction with `make clean`

`make clean` is symlink-aware. When `target/` is a tmpfs symlink, `make clean` empties its **contents** (in tmpfs) but leaves the symlink intact. The tmpfs association survives `make clean`. When it's a real directory, `make clean` runs `cargo clean` as expected.

## Trade-offs to know about

### `/tmp` is tmpfs → wiped on reboot

After a reboot, the next `cargo build` starts from scratch (~1–3 min cold for a typical Rust workspace). The symlink is still there, but the directory it points to is gone. Re-running `make tmpfs-setup` after a reboot recreates the tmpfs subdir (the symlink already exists) — the next `cargo build` rebuilds.

If you want the tmpfs subdir to auto-recreate on every shell start, add to your shell startup:
```sh
alias cargonaut-tmpfs='cd /path/to/cargonaut && make tmpfs-setup'
```

### `/tmp` size is RAM-bounded

`tmpfs` typically gets half your physical RAM by default. On a 16 GB machine that's 8 GB; the working set is ~1–2 GB so there's plenty of headroom. If you're on a 4 GB machine, check `df -h /tmp` before adopting this.

### Multiple checkouts

Two checkouts of cargonaut each get their own subdirectory because the path is hashed. No collision risk, no extra setup needed.

### CI runners

`tmpfs-setup` checks `$CI` and short-circuits if it's `true`. CI runners' `/tmp` is disk-backed and ephemeral; the tmpfs trick doesn't help there. Mandatory for dev; no-op for CI.

### What if something I care about ends up in `/tmp`?

`target/` is fully ignored by git (no tracked files inside it). Everything in `target/` is regenerable from sources in seconds-to-minutes. There is nothing irreplaceable in there.

If you build a release binary you want to keep across reboots, copy it out before rebooting:
```sh
cp target/release/cargonaut ~/keepers/cargonaut-$(date +%F)
```

## Verifying the win

Before:
```text
$ du -sh target/
850M    target/
```

After `make tmpfs-setup`:
```text
$ make tmpfs-status
[tmpfs-status] tmpfs root: /tmp/cargonaut/46e66e2e2310

  [link]  target                 → /tmp/cargonaut/46e66e2e2310/target  (850M)

  Filesystem      Size  Used Avail Use% Mounted on
  tmpfs            16G  1.0G   15G   7% /tmp
```

To confirm writes are actually going to RAM (not crossing the SSD), watch `iostat -d 1` on your SSD device while running `cargo build` — the write rate should stay near zero.

## When to NOT use this

- **Limited-RAM machines** (< 4 GB) where tmpfs can't afford a 1–2 GB build cache
- **Workflows that need build artifacts to survive reboots** (rare; `cargo build` is fast)
- **CI / batch automation** — already covered by the `$CI` check
- **Debugging build determinism** — if you suspect a build is producing different output across runs and you want to compare, having the artifacts go to a stable on-disk location is easier to reason about. Run `make tmpfs-teardown` for the duration.

## Where this lives in the tree

```
Makefile                         # tmpfs-setup, tmpfs-status, tmpfs-teardown targets
scripts/tmpfs-setup.sh           # idempotent migration + symlink creation
scripts/tmpfs-status.sh          # read-only inspection
scripts/tmpfs-teardown.sh        # remove symlink; optional WIPE
.git/info/exclude                # local-only; symlinks added so `git status` stays clean
                                 # (the shared .gitignore is intentionally NOT modified)
```

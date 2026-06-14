# Building & running Cargonaut

Everything you need to build, run, test, and hack on Cargonaut. For the *why*
and the project story, see the [README](../README.md); for architecture, see
[`architecture.md`](./architecture.md) and the design tunnel under
[`../design/`](../design/INDEX.md).

## Requirements

- A recent stable Rust toolchain (`rustup` recommended) — the workspace pins its
  MSRV via `rust-version` in `Cargo.toml`.
- A POSIX terminal. Linux is the primary target; `crossterm` keeps it portable.

## Quick start

```bash
git clone https://github.com/mohnkhan/cargonaut
cd cargonaut
cargo build --release
./target/release/cargonaut ~ /tmp        # left pane, right pane
```

Defaults to `$HOME` (left) and `/tmp` (right) if no paths are given.

Useful flags:

| Flag | Effect |
|---|---|
| `--theme <name>` | Pick a built-in theme (`commander-dark` default, `monochrome`) |
| `--no-mouse` | Disable mouse capture (mouse is on by default) |
| `--config <path>` | Use an alternate config file |
| `-v` | Debug logging to stderr |

### Keys & mouse (today)

`j`/`k`/arrows move · `Enter` enter dir · `Backspace`/`..` ascend · `Tab` switch pane ·
`Insert` tag · `+`/`-` tag by glob · `F3` view (`$PAGER`) · `F4` edit (`$EDITOR`) ·
`F5` copy · `F6` move · `F7` mkdir · `F8` delete · `F9` menu · `F10` quit ·
`Ctrl-s` cycle sort · `M-t` cycle view (brief/full/quick-view).
Mouse (default on): click to focus + move cursor, double-click to enter, wheel to
scroll, click the menu titles / function-key buttons.

### cd-on-exit shell integration (FR-017)

Source one of the `contrib/` wrappers so the shell `cd`s into the pane you left:

```bash
# bash / zsh
source /path/to/cargonaut/contrib/cargonaut.sh
# fish
source /path/to/cargonaut/contrib/cargonaut.fish
```

The wrapper passes `$CARGONAUT_EXIT_CWD_FILE`; on graceful exit the binary writes
the active pane's cwd there and the wrapper `cd`s into it. Running cargonaut
without the wrapper still works — the shell just stays put.

## Workspace layout

```
crates/
  cargonaut-bin/       binary entrypoint (clap CLI + main)
  cargonaut-core/      app state, command dispatch, view modes, progress projection
  cargonaut-ui-tui/    ratatui rendering: theme, chrome (menu/F-key/mini-status),
                       panes, dialogs, mouse + keymap event loop
  cargonaut-vfs/       VFS trait + LocalFs (Phase 1)
  cargonaut-transfer/  resumable copy/move engine (CRC checkpoints + SHA verify)
  cargonaut-config/    config schema + figment loader
design/                planning tunnel: spec, plan, tasks, contracts, wireframes
specs/                 per-feature spec-kit artifacts (spec/plan/tasks/...)
```

## Make targets

All build/test targets run `check-tmpfs` first (Constitution §V; auto-skipped on CI).

| Target | What it does |
|---|---|
| `make build` / `make build-release` | `cargo build [--release] --workspace` |
| `make run ARGS='~ /tmp'` | run the binary with args |
| `make test` | `cargo test --workspace --lib --tests` |
| `make bench` | `cargo bench --workspace` |
| `make clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `make fmt` / `make fmt-check` | format / check formatting |
| `make clean` | symlink-aware clean (preserves the tmpfs association) |
| `make ci-local` | the full CI pipeline locally (see below) |
| `make tmpfs-setup` / `tmpfs-status` / `tmpfs-teardown` | SSD-preservation helpers |

Run `make help` for the authoritative list.

## SSD preservation (Constitution §V — dev host only)

On the dev box, `target/` MUST be a symlink into tmpfs so heavy Cargo iteration
doesn't wear the SSD. This discipline came from the author's
[ReduceSSDWrites](https://github.com/mohnkhan/ReduceSSDWrites) work and is now a
NON-NEGOTIABLE constitutional principle.

```bash
make tmpfs-setup     # once per checkout: target/ -> /tmp/cargonaut/<hash>/target/
make tmpfs-status    # audit
```

`make check-tmpfs` (a prereq of build/test/bench/clippy) errors loudly if `target/`
is a real on-SSD directory. CI is exempt via `$CI=true`; a per-session waiver
`CARGONAUT_ALLOW_SSD_TARGET=1` requires a documented `Learnings.md` entry. Full
guide: [`dev-tmpfs.md`](./dev-tmpfs.md) and constitution
[`§V`](../.specify/memory/constitution.md).

## CI pipeline

The GitHub `ci` check (required by branch protection on `main`) runs, in order:

```
clippy (-D warnings) → cargo test --workspace --lib --tests
  → cargo build --release → check-pr-body → docs-gate → binary-size
```

`make ci-local` mirrors it (same flags) — run it before pushing (~3–5 min).
Failed runs upload a `ci-failure-*.zip` artifact to the PR's Checks tab.

## Acceptance gates

Phase-1 success criteria, each enforced by a bench or test:

| Gate | Where verified |
|---|---|
| SC-001: local↔local copy ≥ 80% of `cp(1)` | `cargo bench -p cargonaut-transfer --bench local_copy_vs_cp` (release) |
| SC-002: resume after SIGKILL within one checkpoint; SHA-256 match | `crates/cargonaut-bin/tests/resume_sigkill.rs` |
| SC-003: RSS ≤ 64 MiB | `cargo bench -p cargonaut-core --bench rss_headroom` (Linux) |
| SC-004: cold launch ≤ 150 ms | `cargo bench -p cargonaut-core --bench startup` |
| NFR-001: ≤ 8 MiB stripped binary | `scripts/check-binary-size.sh` (CI-gated) |
| NFR-002: ≤ 16 ms keypress→paint | `cargo bench -p cargonaut-ui-tui --bench keypress_latency` |

Full SC + NFR matrix: [`design/contracts/requirements.toml`](../design/contracts/requirements.toml).

## Delivery phases

| Phase | Goal | Gates |
|---|---|---|
| 1 | Prototype + core: dual-pane local + resumable copy + panel ergonomics + visual/interactive parity | SC-001..004 |
| 2 | VFS + transfer adapters (SFTP, S3, archive) | + SC-005 |
| 3 | Plugins + internal viewer/editor + power features (mask-rename, panelize, user-menu, openers, hex-view, fuzzy, zoxide) | + SC-006 |
| 4 | Terminal emulator + undo + audit + compare-dirs | + SC-007/008 |
| 5 | UX polish + theming + l10n + a11y | usability test |
| 6 | Security hardening + perf tuning + migration tooling | + SC-009/010 |

Forward-looking, issue-backed detail lives in [`../ROADMAP.md`](../ROADMAP.md).

## Where to start contributing

1. Read [`design/spec.md`](../design/spec.md) — vision + user stories + functional requirements.
2. Skim [`design/milestones.md`](../design/milestones.md) — the phased delivery plan.
3. Pick a task from [`design/tasks.md`](../design/tasks.md) or an open
   [`follow-up` issue](https://github.com/mohnkhan/cargonaut/issues?q=label%3Afollow-up),
   write tests first (Constitution Principle II), implement, open a PR.

Conventions (branch-per-change, docs discipline, commit rules) are in
[`../CONTRIBUTING.md`](../CONTRIBUTING.md) and [`../CLAUDE.md`](../CLAUDE.md).

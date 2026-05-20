# Cargonaut

> Rust-native, terminal, keyboard-first dual-pane file manager — Midnight Commander reimagined for 2026.

**Status**: Phase 1 — runnable dual-pane TUI (`cargonaut <LEFT> <RIGHT>`). VFS + transfer engine + config + keymap + dialogs all merged; T1.07/T1.08 integration tests + benches + MC-parity features still to come.

## At a Glance

| Target | Goal | Result |
|--------|------|--------|
| Cold launch | < 150 ms | _pending impl_ |
| Local-local copy throughput | ≥ 80% of `cp(1)` | _pending impl_ |
| Resident memory | ≤ 64 MiB | _pending impl_ |
| Unit tests | All pass | **144/144** (15 VfsPath + 10 Config + 4 dyn-dispatch + 35 LocalFs + 23 transfer + 16 keymap + 11 PaneView + 13 dialogs + 17 App) |
| Clippy | `-D warnings` clean | **green** (workspace, `--all-targets`) |
| CI pipeline | `make ci-local` green | lint + build + unit-test **green**; docs-gate per-PR |

Update this table on every feature merge (per [CLAUDE.md](./CLAUDE.md) Documentation discipline).

## Feature History

Most recent first.

- **Feature 022 — T1.26 + T1.27 + T1.28: MC-parity panel ergonomics** (2026-05-20). Five new commands batched: `TogglePanelFilter` (FR-013 — clear-only for Phase 1; prompt dialog deferred), `SyncOtherPanelPath` + `ShowFocusedInOtherPanel` (FR-014), `ToggleSplitOrientation` (FR-015; new `SplitOrient` enum on `App`). All async-mapped via `ui_command_to_core`. 5 new tokio tests in `cargonaut-core`. Branch `021-t1.26-27-28-mc-parity` → PR #22.

- **Feature 021 — T1.30: exit-cwd writer + bash/fish wrappers** (2026-05-20). `bin/main.rs` writes the active pane's cwd to `$CARGONAUT_EXIT_CWD_FILE` on graceful exit; `contrib/cargonaut.sh` (bash/zsh) and `contrib/cargonaut.fish` shell functions wrap the binary to cd into that path on exit (FR-017). `ui_tui::run` signature changed to `&mut App` so the caller retains ownership for the post-exit cwd read. Branch `020-t1.30-exit-cwd` → PR #21.

- **Feature 020 — T1.22a: binary-size CI gate (NFR-001)** (2026-05-20). `scripts/check-binary-size.sh` strips the release binary and fails if > 8 MiB. Wired into the `build` job in `.github/workflows/ci.yml`. Current local measure: **1.91 MiB**, well within ceiling. Branch `019-t1.22a-binary-size` → PR #20.

- **Feature 018 — T1.21: binary main + event loop** (2026-05-20). `crates/cargonaut-bin/src/main.rs` becomes runnable: clap CLI (positional `LEFT`/`RIGHT` paths, `--config`, `--theme`, `--mc-keys`, `--enable-plugin`, `--a11y-output`, `-v`), `Config::load(--config)` or default, `App::new(config, left, right).await`, then `cargonaut_ui_tui::run(app).await`. Subcommands `list-plugins` / `audit` / `resume` stub future phases. The event loop (now in `cargonaut-ui-tui/src/lib.rs::run`) drives `tokio::select!` over `crossterm::EventStream` + `tokio::signal::ctrl_c` + a 100ms redraw tick; routes keys through the keymap (with multi-chord `Pending` state), maps `keymap::Command` → `core::Command`, dispatches into `App`, handles `DialogRequested` via `ConfirmDialog`, calls `App::confirm_copy` on Confirm. Terminal teardown (raw-mode off + leave alternate-screen + show cursor) always runs even on error. Branch `018-t1.21-bin-and-event-loop` → PR #N.

- **Feature 017 — T1.19: App event loop core** (2026-05-20). `crates/cargonaut-core/src/lib.rs` adds `App` (config + 2× `PaneState` + active-pane id + transfer registry + status). `PaneState` holds the pure data (cwd, listing, cursor, selected, show_hidden, filter) — ratatui-free so ui-tui can render from `&PaneState` per frame without circular deps. `Command` enum, `Event` enum, `DialogKind` for modal requests. `dispatch(Command).await -> Result<Vec<Event>, _>` covers cursor/focus/selection/toggle-hidden and async ops (descend/ascend via VFS list, copy via `submit_transfer` after `confirm_copy`). Destructive ops (`Copy/Move/Delete`) emit `DialogRequested` rather than acting directly. `CancelCurrentTransfer` triggers the latest transfer's `CancellationToken`. 12 tokio tests via `TempDir`+`LocalFs`. The full `tokio::select!` event loop (input ↔ transfer progress polling) is T1.21's job. Branch `017-t1.19-app` → PR #N.

- **Feature 016 — T1.20: dialogs (confirm + resume-prompt)** (2026-05-20). `crates/cargonaut-ui-tui/src/dialog.rs` adds `ConfirmDialog` (modal yes/no with Cancel-as-default-focus per FR-005 destructive-op safety; handles Esc/Enter/Tab/y/n) and `ResumePromptDialog` (list of resumable transfers, per-row `[r]esume/[s]tart over/[c]ancel` driven by `ResumableSummary` derived from `cargonaut_transfer::ResumableTransfer`). Both render via `Clear` (modal overlay) + `Block` + ratatui widgets. 13 tests cover focus default, key shortcuts, navigation, dismiss outcomes, and `TestBackend` rendering. Branch `016-t1.20-dialogs` → PR #N.

- **Feature 015 — T1.17: PaneView widget** (2026-05-20). `crates/cargonaut-ui-tui/src/pane.rs` wraps ratatui's `List` + `ListState` to render a `DirListing` with cursor (highlight-reversed row), selection (`*` prefix), hidden-file masking (FR-015 `Alt-.`), and a substring filter (placeholder for FR-013's globset in T1.26). Cursor moves track the *visible* subset (filter + hidden-masked) so the filter + cursor interact correctly. Virtual scrolling falls out of ratatui's stateful widget. 11 tests cover cursor bounds, selection toggle, hidden-file filter, substring filter, set_listing reset, and rendering via `TestBackend` (including a 10000-entry stress that scrolls cursor to row 5000). Branch `015-t1.17-paneview` → PR #N.

- **Feature 014 — T1.18: keymap parser** (2026-05-20). `crates/cargonaut-ui-tui/src/keymap.rs` parses `design/contracts/keymap.toml` (60+ bindings, 6 modes) into a `Keymap` indexed by `(Mode, KeySequence)` → `Command` (60-variant enum). `parse_key_sequence` handles single chords (`F10`, `M-1`, `j`) and multi-key chords (`C-x !`, `C-x C-d` — required by FR-205/208/209/305). `lookup_sequence` returns three-state `SeqLookup::{Command, Pending, NoMatch}` for the dispatcher's wait-for-next-key state machine. 16 tests cover full default-keymap parse, named specials, modifier prefixes, multi-chord prefix/match/no-match, and user-override merge semantics. Branch `014-t1.18-keymap` → PR #N.

- **Feature 013 — T1.15: implement resume_transfer** (2026-05-20). New `resume_transfer(src, dst, checkpoint, opts)` in `crates/cargonaut-transfer/src/job.rs` that defensively re-verifies the destination CRC chain (fast-fails before spawning if invalid), preserves the checkpoint's job_id (audit-log correlation), opens streams at `bytes_written` (`src.read_stream(ByteRange{start, end:None})` + `dst.write_stream(offset, AppendAtOffset)`), and runs `run_transfer_with_state` — a sibling of `submit_transfer`'s `run_transfer` that starts with the existing `chunk_crcs` chain pre-loaded and continues the loop. 5 tokio tests cover successful completion, CRC-corrupted dst rejection, bytes_written > src_size rejection, job_id preservation, and "first Running.bytes_done > checkpoint.bytes_written". Branch `013-t1.15-resume-transfer` → PR #N.

- **Feature 012 — T1.14: implement scan_resumable** (2026-05-20). Moves `scan_resumable` from `job.rs` to `crates/cargonaut-transfer/src/checkpoint.rs` (where the task spec places it) and implements it: lists dst dir via `VfsBackend::list`, filters `.cargonaut-transfer-*.json` entries, parses each as `TransferCheckpoint`, then pre-validates both halves of the resume contract — re-computes the source SHA-256 prefix (Phase 1 only resolves `file://` sources) and re-walks the destination CRC chain at the recorded chunk size. Sidecars with wrong schema versions / parse failures / read failures are silently skipped (per-sidecar errors don't block siblings). 9 tokio tests cover empty dir, unrelated files, happy path, src-modified, dst-corrupt (CRC mismatch), dst-truncated, wrong schema version, malformed JSON, multiple checkpoints. Branch `012-t1.14-scan-resumable` → PR #N.

- **Feature 011 — T1.13: implement submit_transfer over VfsBackend** (2026-05-20). Implements the resumable copy loop in `crates/cargonaut-transfer/src/job.rs`: stats source up-front (immediate caller feedback on NotFound), spawns a tokio task that streams `opts.buffer_size_bytes` chunks from src to dst, accumulates pending bytes and drains them as `checkpoint_interval_bytes` chunks (CRC32 each + JSON sidecar rewrite + flush), emits `Running` state on every chunk read (with throughput + ETA), honors `CancellationToken` between iterations (leaves sidecar in place — `Canceled` is resumable), and on EOF optionally re-reads both sides to verify full SHA-256 before deleting the sidecar. 5 tokio integration tests via `LocalFs` + `TempDir`. Branch `011-t1.13-submit-transfer` → PR #N.

- **Feature 010 — T1.09: TransferCheckpoint roundtrip property test** (2026-05-20). Proptest in `crates/cargonaut-transfer/src/checkpoint.rs::tests` covers `TransferCheckpoint` serialize/deserialize round-trip for both compact and pretty-printed JSON forms (operators occasionally `vim` checkpoint sidecars per FR-006). `pub const VERSION = 1` + a unit test guard catch silent schema bumps. 4 new tests; total workspace now 68/68. Branch `010-t1.09-checkpoint-roundtrip` → PR #N.

- **Feature 002 — T1.06: implement LocalFs over tokio::fs** (2026-05-20). Lands the concrete `LocalFs: VfsBackend` (red + green commits): 35 per-method tempdir tests across `list` / `stat` / `read_stream` / `write_stream` / `unlink` / `rmdir` / `rename` / `mkdir`. Bridges tokio's `AsyncRead`/`Write` to the trait's `futures::` return types via `tokio_util::compat` (the `compat` cargo feature). Symlink-correct (`stat`/`list`/`unlink` all use `symlink_metadata` so symlinks stay reported as `Symlink`, never silently followed). `read_stream` clamps past-EOF starts to file size (yields at-EOF reader per spec, not error). `write_stream::AppendAtOffset` opens without create/truncate and seeks; file must exist (resume contract). `rename` rejects cross-authority moves with `Unsupported`. Branch `002-localfs-vfs-backend` → PR #6.

- **Feature 004 — T1.05: VfsBackend trait docs + dyn-dispatch smoke test** (2026-05-20). Expands every `VfsBackend` method's docs with semantics + invariants + error contract (`crates/cargonaut-vfs/src/traits.rs`); adds `tests/dyn_dispatch.rs` pinning trait object-safety + `Send + Sync` bounds at compile time + `Arc<dyn VfsBackend>` construction at runtime, guarding the load-bearing `VfsRef` invariant against accidental regression. Introduces `ByteRange::FULL` for the whole-file invariant. Branch `004-t1.05-vfsbackend-docs` → PR #5.

- **Feature 009 — T1.16: cargonaut-config schema expansion + figment loader** (2026-05-20). Expands the `Config` struct surface to full coverage of `design/contracts/config.schema.json` (new fields across `ui` / `transfer` / `plugins` / `credentials` / `audit`; new top-level `remote.sftp` / `remote.s3` / `search` sections; new enums `ZoxideMode` / `OnCancel` / `CredentialsBackend` / `ListingMode` / `PatternType`). Implements `Config::load` (XDG path → TOML → `CARGONAUT_*` env), `load_from_path` / `load_from_str` (pure TOML), `load_from_str_with_env` (opt-in env layer), and `json_schema_pretty` (schemars-derived). All structs gain `#[serde(default, deny_unknown_fields)]` for partial-TOML support + typo rejection. 10 tests cover defaults, round-trip, partial-TOML, env override, unknown-field rejection, schema generation, and the `ZoxideMode` custom `oneOf: [bool, "auto"]` serde shape. Branch `009-t1.16-config-loader` → PR #N.

- **Feature 003 — Phase 1 foundational: scaffold compile fix + T1.04 VfsPath types** (2026-05-20). Fixed the initial scaffold's compile errors (missing `bitflags` workspace dep, mis-located `Sort` re-export, oversized `VfsKind::Symlink` variant, derivable Default impls flagged by clippy) and lands the `VfsPath` / `VfsMetadata` / `DirListing` / `VfsKind` / `VfsCaps` types per T1.04 (red + green commits). Proptest covers the parse/display round-trip across schemes/authorities/segment counts; concrete tests cover edge cases (root paths, rejected `..`/empty/trailing-slash). Branch `003-pre-session` → PR #3.

- **Feature 001 — dev-culture-bootstrap** (2026-05-20). Transferred development conventions from sibling MyOS2026: `.specify/` + `.claude/` (spec-kit slash commands), `CLAUDE.md` (workflow rules), `CONTRIBUTING.md` + `CODE_OF_CONDUCT.md`, `.github/workflows/ci.yml` (cargo-shaped CI rollup), CI scripts (`check-pr-body.sh`, `docs-gate.sh`, `ci-local.sh`), `Makefile` with cargo wrappers + tmpfs targets, `ROADMAP.md` + `Learnings.md` skeletons. Added SSD-preservation tmpfs discipline (`make tmpfs-setup` redirects `target/` → `/tmp/cargonaut/<hash>/target/`) as mandatory dev-machine convention. Branch `001-dev-culture-bootstrap` → PR #2 → merged.


## Origins

Cargonaut was designed through a single comprehensive spec-kit-shaped planning
pass (specify → clarify → plan → research → tasks → analyze, run three times
to convergence). The full design tunnel — vision, 45 functional requirements,
10 success criteria, 8 non-functional requirements, MC-feature gap analysis,
phased delivery plan, machine-readable contracts, ASCII wireframes, MC
migration table — lives under [`design/`](./design/). Start with
[`design/INDEX.md`](./design/INDEX.md).

## Quick start (Phase 1 stub)

```bash
git clone https://github.com/mohnkhan/cargonaut
cd cargonaut
cargo build --release
./target/release/cargonaut --help
```

The Phase 1 binary launches a dual-pane TUI for the two given paths
(defaulting to `$HOME` and `/tmp`). `F10` quits; basic navigation
(`j`/`k`/`Enter`/`Backspace`/`Tab`) works.

### cd-on-exit (FR-017)

Source one of the `contrib/` shell wrappers to have cargonaut cd into
the active pane's directory when you quit:

```bash
# bash / zsh
source /path/to/cargonaut/contrib/cargonaut.sh

# fish
source /path/to/cargonaut/contrib/cargonaut.fish
```

The wrapper passes `$CARGONAUT_EXIT_CWD_FILE` through the environment;
on graceful exit the binary writes the active pane's cwd there and the
wrapper `cd`s into it. Running cargonaut without the wrapper still
works — the shell just stays in your launch directory.

## Workspace layout

```
crates/
  cargonaut-bin/       binary entrypoint (CLI + main)
  cargonaut-core/      app state, command dispatch, event loop
  cargonaut-ui-tui/    ratatui rendering
  cargonaut-vfs/       VFS trait + LocalFs (Phase 1)
  cargonaut-transfer/  resumable copy/move engine
  cargonaut-config/    config schema + figment loader
design/                planning tunnel: spec, plan, tasks, contracts, ...
```

## Phase 1 acceptance gates

| Gate | Where verified |
|---|---|
| SC-001: local-local copy ≥ 80% of `cp(1)` | `benches/local-copy-vs-cp.rs` |
| SC-002: resume after SIGKILL within 8 MiB; SHA-256 match | `tests/integration/resume_sigkill.rs` |
| SC-003: RSS ≤ 64 MiB | `benches/rss-headroom.rs` |
| SC-004: cold launch ≤ 150 ms | `benches/startup.rs` |

Full SC + NFR matrix in [`design/contracts/requirements.toml`](./design/contracts/requirements.toml).

## Where to start contributing

1. Read [`design/spec.md`](./design/spec.md) — vision + 6 user stories + 45 FRs.
2. Read [`design/milestones.md`](./design/milestones.md) — 6-phase delivery plan, ~90 eng-weeks for a 4-engineer team (~6.5 months calendar).
3. Read [`design/tasks.md`](./design/tasks.md) — Phase 1-3 task backlog (78 tasks, all timeboxed and traceable to FRs).
4. Pick a `[ ] T1.NN` task, claim it in your tracker, write tests first (Constitution Principle II), implement, PR.

## Phases at a glance

| Phase | Goal | Eng-weeks | Gates |
|---|---|---|---|
| 1 | Prototype + Core (dual-pane local + resumable copy + MC-parity panel ergonomics) | 16.45 | SC-001..004 |
| 2 | VFS + Transfer adapters (SFTP, S3, archive) | 14.75 | + SC-005 |
| 3 | Plugins + Preview/Editor + MC-killer features (mask-rename, panelize, user-menu, openers, bulk-rename, hex-view, fuzzy, zoxide) | 22.0 | + SC-006 |
| 4 | Terminal emulator + undo + audit + compare-dirs | 15.5 | + SC-007/008 |
| 5 | UX polish + theming + l10n + a11y + menu-bar + listing-modes | 11.5 | usability test |
| 6 | Security hardening + perf tuning + MC migration | 10 | + SC-009/010 |

## Design discipline carried over

The design tunnel was produced inside the MyOS2026 spec-kit workflow; we
adopted its four constitutional principles verbatim (with one scoped
reinterpretation noted in [`design/plan.md`](./design/plan.md) §"Constitution
Check"):

1. **Code Quality** — clippy `-D warnings`, missing-docs on every public crate, peer review required
2. **Test-First** (NON-NEGOTIABLE) — failing test SHA committed before any implementation merge
3. **UX Consistency** — keymap centralized; theme variables typed; FR-403 plain-text event stream for screen readers
4. **Performance** — SC-001/003/004 enforced by criterion benches in CI; >10% regression blocks merge

## License

Dual-licensed under MIT OR Apache-2.0 — pick whichever fits your project.

See [`LICENSE-MIT`](./LICENSE-MIT) and [`LICENSE-APACHE`](./LICENSE-APACHE).

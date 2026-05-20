---
description: "Phase 1-3 task backlog for Cargonaut. Phases 4-6 sketched at end."
---

# Tasks: Cargonaut (Phases 1-3 detailed; 4-6 high-level)

**Input**: Design documents in `design/`.

**Prerequisites**: spec.md, plan.md, research.md, data-model.md, contracts/, milestones.md, tests-plan.md.

**Team assumption**: 3-5 Rust engineers. Each task is owner-week-sized (one engineer × one focused week ≈ 30 h productive). Tasks marked `[P]` can run in parallel.

## Format: `[ID] [P?] [Story?] Description with file path`

---

## Phase 1: Prototype + Core (16.45 owner-weeks)

### Setup (Phase 1)

- [ ] T1.01  Initialize cargo workspace at `cargonaut/Cargo.toml` with members: cargonaut-bin, cargonaut-core, cargonaut-ui-tui, cargonaut-vfs, cargonaut-transfer, cargonaut-config. Shared deps in `[workspace.dependencies]`. MSRV pin `rust-version = "1.76"`.  **(0.5 owner-week)**
- [ ] T1.02 [P] Add `.github/workflows/ci.yml` skeleton (lint + test). Wire `cargo fmt --check`, `cargo clippy -D warnings`, `cargo test --workspace`.  **(0.25)**
- [ ] T1.03 [P] Initial README.md + CONTRIBUTING.md + LICENSE (MIT OR Apache-2.0).  **(0.25)**

### Foundational (Phase 1)

- [X] T1.04 [US1] Define `VfsPath`, `VfsMetadata`, `DirListing`, `VfsKind`, `VfsCaps` types in `cargonaut-vfs/src/types.rs`. Round-trip property test for `VfsPath::display(parse(s))`.  **(0.5)**
- [X] T1.05 [US1] Define `VfsBackend` trait in `cargonaut-vfs/src/traits.rs`. Async-trait. Doc every method with semantics + invariants.  **(0.25)**
- [X] T1.06 [US1] Implement `LocalFs: VfsBackend` in `cargonaut-vfs/src/local.rs` using `tokio::fs`. **TDD: write the per-method tempdir tests in `local.rs::tests` first and confirm failing before implementing each method body** (Principle II). Unit tests cover: list (empty, populated, large), stat (file/dir/symlink/missing), read_stream (range + whole), write_stream (truncate/append/create-new), unlink, rename.  **(1.0)**

### US1 — Two-pane local navigation + resumable copy (P1 MVP — 7 tasks, 8.5 owner-weeks)

#### Tests for US1 (write first, confirm failing per Constitution Principle II)

- [ ] T1.07 [US1] Integration test `tests/integration/local_navigation.rs`: launch cargonaut with two tempdir args, simulate keypresses (Tab, j, Enter, Backspace, :cd), assert pane state evolves correctly. Use `crossterm-test` or similar input-injection helper.  **(0.5)**
- [ ] T1.08 [US1] Integration test `tests/integration/resume_sigkill.rs`: create 4 GiB random file, spawn cargonaut subprocess to F5 copy, wait 1 s, SIGKILL, relaunch, automate the `[r]esume` prompt, wait for completion, assert SHA-256 match. **SC-002 gate.**  **(0.75)**
- [X] T1.09 [US1] Property test `cargonaut-transfer/src/checkpoint.rs::tests::roundtrip`: random `TransferCheckpoint` serializes + deserializes equal.  **(0.25)**
- [ ] T1.10 [US1] Bench `benches/local-copy-vs-cp.rs`: criterion bench comparing cargonaut's copy throughput to `cp(1)` for 100 MiB, 1 GiB. **SC-001 gate (≥80%).**  **(0.5)**
- [ ] T1.11 [US1] Bench `benches/startup.rs`: hyperfine wrapper measuring cold + warm startup. **SC-004 gate (≤150 ms cold).**  **(0.25)**
- [ ] T1.12 [US1] Bench `benches/rss-headroom.rs`: spawn cargonaut, drive it into 3 panes × 10k-entry-each session, sample RSS. **SC-003 gate (≤64 MiB).**  **(0.25)**

- [ ] T1.12b [US1] Integration test `tests/integration/cancellation.rs`: start a 1 GiB local-to-local copy, send Ctrl-c (or SIGINT) at offset N, assert: (a) the transfer task observes cancellation within 500 ms (wall-clock); (b) per `[transfer] on_cancel = "delete" | "keep"` config, the partial destination is either removed or remains with a valid `.cargonaut-transfer-*.json` checkpoint; (c) no tokio task survives 1 s after cancel (verified via `tokio-metrics` or task-count probe). **Covers FR-008 + NFR-005.**  **(0.5)**

#### Implementation for US1

- [X] T1.13 [US1] Implement `cargonaut-transfer::submit_transfer` in `cargonaut-transfer/src/job.rs`. Spawns a tokio task that: opens src+dst streams, copies in 16 MiB chunks, writes a `TransferCheckpoint` after every 8 MiB (config), emits `Progress` via `watch::channel`. Honors `CancellationToken`.  **(1.5)**
- [X] T1.14 [US1] Implement `cargonaut-transfer::scan_resumable(dst_dir)` in `cargonaut-transfer/src/checkpoint.rs`: scans dest dir for `.cargonaut-transfer-*.json`, validates each (CRC chain, source SHA-256 prefix), returns `Vec<ResumableTransfer>`.  **(1.0)**
- [X] T1.15 [US1] Implement `cargonaut-transfer::resume_transfer(checkpoint)`: opens streams at offset, verifies dst-side CRC chain, continues the copy loop. Emits the SAME `Progress` shape.  **(0.5)**
- [X] T1.16 [US1] Implement `cargonaut-config` schema + figment loader in `cargonaut-config/src/lib.rs`. JSON Schema generation via `schemars`. Defaults match `contracts/config.schema.json`.  **(0.5)**
- [X] T1.17 [US1] Implement `cargonaut-ui-tui::PaneView` in `cargonaut-ui-tui/src/pane.rs`: renders a directory listing with cursor + selection. Virtual scrolling for >100 entries.  **(1.0)**
- [X] T1.18 [US1] Implement `cargonaut-ui-tui::keymap` in `cargonaut-ui-tui/src/keymap.rs`: loads `contracts/keymap.toml`, maps key events to `Command` enum.  **(0.5)**
- [X] T1.19 [US1] Implement `cargonaut-core::App` in `cargonaut-core/src/app.rs`: command queue, two PaneViews, status bar, dispatch loop. Async; uses `tokio::select!` to multiplex input + transfer progress.  **(1.5)**
- [X] T1.20 [US1] Implement dialogs: copy/move/delete confirmation (`cargonaut-ui-tui/src/dialog.rs`) + resume prompt on launch.  **(0.75)**
- [X] T1.21 [US1] Implement `cargonaut-bin/src/main.rs`: parse CLI (`clap`), load config, build App, install SIGINT/SIGTERM handlers, run event loop. `--version`, `--help`, subcommands list-plugins/audit/resume.  **(0.5)**

#### MC-parity panel ergonomics (Phase 1 additions from MC gap analysis)

> **TDD note (Constitution Principle II)**: each task below bundles test + impl into one line for brevity, BUT the per-task git history MUST show test-first ordering — commit the failing test SHA before merging the implementation. Same rule as T1.07-T1.12 in the original Phase 1 set.

- [ ] T1.24 [US1] Implement directory + command history (FR-011): in-session ring buffer for cwd history (per pane, depth 100); persistent shell-line history at `~/.local/state/cargonaut/history`. Alt-Shift-h opens dir-history popup; Alt-h opens cmd-history popup; Alt-y/Alt-u step prev/next dir. Unit + integration tests.  **(0.5)**
- [ ] T1.25 [US1] Implement quick-cd popup (FR-012): Alt-c opens inline cd prompt with tab-completion over the focused VFS + recent dirs. Test injected-input fixture.  **(0.25)**
- [ ] T1.26 [US1] Implement panel filter (FR-013): Alt-! prompt; pane state stores `Option<GlobPattern>`; render filters in `PaneView::visible_entries`. Toggle off on empty input. Test with synthetic 1k-entry dir.  **(0.25)**
- [ ] T1.27 [US1] Implement sync/show-in-other (FR-014): Alt-i copies other pane's cwd; Alt-o opens focused entry's dir in other pane. Test asserts focus stays put.  **(0.25)**
- [ ] T1.28 [US1] Implement panel niceties (FR-015): Alt-. toggle hidden-file mask in `PaneView`; Alt-, toggle `LayoutOrient::Vertical|Horizontal`; Ctrl-Space spawn tokio `walk-sum` task for focused/tagged entries, render result inline. Three small integration tests.  **(0.5)**
- [ ] T1.29 [US1] Implement tasks/jobs panel (FR-016): F12 / :jobs opens a transient panel from `Vec<TransferJob>` snapshot; per-row pause/resume/cancel actions (pause via task cooperation, resume via re-arm cancellation token). Required for NFR-004 sanity. Integration test: submit 3 jobs, pause one, verify others continue.  **(1.0)**
- [ ] T1.30 [US1] Implement exit-cwd writer (FR-017): cargonaut writes its last pane's cwd to `$CARGONAUT_EXIT_CWD_FILE` on graceful exit. Ship `contrib/cargonaut.sh` + `contrib/cargonaut.fish` wrapper functions. Document in README. Integration test: invoke binary via wrapper, set var, verify $PWD changes in shell after exit.  **(0.1)**

### Polish (Phase 1)

- [ ] T1.22a [P] [US1] CI check `scripts/check-binary-size.sh`: cargo build --release, run `strip`, fail if size > 8 MiB. Wire into `.github/workflows/ci.yml`. **Covers NFR-001.**  **(0.1)**
- [ ] T1.22b [P] [US1] Bench `benches/keypress-latency.rs`: criterion harness that simulates one keypress through the dispatch loop, measures dispatch → first-paint render latency. Fail >16 ms p95. **Covers NFR-002.**  **(0.25)**
- [ ] T1.22c [P] [US1] Bench `benches/large-dir-scroll.rs`: synthesize a 1M-entry tempdir, open it in one pane, scroll cursor j 10k times, sample RSS at each 100k. Assert RSS stays ≤64 MiB (per FR-009). **Covers NFR-003.**  **(0.25)**
- [ ] T1.22d [P] [US1] Integration test `tests/integration/concurrent_transfers.rs`: submit 8 simultaneous LocalFs→LocalFs 100 MiB copies; assert all complete; assert no UI render frame exceeded 16 ms during the burst (via tracing-subscriber latency probe). **Covers NFR-004.**  **(0.5)**

- [ ] T1.22  Docs: README quick-start (10 lines + screenshot), `docs/architecture.md` (link to spec architecture/), per-crate `lib.rs` rustdoc.  **(0.5)**
- [ ] T1.23  Phase 1 release: bump to 0.1.0, GitHub release with linux-x86_64 binary + sha256, demo recording.  **(0.5)**

---

## Phase 2: VFS + Transfer adapters (14.75 owner-weeks)

### US2 — SFTP backend with resume across disconnect (P2, 7.75 owner-weeks)

- [ ] T2.01 [US2] Integration test `tests/integration/sftp_basic.rs`: docker-compose with openssh-server; list, copy, delete a remote file. Skipped unless `CARGONAUT_DOCKER_TESTS=1`.  **(0.75)**
- [ ] T2.02 [US2] Integration test `tests/integration/sftp_resume_disconnect.rs`: copy 100 MiB SFTP→local, kill SSH mid-transfer, assert SHA-256 match after auto-reconnect+resume.  **(0.5)**
- [ ] T2.03 [US2] Bench `benches/sftp-throughput.rs`. **SC-005 gate (≥200 MiB/s localhost).**  **(0.25)**
- [ ] T2.04 [US2] Implement `cargonaut-vfs-sftp` adapter (`russh-sftp`-based). Pipelined reads. Credentials via SSH agent → keychain → prompt.  **(2.5)**
- [ ] T2.04b [US2] Integration test `tests/integration/credentials_paths.rs`: exercise each credential path (ssh-agent unix-socket, keychain via `keyring` mock, interactive prompt via PTY); after each, walk `$HOME` + `$XDG_*` + `/tmp` + the cargonaut config/state dirs and `grep -a` for the test password — assert zero matches. Also assert audit log entries redact the secret to `***`. **Covers FR-102.**  **(0.75)**
- [ ] T2.05 [US2] Wire `cargonaut-vfs-sftp` into the VFS registry; `:cd sftp://user@host/path` works.  **(0.5)**
- [ ] T2.06 [US2] Extend `cargonaut-config` with `[remote.sftp]` (timeouts, pipelined-reads, keep-alive).  **(0.25)**
- [ ] T2.07 [US2] Docs: VFS adapter authoring guide (`docs/vfs-adapters.md`).  **(0.5)**

### S3 backend (P2, 4 owner-weeks)

- [ ] T2.08 [US2] Integration test `tests/integration/s3_basic.rs`: MinIO docker fixture; list, multi-part-upload, range-read, delete.  **(0.75)**
- [ ] T2.09 [US2] Implement `cargonaut-vfs-s3` adapter using `aws-sdk-s3`. Multi-part upload + resume via stored ETag chain.  **(2.5)**
- [ ] T2.10 [US2] Cross-VFS transfer (SFTP → S3): integration test + tuning.  **(0.75)**

### Archive adapter (P3 brought into Phase 2, 1.5 owner-weeks)

- [ ] T2.11 [US2] Implement `cargonaut-vfs-archive` (read-only `tar`/`zip` via `archive-rs`/`zip` crates). `:cd archive://path/to/file.tar.gz/inside-path` works.  **(1.5)**

### Polish (Phase 2)

- [ ] T2.12  Phase 2 release: 0.2.0, demo recording showing remote file operations.  **(0.5)**
- [ ] T2.13  Buffer (credential UX iteration, dependency churn).  **(1.5)**

---

## Phase 3: Plugins + Preview/Editor + MC-killer features (22.0 owner-weeks)

### US3 — Built-in previewer + editor handoff (P2 → Phase 3, 5 owner-weeks)

- [ ] T3.01 [US3] Integration test `tests/integration/preview_text.rs`: open a .rs file, assert syntax highlighting present.  **(0.25)**
- [ ] T3.02 [US3] Integration test `tests/integration/preview_image.rs`: open a .png, assert sixel/iTerm/Kitty escape sequences in serial output (per terminal cap).  **(0.5)**
- [ ] T3.03 [US3] Integration test `tests/integration/editor_handoff.rs`: F4 spawns `EDITOR=true`, returns to cargonaut, refreshed mtime visible.  **(0.5)**
- [ ] T3.04 [US3] Implement previewer dispatcher (`cargonaut-ui-tui/src/preview.rs`): MIME detection via `infer`, route to text/image/media renderers.  **(1.0)**
- [ ] T3.05 [US3] Text previewer: `syntect`-based syntax highlighting + line numbers + virtual scroll for large files.  **(1.0)**
- [ ] T3.06 [US3] Image previewer: detect terminal capability (Kitty / iTerm2 / sixel / ASCII fallback). Sidecar binary for Kitty/iTerm encoding.  **(1.25)**
- [ ] T3.07 [US3] Media previewer: spawn `ffprobe` subprocess; render structured metadata.  **(0.5)**

### US4 — WASM plugin host (P3, 7.5 owner-weeks)

- [ ] T3.08 [US4] WIT interface in `contracts/plugin.wit` matching `contracts/plugin-api.md`. Generate Rust bindings via `wit-bindgen`.  **(0.5)**
- [ ] T3.09 [US4] Implement `cargonaut-plugin-host::PluginInstance` in `cargonaut-plugin-host/src/instance.rs`: load .wasm + plugin.toml, instantiate via wasmtime, install host imports with capability checks.  **(2.0)**
- [ ] T3.10 [US4] Plugin lifecycle: `enable`, `disable`, `reload` commands; per-plugin sandbox config from `Config::plugins`.  **(1.0)**
- [ ] T3.11 [US4] Implement `cargonaut list-plugins` subcommand: shows installed + enabled + granted capabilities + per-plugin denial counter.  **(0.5)**
- [ ] T3.11b [US4] Write `security/threat-model.md`: STRIDE per plugin capability (read-dir, read-file, write-file, network); attacker classes (malicious plugin author, untrusted .wasm download); mitigations mapped to FR-201 + NFR-006 + T3.12 fuzz target. Reviewed against `contracts/plugin-api.md`.  **(0.5)**
- [ ] T3.12 [US4] Fuzz target `tests/fuzz/sandbox_escape/`: generate random wasm components, attempt syscalls outside cap set, assert reject. **SC-006 gate (100k iters, zero escapes).**  **(1.5)**
- [ ] T3.13 [US4] Reference plugin `examples/plugins/git-status/`: declares `read-dirs = ["**/.git/**"]`, reads `.git/HEAD` + `index`, emits per-file `M`/`A`/`?`/blank.  **(1.0)**
- [ ] T3.14 [US4] Reference plugin `examples/plugins/hello-world/`: minimum-viable plugin, used as starter template.  **(0.25)**
- [ ] T3.15 [US4] Wire plugin columns into pane rendering: pane reads each plugin's `render-column` per-file output, appends as a column.  **(0.5)**

### US5 — Search (P3, 2 owner-weeks)

- [ ] T3.16 [US5] `cargonaut-search`: ripgrep subprocess wrapper + result parser; `globset` for instant filename glob.  **(1.0)**
- [ ] T3.17 [US5] Search UI: Ctrl-f opens filter mode; `:find -name '*.rs' -size +10M` opens advanced-find dialog. Results = virtual directory.  **(1.0)**

### MC-killer features + modern TUI niceties (Phase 3 additions from MC gap analysis, 5.25 owner-weeks)

> **TDD note (Constitution Principle II)**: each task below bundles test + impl into one line for brevity, BUT the per-task git history MUST show test-first ordering — commit the failing test SHA before merging the implementation.

- [ ] T3.21 [US3] Implement advanced mask rename (FR-204): "Advanced Rename" dialog with glob|regex toggle, target template with `$1..$9` backrefs (Rust regex convention; NOT sed-style `\1..\9`) / `*` wildcards, before→after preview table with per-row untoggle, dry-run mandatory before apply. Use `regex` crate. Integration test on 50 tagged files.  **(1.0)**
- [ ] T3.22 [US3] Implement external panelize (FR-205): `:!cmd` / `Ctrl-x !` runs cmd via `$SHELL -c`, captures stdout, builds an ephemeral panel from each line treated as a VfsPath. Strike-through non-resolving lines. Integration test with `:!find . -name '*.rs'`.  **(0.5)**
- [ ] T3.23 [US3] Implement user menu (FR-206): F2 opens menu from `~/.config/cargonaut/menu.toml` ∪ `./.cargonaut.menu.toml`. Macro expander (`%f`/`%F`/`%d`/`%D`/`%t`/`%T`/`%s`/`%S`/`%b`/`%x`/`%%`). Conditions (`is-file`, `is-dir`, `match-glob`, `has-cap`). Each invocation logged to audit log (Phase 4) when present, stdout/stderr captured. Bundle 3 starter menu entries.  **(1.0)**
- [ ] T3.24 [US3] Implement openers.toml ext-binding (FR-207): TOML loader; (ext|glob|mime) match priority order; Enter dispatches `open`, F3 `view`, F4 `edit` (with `$EDITOR` fallback for FR-104 compat). Bundle defaults for chafa/pdftotext/glow/syntect/zcat. Integration test exercises 5 ext types.  **(0.5)**
- [ ] T3.25 [US3] Implement bulk rename via $EDITOR (FR-208): "Bulk Rename" command (default Ctrl-x r) on ≥2 tagged files writes names to a tempfile, opens `$EDITOR`, on save diffs old↔new rows and applies. Hard-error on line-count mismatch; per-row conflict prompt. Integration test asserts $EDITOR=true returns to cargonaut with no rename; $EDITOR=fake-editor-that-rewrites returns with renames applied.  **(0.5)**
- [ ] T3.26 [US3] Implement previewer hex view + search (FR-209): `:hex` / Ctrl-x X toggles xxd-style hex view (NOT `Ctrl-x h` — reserved for FR-202 hotlist-add in Phase 5); `/<regex>` forward, `?<regex>` backward, `n`/`N` next/prev, `:g <n>` goto line/offset. Re-uses syntect for text mode. Integration tests cover both modes.  **(1.0)**
- [ ] T3.27 [US5] Implement fuzzy filter (FR-210): `<` / `:filter` opens inline fuzzy prompt; `nucleo` scorer over visible names; results re-rank per keystroke. Also add `--fuzzy` switch to FR-203 Find dialog. Bench: 10k entries, scorer p99 < 16 ms.  **(0.5)**
- [ ] T3.28 [US5] Implement zoxide integration (FR-211): detect `zoxide` on $PATH at startup; auto-set `[ui] zoxide = true` if found. `:z <fragment>` invokes `zoxide query -i`; every `:cd` / `Alt-c` accepted path runs `zoxide add` (best-effort, no error surfacing).  **(0.25)**

### Polish (Phase 3)

- [ ] T3.18  Docs: `docs/plugin-developer-guide.md`, `docs/previewer-authoring.md`, `docs/user-menu-cookbook.md` (FR-206 examples), `docs/openers-recipes.md` (FR-207 bundled + custom).  **(0.75)**
- [ ] T3.19  Phase 3 release: 0.3.0 + demo with git-status plugin enabled + image preview + mask rename + user menu.  **(0.5)**
- [ ] T3.20  Buffer.  **(2.5)**

---

## Phases 4-6 (high-level)

Each phase will run its own clarify+plan+tasks pass when picked up. Sketch entries below name the FRs each phase MUST cover but DO NOT enumerate per-FR tasks — that's the deliverable of the per-phase tasks pass. New FRs added by the MC gap analysis (FR-305 in Phase 4; FR-404, FR-405 in Phase 5) follow the same policy: their full task breakdown happens when the team picks up that phase.

Sketch:

### Phase 4: terminal emulator + undo + audit + compare-dirs (15.5 owner-weeks)
- T4.x: built-in terminal emulator via `portable-pty` + own VT100 decoder
- T4.x: undo engine with persisted reverse-plan
- T4.x: HMAC-chain audit log + tamper-detection on launch
- T4.x: Compare-Directories + side-by-side diff viewer (FR-305) — three modes (quick/thorough/mtime); auto-tag diffs; `similar` crate for diff. ~1.5 owner-weeks.
- New SCs: SC-007 (undo correctness), SC-008 (audit tamper)

### Phase 5: theming + l10n + a11y + menu-bar + listing-modes (11.5 owner-weeks)
- T5.x: bookmarks (persisted) + tabs + tags (per-path metadata, indexed for search) — FR-202; closes the US5 "stickiness" leg
- T5.x: Windows port — crossterm Windows backend smoke-test; `windows-latest` CI job; keychain via `wincred`; document known Windows-specific limitations (paths, console color, signal handling)
- T5.x: 6 bundled themes; `Ctrl-r` reload
- T5.x: `fluent-rs` integration + 5 launch locales
- T5.x: `--a11y-output text` mode (plain-text event stream)
- T5.x: Menu bar (FR-404) — F9 top-line menu with File/Edit/View/Navigate/Tools/Help dropdowns; mouse + arrow-key navigable. Discoverability for MC migrants. ~1 ow.
- T5.x: Listing modes (FR-405) — Alt-t cycles Brief/Standard/Long/User-defined column layouts; user-defined block in `[ui.listing.user]` config. ~0.5 ow.
- New SCs: usability test pass

### Phase 6: security hardening + perf tuning + MC migration (12 owner-weeks)
- T6.x: seccomp + landlock filters; per-plugin stricter seccomp
- T6.x: io_uring on Linux; SIMD-accelerated checksums
- T6.x: MC bookmarks importer + `--mc-keys` verification (30-shortcut checklist)
- T6.x: FISH backend (`sh://` over SSH for boxes without sftp-server) — ~2 ow per spec §15
- New SCs: SC-009 (MC migration), SC-010 (coverage)

---

## Dependencies + parallel execution

```
Phase 1
  ├── Setup (T1.01-03) ──→ Foundational (T1.04-06) ──→ US1 (T1.07-21) ──→ Polish (T1.22-23)
  └── Engineer parallelism within Phase 1:
       • Eng A: T1.04, T1.05, T1.06 (VFS trait + LocalFs)
       • Eng B: T1.13, T1.14, T1.15 (Transfer engine)
       • Eng C: T1.17, T1.18, T1.19, T1.20 (UI)
       • Eng D: T1.07-12, T1.21, T1.22-23 (tests, bin, docs)

Phase 2 (after Phase 1 lands)
  ├── US2-SFTP (T2.01-07) and US2-S3 (T2.08-10) parallelizable across engineers
  └── Archive (T2.11) parallel with above

Phase 3 (after Phase 2 lands)
  ├── US3-Previewer (T3.01-07) and US4-Plugin (T3.08-15) parallelizable
  └── US5-Search (T3.16-17) parallel; depends on US3 only for the "results as virtual directory" UI affordance
```

## MVP scope (smallest shippable)

**Phase 1 alone is the MVP.** Ships a `cargonaut` binary that does dual-pane local navigation + F5 resumable copy + meets SC-001 through SC-004. Every other capability layers on top.

## Implementation strategy

- **Test-first non-negotiable** for every FR (Constitution Principle II adopted from MyOS2026).
- **Ship every phase** with a tagged release + demo recording. Don't merge phase work to main without the gate SCs passing.
- **Plugin host is the highest-risk surface** — Phase 3 budgets 1.5 owner-weeks for fuzzing alone (T3.12).
- **Cross-VFS testing** is Phase 2's hidden cost — T2.10 (SFTP↔S3 transfer) gets its own task because the resumable engine's checkpoint format MUST handle either side dropping out.
- **Buffer per phase** (T1.0 has none; T2.13 = 1.5; T3.20 = 2.5) increases because phase complexity grows.

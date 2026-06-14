# Release Milestones + Phase Plan: Cargonaut

**Team assumption**: 3-5 Rust engineers. Estimates given in **engineer-weeks** (one engineer × one calendar week of focused work, ~30 productive hours). A 4-person team divides the work; cross-cutting refactors compress the schedule less than the raw division suggests.

## Phase summary

| Phase | Goal | Eng-weeks (4-eng team) | Calendar | Shippable artifact | Gates |
|---|---|---|---|---|---|
| **1 — Prototype + Core** | dual-pane local + resumable copy + orthodox-FM parity panel ergonomics | 15.45 (~4 wk) | 4 weeks | `cargonaut` binary; F5 copy works; SIGKILL-resumable; history/quick-cd/filter/sync/tasks-panel/exit-cwd-wrapper | SC-001/002/003/004 |
| **2 — VFS + Transfer adapters** | SFTP + S3 backends | 14.75 (~4 wk) | 4 weeks | `sftp://`, `s3://` URIs work in panes; credentials redacted | + SC-005 |
| **3 — Plugins + Preview/Editor + power features** | WASM plugins + previewers + mask-rename + panelize + user-menu + openers + bulk-rename + hex-view + fuzzy + zoxide | 21.75 (~5.5 wk) | 5.5 weeks | git-status plugin runs; F3 preview; F4 edits; F2 user menu; `:!cmd` panelize; advanced rename | + SC-006 |
| **4 — Terminal emulator + undo + audit + compare-dirs** | Ctrl-o subshell + Ctrl-z undo + audit + Compare-Directories + diff | 16.5 (~4 wk) | 4 weeks | Subshell launches; deletions undoable; audit log integrity; `Ctrl-x d` compare | + SC-007/008 |
| **5 — UX polish + theming + l10n + a11y + menu-bar + listing-modes** | 6 themes + 5 locales + screen-reader + menu bar + Brief/Long/User-defined columns | 11.5 (~3 wk) | 3 weeks | `--theme dracula` works; `LANG=ja_JP.UTF-8` works; F9 menu bar; Alt-t cycles | Usability test |
| **6 — Security hardening + perf tuning + orthodox-FM migration** | seccomp/landlock + io_uring + orthodox-FM importer | 10 (~2.5 wk) | 2.5 weeks | `--mc-keys` works; 1 GiB copy ≤ 1.1 × cp(1); orthodox-FM bookmarks import | + SC-009/010 |
| **Total** | **89.95 eng-weeks** | **~23 calendar weeks (5.5 mo)** | 1.0 release | All SCs |

Plus ~3 weeks buffer (incidents, vacation, dependency churn) → realistic **6.5 months calendar** to 1.0 (vs original 5.5-month estimate; the orthodox-FM gap analysis added ~14 eng-weeks of scope across Phases 1-5).

> **Note**: The Phase 1/2/3 task tables below predate the orthodox-FM gap analysis additions. **`tasks.md` is the source of truth for the full task backlog**; tables here are kept for narrative continuity only.

## Phase 1 backlog (timeboxed tasks)

**Goal**: Ship a binary that does dual-pane local navigation + F5 resumable copy + meets SC-001/002/003/004.

| Task | Owner-week | Notes |
|---|---|---|
| T1.01 — Initialize cargo workspace + CI skeleton (clippy + cargo test + tarpaulin) | 0.5 | Phase 1 LOC budget locked at 5k |
| T1.02 — `cargonaut-vfs` trait + `LocalFs` impl (list, stat, read_stream, write_stream, unlink, rename) | 1.5 | Async + `tokio::fs`; benchmark stat() on a 10k-entry dir |
| T1.03 — `cargonaut-vfs` unit + property tests (round-trip path parse, sort stability, glob matching) | 0.5 | proptest |
| T1.04 — `cargonaut-transfer` engine: `submit_transfer`, checkpoint write/read, resume logic | 2.0 | The crown jewel; check vs cp(1) benchmark |
| T1.05 — `cargonaut-transfer` integration tests: SIGKILL-resume, source-swap detection, chunk-CRC mismatch | 1.0 | tests/integration/resume_*.rs |
| T1.06 — `cargonaut-config` schema + figment loader + JSON Schema generation | 0.5 | schemars |
| T1.07 — `cargonaut-ui-tui` skeleton: 2 panes, status bar, keymap dispatcher, command queue | 2.0 | ratatui; build a "click-through" demo before wiring real backends |
| T1.08 — `cargonaut-ui-tui` keybindings: Tab, j/k, Enter, Backspace, Insert, F5, F8, :cd, Esc | 1.0 | Keymap is TOML-loaded |
| T1.09 — `cargonaut-bin`: CLI args, main(), event loop wiring, signal handlers (SIGINT → cancel, SIGTERM → save checkpoints) | 1.0 | clap + tokio::signal |
| T1.10 — `bench/local-copy-vs-cp.sh` + criterion benches; SC-001 enforcement | 0.5 | CI runs criterion; threshold gate |
| T1.11 — `bench/startup.sh` + `bench/rss-headroom.sh`; SC-003/004 enforcement | 0.5 | hyperfine + custom RSS sampler |
| T1.12 — Docs: README quick-start, architecture.md, CONTRIBUTING.md | 0.5 | Crate-level rustdoc on every public item |
| T1.13 — Phase 1 release: tag v0.1.0, GitHub release with linux-x86_64 binary + sha256 | 0.5 | release.yml workflow |
| **Phase 1 total** | **12.0 eng-weeks** | **~3 weeks calendar for 4-eng team** |

## Phase 2 backlog (timeboxed tasks)

**Goal**: SFTP + S3 backends; SC-005 verified.

| Task | Owner-week | Notes |
|---|---|---|
| T2.01 — `cargonaut-vfs-sftp` adapter (russh-sftp); credential plumbing through OS keychain | 2.5 | Pipelined reads for SC-005 throughput |
| T2.02 — `cargonaut-vfs-sftp` integration tests (Docker openssh-server fixture; resume across disconnect) | 1.0 | docker-compose in CI |
| T2.03 — `cargonaut-vfs-s3` adapter (aws-sdk-s3); multi-part upload + resume | 2.5 | Tested vs MinIO fixture in CI |
| T2.04 — `cargonaut-config` extension: `[credentials.sftp]`, `[credentials.s3]` sections | 0.5 | Backward-compat with v0.1.0 config files |
| T2.05 — UI: scheme switcher (Ctrl-l opens "navigate to URI" prompt) | 1.0 | History-suggested URIs |
| T2.06 — `bench/sftp-throughput.sh`; SC-005 enforcement | 0.5 | Localhost openssh fixture |
| T2.07 — `cargonaut-transfer` adapter integration: SFTP→local, local→S3, SFTP→S3 (cross-VFS) | 2.0 | Resumability across schemes |
| T2.08 — `cargonaut-vfs-archive` (read-only tar/zip mounted as a directory) | 1.5 | Defer write-into-archive to v2 |
| T2.09 — Docs: VFS adapter authoring guide | 0.5 | Plugin authors who want to add another scheme |
| T2.10 — Phase 2 release: tag v0.2.0; SFTP + S3 in release notes | 0.5 | |
| T2.11 — Buffer: dependency churn, credential UX iteration | 1.5 | |
| **Phase 2 total** | **14.0 eng-weeks** | **~3.5 weeks calendar** |

## Phase 3 backlog (timeboxed tasks)

**Goal**: WASM plugin host + previewers (text, image, media metadata) + `$EDITOR` handoff; SC-006 verified.

| Task | Owner-week | Notes |
|---|---|---|
| T3.01 — `cargonaut-plugin-host` skeleton: wasmtime component runtime, capability ledger | 2.0 | WIT interface in `contracts/plugin.wit` |
| T3.02 — Plugin lifecycle: enable/disable via config; per-plugin sandbox config | 1.0 | `cargonaut list-plugins` subcommand |
| T3.03 — Capability enforcement: read-dir allowlist + read-file/write-file/network flags | 1.5 | Every wasm syscall checked |
| T3.04 — `examples/plugins/git-status/`: canonical WASM plugin in Rust | 1.0 | Tested against the repo's own git history |
| T3.05 — `examples/plugins/hello-world/`: starter plugin + docs | 0.5 | |
| T3.06 — `tests/fuzz/sandbox_escape/`: cargo-fuzz target generating random wasm | 1.5 | SC-006 enforcement |
| T3.07 — `cargonaut-ui-tui` previewer module: dispatch by MIME → renderer | 1.0 | infer + tree_magic_mini |
| T3.08 — Previewers: text (syntect), image (sixel/iTerm/Kitty), media (ffprobe sidecar) | 2.5 | Per-terminal capability detection |
| T3.09 — `$EDITOR` handoff: suspend cargonaut, exec editor, resume on exit | 1.0 | Tested with vi, vim, helix, nano |
| T3.10 — `cargonaut-search`: ripgrep subprocess integration + glob via globset | 1.5 | Background search → virtual directory |
| T3.11 — Docs: plugin-developer-guide.md, previewer authoring | 0.5 | |
| T3.12 — Phase 3 release: tag v0.3.0; first non-trivial plugin demoable | 0.5 | |
| T3.13 — Buffer | 2.5 | |
| **Phase 3 total** | **16.0 eng-weeks** | **~4 weeks calendar** |

## Migration from the reference OFM (Phase 6)

| Reference-OFM feature | Cargonaut equivalent | Migration path |
|---|---|---|
| `F1-F10` keys | Identical defaults; `--mc-keys` is a no-op | None |
| `Insert` toggle selection | Identical (FR-004) | None |
| `Ctrl-o` subshell | `Ctrl-o` drops to a real terminal emulator pane (FR-301) | Different shape — drop-down vs whole-screen — config flag `[ui] mc_subshell = "fullscreen"` for reference-OFM behavior |
| `F9` menu bar | Identical (FR-404, Phase 5) | None |
| `F2` user menu | Identical, TOML schema instead of the reference OFM's text format (FR-206); macros %f/%t/%d preserved | One-time conversion script `cargonaut import-mc-menu ~/.config/mc/menu` |
| `mc.ext.ini` extension binding | `openers.toml` (FR-207); per-(ext\|glob\|mime) Open/View/Edit commands | One-time `cargonaut import-mc-ext` |
| Bookmarks / hotlist (`~/.config/mc/hotlist`) | Importable via `cargonaut import-mc-bookmarks` (FR-503); native bookmarks via FR-202 (Phase 5) | One-time |
| Directory history (`Alt-Shift-h`) | Identical (FR-011, Phase 1) | None |
| Command history (`Alt-h`) | Identical (FR-011, Phase 1) | None |
| Quick cd (`Alt-c`) | Identical (FR-012, Phase 1) | None |
| Panel filter (`Alt-!`) | Identical (FR-013, Phase 1) | None |
| Sync other panel (`Alt-i`/`Alt-o`) | Identical (FR-014, Phase 1) | None |
| Hidden toggle / split orient / dir size (`Alt-.` / `Alt-,` / `Ctrl-Space`) | Identical (FR-015, Phase 1) | None |
| Background jobs (`Ctrl-x j`) | `F12` / `:jobs` tasks panel (FR-016, Phase 1) — pause/resume/cancel per job | Different key (F12); rebindable via keymap |
| Advanced rename (`F6` with tagged set) | Identical dialog shape; regex + glob both supported (FR-204, Phase 3) | None |
| External panelize (`Ctrl-x !`) | Identical, also `:!cmd` (FR-205, Phase 3) | None |
| Compare directories (`Ctrl-x d`) | Identical (FR-305, Phase 4) | None |
| Diff viewer (reference diff viewer) | Identical, `Ctrl-x Ctrl-d` on two tagged files (FR-305, Phase 4) | None |
| FTP / SFTP / smb panel | `cargonaut-vfs-sftp`, etc. URIs in `:cd` prompt | Different syntax (`sftp://` vs `/#sftp:`) — migration script in docs |
| FTP (insecure) | **Not supported (FOREVER)** — use SFTP or rsync-over-SSH | Hard migration; document SFTP setup |
| FISH (`sh://`) | **Phase 6+ (deferred)** — covers boxes without sftp-server | None until shipped |
| Audio CD / mailfs / undelete VFS | **Not supported (FOREVER)** | Use dedicated tools (cdparanoia, mutt, debugfs) |
| Editor (the reference editor) | `$EDITOR` handoff (FR-104) | User must set `$EDITOR=vim` (or equivalent); cargonaut does not embed an editor |
| Viewer (the reference viewer) hex toggle | `:hex` / Ctrl-x h in previewer (FR-209, Phase 3) | None |
| Viewer (the reference viewer) regex search | `/<regex>` `?<regex>` `n`/`N` in previewer (FR-209, Phase 3) | None |
| Bulk rename via shell scripts | Bulk rename in `$EDITOR` (FR-208, Phase 3) — wdired-style | New idiom; documented in migration guide |
| Shell wrapper for cd-on-exit (`mc -P`) | `cargonaut.sh` / `cargonaut.fish` (FR-017, Phase 1) | Add wrapper to `~/.bashrc` |
| `Alt-Enter` insert current selection at command line | `Alt-Enter` inserts at the search bar (if open) OR drops into the subshell | Minor behavior delta — documented |

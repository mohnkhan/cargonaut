# Feature Specification: Cargonaut — Rust-native terminal file manager

**Status**: Phase 1 in progress (artifacts live in `design/`; work happens on `main`)

**Created**: 2026-05-17

**Status**: Draft — Phase 1 scope locked; Phases 2-6 high-level only

**Input**: User description: "Rust-native, terminal, keyboard-first dual-pane file manager inspired by the reference orthodox file manager (optional mouse) that ships incrementally in phases..."

## 1. Vision (one paragraph)

Cargonaut is a Rust-native, terminal, keyboard-first dual-pane file manager — the reference orthodox file manager reimagined for 2026 hardware, security expectations, and async I/O. The dominant interaction is a keyboard with mouse as opt-in. Two panes, navigation by hjkl/arrows/Tab, transactional file operations with progress + resumability + undo, a Lua/WASM plugin surface, transparent VFS adapters for local + remote (SFTP, S3, smb, archive) backends, and built-in previewers + terminal emulator. Defaults are conservative (no plugin runs without explicit opt-in; no destructive op without confirmation); power-user behavior is one config tweak away. The implementation language and the cultural lineage (cargo, crates, async-first, fearless concurrency) are intentional — Cargonaut SHOULD feel obviously Rust.

## 2. User Scenarios & Testing *(mandatory)*

### User Story 1 — Two-pane local navigation + resumable copy (Priority: P1, MVP)

A developer launches `cargonaut`, sees two panes (left = `$HOME`, right = `/tmp`), navigates with `j`/`k`/`Enter`, selects a 4-GB tarball with `Insert`, presses `F5` to copy to the opposite pane, sees a progress bar with bytes/sec ETA. Mid-transfer they press `Ctrl-Z` to suspend → switch to another terminal → run `kill -KILL` on cargonaut → relaunch → cargonaut recognizes the prior incomplete transfer and offers `[r]esume / [s]tart over / [c]ancel`. They press `r` and the copy continues from the last fsync'd checkpoint.

**Why this priority**: This is the MINIMUM useful slice — without dual-pane + resumable copy, the project is a worse `ls`. Every other capability is layered on top.

**Independent Test**: Launch cargonaut, navigate to a 4-GB file, F5 to copy, SIGKILL the process mid-transfer, relaunch, accept the resume prompt, verify the final file matches the source (SHA-256 equal).

**Acceptance Scenarios**:

1. **Given** cargonaut is launched, **When** the user presses `Tab`, **Then** focus alternates between left and right panes; the focused pane's selection cursor is visually distinct.
2. **Given** a 4-GB file is selected via `Insert`, **When** the user presses `F5`, **Then** a confirmation dialog appears with source path, destination path, estimated time, and `[Enter] confirm / [Esc] cancel` keys.
3. **Given** a copy is in progress at byte offset N, **When** the cargonaut process receives SIGKILL, **Then** the next launch presents a "resume previous transfer" prompt for the same source+destination pair and `r` resumes from offset N (not from 0).
4. **Given** a copy has resumed from offset N, **When** the copy completes, **Then** the destination file's SHA-256 matches the source file's SHA-256.

---

### User Story 2 — Transparent remote VFS via SFTP (Priority: P2)

A developer connects to a remote box: in the left pane, types `:cd sftp://user@host:/var/log` (or selects "Remote → SFTP" from a menu). Cargonaut prompts for password OR uses the SSH agent automatically (configurable). The pane now shows the remote directory listing. The developer navigates, previews a 200-MB log file (Cargonaut streams the first/last 1000 lines instead of downloading the whole thing), and copies a 50-MB log file from remote to local with F5. The transfer uses pipelined reads (parallel SFTP file-handle reads where supported) and shows real throughput. Disconnect during transfer → cargonaut keeps the partial file + checkpoint metadata → reconnect resumes.

**Why this priority**: Remote file-system access is the #1 thing orthodox-FM users miss when the reference OFM doesn't have a good adapter for the remote system. Putting it in Phase 2 (not Phase 1) is honest — getting the SFTP credential handling and resumable-via-SFTP right is non-trivial.

**Independent Test**: Set up a Docker SFTP server with one large file. Boot cargonaut, navigate to `sftp://`, copy file to local, disconnect SSH mid-transfer, reconnect, verify SHA-256 match after resume.

**Acceptance Scenarios**:

1. **Given** an SFTP URI in the navigation bar, **When** the user presses Enter, **Then** cargonaut resolves credentials in order: SSH agent → keychain → password prompt. No password is stored in plaintext anywhere on disk.
2. **Given** a remote-to-local copy is in progress, **When** the underlying network connection drops, **Then** cargonaut emits a non-fatal status line, retries with exponential backoff up to 3 times, and either resumes the transfer or surfaces a clear error.

---

### User Story 3 — Built-in previewer + editor handoff (Priority: P2)

A developer presses `F3` on a `.png` to open the inline image preview (rendered via sixel/iTerm protocol/Kitty graphics protocol depending on terminal capability) — falls back to ASCII art if the terminal can't render. Presses `F3` on a `.json` → syntax-highlighted preview with line numbers (uses `syntect`). Presses `F4` → edits the file via `$EDITOR` (handoff to the user's existing editor: vim, helix, nvim, etc.). On editor exit, cargonaut refreshes the file's metadata in the pane.

**Why this priority**: Previewer is the #2 thing orthodox-FM users want. Editor handoff is the simplest win — cargonaut DOES NOT try to be the editor; it delegates to `$EDITOR`.

**Independent Test**: Open cargonaut in a Kitty terminal, navigate to a PNG, press F3, verify the image renders. Press F4 on a .rs file, verify cargonaut spawns `$EDITOR` with the file path, on exit return to cargonaut with refreshed mtime.

---

### User Story 4 — Plugin: custom column showing git status (Priority: P3)

A power user enables the `git-status` plugin via `cargonaut --enable-plugin git-status` (or a `[plugins]` block in their config). Each line in the file pane now shows a column with the file's git status: `M` modified, `A` added, `?` untracked, blank for unchanged. The plugin is a WASM module loaded into a sandbox; it cannot read any path OUTSIDE the current pane's directory (capability-restricted).

**Why this priority**: Plugins are the long-tail feature surface. A real plugin (git status) is the proof that the API is usable.

**Independent Test**: Build the git-status plugin from `examples/plugins/git-status/`, enable it via config, navigate into a git repo, verify each file shows its git status; verify the plugin can't read `/etc/passwd` (capability rejection logged).

---

### User Story 5 — Power features: bookmarks, tabs, tags, search (Priority: P3)

A power user presses `Ctrl-b` to bookmark the current directory; sees it in a Ctrl-b bookmark list later. Opens a second tab with `Ctrl-t`. Tags a file with `+work` via a tag dialog. Searches across the visible pane with `Ctrl-f` (substring + glob) or invokes `:find -name "*.rs" -size +10M` (Find dialog → background search → results presented as a virtual directory).

**Why this priority**: These are the "stickiness" features that turn a basic file manager into a daily driver. They land in Phase 5 (UX polish).

---

### User Story 6 — Audit log + transactional undo (Priority: P4)

After a destructive operation (delete, overwrite, rename, move across filesystems), cargonaut writes an entry to a per-user audit log at `~/.local/share/cargonaut/audit.log` and presents `Ctrl-z` undo for the LAST operation (best-effort: delete = trash, overwrite = restore via temporary backup, rename = reverse-rename, cross-fs move = reverse-copy + delete-new). Undo state is persisted so it survives a process restart.

**Why this priority**: Trust is built incrementally; without "I can undo my mistake" users hesitate to use the tool at speed. Same with "what did I do yesterday" via audit log.

---

### Edge Cases

- Source filesystem and destination filesystem have different case sensitivity / unicode normalization (ext4 vs APFS vs NTFS). Copying produces a name collision that wasn't visible. Cargonaut MUST detect via lexical pre-check and surface as a conflict, not silently overwrite.
- Network filesystem (NFS/SMB) reports a stat() success but the file is locked by another writer; the read returns partial data. Cargonaut MUST detect the lock via fcntl/leases when possible AND on read-short surface the inconsistency rather than commit a truncated copy.
- Plugin attempts to read a file outside its capability boundary. The sandbox MUST reject the syscall and emit a structured event (audit log entry + UI status line + plugin reload prompt).
- Terminal emits an SIGWINCH during a long transfer with the progress bar drawn. Cargonaut MUST re-layout the panes + progress widget without aborting the transfer.
- The transfer engine's checkpoint file becomes corrupt (e.g., partial write at unclean shutdown). Cargonaut MUST detect via CRC mismatch and prompt "checkpoint corrupt — restart from beginning?" rather than silently overwriting with garbage.

## 3. Requirements *(mandatory)*

### 3.1 Functional Requirements

- **FR-001** *(P1)*: Two-pane layout with focus indication; `Tab` swaps focus; `Alt-1` / `Alt-2` jumps directly.
- **FR-002** *(P1)*: Pane displays directory contents with columns: name, size, mtime, perms (octal). Sort by: name (asc/desc), size, mtime, ext. Sort key cycles via `Ctrl-s`.
- **FR-003** *(P1)*: Navigation keys: `j`/`k` or arrows (move cursor), `Enter` (descend into directory), `h` / `Backspace` (parent), `~` (home), `/` (root), `:cd PATH` (typed jump).
- **FR-004** *(P1)*: Selection: `Insert` toggles, `*` inverts, `+` / `-` opens a glob-pattern dialog.
- **FR-005** *(P1)*: File operations: `F5` copy, `F6` move/rename, `F7` mkdir, `F8` delete. Every destructive operation requires confirmation (single Enter); a `--no-confirm` config flag is available but defaults to OFF.
- **FR-006** *(P1)*: Copy/move engine MUST be resumable: writes a `.cargonaut-transfer.json` checkpoint file at the destination directory, updated after every N MiB fsync'd (configurable, default 8 MiB). On relaunch, scanning destination directories for orphaned checkpoint files MUST yield resume offers.
- **FR-007** *(P1)*: Copy throughput on local-to-local transfers MUST reach **≥ 80% of `cp(1)`** for files ≥ 100 MiB on the same filesystem. Enforced by SC-001 (CI bench gate).
- **FR-008** *(P1)*: Cancellation: any in-progress operation is cancelable via `Ctrl-c`; cancellation MUST be observed within 500 ms; partial state MUST be cleaned up (delete partial destination OR keep + mark for resume — user-configurable).
- **FR-009** *(P1)*: Memory ceiling: the working-set RSS of a cargonaut process MUST stay ≤ **64 MiB** for the canonical "typical session" — defined as **3 panes × 10k-entry directories, no plugins, no concurrent transfers > 3** (same definition used by SC-003). The implementation MUST NOT load entire directory listings into memory for large dirs (>10k entries); streaming + virtual scrolling required.
- **FR-010** *(P1)*: Startup time: cold-cache launch to interactive prompt MUST be ≤ **150 ms** on the 2026-baseline reference laptop (see §13 Assumptions: 4-core x86_64 / 16 GiB RAM / NVMe SSD).

#### Phase 1 additions — orthodox-FM parity panel ergonomics

- **FR-011** *(P1)*: Directory + command history. `Alt-Shift-h` opens directory-history popup (chronological in-session); `Alt-y`/`Alt-u` step prev/next. `Alt-h` opens command-history popup (persisted at `~/.local/state/cargonaut/history`). Both bounded; default depth 100; configurable via `[ui.history]`.
- **FR-012** *(P1)*: Quick-cd popup. `Alt-c` opens an inline cd prompt with tab-completion against the current FS, recent dirs, and (if FR-211 enabled) zoxide DB. Same semantics as `:cd PATH` but one keystroke instead of three.
- **FR-013** *(P1)*: Panel filter. `Alt-!` prompts for a glob; only matching entries are displayed (storage unchanged). Status bar shows `filter: <pattern>`. Clear by re-pressing `Alt-!` on empty input. Persisted per-pane until cleared.
- **FR-014** *(P1)*: Sync / show-in-other panel. `Alt-i` copies the OTHER pane's current path into the focused pane. `Alt-o` opens the focused entry's directory in the OTHER pane (keeps focus on origin). Two-pane workflow staples.
- **FR-015** *(P1)*: Panel niceties — `Alt-.` toggles hidden-file visibility per-pane; `Alt-,` toggles split orientation (vertical ↔ horizontal); `Ctrl-Space` computes recursive total size for the focused entry (or all tagged entries) and displays inline next to the name.
- **FR-016** *(P1)*: Tasks/jobs panel. `F12` (or `:jobs`) opens a transient panel listing all in-flight `TransferJob`s with throughput, ETA, %, and per-job actions (pause/resume/cancel). Required once NFR-004 (≥8 concurrent transfers) is verifiable — without this the user has no way to act on a stuck job.
- **FR-017** *(P1)*: Shell wrapper for cd-on-exit. Cargonaut writes its final cwd to `$CARGONAUT_EXIT_CWD_FILE` on graceful exit if the env var is set. Ship `contrib/cargonaut.sh` (bash/zsh) and `contrib/cargonaut.fish` that set the env var, invoke the binary, then `cd "$(cat $file)"`. Documented in README + `man cargonaut`. (Equivalent to `mc -P`, `br` (broot), `y` (yazi).)

- **FR-101** *(P2)*: VFS abstraction with adapters for: `local` (P1), `sftp` (P2), `s3` (P2), `smb` (P3), `archive` (P3 — tar/zip/7z read-only mounted as a directory).
- **FR-102** *(P2)*: Credential handling: SSH agent socket, system keychain (libsecret / Keychain / wincred), explicit password prompt. NO plaintext credentials on disk; NO credentials in audit log; encrypted at-rest via OS keychain only.
- **FR-103** *(P2)*: Built-in previewers: text (syntect), images (sixel/iTerm2/Kitty/fallback ASCII), media metadata (ffprobe via sidecar binary).
- **FR-104** *(P2)*: External-editor handoff via `$EDITOR` (default `vi`).

- **FR-201** *(P3)*: Plugin system: WASM (`wasmtime`) sandboxed plugins with capability tokens (read-dir / read-file / write-file / network — each requires explicit grant); native Rust plugins (cargo subcommand-style) for trusted local builds.
- **FR-202** *(P3)*: Bookmarks, persistent across sessions; tabs (multiple pane configurations); tags (per-path metadata, indexed for search).
- **FR-203** *(P3)*: Search: glob/regex over filenames (instant); content search via `ripgrep` integration; results as virtual directory.

#### Phase 3 additions — power features + modern TUI niceties

- **FR-204** *(P3)*: Advanced mask rename. `F6` on ≥2 tagged files opens an "Advanced Rename" dialog: SOURCE pattern (glob OR regex toggle) → TARGET template with `$1..$9` backrefs in regex mode (Rust `regex` crate convention; NOT sed-style `\1..\9`) or `*` wildcards in glob mode. Preview table shows before→after for each tagged row with per-row untoggle. Dry-run mandatory before apply.
- **FR-205** *(P3)*: External panelize. `:!cmd` (or `Ctrl-x !`) runs `cmd` via `$SHELL -c`, captures stdout line-by-line, and presents each line as an entry in the focused pane. Non-resolving lines (no such path) shown with strike-through and skipped on operation. Ephemeral until next `:cd`. Enables `:!fd -e rs`, `:!rg -l TODO`, `:!git diff --name-only` as panel sources. **Shell-injection rule (applies to FR-205/206/207)**: the `cmd` body is opaque shell text — users are trusted with their own config — but every macro substitution (`%f`/`%t`/`%d`/etc.) MUST be shell-quoted via the `shell-quote` crate before splicing. Where possible (no shell metacharacters in `cmd`), implementations SHOULD prefer `Command::new(prog).arg(arg)` over `sh -c` to bypass the shell entirely. Every external invocation MUST be appended to the audit log with full argv (FR-304, when Phase 4 lands).
- **FR-206** *(P3)*: User menu. `F2` opens a context menu from `~/.config/cargonaut/menu.toml` merged with `./.cargonaut.menu.toml` (per-directory; auto-loaded when the focused pane's cwd contains it). Schema in [`contracts/menu.schema.json`](./contracts/menu.schema.json). Each entry: `{ label, key, command, condition?, background? }`. Macros expanded in `command` (all shell-quoted per the FR-205 shell-injection rule): `%f`/`%F` (focused file active/passive), `%d`/`%D` (dirs), `%t`/`%T` (tagged list — `%t` is **NUL-separated** for compatibility with `xargs -0` / `find -print0`; `%T` returns a JSON array for structured consumers; filenames containing NUL byte are rejected by LocalFs upstream so the separator is unambiguous), `%s`/`%S` (tagged-or-focused), `%b` (basename), `%x` (extension), `%%` (literal). Conditions: `is-file`, `is-dir`, `is-symlink`, `is-executable`, `has-tagged`, `match-glob='*.rs'`, `match-mime='text/*'`, `has-cap='execute'`. Sandbox: same shell environment as the parent cargonaut; commands run under the audit log (Phase 4).
- **FR-207** *(P3)*: Extension binding (`openers.toml`). `~/.config/cargonaut/openers.toml` maps `(ext|glob|mime)` → `{ open=cmd, view=cmd, edit=cmd }`. Schema in [`contracts/openers.schema.json`](./contracts/openers.schema.json). `Enter` triggers `open` (falls back to `view` if not set); `F3` triggers `view`; `F4` triggers `edit` (falls back to `$EDITOR`, preserving FR-104). Commands use `%f` macro (shell-quoted per FR-205 rule). Bundled defaults: `.png/.jpg/.gif` → `chafa`, `.pdf` → `pdftotext`, `.md` → `glow`, `.json/.yaml/.toml` → fenced `syntect`, `.gz/.bz2/.xz` → decompress-and-view.
- **FR-208** *(P3)*: Bulk rename via `$EDITOR`. With ≥2 tagged files, the "Bulk Rename" command (default `Ctrl-x r`) writes the tagged names to a temp file, opens in `$EDITOR`, then on editor exit diffs old↔new lines and applies a rename for each row whose right side differs. Line-count mismatch = hard error; existing-name conflict prompts per-row (overwrite / skip / abort).
- **FR-209** *(P3)*: Previewer hex view + in-previewer search. Inside the previewer (F3), `:hex` (or `Ctrl-x X` while previewer focused — NOTE: deliberately NOT `Ctrl-x h`; that binding is reserved for FR-202 hotlist-add in Phase 5 to preserve orthodox-FM migrant muscle memory) toggles hex view (16 bytes/row, address + bytes + ASCII gutter, xxd-style). `/<regex>` forward-search; `?<regex>` backward; `n`/`N` next/prev; `:g <n>` jumps to line (text mode) or offset (hex mode). **Mode dispatch**: keys `/`, `?`, `n`, `N` are dispatched to the previewer iff the previewer pane has keyboard focus; otherwise they fall through to the file-pane (so FR-003 `/` = cd-root still works when the previewer is not focused).
- **FR-210** *(P3)*: Fuzzy filter. `<` (or `:filter`) opens an inline fuzzy prompt over visible names; results re-rank as user types; uses `nucleo` crate (fzf-equivalent scorer). FR-203 Find dialog also gains a `--fuzzy` switch.
- **FR-211** *(P3)*: Zoxide integration. `[ui] zoxide` is tri-state: `"auto"` (default — enable iff `zoxide` is on `$PATH` at startup; silently disable otherwise), `true` (force on — fail loudly at startup if missing), `false` (force off). When enabled: `:z <fragment>` jumps via `zoxide query -i`; every `:cd` and `Alt-c` accepted path also records via `zoxide add` (best-effort, errors swallowed). Soft dependency — Cargonaut works fine without it.

- **FR-301** *(P4)*: Built-in terminal emulator (drop-down or split, like the reference orthodox file manager's `Ctrl-o` subshell but a real emulator using `portable-pty` + own VT100 renderer).
- **FR-302** *(P4)*: Transactional undo: per-session undo stack persisted to `~/.local/share/cargonaut/undo/`; bounded depth (config; default 100 ops); destructive ops surface to undo within 10 ms.
- **FR-303** *(P4)*: Conflict resolution dialogs: name collision (overwrite / skip / rename / merge); permission denied (retry-as-root / skip); checksum mismatch (re-copy / accept / abort).
- **FR-304** *(P4)*: Audit log: append-only `~/.local/share/cargonaut/audit.log` with timestamp, op, source, dest, byte counts, exit status. Rotated daily; integrity-protected via per-line HMAC chain (keyed from OS keychain).
- **FR-305** *(P4)*: Directory comparison + diff viewer. `Ctrl-x d` on the two panes opens a Compare-Directories dialog with three modes: **Quick** (name+size match), **Thorough** (byte-by-byte hash), **Mtime** (name + mtime). Differing entries are auto-tagged in both panes. With two files tagged across panes, `Ctrl-x Ctrl-d` opens a side-by-side diff viewer (uses `similar` crate for diff algorithm).

- **FR-401** *(P5)*: Theming via TOML files; bundled themes: solarized-dark, solarized-light, dracula, gruvbox, nord, monochrome.
- **FR-402** *(P5)*: Localization: English + at least 5 community-translated locales (Spanish, French, German, Russian, Japanese) at first ship; `fluent-rs` for runtime selection.
- **FR-403** *(P5)*: Accessibility: high-contrast mode, screen-reader-friendly output via `--a11y-output text-stream` mode that emits plain text events instead of ANSI cursor jumps.
- **FR-404** *(P5)*: Menu bar (F9). Top-line menu bar with dropdowns: **File** (operations), **Edit** (selection), **View** (previewer/sort/filter), **Navigate** (history/bookmarks), **Tools** (compare/diff/find/panelize), **Help**. Mouse-clickable; arrow-key navigable. Sole purpose: discoverability for orthodox-FM migrants who don't remember shortcuts.
- **FR-405** *(P5)*: Listing modes. `Alt-t` cycles per-pane listing mode: **Brief** (name only, multi-column auto-fit), **Standard** (FR-002 default), **Long** (one row per file with ino, blocks, ctime, atime, xattr count, target-for-symlinks), **User-defined** (columns enumerated in `[ui.listing.user]` config block, e.g. `columns = ["name", "size", "perms", "git-status"]` where `git-status` is plugin-provided).

- **FR-501** *(P6)*: Security hardening: seccomp/landlock filters for the main process (no exec, no network unless using a VFS that requires it); seccomp for plugins (stricter — no network, no exec).
- **FR-502** *(P6)*: Performance tuning: io_uring on Linux for the copy engine; pipelined SFTP for remote; SIMD-accelerated checksums.
- **FR-503** *(P6)*: Migration guide: importer for the reference manager's bookmarks (`~/.config/mc/`), key-binding compatibility mode (`--mc-keys`).

### 3.2 Non-Functional Requirements

- **NFR-001**: Binary size: release build ≤ **8 MiB stripped** (no statically-linked LLVM, no embedded ML models).
- **NFR-002**: First-paint latency after every keypress ≤ **16 ms** on a 60-Hz terminal (rendering must keep up with one frame).
- **NFR-003**: Maximum number of file entries in a single pane: **virtual scrolling supports 10⁶ entries** without dropping below the FR-009 RSS ceiling.
- **NFR-004**: Concurrent transfers: at least **8 simultaneous file copies** without UI stalling.
- **NFR-005**: All async I/O paths MUST be cancellation-safe (no detached background tasks left running after user cancels).
- **NFR-006**: Plugin sandbox MUST reject all syscalls outside the granted capability set; verified by a sandbox-escape fuzzer in CI.
- **NFR-007**: Code coverage: ≥ **80%** on the core crates (vfs, copy-engine, ui-tui); ≥ **60%** on adapter crates.
- **NFR-008**: No unsafe code in `core/` crates without explicit `// SAFETY:` comment AND a unit test for the unsafe invariant.

### 3.3 Machine-readable requirements (excerpted; full file in `contracts/requirements.toml`)

Every FR above is mirrored in `contracts/requirements.toml` with `id`, `priority`, `acceptance` (executable predicate), `verification` (test/manual/CI-check). See [`contracts/requirements.toml`](./contracts/requirements.toml) for the canonical machine-readable manifest.

## 4. Success Criteria *(mandatory)*

### 4.1 Measurable outcomes

- **SC-001** *(Performance, P1)*: Local-to-local copy of a 1 GiB random file from `/tmp` to `/var/tmp` on the same ext4 filesystem completes in ≤ `1.25 × cp(1)`. Verified by `bench/local-copy-vs-cp.sh`.
- **SC-002** *(Resumability, P1)*: After SIGKILL at byte offset N of a 4 GiB copy, the next launch resumes from offset N±8 MiB (one checkpoint interval) and the final SHA-256 matches source. Verified by `tests/integration/resume_sigkill.rs`.
- **SC-003** *(Memory, P1)*: RSS of a session with 3 panes, each browsing a 10k-entry directory, stays ≤ 64 MiB. Verified by `bench/rss-headroom.sh`.
- **SC-004** *(Startup, P1)*: Cold-cache `cargonaut` launch to first paint ≤ 150 ms on the reference machine (see §13 Assumptions); warm-cache ≤ 40 ms. Verified by `bench/startup.sh`.
- **SC-005** *(SFTP throughput, P2)*: SFTP copy on a localhost loopback openssh-server achieves ≥ 200 MiB/s for files ≥ 100 MiB. Verified by `bench/sftp-throughput.sh`.
- **SC-006** *(Plugin sandbox, P3)*: Sandbox-escape fuzzer (CI job) runs 100k random WASM modules attempting to escape; zero successful escapes. Verified by `tests/fuzz/sandbox_escape.rs`.
- **SC-007** *(Undo correctness, P4)*: After a sequence of 10 random destructive operations, pressing Ctrl-z 10 times restores the original directory tree (bit-identical for files; sha-256 match). Verified by `tests/integration/undo_sequence.rs`.
- **SC-008** *(Audit-log integrity, P4)*: Tampering with a line in `audit.log` is detected on next launch via HMAC-chain verification. Verified by `tests/integration/audit_tamper.rs`.
- **SC-009** *(orthodox-FM migration)*: An orthodox-FM user's `~/.config/mc/bookmarks` is importable; key-binding compat mode passes a checklist of 30 most-used orthodox-FM shortcuts. Verified by `tests/integration/mc_migration.rs`.
- **SC-010** *(Test coverage)*: `cargo tarpaulin --lcov` reports ≥ 80% on core crates. Verified by CI gate.

### 4.2 Phased acceptance gates

Each phase MUST PASS all SCs whose priority ≤ phase.priority before moving on:

- **Phase 1 (Prototype + Core)**: SC-001, SC-002, SC-003, SC-004
- **Phase 2 (VFS + Transfer Adapters)**: + SC-005
- **Phase 3 (Plugin + Preview/Editor)**: + SC-006
- **Phase 4 (Built-in terminal + undo + audit)**: + SC-007, SC-008
- **Phase 5 (Polish: theming, l10n, a11y)**: no new SC; perceptual quality bar — usability test with 5 orthodox-FM users
- **Phase 6 (Security hardening + perf tuning + migration docs)**: + SC-009, SC-010

## 5. Key Entities

See [`data-model.md`](./data-model.md) for the full data model. Top-level entities:

- **Pane** — viewport over one directory of one VFS, with cursor + selection state.
- **VfsBackend** — trait + concrete impls (LocalFs, SftpFs, S3Fs, ArchiveFs).
- **TransferJob** — a source+destination pair with a transfer-engine task, progress, and a serializable checkpoint.
- **TransferCheckpoint** — on-disk JSON describing source URI + dest URI + byte offset + chunk-size + SHA-256 prefix; used to detect resumable jobs across restarts.
- **PluginInstance** — a loaded WASM module + its granted capability set + its event channel.
- **AuditEntry** — append-only structured event in the audit log; includes HMAC chain.
- **Config** — TOML/JSON-parseable settings tree (see Section 7 for schema).

## 6. Architecture (high-level)

```
┌─────────────────────────────────────────────────────────────────────┐
│  Cargonaut process                                                  │
│  ┌───────────────────────────────────────────────────────────────┐  │
│  │  UI layer (ratatui — TUI renderer)                            │  │
│  │  • PaneView ×2 / TabBar(P5) / StatusBar / Dialogs / Previewer│  │
│  │  • Keymap dispatcher → command queue                          │  │
│  └────────────────────────┬──────────────────────────────────────┘  │
│                           │ commands                                │
│  ┌────────────────────────┴──────────────────────────────────────┐  │
│  │  Core engine (tokio-runtime)                                  │  │
│  │  ┌───────────┐ ┌─────────────┐ ┌────────────┐ ┌────────────┐  │  │
│  │  │ Vfs       │ │ Transfer    │ │ Search     │ │ Plugin host│  │  │
│  │  │ (trait    │ │ (resumable, │ │ (ripgrep   │ │ (wasmtime  │  │  │
│  │  │  + impls) │ │  checkpts)  │ │  + glob)   │ │  + caps)   │  │  │
│  │  └───────────┘ └─────────────┘ └────────────┘ └────────────┘  │  │
│  │  ┌───────────┐ ┌─────────────┐ ┌────────────┐ ┌────────────┐  │  │
│  │  │ Audit log │ │ Undo engine │ │ Config     │ │ Credential │  │  │
│  │  │ (HMAC)    │ │ (persist)   │ │ (figment)  │ │ (keyring)  │  │  │
│  │  └───────────┘ └─────────────┘ └────────────┘ └────────────┘  │  │
│  └───────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────┘
```

Sequence diagrams and Cargo workspace layout are in [`architecture/`](./architecture/).

## 7. Configuration schema (preview)

`~/.config/cargonaut/config.toml`:

```toml
[ui]
theme = "solarized-dark"
mouse = false
mc_keys = false

[transfer]
checkpoint_interval_mib = 8
parallelism = 4
verify_after_copy = true       # sha256 destination after copy

[plugins]
enabled = ["git-status"]
allow_network = false
allow_exec = false

[credentials]
backend = "system-keychain"    # or "agent" / "prompt"

[audit]
enabled = true
rotate_daily = true
hmac_keyring_entry = "cargonaut/audit-hmac"
```

Full JSON Schema in [`contracts/config.schema.json`](./contracts/config.schema.json).

## 8. CLI surface

```
cargonaut [OPTIONS] [LEFT_PATH] [RIGHT_PATH]

OPTIONS:
  --config <PATH>          alternate config file (default ~/.config/cargonaut/config.toml)
  --theme <NAME>           override config theme
  --mc-keys                orthodox-FM compat keymap (overrides config)
  --enable-plugin <NAME>   enable plugin for this session only
  --a11y-output text       emit plain-text event stream (screen reader)
  --verbose                debug logging to stderr
  --version
  --help

SUBCOMMANDS:
  list-plugins             list installed plugins + their granted capabilities
  audit                    dump or rotate the audit log
  resume                   list resumable transfers, optionally resume one
```

## 9. Plugin interface + threat model

See [`contracts/plugin-api.md`](./contracts/plugin-api.md) for the full WASI-aligned interface. Threat model in [`security/threat-model.md`](./security/threat-model.md) (Phase 3 deliverable).

## 10. UX wireframes + keyboard shortcut map

Wireframes (ASCII) in [`wireframes/`](./wireframes/) — main view (`main-view.txt`) + copy/conflict dialogs (`copy-dialog.txt`). Preview-pane + plugin-manager wireframes are Phase 3 deliverables (added with the previewer / plugin-manager tasks).

Full keyboard shortcut map in [`contracts/keymap.toml`](./contracts/keymap.toml).

## 11. Test plan + CI pipeline

See [`tests-plan.md`](./tests-plan.md) for unit/integration/fuzz/property breakdown and CI matrix.

## 12. Release milestones + migration path

See [`milestones.md`](./milestones.md) for phased delivery + the orthodox-FM migration guide.

## 13. Assumptions

- Targeted terminals: xterm-256color, kitty, alacritty, wezterm, iTerm2, Windows Terminal. ratatui handles the differences.
- Targeted OSes: Linux (primary), macOS, Windows (Phase 5+). FreeBSD as best-effort.
- Reference hardware: 2026-baseline laptop = 4-core x86_64 / 16 GiB RAM / NVMe SSD.
- Rust MSRV: 1.76 stable (no nightly features in published crates; nightly only for fuzzer + benchmarks).
- The user has `$EDITOR` set OR has `vi` installed (cargonaut does not embed an editor).
- For SFTP: an openssh-server is reachable; SSH agent or keychain has the credentials (no in-app SSH key generation).
- Mouse support is OPTIONAL and OFF by default — the keyboard-first principle is non-negotiable.

## 14. Clarifications

### Session 2026-05-17 (Phase 1 scope only)

*This spec is large — the clarify pass below resolves only the highest-impact Phase 1 decisions. Phases 2-6 will run their own clarify passes when picked up.*

- **Q (Phase 1)**: WASM vs Lua for the plugin runtime? → **A**: WASM (`wasmtime`) — typed interfaces (WIT/component model), better sandboxing, polyglot (any source language). Lua is rejected because it would couple plugin developers to a dynamically-typed language and require yet another sandbox audit.
- **Q (Phase 1)**: Checkpoint file location — beside the destination, beside the source, or in `$XDG_DATA_HOME/cargonaut/checkpoints/`? → **A**: Beside the destination, hidden filename `.cargonaut-transfer-<uuid>.json`. Reason: a destination-side checkpoint survives partial copies that don't reach the source filesystem; XDG_DATA_HOME wouldn't survive a chroot/container restart.
- **Q (Phase 1)**: Default to orthodox-FM-compatible F-keys (F5=copy etc.) or modern Ctrl-shortcuts (Ctrl-c=copy)? → **A**: orthodox-FM-compatible F-keys by default; `--mc-keys` is a no-op (already on). Users opt into modern keys via `[ui] mc_keys = false` (which renames the binding scheme, not just unbinds F-keys). Justifies the F-key choice via "orthodox-FM migration is the #1 user persona".

## 15. Out of scope (forever, OR for v1)

- Built-in editor (FOREVER — `$EDITOR` handoff via FR-104; ext-binding via FR-207).
- GUI version (FOREVER — TUI is the product).
- Mobile/touch UI (FOREVER).
- FTP backend (FOREVER — SFTP via FR-101 supersedes; rsync-over-SSH for low-trust networks).
- Audio-CD / mailfs / undelete (ext2 unlinked inodes) VFS backends (FOREVER — orthodox-FM niche backends, near-zero modern demand).
- Built-in orthodox-FM-style reference editor (FOREVER — `$EDITOR` is the editor; ext-binding via FR-207 is the customization surface).
- Cloud sync of cargonaut config across machines (v2+ — not in any of the 6 phases).
- File system mounting (FUSE) (v2+).
- Native macOS/Windows installers / code-signing (Phase 6+).
- Translations beyond the 5 launch locales (community-driven post-1.0).
- **FISH** (`sh://` over SSH; covers SSH boxes without sftp-server) — DEFERRED to Phase 6; ~2 owner-weeks to implement.
- **Miller-columns layout** (ranger-style auto-preview-next-pane); **server-client mode** (lf-style); **`--vim-keys` modal mode** (vifm migrants) — all noted as post-1.0 directions, not in any of the 6 phases.

---

## 16. The five candidate names

| # | Name | One-line rationale | Choice |
|---|---|---|---|
| 1 | **Cargonaut** | Cargo (Rust's tool) + naut (navigator) — Rust-native heritage + file navigation; brandable single word. | **TOP** |
| 2 | Bicommander | Direct dual-pane heritage ("bi" = dual-pane); recognizable lineage but too referential. | Runner-up |
| 3 | Twain | "Twain" = two + literary recognition; short, memorable but unclear domain. | Honorable |
| 4 | Dyad | Mathematical pair; concise but academic-feeling. | Honorable |
| 5 | Rusk | Rust + desk; short, but trademark risk (food brand). | Drop |

**Top choice: Cargonaut.** Crate name: `cargonaut`. Binary: `cargonaut`. Default config dir: `~/.config/cargonaut/`. Pronounced "CAR-go-nawt".

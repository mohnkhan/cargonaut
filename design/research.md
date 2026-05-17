# Research: Cargonaut

**Status**: Phase 0. Resolves decisions that downstream architecture depends on. Phases 2-6 will run their own research passes when picked up.

## R1 — TUI library (Phase 1 critical)

**Decision**: `ratatui` 0.27+ with `crossterm` backend.

**Rationale**: ratatui is the actively-maintained fork of tui-rs; broad terminal compat (xterm, kitty, alacritty, wezterm, iTerm2, Windows Terminal); large widget ecosystem; immediate-mode rendering plays well with cargonaut's command-queue architecture. crossterm is the cross-platform input/output abstraction.

**Alternatives**:
- `cursive` — retained-mode; harder to do streaming/virtual-scroll for 10⁶-entry directories.
- Roll own renderer — wastes 1-2 months for no clear win.

## R2 — Async runtime

**Decision**: `tokio` multi-thread with `rt-multi-thread`, `fs`, `net`, `process`, `signal` features.

**Rationale**: VFS adapters (SFTP, S3) need async; tokio is the dominant ecosystem; multi-thread runtime keeps the UI responsive while transfers saturate I/O on worker threads.

**Alternatives**: `async-std` (smaller ecosystem; SFTP/S3 crates don't target it); `smol` (similar issue); blocking I/O (locks the UI during large dir scans).

## R3 — Resumable copy: checkpoint format + frequency

**Decision**: JSON checkpoint at `{dest_dir}/.cargonaut-transfer-{uuid}.json`; updated after every 8 MiB fsync'd (configurable). Contains: source URI, dest URI, source size, source SHA-256 prefix (first 1 MiB), bytes-written, chunk-CRC chain (one CRC32 per checkpoint interval), version.

**Rationale**:
- JSON (vs binary) is debuggable; cost is negligible at 8-MiB intervals.
- Dest-side (vs source-side) survives source unmount/disconnect.
- 8-MiB interval = ~80ms fsync at typical NVMe → checkpoint overhead < 1%.
- Source SHA-256 prefix (not full file) gives "is this the same source?" check at resume time without re-reading the whole source.
- Chunk-CRC chain lets resume verify the EXISTING destination bytes match before extending.

**Alternatives**:
- Per-byte journal — too much I/O.
- Per-file mtime+size — too weak (false-match on coincidental same-size files).
- XDG_DATA_HOME location — doesn't survive container/chroot restart.

## R4 — Plugin runtime

**Decision**: WASM via `wasmtime` 26+, component-model interface (WIT).

**Rationale**: Component model gives typed cross-language interfaces (plugins in Rust, Go, Zig, C, AssemblyScript all compose to .wasm). Capability-token-based imports map cleanly onto the FR-201 capability set. wasmtime is the reference impl with active fuzzing.

**Alternatives**:
- Lua (`mlua`) — dynamic typing pushes errors to runtime; sandbox audit per release; ties plugin authors to one language.
- Native dylibs (`libloading`) — no sandbox.
- Embedded JS (`boa`, `deno_core`) — VM is 10× wasmtime's size; security audit is harder.

## R5 — VFS path representation

**Decision**: `VfsPath` is `(scheme: SmolStr, authority: Option<SmolStr>, segments: SmallVec<[SmolStr; 8]>)`. Display as `scheme://authority/seg1/seg2`. No URL crate dependency in the trait (Url is too tied to web semantics).

**Rationale**: Path manipulation is hot (every keypress); small-vec avoids alloc for typical depths; smol-str avoids alloc for typical segment lengths. Decoupling from `url` keeps the trait minimal.

**Alternatives**: `std::path::PathBuf` (no scheme/authority); `url::Url` (no segment-vec; %-encoding overhead per push); String (no segmentation).

## R6 — Credential storage

**Decision**: `keyring` crate (cross-platform OS keychain access — Secret Service on Linux, Keychain on macOS, Credential Manager on Windows). SSH agent socket via `ssh-agent-client-rs`.

**Rationale**: OS keychain is the only trustable place; rolling our own encrypted store re-implements ssh-agent without the kernel/user trust boundary.

**Alternatives**:
- Plaintext file with libsodium encryption — re-invents keychain; requires user-supplied passphrase per session.
- In-memory only — forces re-entry every launch; unusable for SFTP daily-driver.

## R7 — Audit-log integrity

**Decision**: Append-only flat file with HMAC-SHA-256 chain. Each line = `{ts, op, src, dst, bytes, status, hmac}`. HMAC over `(prev_hmac || line_fields)`. Key stored in OS keychain.

**Rationale**: Chain detects any single-line tamper. Keyring-resident key means an attacker with disk-only access can't forge entries.

**Alternatives**: Per-line signature (rotates; ~3× verification cost); blockchain-style merkle tree (overkill); no integrity (false sense of audit).

## R8 — Config layering

**Decision**: `figment` with this precedence: CLI args > env vars (`CARGONAUT_*`) > `~/.config/cargonaut/config.toml` > built-in defaults.

**Rationale**: CLI overrides for one-off use, env for containers/CI, config for daily defaults. figment handles the merge.

**Alternatives**: `config` crate (less ergonomic API); hand-rolled (re-invents); `clap` only (no file layer).

## R9 — Theme system

**Decision**: Themes are TOML files describing 16-color palette + bg/fg per semantic role (selection, status, error, ...). Loaded at startup; reloadable on `Ctrl-r`.

**Rationale**: TOML matches config; semantic-role keys (not raw color hex per widget) means themes survive UI refactors.

**Alternatives**: Hardcoded per-theme Rust module (every theme requires rebuild); CSS-like (over-engineered).

## R10 — Localization

**Decision**: `fluent-rs` (Mozilla Fluent). Strings extracted from source into `locales/<lang>/messages.ftl`.

**Rationale**: Fluent handles plurals + gender + complex inflection correctly; the same string file works for runtime selection.

**Alternatives**: `gettext` (older, no Rust-native tooling); raw TOML (doesn't handle plurals).

---

## Decisions deferred to per-phase research

Phases 2-6 will each run their own research pass. The deferred decisions include:
- **Phase 2 R**: SFTP library choice (`russh-sftp` vs `openssh-sftp-client` vs spawning `openssh-client`); S3 SDK (`aws-sdk-s3` vs `rusoto` vs raw `reqwest`).
- **Phase 3 R**: ripgrep integration shape (subprocess vs library link); preview rendering protocols (sixel vs Kitty vs iTerm — runtime detection).
- **Phase 4 R**: Built-in terminal emulator scope (full VT100 vs basic escape-sequence pass-through to host); undo persistence format.
- **Phase 5 R**: Theme format (TOML decided here; specific color palette TBD per theme); locale file structure.
- **Phase 6 R**: seccomp filter granularity; landlock vs OpenBSD pledge; io_uring fallback paths on pre-5.10 kernels.

# Implementation Plan: Cargonaut

**Branch**: `082-cargonaut-file-manager` | **Date**: 2026-05-17 | **Spec**: [spec.md](./spec.md)

## Summary

Build Cargonaut as a Rust 2021-edition cargo workspace with one binary crate and ~12 library crates organized around stable contracts. The Phase 1 MVP ships a runnable prototype with dual-pane local file navigation + resumable copy engine, byte-for-byte tested against `cp(1)` for throughput parity (≥80%). Phases 2-6 layer adapters (SFTP/S3/archive), previewers + editor handoff, plugin sandbox, terminal emulator + undo + audit, theming + l10n + a11y, and finally security hardening + perf tuning + MC migration. Each phase is independently shippable with measurable acceptance criteria (SC-001 … SC-010 in spec.md §4.1).

## Technical Context

**Language/Version**: Rust 2021 edition, MSRV `1.76` stable. No nightly in published crates (nightly only for `cargo fuzz` + criterion benches). (Note: 2024 edition requires Rust ≥ 1.85; stay on 2021 until MSRV ratchets forward.)

**Primary Dependencies**:
- TUI: `ratatui` 0.27+, `crossterm` 0.28+
- Async runtime: `tokio` (multi-thread, `rt-multi-thread` + `fs` + `net` + `process` features)
- Config: `figment` (TOML + env + CLI layering) + `serde` + `schemars` (JSON Schema generation)
- VFS: `async-trait` for the trait; per-adapter crates (`russh-sftp`, `aws-sdk-s3`, `archive-rs` or similar)
- Plugin host: `wasmtime` with the component model
- Search + filter: spawn `ripgrep` as a subprocess; `globset` for glob matching; `regex` for mask rename + previewer search; `nucleo` for fuzzy filter (FR-210)
- Diff: `similar` crate (Compare-Directories + diff viewer, FR-305)
- Credentials: `keyring` crate for OS keychain access; `ssh-agent-client-rs` for the SSH agent
- Logging/audit: `tracing` + `tracing-subscriber`; HMAC chain via `hmac` + `sha2`
- Sandboxing: `wasmtime` for plugins; `landlock` on Linux for the main process (best-effort); `seccompiler` for Phase 6
- Integrations (soft deps): `zoxide` binary detection at runtime (FR-211, optional); `$EDITOR` for FR-104/FR-208 bulk rename; `chafa`/`pdftotext`/`glow` as bundled `openers.toml` defaults (FR-207, optional per-user)

**Testing**: `cargo test` (unit + integration), `cargo fuzz` (sandbox-escape, transfer-engine), `proptest` (config-roundtrip, undo correctness), `criterion` (perf benches), `cargo-tarpaulin` (coverage).

**Target Platform**: Linux x86_64 + aarch64 (primary); macOS x86_64 + aarch64 (Phase 2+); Windows x86_64 (Phase 5+); FreeBSD best-effort.

**Project Type**: Multi-crate cargo workspace; single binary `cargonaut`.

**Performance Goals** (from spec §4.1):
- SC-001: copy throughput ≥ 80% of `cp(1)` for files ≥ 100 MiB local-to-local
- SC-002: resume from SIGKILL within one checkpoint interval (8 MiB default)
- SC-003: RSS ≤ 64 MiB for typical session
- SC-004: startup ≤ 150 ms cold-cache

**Constraints**:
- No detached background tasks after user cancels (NFR-005)
- No unsafe in core/ without `// SAFETY:` + unit test (NFR-008)
- ≥80% code coverage on core crates (NFR-007)
- Plugin sandbox MUST reject all out-of-capability syscalls (NFR-006)

**Scale/Scope** (revised post-MC-gap-analysis):
- Phase 1: ~4,500 LoC + ~2,000 LoC tests = 6.5k LoC; 6 crates total (1 binary + 5 lib: bin, core, ui-tui, vfs, transfer, config). Added FR-011..017 panel ergonomics: history, quick-cd, filter, sync, niceties, tasks-panel, exit-cwd.
- Phase 1-3 cumulative: ~16k LoC; 9 crates (adds cargonaut-vfs-sftp/-s3/-archive + cargonaut-search + cargonaut-plugin-host). Phase 3 adds FR-204..211 mask-rename, panelize, user-menu, openers, bulk-rename, hex-view, fuzzy, zoxide — all hosted in the existing core/ui-tui crates (no new sibling crates).
- Full vision (all phases): ~40-55k LoC; ~12 crates

## Cargo workspace layout

```
cargonaut/                          # workspace root (this repo OR separate)
├── Cargo.toml                      # [workspace] members + shared deps
├── README.md
├── crates/
│   ├── cargonaut-bin/              # the binary crate; CLI + main()
│   │   ├── Cargo.toml
│   │   └── src/main.rs
│   ├── cargonaut-core/             # event loop, command queue, app state
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, app.rs, command.rs, event.rs}
│   ├── cargonaut-ui-tui/           # ratatui rendering layer
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, pane.rs, statusbar.rs, dialog.rs, preview.rs, keymap.rs}
│   ├── cargonaut-vfs/              # VFS trait + LocalFs impl (Phase 1)
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, traits.rs, local.rs}
│   ├── cargonaut-vfs-sftp/         # SFTP adapter (Phase 2)
│   ├── cargonaut-vfs-s3/           # S3 adapter (Phase 2)
│   ├── cargonaut-vfs-archive/      # tar/zip read-only adapter (Phase 3)
│   ├── cargonaut-transfer/         # resumable copy/move engine (Phase 1)
│   │   ├── Cargo.toml
│   │   └── src/{lib.rs, job.rs, checkpoint.rs, parallel.rs}
│   ├── cargonaut-search/           # ripgrep integration + glob (Phase 3)
│   ├── cargonaut-plugin-host/      # wasmtime host + capability ledger (Phase 3)
│   ├── cargonaut-audit/            # HMAC-chain audit log (Phase 4)
│   ├── cargonaut-undo/             # transactional undo engine (Phase 4)
│   └── cargonaut-config/           # config schema + figment loader (Phase 1)
├── examples/
│   ├── plugins/
│   │   ├── git-status/             # canonical WASM plugin example
│   │   └── hello-world/            # plugin starter
│   └── prototype.rs                # Phase 1 standalone demo
├── benches/
│   ├── local-copy-vs-cp.rs         # SC-001 enforcement
│   ├── startup.rs                  # SC-004 enforcement
│   └── rss-headroom.rs             # SC-003 enforcement
├── tests/                          # integration tests (workspace-level)
│   ├── integration/
│   │   ├── resume_sigkill.rs       # SC-002 enforcement
│   │   ├── undo_sequence.rs        # SC-007
│   │   ├── audit_tamper.rs         # SC-008
│   │   └── mc_migration.rs         # SC-009
│   └── fuzz/
│       └── sandbox_escape/          # SC-006 (cargo-fuzz target)
├── docs/
│   ├── architecture.md
│   ├── migration-from-mc.md
│   ├── plugin-developer-guide.md
│   └── security.md
└── .github/workflows/
    ├── ci.yml                      # lint + test + bench-regress + coverage
    ├── release.yml                 # cargo publish + GitHub release + sha256
    └── fuzz.yml                    # nightly fuzz-target run
```

## Crate dependency DAG (Phase 1)

```
cargonaut-bin
  └── cargonaut-core
        ├── cargonaut-ui-tui
        ├── cargonaut-vfs        (LocalFs only in Phase 1)
        ├── cargonaut-transfer
        └── cargonaut-config
```

Phases 2+: add `cargonaut-vfs-*`, `cargonaut-search`, `cargonaut-plugin-host`, `cargonaut-audit`, `cargonaut-undo` as sibling deps of cargonaut-core. None depend on each other (acyclic).

## Public APIs (Phase 1, anchor types)

```rust
// cargonaut-vfs/src/traits.rs
#[async_trait]
pub trait VfsBackend: Send + Sync + 'static {
    fn scheme(&self) -> &'static str;  // "file", "sftp", "s3", ...
    async fn list(&self, path: &VfsPath, sort: Sort) -> Result<DirListing, VfsError>;
    async fn stat(&self, path: &VfsPath) -> Result<VfsMetadata, VfsError>;
    async fn read_stream(&self, path: &VfsPath, range: ByteRange) -> Result<Pin<Box<dyn AsyncRead + Send>>, VfsError>;
    async fn write_stream(&self, path: &VfsPath, offset: u64, mode: WriteMode) -> Result<Pin<Box<dyn AsyncWrite + Send>>, VfsError>;
    async fn unlink(&self, path: &VfsPath) -> Result<(), VfsError>;
    async fn rename(&self, src: &VfsPath, dest: &VfsPath) -> Result<(), VfsError>;
    fn caps(&self) -> VfsCaps;  // resumable | seekable | random-write | metadata-rich | ...
}

// cargonaut-transfer/src/lib.rs
pub struct TransferJob {
    pub id: TransferId,
    pub src: VfsPath, pub dst: VfsPath,
    pub mode: TransferMode,        // Copy | Move
    pub progress: watch::Receiver<Progress>,
    pub cancel: CancellationToken,
}

pub async fn submit_transfer(
    src: Arc<dyn VfsBackend>, src_path: VfsPath,
    dst: Arc<dyn VfsBackend>, dst_path: VfsPath,
    opts: TransferOptions,
) -> Result<TransferJob, TransferError>;

pub async fn scan_resumable(dst_dir: &VfsPath) -> Vec<ResumableTransfer>;
```

Full API surface in [`contracts/public-apis.md`](./contracts/public-apis.md).

## Constitution Check

This project is *separate from* MyOS2026; the MyOS2026 constitution doesn't directly bind it. But the same 4 principles transfer cleanly:

- **I. Code Quality**: clippy `-D warnings` in CI; `#![warn(missing_docs)]` on all `cargonaut-*` crates; no `unsafe` in `core/` crates without justification.
- **II. Test-First**: Tests written before implementation for every FR; SC-* are CI-gated; `cargo test --features ci-strict` runs the full integration matrix.
- **III. UX Consistency**: All dialogs reuse a shared `dialog!` macro; keymap is centralized; theme variables are typed.
  *Transfer note*: MyOS2026's Principle III mandates WCAG 2.1 AA, written for graphical UIs with a DOM. Cargonaut is a TUI without DOM/ARIA semantics; WCAG conformance is not directly applicable. Cargonaut's a11y commitment is FR-403 (Phase 5): `--a11y-output text` mode emits a plain-text event stream consumable by screen readers. This is a scoped reinterpretation of Principle III, not a waiver — the *spirit* (every UI shipped is usable by assistive tech) is honored.
- **IV. Performance**: SC-001/003/004 enforced by criterion benches in CI; regressions >10% block merge.

## Project structure (this repo's relationship)

Cargonaut is a STANDALONE project that lives in its own repo. The spec-kit artefacts here under `specs/069-cargonaut-file-manager/` are PLANNING materials produced as part of MyOS2026's spec-kit workflow; the actual implementation repo would be initialized FROM the [`scaffold/`](./scaffold/) directory once the user is ready to begin Phase 1.

## Complexity tracking

| Violation candidate | Why needed | Simpler alternative rejected because |
|---|---|---|
| 12-crate workspace (vs single crate) | Each crate has a stable API consumed by the next layer; one giant crate would have rebuild times that make Phase 1 hostile to iterate on. Crates also force public-API discipline. | A single crate would speed up Phase 1 by ~1 day but cost compile-iteration time forever. |
| `wasmtime` (vs custom JS/Lua sandbox) | Capability model + WASM component model already designed for this exact use case; rolling our own would re-implement the sandbox + the type system. | Lua + a hand-rolled sandbox is ~1 month of work and adds a permanent attack surface. |
| Build-time `requirements.toml` + `keymap.toml` | Machine-readable spec means CI can grep for "every FR has a test"; keymap as TOML means themes/l10n can override without recompiling. | Hardcoding in Rust would be faster Phase 1 but block Phase 5 work. |

## Post-Phase-1 Constitution Re-check

Will be re-verified after each phase's clarify+plan cycle. Phase 1 above is PASS as drafted.

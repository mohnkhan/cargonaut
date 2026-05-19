# Cargonaut

> Rust-native, terminal, keyboard-first dual-pane file manager — Midnight Commander reimagined for 2026.

**Status**: Phase 1 scaffold + design tunnel complete; implementation has not started.

## At a Glance

| Target | Goal | Result |
|--------|------|--------|
| Cold launch | < 150 ms | _pending impl_ |
| Local-local copy throughput | ≥ 80% of `cp(1)` | _pending impl_ |
| Resident memory | ≤ 64 MiB | _pending impl_ |
| Unit tests | All pass | _0/0 — implementation not started_ |
| Clippy | `-D warnings` clean | _pending impl_ |
| CI pipeline | `make ci-local` green | _pending impl_ |

Update this table on every feature merge (per [CLAUDE.md](./CLAUDE.md) Documentation discipline).

## Feature History

Most recent first.

- **Feature 001 — dev-culture-bootstrap** (2026-05-20). Transferred development conventions from sibling MyOS2026: `.specify/` + `.claude/` (spec-kit slash commands), `CLAUDE.md` (workflow rules), `CONTRIBUTING.md` + `CODE_OF_CONDUCT.md`, `.github/workflows/ci.yml` (cargo-shaped CI rollup), CI scripts (`check-pr-body.sh`, `docs-gate.sh`, `ci-local.sh`), `Makefile` with cargo wrappers + tmpfs targets, `ROADMAP.md` + `Learnings.md` skeletons. Added SSD-preservation tmpfs discipline (`make tmpfs-setup` redirects `target/` → `/tmp/cargonaut/<hash>/target/`) as mandatory dev-machine convention. Branch `001-dev-culture-bootstrap` → PR #N → merged.


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

The Phase 1 binary right now prints a placeholder showing the loaded config
plus the two pane paths it would open. The real UI lands in tasks T1.07+
(see [`design/tasks.md`](./design/tasks.md)).

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

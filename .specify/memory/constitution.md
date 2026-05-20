# Cargonaut Constitution

## Core Principles

### I. Code Quality (NON-NEGOTIABLE)

- CI MUST run `cargo clippy --workspace --all-targets -- -D warnings` and fail on warnings.
- Every published `cargonaut-*` crate MUST carry `#![warn(missing_docs)]`.
- `unsafe` code is forbidden in `cargonaut-core`, `cargonaut-vfs`, `cargonaut-ui-tui`, `cargonaut-transfer`, and `cargonaut-config` unless each block carries a `// SAFETY:` comment documenting the invariant AND is covered by a unit test exercising that invariant (per NFR-008).
- `cargo fmt --check` MUST pass on every PR.
- Public APIs MUST build clean under `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links"`.

### II. Test-First (NON-NEGOTIABLE)

- TDD applies to every functional requirement (FR-###). Tests are authored and committed in a failing state before the implementation that makes them pass. Per-task git history MUST show the red commit preceding the green commit (e.g. `T1.04 (red): …` → `T1.04 (green): …`).
- Each Success Criterion (SC-###) MUST have a CI gate — a bench, integration test, or fuzz target — that fails the build on regression.
- Coverage on core crates MUST stay ≥80% per NFR-007, verified by `cargo tarpaulin --lcov` in CI.
- Pure-doc tasks (e.g. trait-shape passes where no behavior changes) MAY ship in a single commit, provided they still add an object-safety or contract smoke test.

### III. UX Consistency

- All TUI dialogs reuse the shared `dialog!` macro / shared widgets — no ad-hoc layouts in feature code.
- Keymap is defined in one source of truth: `design/contracts/keymap.toml` (loaded at startup; per-user override via config). New bindings MUST land in that file first.
- Theme variables are typed; no hardcoded ANSI escapes in feature code.
- **A11y commitment**: Cargonaut is a TUI without DOM/ARIA semantics, so WCAG 2.1 AA does not apply literally. FR-403's `--a11y-output text` mode (a plain-text event stream consumable by screen readers) is the concrete a11y deliverable that honors the spirit of this principle.

### IV. Performance (NON-NEGOTIABLE)

- The four Phase-1 success criteria MUST be enforced by criterion benches in CI:
  - SC-001: ≥80% `cp(1)` throughput on local-to-local copies ≥100 MiB
  - SC-002: resume from SIGKILL within one checkpoint interval
  - SC-003: ≤64 MiB RSS for the canonical session
  - SC-004: ≤150 ms cold-cache startup
- Performance regressions >10% on any tracked bench MUST block merge.
- NFR-002 (≤16 ms keypress→first-paint, 60 Hz frame budget) is enforced by `benches/keypress-latency.rs` (T1.22b).
- NFR-001 (≤8 MiB stripped release binary) is enforced by `scripts/check-binary-size.sh` (T1.22a).

## Quality Gates

- **Per-PR**: fmt, clippy, build, test, doc build (with strict intra-doc-link checking), binary-size check, coverage threshold.
- **Per-phase release**: every SC whose priority ≤ phase.priority MUST PASS (per spec §4.2). No phase ships without its gate SCs green.
- **Plugin host (Phase 3+)**: the sandbox-escape fuzzer (SC-006) MUST complete 100k iterations with zero successful escapes before plugin host GA.
- **Audit log (Phase 4+)**: tamper-detection (SC-008) MUST pass before audit-dependent features (FR-206 user-menu logging) are advertised as audited.

## Development Workflow

- Default branch: `main`. Broader phase-level work whose design lives under `design/` may proceed directly on `main`; per-feature work uses the speckit feature-branch flow (`/speckit-specify` → `001-feature-name` etc.).
- Every destructive operation surfaced to users MUST require confirmation by default (per FR-005); a `--no-confirm` opt-out is permitted but MUST default to OFF.
- Plugins default to disabled. No plugin may be auto-enabled by config alone — explicit `--enable-plugin <NAME>` flag or interactive capability grant is required (per FR-201, FR-206).
- Credentials MUST NOT touch plaintext disk (per FR-102): SSH agent → OS keychain → interactive prompt, in that order; secrets MUST be redacted (`***`) in the audit log.
- Macro expansion in user-supplied commands (FR-205 / FR-206 / FR-207) MUST shell-quote every substitution via the `shell-quote` crate; where possible, prefer `Command::new(prog).arg(arg)` over `sh -c` to bypass the shell entirely.

## Governance

- This constitution supersedes ad-hoc conventions. Conflicts between this document and `plan.md` or an individual PR are resolved in favor of this document.
- Amendments require: (1) a PR editing this file, (2) corresponding updates to `plan.md §Constitution Check` and any dependent templates, (3) explicit reviewer sign-off referencing the changed principle in the PR body.
- Complexity that violates a principle MUST be justified in `plan.md §Complexity tracking` with the simpler alternative considered and the reason for rejection recorded.
- Use `CLAUDE.md` for runtime development guidance (commit conventions, test/CI ergonomics, working-directory rules).

**Version**: 1.0.0 | **Ratified**: 2026-05-20 | **Last Amended**: 2026-05-20

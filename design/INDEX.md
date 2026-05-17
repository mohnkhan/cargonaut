# Cargonaut — bundle index

Spec-kit-shaped planning bundle for the Cargonaut file manager. Produced
2026-05-17 as a single planning pass for a 3-5 person Rust team to begin
Phase 1 implementation.

## Documents (read in this order)

1. [`spec.md`](./spec.md) — vision, 6 user stories, FRs/SCs/NFRs, name choice
2. [`research.md`](./research.md) — 10 R-numbered decisions locked at Phase 1
3. [`plan.md`](./plan.md) — tech context + Cargo workspace + constitution check
4. [`data-model.md`](./data-model.md) — entities + invariants + storage layout
5. [`milestones.md`](./milestones.md) — phased delivery + eng-week estimates + MC migration table
6. [`tasks.md`](./tasks.md) — Phase 1-3 task backlog (timeboxed; per-engineer parallelism noted)
7. [`tests-plan.md`](./tests-plan.md) — unit/integration/fuzz/property + CI YAML
8. [`checklists/requirements.md`](./checklists/requirements.md) — bundle completeness checklist (all PASS)

## Contracts

- [`contracts/requirements.toml`](./contracts/requirements.toml) — machine-readable FR/SC/NFR manifest
- [`contracts/config.schema.json`](./contracts/config.schema.json) — JSON Schema for `~/.config/cargonaut/config.toml`
- [`contracts/openers.schema.json`](./contracts/openers.schema.json) — JSON Schema for `openers.toml` (FR-207 ext binding)
- [`contracts/menu.schema.json`](./contracts/menu.schema.json) — JSON Schema for `menu.toml` (FR-206 user menu)
- [`contracts/keymap.toml`](./contracts/keymap.toml) — default keymap; overridable
- [`contracts/commands.toml`](./contracts/commands.toml) — `:cmd` command palette (FR-016 et al.)
- [`contracts/plugin-api.md`](./contracts/plugin-api.md) — WIT interface + threat model excerpt

## Architecture

- [`architecture/sequence-copy-resume.txt`](./architecture/sequence-copy-resume.txt) — sequence diagram for F5 + SIGKILL + resume

## Wireframes

- [`wireframes/main-view.txt`](./wireframes/main-view.txt) — main two-pane view
- [`wireframes/copy-dialog.txt`](./wireframes/copy-dialog.txt) — copy + conflict + resume dialogs

## Scaffold (implementation starter)

The scaffold IS this repository's root — see [`../Cargo.toml`](../Cargo.toml), [`../crates/`](../crates/), [`../README.md`](../README.md). (In the original MyOS2026 design bundle this was nested at `specs/069-cargonaut-file-manager/scaffold/`; when seeding this standalone repo it was promoted to the root.)

## Quick links

- Top name choice: **Cargonaut** (see [`spec.md` §16](./spec.md))
- MVP scope: **Phase 1 only** = dual-pane local + resumable copy + SC-001/002/003/004
- Total effort to 1.0: **~76 engineer-weeks** = ~5.5 calendar months for a 4-eng team
- Phase 1 effort: **12 engineer-weeks** = ~3 calendar weeks for a 4-eng team

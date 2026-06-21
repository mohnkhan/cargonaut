# `design/` — Historical Planning Archive

> **Status: ARCHIVE.** This directory is the project's *original* single-pass
> planning bundle (produced 2026-05-17 to kick off Phase 1). It predates the
> per-feature **spec-kit** workflow the project adopted at **feature 031**.
> Treat these documents as **historical context, not current truth.**

## Where current work lives

| You want… | Look in |
|-----------|---------|
| Current requirements / plan / tasks for a feature | `specs/NNN-name/{spec.md,plan.md,tasks.md}` |
| What tests verify a requirement | each feature's own tests under `crates/*/tests` and `crates/*/benches` |
| The active development rules | [`../CLAUDE.md`](../CLAUDE.md) + [`../.specify/memory/constitution.md`](../.specify/memory/constitution.md) |
| The current feature pointer | [`../.specify/feature.json`](../.specify/feature.json) |

## Historical documents (no longer maintained)

`spec.md`, `research.md`, `plan.md`, `data-model.md`, `milestones.md`, `tasks.md`,
`tests-plan.md`, and `contracts/requirements.toml` describe the original 6-phase
master plan (FR-001…FR-503). They are retained for provenance. In particular,
**`contracts/requirements.toml` is archived** — ~97% of its `verification` links
are dead and nothing reads it (see its in-file banner; Feature 058 housekeeping).

## Still LIVE in `design/contracts/` (authoritative — do not archive)

These machine-readable contracts are still consumed by code/config and remain the
source of truth:

- `keymap.toml` — default keymap, **Constitution §III single source of truth**
- `config.schema.json` — schema for `~/.config/cargonaut/config.toml`
- `commands.toml` — `:cmd` command palette
- `menu.schema.json`, `openers.schema.json` — schemas for user menu / openers
- `plugin-api.md` — plugin WIT interface + threat-model excerpt

See [`INDEX.md`](./INDEX.md) for the original bundle's reading order.

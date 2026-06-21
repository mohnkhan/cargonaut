# Data Model: Repository Housekeeping (Feature 058)

This feature has no runtime data model. The "entities" are repository artifacts,
classified by lifecycle state. The model below is the classification that drives which
files are touched.

## Entity: Repository Artifact

| Attribute | Values |
|-----------|--------|
| `path` | repo-relative path |
| `state` | `live` \| `historical` \| `orphaned` |
| `action` | `preserve` \| `archive` \| `correct` \| `delete` \| `create` |
| `tracked` | git-tracked? (affects whether removal yields a diff) |

### State definitions

- **live** — still consumed by code, config, CI, or contributors as authoritative.
- **historical** — retained for context but superseded; not authoritative.
- **orphaned** — empty/placeholder; implies an unused layout.

## Instances (the full change set)

| Path | State | Action | Tracked | Rationale |
|------|-------|--------|---------|-----------|
| `design/contracts/requirements.toml` | historical | archive | yes | 57/59 dead links; false CI claim; superseded by `specs/NNN/` (R-001) |
| `design/contracts/keymap.toml` | live | preserve | yes | Constitution §III source of truth (R-002) |
| `design/contracts/config.schema.json` | live | preserve | yes | consumed by config validation |
| `design/contracts/{commands,menu,openers}.*` + `plugin-api.md` | live | preserve | yes | active contracts |
| `design/plan.md`, `design/spec.md`, `design/tasks.md`, `design/research.md` | historical | (marker only) | yes | original master plan; pointed-to by `design/README.md` banner |
| `design/README.md` | — | create | new | archive marker for `design/` (FR-004) |
| `Cargo.toml` (header comment) | live (stale text) | correct | yes | "Phase 1 in progress" / `design/plan.md` pointer wrong (R-003) |
| `tests/integration/` | orphaned | delete | **no** (untracked) | empty; real tests per-crate (R-004) |
| `benches/` (`.gitkeep`) | orphaned | delete | yes | placeholder; real benches per-crate (R-004) |
| `ROADMAP.md` | live | correct (add row) | yes | track `cargonaut-core` split follow-up (FR-010) |
| `README.md` | live | correct | yes | docs rule (FR-011) |
| `Learnings.md` | live | correct | yes | docs rule (FR-011) |
| GitHub issue (cargonaut-core split) | — | create | n/a | deferral paper trail (FR-009) |

## Invariants

- **INV-1**: No file under `crates/*/src/` changes state or content. (SC-006)
- **INV-2**: Every `live` file in `design/contracts/` is byte-for-byte unchanged. (SC-002)
- **INV-3**: After the change, `state == orphaned` set is empty (both deleted). (SC-004)
- **INV-4**: The deferral (`cargonaut-core` split) has exactly one issue and one ROADMAP
  row pointing at it. (SC-007)

## State transitions (this feature)

```text
requirements.toml:  live-claimed  ──archive──▶  historical (banner, honest header)
Cargo.toml header:  stale         ──correct──▶  accurate pointer
tests/integration/: orphaned      ──delete──▶   (gone)
benches/:           orphaned      ──delete──▶   (gone)
cargonaut-core:     undocumented-debt ──defer──▶ tracked (issue + ROADMAP)
```

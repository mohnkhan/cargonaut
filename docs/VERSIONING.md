# Versioning Policy

Cargonaut follows [Semantic Versioning 2.0.0](https://semver.org/) with the
standard pre-1.0 interpretation.

## Single source of truth

The version lives in **one place**: `[workspace.package] version` in the root
`Cargo.toml`. Every crate inherits it via `version.workspace = true`. The binary
exposes it through `CARGO_PKG_VERSION` (shown by `cargonaut --version`). **Never**
hand-edit a per-crate version — bump the workspace value only.

## What the numbers mean

`MAJOR.MINOR.PATCH`.

### Pre-1.0 (the `0.y.z` rules) — current

While the version is `0.y.z`, the public surface is still stabilizing, so:

- **`0.MINOR` (the `y`)** — bump for **breaking changes** *and* significant new
  features. A `0.x → 0.(x+1)` release MAY break behavior, on-disk formats, CLI
  flags, config/keymap schema, or the crate API.
- **`0.y.PATCH` (the `z`)** — bump for backwards-compatible bug fixes and small
  additive changes that don't break documented behavior.

Users on `0.x` should read the CHANGELOG before every `0.(x+1)` upgrade.

### Post-1.0 (for reference)

Once we ship `1.0.0`:

- **MAJOR** — incompatible/breaking changes.
- **MINOR** — backwards-compatible new functionality.
- **PATCH** — backwards-compatible bug fixes.

## What counts as a breaking change

- A removed/renamed CLI flag or subcommand, or a changed default.
- A removed/renamed config or keymap key, or a changed default binding.
- An incompatible change to an on-disk format (config, hotlist, transfer
  checkpoint, crash-report layout that tooling parses).
- A change to a published crate's public API (the `cargonaut-*` libraries are
  currently internal; treat cross-crate API churn as breaking once any are
  published).

Behavior-preserving refactors, internal-only changes, new opt-in features, and
added (non-default-changing) flags/keys are **not** breaking.

## MSRV (Minimum Supported Rust Version)

- The MSRV is `rust-version` in `[workspace.package]` (currently `1.76`).
- CI builds on stable. A raise of the MSRV is itself at least a **minor**
  (`0.y`) change and MUST be noted in the CHANGELOG.

## Build profiles & the binary-size gate

The release profile (`panic = "unwind"`, `opt-level = "z"`, `lto = "fat"`,
`strip`) and the ≤ 8 MiB binary-size gate (NFR-001) are part of the release
contract — see `scripts/check-binary-size.sh`, enforced in CI.

See [`RELEASING.md`](./RELEASING.md) for how to actually cut a release.

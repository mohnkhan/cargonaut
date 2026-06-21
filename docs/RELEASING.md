# Releasing Cargonaut

This is the repeatable checklist for cutting a tagged release. Versioning rules
are in [`VERSIONING.md`](./VERSIONING.md). All work goes through a branch + PR
(per `CLAUDE.md`) — including the release-prep commit.

## Overview

A release is a `vX.Y.Z` **git tag**. Pushing that tag triggers
`.github/workflows/release.yml`, which builds the static binary, checksums it,
and publishes a **GitHub Release** with notes taken from the CHANGELOG section
for that version. The maintainer's job is only steps 1–6 below; the tag push
automates the rest.

## Checklist

1. **Pick the version.** Per [`VERSIONING.md`](./VERSIONING.md): while `0.y.z`,
   bump `y` for breaking/feature releases, `z` for fixes. Call it `X.Y.Z`.

2. **Bump the single source.** Edit `[workspace.package] version` in the root
   `Cargo.toml` to `X.Y.Z` (every crate inherits it). Run `cargo build` once so
   `Cargo.lock` updates.

3. **Roll the CHANGELOG.** In `CHANGELOG.md`, rename the top
   `## [Unreleased]` heading to `## [X.Y.Z] — YYYY-MM-DD`, and add a fresh empty
   `## [Unreleased]` above it. Make sure the new version section lists the
   user-facing changes since the last release.

4. **Preflight.** From a clean tree on your release-prep branch:

   ```sh
   make release-check          # version ↔ CHANGELOG section ↔ clean tree
   make ci-local               # fmt + clippy + tests + release build + gates
   ```

   Fix anything red. `make release-check REF=vX.Y.Z` additionally asserts the
   intended tag matches the workspace version.

5. **Land the prep PR.** Open a PR with the version bump + CHANGELOG roll, get
   CI green, and merge to `main` (the docs-gate accepts this as an infra/release
   change; include `[no-docs]` if it doesn't touch README/Learnings).

6. **Tag and push.** On the merged `main` commit:

   ```sh
   git checkout main && git pull
   git tag -a vX.Y.Z -m "cargonaut vX.Y.Z"
   git push origin vX.Y.Z
   ```

7. **Automation (no action needed).** The `release` workflow then:
   - re-runs the preflight (tag ↔ version ↔ CHANGELOG),
   - builds `make dist` (stripped static musl binary + tarball),
   - writes a `.sha256` checksum,
   - extracts the `## [X.Y.Z]` CHANGELOG section as the release notes (it
     **fails** rather than publish empty notes), and
   - creates the GitHub Release with the tarball + checksum attached.

8. **Verify.** Check the Releases page: the artifact, checksum, and notes are
   present and correct. Download the tarball and run `cargonaut --version`.

## Notes

- **One version source.** Never edit per-crate versions; only
  `[workspace.package] version`.
- **Re-running a release.** Tags are immutable by convention. To fix a botched
  release, bump to the next patch (`X.Y.(Z+1)`) rather than re-tagging.
- **First release.** `v0.1.0` ships the historical development log already
  captured under `## [0.1.0]` in the CHANGELOG; cut it once the team is ready.
- **No crates.io publish** in this process — the distributable is the binary;
  the `cargonaut-*` libraries are internal.

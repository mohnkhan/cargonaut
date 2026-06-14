For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan
at the path stored in `.specify/feature.json` under `feature_directory`,
plus `plan.md` inside that directory.

## Git Workflow — MANDATORY

**All changes must go through a feature branch and pull request. No exceptions.**

- **Main branch is `main`** — all PRs target `main`.
- Never commit directly to `main` — not for code, not for docs, not for Cargo.toml, not for config.
- Every piece of work gets its own branch: `NNN-short-description` (e.g. `007-cli-fuzzy-finder`).
- Use `/speckit-git-feature` or `git checkout -b <branch> origin/main` to create the branch before making any changes.
- Open a PR targeting `main` and merge via GitHub. Direct pushes to main are prohibited.
- This convention applies to ALL changes regardless of size — a one-line README fix still needs a branch.

## Documentation — MANDATORY on every feature merge

After every feature is merged, **immediately update these two files** on the same branch before the PR:

- **`README.md`** — update the "At a Glance" metrics table (test count, feature count, binary size) and the "Feature History" section with a one-line entry for the new feature.
- **`Learnings.md`** — append a section for the feature: what was hard, what the root causes were, and any non-obvious decisions made. Minimum 3 bullet points per feature.

Both files must be committed on the feature branch and reviewed in the PR. A feature PR that omits these updates is incomplete.

The `docs-gate` step (in `scripts/ci/docs-gate.sh`) rejects feature-branch PRs (`NNN-name`) that do not modify both `Learnings.md` and `README.md`. Bypass for docs-only or infra-only PRs by including the case-insensitive substring `[no-docs]` in any commit message between the branch's merge-base with `main` and HEAD. Example: `git commit --amend -m "Tweak CI config [no-docs]"`.

## Deferrals — MANDATORY paper trail for descoped work

When a feature defers a user story, FR, or sub-task to a follow-up (e.g., "US2 deferred to future feature", "panel-D is descoped"), the deferral counts as documented **only when both** of the following exist before the PR merges:

1. **A GitHub issue** opened against the repo with:
   - Problem statement (what the deferred work would deliver)
   - Why deferred (the constraint that forced the descope — usually complexity, budget, or missing precondition)
   - Suggested approach (enough that a future contributor can pick it up cold)
   - Pointer to where the deferral was decided (spec.md section, research.md R-number, PR description, etc.)
   - Effort estimate
   - Appropriate tier + `follow-up` label

2. **A row in `ROADMAP.md`** in the right tier referencing that issue, with a one-line context note ("Feature NNN US2 follow-up — local-fs side already in place; SFTP backend work remaining").

Without both, the deferral lives only in the PR description — which decays as soon as the PR is merged and scrolls out of view. The deferred work then either gets silently forgotten OR rediscovered months later as a "why is this stubbed out?" surprise.

A descoped FR that ships as a permanent stub (e.g., a `next_frame` always returning `None`) is acceptable IF the issue + ROADMAP row exist to track when the stub gets replaced.

## CI Pipeline — MANDATORY

All PRs targeting `main` MUST pass the `ci` GitHub Actions check before merging. This is enforced by branch protection on `main`; the GitHub merge button is disabled while CI is red, queued, or missing.

- The pipeline runs, in order: `clippy` (`-D warnings`) → `cargo test --workspace` → `cargo build --release` → `check-pr-body` → `docs-gate`. A single rollup job named `ci` is the required check.
- Run the same pipeline locally before pushing: `make ci-local`. Same step order, same flags; takes ~3–5 minutes for a typical change.
- Failed runs upload an artifact named `ci-failure-<run-id>-<job>.zip` to the PR's Checks tab; retained 30 days.
- Concurrency: a newer push to the same non-main branch auto-cancels the prior in-progress run (saves runner minutes on rapid iteration). Runs on `main` are never auto-cancelled.

## SSD preservation via tmpfs — Constitution §V (NON-NEGOTIABLE)

**This is a constitutional rule.** See [`.specify/memory/constitution.md`](.specify/memory/constitution.md) §V (SSD Preservation). All build artifact trees MUST live in tmpfs (`/tmp/cargonaut/<hash>/...`), not on the host SSD. The dev host runs `/tmp` as `tmpfs size=16G` backed by zram (`/dev/zram0`, lzo-rle, ~3-4× effective capacity).

The pattern is borrowed from the sibling MyOS2026 project (where it cut SSD writes by ~3 GB/day during heavy iteration). For Cargo projects, the only large gitignored output tree is `target/` — `cargo build` rewrites multi-hundred-MB of incremental artifacts on every change.

- **`make tmpfs-setup`** — one-time per checkout. Replaces `target/` with a symlink to `/tmp/cargonaut/<hash-of-repo-path>/target/`, migrating any existing content. Idempotent. Reversible.
- **`make tmpfs-status`** — show what's linked, where, and tmpfs usage.
- **`make tmpfs-teardown`** — remove the symlink; recreate empty real `target/`. Build artifacts in tmpfs are kept by default; pass `WIPE=1` to also `rm -rf` the tmpfs subdir.
- **`make clean`** is symlink-aware: empties tmpfs contents, leaves the symlinks intact so the tmpfs association survives.
- **`make check-tmpfs`** — the enforcement guard. Every `make build` / `test` / `bench` / `clippy` runs it as a prereq; errors loudly if `target/` is a real on-SSD directory.
- **Skipped on CI** via `$CI=true` short-circuit — the GitHub runner is ephemeral, no SSD to protect.
- **Forbidden**: `cargo clean` (deletes the symlink; next build materializes `target/` on SSD) and `rm -rf target` (same shape). Use `make clean` / `rm -rf "$(readlink -f target)"/{debug,release}` instead.
- **Waiver**: `CARGONAUT_ALLOW_SSD_TARGET=1 make build` bypasses the guard for justified situations (low-RAM hosts, container dev environments, tmpfs-specific bug repro). Constitution §V requires the reason be recorded in Learnings.md or the PR body.
- `/tmp` is tmpfs → wiped on reboot. After reboot, the symlink remains but its target is gone; the next `cargo build` rebuilds from scratch (~1–3 min cold). This is the price; the benefit is your SSD lasts 3× longer.

**When asked "is the SSD getting hammered?" or "where do build artifacts live?", confirm `make tmpfs-status` shows the link is active. If it isn't, set it up.**

Full guide: [`docs/dev-tmpfs.md`](docs/dev-tmpfs.md).

## Adding a new Make target — discoverability checklist (MANDATORY)

When adding a top-level target to `Makefile`, update **all three** of:

1. The `.PHONY:` declaration (so Make doesn't treat it as a file)
2. The file-header comment (top of `Makefile`) — alphabetical-ish in its group
3. The body of the `help:` target — under the right group, with a one-line description

Verify with `make help | grep <new-target>` before committing. Missing any of these is a regression that adds babysitting overhead the next time someone runs `make help` and the target isn't there.

## Git Commit Authorship — MANDATORY

**Never add a `Co-Authored-By: Claude` trailer (or any Claude signature) to commit messages.**

- Commits are authored solely by the configured git user (`git config user.name` / `git config user.email`).
- Do not append `Co-Authored-By: Claude Sonnet 4.6 <noreply@anthropic.com>` or any variant.
- Write commit messages with subject line and optional body only — no Claude attribution trailers.

**The same applies to PR bodies, issue bodies, and inline comments**: zero AI attribution anywhere user-facing. STRIP `🤖 Generated with [Claude Code]` (and equivalents) from PR bodies BEFORE `gh pr create`.

## Spec-kit workflow

Use the standard spec-kit slash commands in this order for any non-trivial feature:

1. `/speckit-specify <description>` — generates `specs/NNN-name/spec.md` + creates the branch.
2. `/speckit-clarify` — asks ≤5 high-impact clarification questions; integrates answers into spec.md.
3. `/speckit-plan` — generates plan.md + research.md + data-model.md + contracts/ + quickstart.md.
4. `/speckit-tasks` — generates tasks.md organised by user story.
5. `/speckit-analyze` — read-only cross-artifact consistency check.
6. `/speckit-implement` — executes tasks.md phase by phase.

For trivial single-line fixes, skip straight to a feature branch — the spec-kit overhead is wrong-sized for one-line changes.

## In-progress file marker

When editing this file or adding project-wide conventions, follow the discipline:

- One rule per section; rules are MANDATORY unless explicitly marked SHOULD or MAY.
- Cite *why* (the past incident or principle that justifies the rule), not just *what*. A rule without a "why" gets argued with the next time it's inconvenient.
- Keep rules near the top; reference material (paths, contract pointers) goes further down.
- New rules go in their own `## Section Name` — don't bury them in another section's prose.

<!-- SPECKIT START -->
For additional context about technologies to be used, project structure,
shell commands, and other important information, read the current plan at
`specs/031-visual-interactive-parity/plan.md`.
<!-- SPECKIT END -->

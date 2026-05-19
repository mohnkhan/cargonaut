# Contributing to Cargonaut

Thanks for your interest. This document captures the conventions every contributor — human or AI — is expected to follow. It externalises the workflow rules so non-Claude contributors get the same playbook.

## Project ethos

Cargonaut is a Rust workspace. A few principles to keep in mind before opening a PR:

- **Earn complexity.** No speculative abstractions, no feature flags for hypothetical futures, no error handling for cases that can't happen. Three similar lines is better than a premature abstraction. (Constitution Principle I.)
- **Diagnosability beats logging.** Prefer reading existing telemetry over adding `println!`/`eprintln!`/`tracing!`s — design observability primitives once and reuse them. If a question is hard to answer, the answer is usually a new observability primitive rather than another debug print.
- **Honesty over polish.** PRs that say "structurally correct, live integration deferred to issue #N" are welcome and reviewable. PRs that fake-pass test plans aren't.
- **Match the platform conventions where it's cheap to.** For CLI tools that's POSIX; for libraries that's idiomatic Rust. Document deliberate deviations; don't quietly diverge.

## Quick start

```sh
git clone git@github.com:mohnkhan/cargonaut.git
cd cargonaut
make tmpfs-setup        # one-time per checkout — see "SSD preservation" below
cargo build --workspace
cargo test --workspace
```

Common targets (see `make help` for the full list):

| Target | What it does |
|---|---|
| `make build` | `cargo build --workspace` |
| `make test` | `cargo test --workspace` |
| `make clippy` | `cargo clippy --workspace --all-targets -- -D warnings` |
| `make ci-local` | Run the entire CI pipeline locally |
| `make tmpfs-setup` | Redirect `target/` to RAM (single-user dev box; see below) |
| `make tmpfs-status` | Show tmpfs link state + usage |
| `make tmpfs-teardown` | Remove tmpfs link; restore real `target/` |

### Mandatory ergonomics: tmpfs build redirection

This is a single-user dev machine; the SSD has finite write-life. `cargo build` rewrites hundreds of MB of incremental artifacts on every change. **All contributors using this checkout MUST run `make tmpfs-setup` once.** It redirects `target/` into `/tmp/cargonaut/<hash-of-repo-path>/` so writes hit RAM. Reversible (`make tmpfs-teardown`). Idempotent. Auto-skipped on CI.

```sh
make tmpfs-setup        # one-time per checkout
make tmpfs-status       # see what's linked + tmpfs usage
make tmpfs-teardown     # restore; add WIPE=1 to also rm -rf the tmpfs subdir
```

`/tmp` is tmpfs — wiped on reboot. After reboot, the symlink remains but its target is gone; the next `cargo build` rebuilds from scratch (~1–3 min cold). This is the price; the benefit is your SSD lasts 3× longer.

Full guide: [`docs/dev-tmpfs.md`](docs/dev-tmpfs.md).

## Mandatory workflow

### 1. Every change goes through a feature branch and a PR

**No commits directly to `main` — not for code, not for docs, not for Cargo.toml, not for config.** This rule applies to ALL changes regardless of size; a one-line README fix still needs a branch.

```sh
git checkout -b NNN-short-description origin/main
# … work …
git push -u origin NNN-short-description
gh pr create --base main
```

The branch name is `NNN-short-description` (e.g. `007-cli-fuzzy-finder`). `NNN` is the next free three-digit feature number; check `specs/` and recent branches.

### 2. CI must be green before merge

Every PR targeting `main` MUST pass the `ci` GitHub Actions check. Branch protection makes the GitHub merge button unavailable while CI is red, queued, or missing.

The pipeline runs (in order): `clippy` → `cargo test --workspace` → `cargo build --release` → `check-pr-body` → `docs-gate`. A single rollup job named `ci` is the required check.

Run the same pipeline locally before pushing:

```sh
make ci-local
```

Failed CI runs upload an artifact named `ci-failure-<run-id>-<job>.zip` to the PR's Checks tab (retained 30 days).

### 3. Feature PRs must update `README.md` and `Learnings.md`

Every feature PR (branch matching `NNN-name`) updates both:

- **`README.md`** — the "At a Glance" metrics table (test count, feature count, binary size) and the "Feature History" section with a one-line entry.
- **`Learnings.md`** — a new section for the feature: what was hard, what the root causes were, any non-obvious decisions. **Minimum 3 bullet points.**

Both files must be committed on the feature branch and reviewed in the PR. The `docs-gate` CI step rejects feature-branch PRs that do not modify both files.

**Bypass for docs-only / infra-only PRs**: include the case-insensitive substring `[no-docs]` in any commit message between the merge-base with `main` and HEAD. Example:

```sh
git commit -m "Tweak GitHub Actions concurrency [no-docs]"
```

### 4. Adding a Make target requires three updates

When adding a top-level target to `Makefile`, update **all three** of:

1. The `.PHONY:` declaration
2. The file-header comment (top of `Makefile`)
3. The body of the `help:` target — under the right group, with a one-line description

Verify with `make help | grep <new-target>` before committing.

### 5. Commit message style

```
Subject line in imperative mood, < 70 chars

Optional body explaining the WHY. Wrap at 72 chars. Link issues with #N.
Use [no-docs] suffix when bypassing the docs-gate.
```

- No `Co-Authored-By: Claude` (or any AI-tool) trailers. Commits are authored solely by the configured git user.
- No `🤖 Generated with [Claude Code]` (or equivalents) in PR bodies, issue bodies, or comments. Strip them before `gh pr create`.
- Reference issues (`Closes #N`, `Refs #N`) in the body when applicable.
- Don't squash unrelated changes into one commit. Keep the diff reviewable.

### 6. Deferrals require a paper trail

If a feature defers work to a follow-up, **both** of these must exist before the PR merges:

1. A GitHub issue with: problem statement, why deferred, suggested approach, pointer to where the deferral was decided, effort estimate, tier + `follow-up` label.
2. A row in `ROADMAP.md` in the right tier referencing that issue.

Without both, the deferral lives only in the PR description — which decays as soon as the PR is merged.

## Spec-kit pattern for non-trivial features

For anything beyond a one-file fix, write a spec in `specs/NNN-<name>/` before implementation. The pattern (driven by the `speckit-*` slash commands) produces:

```
specs/NNN-name/
├── spec.md            # what + why; success criteria SC-001..SC-NNN
├── plan.md            # implementation phases; technology choices
├── research.md        # numbered design decisions (R1, R2, …) with rationale
├── data-model.md      # data structures, public types, schemas
├── contracts/         # behavioral contracts (one .md per externally-visible interface)
├── quickstart.md      # developer workflow for the new feature
└── tasks.md           # dependency-ordered T001..TNNN task list
```

You don't have to use the spec-kit commands — hand-written specs in the same shape are fine. The point is that the design exists in writing before the code does.

## Filing issues

Use the templates under `.github/ISSUE_TEMPLATE/`:

- **Bug report** — symptom / repro / expected / actual / logs.
- **Feature request** — what / why / scope / alternatives.
- **Follow-up** — for a known gap deferred from a shipped PR.

Blank issues are disabled.

## Pull requests

Use the template at `.github/PULL_REQUEST_TEMPLATE.md`. The shape is:

- **Summary** — one paragraph; lead with `Closes #N` if applicable.
- **What lands** — bullets of concrete changes.
- **Test plan** — checkboxes for things you actually ran. Don't fake-pass.
- **Files of interest** — annotated list. One line per non-trivial file.
- **What this does NOT close** *(optional but encouraged)* — deliberate gaps and links to follow-up issues.

## Code review

- Reviews target the diff; reviewers shouldn't have to read the whole subsystem to understand the change. The PR description is the bridge — make it carry its weight.
- Be willing to ship a small "structurally correct, live-integration deferred to #N" PR rather than chasing a flaky integration to make one large PR pass. Splitting reduces review load and makes regressions easier to bisect.
- Reviewers: focus on root causes, observability, and whether the test plan actually exercises the claimed behavior.

## Where to start

- Read [`ROADMAP.md`](ROADMAP.md) for what's prioritised next — each tier (1–4) lists issues sized roughly by effort and prerequisite depth.
- Issues labelled `good first issue` are sized for an outside contributor to land in a single session.
- Follow-up issues are known gaps from shipped PRs and usually have a ranked diagnostic plan ready to follow.
- Read [`Learnings.md`](Learnings.md) for the design history; it'll save you from re-deriving decisions already made (and re-living mistakes already made).

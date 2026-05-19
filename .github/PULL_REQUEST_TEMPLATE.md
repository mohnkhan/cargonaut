<!--
Thanks for the PR! This template codifies the shape used in Cargonaut PRs.
Delete sections that don't apply (e.g. "What this does NOT close" if not relevant).
Keep "Test plan" and "Files of interest" — they make review tractable.
-->

## Summary

<!--
One paragraph. If this closes an issue, lead with "Closes #N." and a one-line description of what shipped.
If this is a follow-up to a prior PR, link it and say which gap of the prior PR this closes.
-->

## What lands

<!--
Bulleted list of the concrete changes (crates, modules, public APIs, CLI flags, tests).
-->

-

## Root cause

<!--
For bug fixes: name the file.ext:line that was wrong and what was wrong with it. The
check-pr-body CI gate looks for file references under this heading. Skip this section
for new-feature PRs (use the Summary section instead).
-->

## Test plan

<!--
Checkboxes for everything you actually ran. Leave unchecked items unchecked (don't fake-pass them).
The CI gate runs clippy + cargo test + cargo build + docs-gate automatically; you don't need to re-list those.
-->

- [ ] `cargo test --workspace` PASS
- [ ] `cargo clippy --workspace --all-targets -- -D warnings` clean
- [ ] `cargo build --release` builds
- [ ]
- [ ]

## Files of interest

<!--
Annotated list. For each non-trivial file, one line on what changed and why.
Reviewers use this to decide where to spend their attention.
-->

-

## What this does NOT close

<!--
Optional but encouraged for substantial PRs. List deliberate gaps and link follow-up issues.
Per CONTRIBUTING.md, deferred work needs BOTH a GitHub issue AND a ROADMAP.md row before this PR can merge.
Delete this section if nothing was deferred.
-->

<!--
Reminder: per CLAUDE.md, every feature PR (branch `NNN-name`) must update both
`README.md` (Feature History + At-a-Glance metrics if changed) and `Learnings.md`
(≥3 bullets on what was hard / root causes / non-obvious decisions). The `docs-gate`
CI step enforces this and will block merge if either file is unchanged.

Docs-only or infra-only PRs can bypass the gate by including `[no-docs]` (case-insensitive)
in any commit message between the merge-base with main and HEAD.
-->

# Cargonaut Learnings

A running log of non-obvious lessons from shipped features. **Every feature PR appends a section here** (≥3 bullets) — what was hard, what the root causes were, what decisions were made that wouldn't be obvious from reading the diff.

The audience is your future self (and any future contributor). Write what you'd want to read six months from now when you're back in this code and have forgotten everything.

## Discipline

- **Focus on the WHY, not the WHAT.** The diff shows what changed; this file explains why. "We split the parser into two modules" is a what; "the original parser allocated on every char; the split lets the inner loop borrow without breaking the public API" is a why.
- **Document the rejected paths.** If you tried approach A and abandoned it for B, write down why A failed. The next person who has the same instinct will save the same exploration time.
- **Cite file:line.** Pointers to specific lines age well; vague references ("the parser module") rot the second something is renamed.
- **Bullets > prose.** This file is reference material, not narrative. Each bullet should be independently readable without surrounding context.

## Format

```markdown
## Feature NNN (short title — issue/PR link)

- **Lesson title in bold.** One-or-two-sentence root cause + the lesson. Cite file:line where useful.

- **Another lesson.** Same shape.

- **A third lesson.** (Minimum 3 per feature.)
```

---

## Feature 001 (dev-culture-bootstrap — initial repo conventions)

- **The transferable parts of MyOS2026's development culture are decoupled from its tech stack.** The .specify/ + .claude/ trees, CONTRIBUTING.md workflow rules, CI gate scripts (check-pr-body, docs-gate), Make-target discoverability checklist, and deferral discipline (issue + ROADMAP row) all transfer 1:1 to any project — they're about *how* you work, not *what* you're building. The non-transferable parts (KASAN, dmesg, panic handler, syscall-diff harness, kernel-specific Make targets) are clearly scoped to their domain and were trivial to identify and drop.

- **SSD-preservation via tmpfs target redirection is more important for a single-user dev machine than it looks.** The MyOS pattern (`make tmpfs-setup` → symlink `target/` and `dist/` into `/tmp/<project>/<hash>/`) was originally a nice-to-have. On a finite-write-life SSD with daily heavy iteration across multiple Rust projects, it cuts SSD writes by several GB/day per project. The cost (post-reboot rebuild from scratch) is negligible because Cargo's incremental build is fast. **Mandatory in CLAUDE.md** for this checkout — not optional.

- **The branch name (`main` here) vs (`master` in MyOS) needs to be threaded everywhere, including default `BASE_REF` in CI scripts.** Easy to miss: `scripts/ci/check-pr-body.sh` and `scripts/ci/docs-gate.sh` both default `BASE_REF` to the upstream branch name; if you forget to flip `master`→`main` in the sed substitution, the local docs-gate passes but the GitHub Actions invocation fails because `origin/master` doesn't exist. Caught during transfer; lesson is to grep `master` across all transferred files before committing.

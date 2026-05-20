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

---

## Feature 003 (Phase 1 foundational: scaffold compile fix + T1.04 VfsPath types — #3)

- **The Phase 1 scaffold didn't actually build.** Local `cargo check` passed because the broken bits only surfaced under `clippy -D warnings` and a stricter feature set than the scaffold opted into: `cargonaut-vfs` was missing the `bitflags` workspace dep entirely, re-exported `Sort` from `traits.rs` where it isn't defined, and had a `VfsKind::Symlink` variant whose unboxed `VfsPath` payload tripped `clippy::large_enum_variant`. Lesson: the first real CI run against a fresh scaffold is the "does this build?" check — `cargo check` locally is not enough. Fix in `7d45df9` adds the dep, re-exports `Sort` from `types.rs`, and boxes the symlink target.

- **Centralizing `VfsPath` invariants in `parse()` lets every other method trust the structure.** `VfsPath::segments` (`SmallVec<[SmolStr; 8]>`) never contains `/`, `..`, or empty strings — `parse` is the single enforcement point (`crates/cargonaut-vfs/src/types.rs:27-67`), `join` panics on the same inputs (line 108-115), and `display` / `parent` iterate without re-validating. The rejected alternative was per-method validation, which would bloat every consumer with checks the constructor already guarantees. The cost is that callers must funnel untrusted input through `parse`; this is documented on the `join` doc comment.

- **Proptest's parse/display round-trip is the right correctness target for a URI type, but the generator alphabet has to be constrained to stay inside the type's invariant.** The segment strategy explicitly filters out `".."` (`types.rs:220-222`) because otherwise the generator would produce `VfsPath` values that `parse` rejects, breaking the round-trip by construction. The regex `[a-zA-Z0-9._-]{1,12}` is similarly narrow on purpose: broader URI-legal alphabets shrink poorly when a property fails, blowing up debug time. Narrow generators are a feature for round-trip properties, not a limitation.

---

## Feature 009 (T1.16: cargonaut-config schema expansion + figment loader — #N)

- **Env-var test isolation collides with parallel test execution and forced an API split.** The first GREEN iteration of `Config::load_from_str` merged the `CARGONAUT_*` env layer on every call (matching the spec's stated load order). The `env_var_overrides_toml` test, which uses `figment::Jail::expect_with` to set `CARGONAUT_UI__THEME=monochrome`, was then immediately racing against `load_from_str_with_partial_toml_fills_defaults` — both tests called `load_from_str`, but the second one read the env var the first one had just set process-wide (`crates/cargonaut-config/src/lib.rs:626`). The fix was to split the API: `load_from_str` / `load_from_path` are pure TOML→Config (no env), and `load_from_str_with_env` is the opt-in production-semantics variant. `Config::load()` (the binary's entry point) keeps the full layering. The lesson: any test that uses `std::env::set_var` is in a process-wide race with every other test that reads env, and `figment::Jail` only snapshots+restores — it does not isolate inside the closure. Split env-touching code paths into their own API so unrelated tests don't get polluted.

- **`#[serde(default, deny_unknown_fields)]` can coexist and gives you the cheapest possible "schema validation".** Both attributes on every struct in `lib.rs` mean: missing fields are filled from `Default::default()`, unknown fields are rejected. That matches the schema's `additionalProperties: false` (so a typo'd `wibble = 42` errors instead of silently dropping) AND lets users write partial TOML (`[ui] theme = "dracula"` works without supplying every other field). Without the deny, typos get silently dropped — exactly the class of bug users won't notice for months.

- **The `oneOf: [bool, "auto"]` shape in the JSON schema requires a custom `Visitor`, not just `#[serde(untagged)]`.** `ZoxideMode::Auto | On | Off` serializes to `"auto"` / `true` / `false` respectively (matching FR-211's tri-state). `#[serde(untagged)]` looks like it should work but fails because a single enum variant with no data can't represent both a string AND a bool at deserialize time. The fix is a hand-rolled `Visitor` with both `visit_bool` and `visit_str` (`crates/cargonaut-config/src/lib.rs:112-136`). Lesson: whenever the JSON schema uses `oneOf` with primitive types, expect to write a `Visitor` — the derive macros don't cover this case.

---

## Feature 004 (T1.05: document VfsBackend trait + dyn-dispatch smoke test — #5)

- **Trait object-safety is a load-bearing invariant for downstream callers and deserves a compile-time pin, not just discipline.** `design/data-model.md` says `VfsRef = (Arc<dyn VfsBackend>, VfsPath)` — the transfer engine, UI, and audit log all dispatch through that `Arc<dyn VfsBackend>`. If someone later adds a generic method or a `Self`-returning method to the trait, all those callers break with cryptic "the trait cannot be made into an object" errors. The new `crates/cargonaut-vfs/tests/dyn_dispatch.rs` fails at *compile time* in that scenario via `const _: () = { _assert_send_sync::<dyn VfsBackend>(); };` plus a runtime `Arc<dyn VfsBackend>` construction. Lesson: pin the invariant in code where it lives, not just in a code review checklist.

- **Doc comments on `async-trait` methods need careful intra-doc-link syntax.** The naive `[`X`]:` in a markdown list item parses as a link-reference definition and trips `clippy::doc_nested_refdefs` (an opt-in lint, but on under `-D warnings`). The fix is `[`X`][]:` — add empty brackets after the link target before the colon (`crates/cargonaut-vfs/src/traits.rs:208,210,212`). Also: module-level `Self::caps` doesn't resolve because `Self` only exists inside a trait/impl block — use the explicit `[`VfsBackend::caps`]` instead (`traits.rs:21`). Both were caught by `RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc` in CI; lesson is to run that doc check locally before pushing trait-shape PRs.

- **A `ByteRange::FULL` constant pays for itself the moment the third caller appears.** Originally each callsite would build `ByteRange { start: 0, end: None }` to mean "whole file". The trait docs explicitly call out this as the canonical invariant ("every backend MUST honor it regardless of `VfsCaps::SEEKABLE`"). Introducing the constant (`traits.rs:79-82`) gives the invariant a name + a single place it can be referenced from tests and adapters. Tiny code, big readability win — and worth the early `pub const`.

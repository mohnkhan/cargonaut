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

---

## Feature 002 (T1.06: implement LocalFs over tokio::fs — #6)

- **Bridging tokio's `AsyncRead/Write` into the trait's `futures::AsyncRead/Write` requires the `tokio-util` crate's `compat` cargo feature, which isn't on by default.** The workspace `tokio-util = { features = ["io"] }` declaration was enough for tokio-util's basic IO codecs but doesn't pull in `tokio_util::compat`. First green attempt fails with `no method named compat found for tokio::fs::File`. Fix: override the dep in `crates/cargonaut-vfs/Cargo.toml:11` to add the `compat` feature. Lesson: when adding `tokio_util::compat::TokioAsyncReadCompatExt` (or the matching `Write`/`Seek` traits), check the feature gate; the error is unhelpful because the trait method "is gated here" is hidden by default cargo output.

- **Use `symlink_metadata` everywhere a symlink should be treated as a first-class entry — not just in `stat`.** The intuitive read of "list a directory" calls `metadata` on each entry, which silently follows symlinks. That breaks the trait contract that symlinks must be reported as `VfsKind::Symlink { target }`, not as the kind of their resolved target. Same trap in `unlink`: `tokio::fs::remove_file` happily removes through a symlink, but if you `metadata()` to gate on "is this a file or a directory?" first, you'll follow into a directory the symlink pointed at and then refuse to unlink it. Fix is uniform: every check uses `symlink_metadata` (`crates/cargonaut-vfs/src/local.rs:90,118,237`). Cost is two extra syscalls per entry in `list()`, which is acceptable given the correctness guarantee.

- **`Pin<Box<dyn AsyncRead + Send>>` (and the `Write` analog) doesn't implement `Debug`, so `Result<Pin<...>, _>::unwrap_err()` fails to compile — use `.map(|_| ()).unwrap_err()` in tests.** The trait return type is `Result<Pin<Box<dyn AsyncRead + Send>>, VfsError>`. Calling `unwrap_err()` requires the `Ok` variant to be `Debug` (so the panic message can format it). The trait object isn't `Debug`-friendly. The cheapest workaround in test code is to discard the `Ok` payload with `.map(|_| ()).unwrap_err()` (`crates/cargonaut-vfs/src/local.rs:291,301,379,390`). Don't reach for `let Err(e) = ... else { unreachable!() }` here — the `.map(|_| ())` pattern is one line and has the same effect.

---

## Feature 010 (T1.09: TransferCheckpoint roundtrip property test — #N)

- **Test both compact and pretty serde forms when the type will be human-edited.** `TransferCheckpoint` lives on disk as `.cargonaut-transfer-<uuid>.json` next to the destination file (per FR-006). Operators will occasionally `vim` these to debug a stuck transfer, which means pretty-printed JSON must round-trip equivalently to compact. The single proptest is cheap (`serde_json::to_string` vs `to_string_pretty`) but catches the class of bug where someone introduces a custom `Serialize` impl that emits one form but not the other (`crates/cargonaut-transfer/src/checkpoint.rs:116-123`).

- **A `pub const VERSION` + a "value is 1" unit test is cheap insurance against silent schema bumps.** The temptation is "of course we'd remember to add a migration when bumping the version" — but in practice the bump lands in a routine refactor, the migration is "deferred", and a year later someone has a checkpoint they can't resume. The test (`checkpoint.rs:126-131`) fails loudly the moment the constant changes; the fixer is then forced to also touch the migration path the test exists to remind them of.

- **Proptest's tuple `.prop_map` with eleven fields needs the explicit destructuring closure to compile — there's no way around the 11-line argument list.** A smaller strategy that returned the tuple raw and let `From` adapt it would be more compact, but rejected because adding a new field to `TransferCheckpoint` would then silently break the test rather than force a compile error. The verbose explicit-field-mapping form (`checkpoint.rs:79-105`) is annoying to read but is the right trade-off: adding a field forces a compile failure in the test until the new field is wired into the strategy. Cost is one big diff per field addition; benefit is the proptest can't drift from the struct.

---

## Feature 011 (T1.13: submit_transfer over VfsBackend — #N)

- **Stat the source synchronously before spawning the copy task.** The naive flow is "spawn task → task fails → user sees error via the watch channel". That works but pessimizes the common case of a typo'd source path: the UI shows a transient "starting…" then jumps to "Failed", which is more disorienting than just refusing to start. `submit_transfer` calls `src_backend.stat(&src_path).await?` (`crates/cargonaut-transfer/src/job.rs:50`) and propagates the error as the function's `Result` — caller renders the dialog immediately, no task is spawned, no checkpoint sidecar exists to clean up. Same shape will work for credential failures on remote backends.

- **`pending_chunk: Vec<u8>` accumulates up to one full checkpoint interval before draining — bounded memory but watch the constant.** First implementation grew `pending_chunk` per-read and drained at EOF only; that meant a 10 GiB transfer with the default 16 MiB buffer would hold 10 GiB resident before the first checkpoint write. Fix: drain in chunks of `opts.checkpoint_interval_bytes` (default 8 MiB) inside the read loop (`job.rs:194-228`). Bounded to one buffer + one checkpoint interval = ~24 MiB worst case. NFR-009's 64 MiB ceiling has headroom; would tighten this further when the UI's listing buffers land.

- **Leave the checkpoint sidecar in place on cancel — it's how `Canceled` becomes resumable.** The instinct on cancel is to clean up. But FR-008 says cancel behavior is user-configurable (`[transfer] on_cancel = "delete" | "keep"`), and "keep + checkpoint" is the resume contract. The implementation honors `keep` semantics by default: `cancel.is_cancelled()` check at the top of the loop (`job.rs:121`) skips the cleanup branch entirely. T1.12b's integration test will exercise both branches per the `on_cancel` config; T1.13 deliberately doesn't read the config to keep the engine concern-free of the UI layer.

- **`Pin<Box<dyn AsyncWrite + Send>>::close()` is required to flush; `drop()` is not enough.** First implementation just dropped the writer at end-of-loop. Worked for the small-file test (tokio's File flushes on drop synchronously) but is wrong for buffered writers and remote backends where drop just queues the close, racing the SHA-256 verify step. Fixed in `job.rs:248`: explicit `writer.close().await` before re-reading. Lesson: any code that closes-then-reads must `.close().await` the writer, not rely on `drop`.

---

## Feature 012 (T1.14: implement scan_resumable — #N)

- **Per-sidecar parse failures absorb silently; only listing failures bubble up.** `scan_resumable` returns `Result<Vec<ResumableTransfer>, TransferError>`. The instinct is to short-circuit on the first malformed sidecar — but the user has *several* unrelated transfers that may have happened to leave checkpoints, and one corrupt sidecar shouldn't block the others from being offered. The implementation `continue`s past parse / read / version-mismatch failures (`crates/cargonaut-transfer/src/checkpoint.rs:103-113`) and only propagates the original `dst_backend.list(...)` failure. Same pattern as `git status` — one untracked weird file doesn't fail the whole walk.

- **The source-backend registry doesn't exist in Phase 1, so `source_unchanged` must be conservative for non-`file://` schemes.** A future plugin registry will resolve any URI scheme to its backend, but for now scan_resumable hardcodes `file://` → `LocalFs` and reports `source_unchanged = false` for everything else (`checkpoint.rs:120-128`). False here means "we couldn't verify" — the user can still choose to resume, but the UI should distinguish "verified clean" from "unverified" rather than collapse both into "go". This is the correct conservative default: claiming `true` when we don't know would be a lie.

- **The CRC chain validator chunks the destination at `checkpoint.chunk_size_bytes`, not at the engine's `buffer_size_bytes`.** Subtle: when `submit_transfer` writes the sidecar it records the *checkpoint interval* size, not the *read buffer* size. Both default to different values (8 MiB vs 16 MiB). The validator (`checkpoint.rs:197-225`) reads the destination in fixed-size chunks matching the recorded `chunk_size_bytes` — chunking at any other size would compute different CRCs even on identical bytes. Treated as a unit-test-locked invariant: the per-chunk validation loop must use the sidecar's chunk size, not a local constant.

- **`ResumableTransfer.checkpoint_path` is `std::path::PathBuf`, not `VfsPath` — Phase 1 forced compromise.** The struct field is defined as `PathBuf` (data-model decision predating this PR). For LocalFs paths that's fine; for a remote backend it would be wrong. Introduced a private helper `vfs_path_to_local_pathbuf` (`checkpoint.rs:156-162`) that bypasses the trait and synthesizes a `PathBuf` from segments. Documented as Phase 1 only; revisit when remote backends land.

---

## Feature 013 (T1.15: implement resume_transfer — #N)

- **`resume_transfer` preserves the original job id from the checkpoint — same id across resume cycles is load-bearing for the audit log.** The naive thing is to generate a fresh UUID for the resumed transfer. But then the audit log can't correlate "transfer X failed at offset N" with "transfer X resumed at offset N+1" — they look like unrelated events. `resume_transfer` parses `checkpoint.job_id` and reuses it (`crates/cargonaut-transfer/src/job.rs:43`); only falls back to a new UUID if the stored id is malformed (defensive — should never happen for a sidecar this engine wrote). The "preserve job id" invariant has its own test (`job.rs::tests::resume_preserves_job_id_from_checkpoint`).

- **Defensive CRC re-validation at resume time, even though `scan_resumable` just did it.** `scan_resumable` might have run minutes (or hours) ago when the user opened the file manager; the destination file could have been edited between then and the user clicking "resume". `resume_transfer` re-runs `verify_dst_crc_chain` (`job.rs:36-44`) and fails fast with `Err` (no spawn) if the chain doesn't match — the UI gets immediate feedback rather than a transient Failed state. The cost is one full-file re-read of the destination, which is bounded by `bytes_written` (not full source size) so it's cheap on partial transfers.

- **The from-scratch and resume copy loops are duplicated, not factored — accepted for Phase 1.** First instinct was to extract a `run_transfer_inner` taking a `StartState` enum. Did NOT do that because: (1) the parameter list would still be 11+ items, (2) the only differences are 3 lines (initial bytes_written, initial chunk_crcs, src open range, dst open mode) — about 1/15 of the loop body, (3) the factored version would be harder to follow for someone touching only one of the two callers. Re-evaluate in Phase 6 perf tuning if the duplication has grown a non-trivial divergence; for Phase 1 simplicity wins. `run_transfer` (submit path, `job.rs:197`) and `run_transfer_with_state` (resume path, `job.rs:130`) live next to each other so divergence is obvious in review.

- **The 3-tuple of `(bytes_written, chunk_size, chunk_crcs.len())` is a hidden invariant that must hold for resume to make sense: `bytes_written == chunk_size * chunk_crcs.len()`.** Otherwise the resume offset doesn't land on a chunk boundary and CRC validation can't reconstruct the chain. `submit_transfer` maintains this invariant by only checkpointing on full-chunk boundaries. `resume_transfer` continues the invariant by appending to `chunk_crcs` only when `pending_chunk.len() >= interval`. Test setup helper `stage_partial_transfer` (`job.rs::tests:328`) `assert!`s `bytes_already_done % chunk_size == 0` to catch invariant violations at fixture-construction time, before they cascade into confusing test failures.

---

## Feature 014 (T1.18: keymap parser — #N)

- **Multi-key chord bindings (e.g. `C-x !`) force the data model to be `Vec<KeyChord>`, not a single `KeyChord` — and the dispatcher needs three lookup outcomes, not two.** First pass modeled bindings as `HashMap<(Mode, KeyChord), Command>` and choked on the 5 multi-chord bindings in the default keymap (FR-205 `C-x !`, FR-208 `C-x r`, FR-209 `C-x X`, FR-305 `C-x d` + `C-x C-d`). Refactored: `KeySequence = Vec<KeyChord>` with `parse_key_sequence` splitting on whitespace. The dispatcher (T1.19) now needs three states from `lookup_sequence`: `Command(c)` (full match → dispatch), `Pending` (prefix of a longer binding → wait for next chord), `NoMatch` (unbound → ignore). Without `Pending`, the user would press `Ctrl-x` and get a beep instead of the dispatcher correctly waiting for the second key.

- **Enumerating every action up-front beats parsing free-form strings.** `Command` is a 60-variant enum with `#[serde(rename_all = "kebab-case")]`; typo'd actions in the TOML cause `Keymap::load` to error with the full list of valid names (`crates/cargonaut-ui-tui/src/keymap.rs:48-193`). Alternative would be `Command(String)` — convenient but defers the typo discovery to runtime (silent no-op when a binding fires for a string the dispatcher doesn't recognize). The enum is verbose to maintain but each new action is one variant + a code path, both of which the compiler enforces stay in sync.

- **`#[serde(rename_all = "lowercase")]` is not the same as `kebab-case` and the choice for `Mode` vs `Command` is deliberate.** Modes (`global`, `pane`, `dialog`, `search`, `subshell`, `preview`) are single words → `lowercase`. Actions (`focus-swap-pane`, `move-or-rename-selection`) are multi-word → `kebab-case`. Mixing them up causes silent parse failures with no helpful error (serde just says "unknown variant"). Caught when adding the `preview` mode (single word, but I'd initially set it under `kebab-case` and the test still passed because no hyphen).

- **`include_str!("../../../design/contracts/keymap.toml")` is the right test pattern for "does the bundled default file parse?".** Tests against an in-tree file would be brittle (depends on CWD); including the file at compile time pins the test to the canonical file the build will ship. If someone adds a binding referencing a new action and forgets to add the enum variant, `parses_full_default_keymap_without_error` fails at compile-test time with the full list of expected variants — a cheap canary for keymap drift.

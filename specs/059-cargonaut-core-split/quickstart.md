# Quickstart / Validation Guide: cargonaut-core God-File Split

**Feature**: 059-cargonaut-core-split

This guide proves the refactor achieved its goal: a thin `lib.rs`, an unchanged public API, and unchanged behavior. Run every check from the repo root on branch `059-cargonaut-core-split`.

## Prerequisites

- tmpfs symlink active (Constitution §V): `make tmpfs-status` shows `target → /tmp/cargonaut/<hash>/target`.
- Default nightly toolchain (for rustdoc JSON) + `python3` (for the surface extractor). `jq` is **not** required.

## 1. Public API is byte-for-byte stable (FR-003 / SC-003)

```bash
# Regenerate the current public surface and diff against the committed baseline.
cargo +nightly rustdoc -p cargonaut-core -- -Z unstable-options --output-format json
python3 specs/059-cargonaut-core-split/contracts/extract-public-api.py \
    "$(readlink -f target)/doc/cargonaut_core.json" > /tmp/surface-after.txt
diff specs/059-cargonaut-core-split/contracts/public-api-baseline.txt /tmp/surface-after.txt
```

**Expected**: no output (exit 0). Any line of diff is a public-API change and a failure.

## 2. Downstream crates + benches compile with zero edits (FR-004 / SC-004)

```bash
git diff --stat origin/main -- crates/cargonaut-ui-tui/src crates/cargonaut-transfer/src crates/cargonaut-bin/src
# Expected: empty (no downstream src changed).

make build              # full workspace build (tmpfs-guarded)
cargo build -p cargonaut-core --benches   # benches are an API consumer too
```

**Expected**: clean build; the benches compile against the re-exported surface unchanged.

## 3. Behavior unchanged — full suite passes, same count (FR-005 / FR-006 / SC-004)

```bash
# Count before (on origin/main) vs after should match.
cargo test --workspace 2>&1 | grep -E 'test result:'
```

**Expected**: every pre-existing test passes; the total number of executed tests equals the pre-refactor count (no test silently dropped). Compare against `git stash` / `origin/main` if in doubt.

## 4. Quality gates green (FR-008 / FR-009 / SC-005)

```bash
cargo clippy --workspace --all-targets -- -D warnings
RUSTDOCFLAGS="-D rustdoc::broken-intra-doc-links" cargo doc -p cargonaut-core --no-deps
cargo fmt --check
```

**Expected**: zero warnings; docs build clean (no *new* warnings vs. the one pre-existing unrelated `private_intra_doc_links` note); fmt clean.

## 5. `lib.rs` is a thin module root (FR-001 / FR-010 / SC-001 / SC-006)

```bash
wc -l crates/cargonaut-core/src/lib.rs                 # expect ≤ ~230
grep -c '^    fn \|^    pub fn \|^    async fn ' crates/cargonaut-core/src/lib.rs   # expect 0 (no methods left)
grep -c '#\[cfg(test)\]' crates/cargonaut-core/src/lib.rs                          # expect 0
ls crates/cargonaut-core/src/*.rs                       # expect ≥ 4 (we target ~14) focused modules
wc -l crates/cargonaut-core/src/*.rs | sort -n | tail   # no module a new god-file
```

**Expected**: `lib.rs` small; no methods or tests remain in it; ≥4 (≈12–13) cohesive submodules; no submodule is itself a god-file.

## 6. Full pipeline (the merge gate)

```bash
make ci-local     # clippy → test → release build → check-pr-body → docs-gate
```

**Expected**: green. This is the same pipeline CI runs; it must pass before the PR can merge.

## One-shot

```bash
make ci-local && \
cargo +nightly rustdoc -p cargonaut-core -- -Z unstable-options --output-format json && \
python3 specs/059-cargonaut-core-split/contracts/extract-public-api.py "$(readlink -f target)/doc/cargonaut_core.json" > /tmp/s.txt && \
diff specs/059-cargonaut-core-split/contracts/public-api-baseline.txt /tmp/s.txt && \
echo "ALL GREEN: API stable + behavior preserved + gates pass"
```

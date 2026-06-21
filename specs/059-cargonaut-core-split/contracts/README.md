# Contract: cargonaut-core Public API Surface

This directory holds the **API-stability gate** for Feature 059. The refactor is move-only; the public surface of `cargonaut-core` must be identical before and after.

## Files

- **`public-api-baseline.txt`** — the normalized public surface captured **before** the split (179 items: 7 structs, 10 enums / 73 variants, 28 public fields, 54 methods, 3 free fns, 4 re-exports). This is the ground truth.
- **`extract-public-api.py`** — renders a `cargonaut-core` rustdoc-JSON file into the same normalized format, sorted, one item per line. Categories: `struct`, `enum`, `variant`, `field`, `method TYPE::name`, `fn`, `const`, `type`, `reexport`.

## How to check (after any change to the crate)

```bash
cargo +nightly rustdoc -p cargonaut-core -- -Z unstable-options --output-format json
python3 specs/059-cargonaut-core-split/contracts/extract-public-api.py \
    "$(readlink -f target)/doc/cargonaut_core.json" > /tmp/surface-after.txt
diff specs/059-cargonaut-core-split/contracts/public-api-baseline.txt /tmp/surface-after.txt
```

Empty diff ⇒ surface preserved. Any line ⇒ a public-API change (must be zero for this feature).

## Why two proofs

This name-level surface diff is **necessary but not alone sufficient** — it would miss a same-named method whose signature changed. The authoritative complement is compiling and testing every **downstream consumer** (`cargonaut-ui-tui`, `cargonaut-transfer`, `cargonaut-bin`) **and the `cargonaut-core` benches** with zero source edits (see `../quickstart.md` §2–3). Together they bracket "public API unchanged" from producer and consumer sides.

## Notes / limitations

- Requires the (default) nightly toolchain for rustdoc JSON; `jq` is intentionally avoided (not installed on the dev host) — extraction is pure Python 3.
- The extractor reads only the local crate (`crate_id == 0`) and public/default-visibility items, mirroring what `rustdoc` exposes without `--document-private-items`.
- rustdoc emits one pre-existing, unrelated `private_intra_doc_links` warning for this crate; it predates Feature 059 and is not the gated lint (the gated lint is `broken-intra-doc-links`).

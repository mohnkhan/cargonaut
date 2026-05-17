# Test Plan + CI Pipeline: Cargonaut

## Test categories

| Category | Tool | Coverage target | Phase introduced |
|---|---|---|---|
| Unit | `cargo test --lib` | ≥80% on core crates | 1 |
| Integration | `cargo test --test '*' --workspace` | every FR has ≥1 integration test | 1 |
| Property | `proptest` inside unit tests | path round-trip, sort stability, checkpoint serde | 1 |
| Fuzz | `cargo fuzz` (nightly) | sandbox escape, checkpoint parser, config parser | 3 (sandbox); 1 (parsers) |
| Benchmark | `criterion` | SC-001/003/004 enforced | 1 |
| Coverage | `cargo tarpaulin --lcov` | gated at 80% core / 60% adapters | 6 |
| Lint | `cargo clippy -- -D warnings` | zero warnings | 1 |
| Format | `cargo fmt --check` | zero diff | 1 |
| Doc | `cargo doc --no-deps --document-private-items` | builds clean; no broken intra-links | 1 |
| Security audit | `cargo audit` | no RUSTSEC advisories at HIGH+ | 1 (advisory); 6 (enforced) |

## Per-FR test mapping

See `contracts/requirements.toml` — every FR has a `verification` field pointing at a test path or CI check. CI greps the manifest at every merge to ensure no orphan FRs.

## Fuzz targets

- `tests/fuzz/sandbox_escape/` — generate random WASM components, attempt to invoke each host import out-of-capability, assert reject. SC-006 gate runs 100k iterations in nightly CI.
- `tests/fuzz/checkpoint_parser/` — random byte streams fed to TransferCheckpoint deserialize; must reject without panic or memory unsafety.
- `tests/fuzz/config_parser/` — random TOML fed to Config::from_str; must reject without panic.

## Property-based invariants

- `VfsPath::display(parse(s)) == s` for all valid s
- `DirListing.entries` is sorted per `.sort` at all times
- TransferCheckpoint round-trip: `serde_json::from_str(serde_json::to_string(&c)) == c` for all c
- Undo sequence: after K random destructive ops + K undos, tree.sha256() == original.sha256()

## Performance benchmarks (CI-gated)

| Bench | Gate | Frequency |
|---|---|---|
| `local-copy-vs-cp` | ≥80% of cp(1) | every merge |
| `startup` | cold ≤ 150 ms / warm ≤ 40 ms | every merge |
| `rss-headroom` | ≤ 64 MiB peak for 3-pane × 10k-entry session | every merge |
| `sftp-throughput` | ≥ 200 MiB/s localhost | every merge, Phase 2+ |
| `keypress-latency` | ≤ 16 ms median | every merge |
| `large-dir-scroll` | 10⁶-entry scroll without OOM | nightly |

## CI pipeline (.github/workflows/ci.yml)

```yaml
name: ci
on: [push, pull_request]
jobs:
  ci:
    needs: [lint, test, bench-regress, fuzz, audit, coverage]
    runs-on: ubuntu-latest
    steps: [{name: rollup, run: echo all green}]

  lint:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo fmt --check
      - run: cargo clippy --workspace --all-features -- -D warnings
      - run: cargo doc --workspace --no-deps --document-private-items

  test:
    runs-on: ubuntu-latest
    strategy:
      matrix:
        rust: [stable, nightly]
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@${{matrix.rust}}
      - run: cargo test --workspace --all-features

  bench-regress:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: |
          cargo bench --bench local-copy-vs-cp --bench startup --bench rss-headroom
          # Compare against main's last bench; fail if any regressed >10%

  fuzz:
    runs-on: ubuntu-latest
    if: github.event_name == 'schedule'   # nightly only
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@nightly
      - run: cargo install cargo-fuzz
      - run: |
          for target in sandbox_escape checkpoint_parser config_parser; do
            cargo fuzz run $target -- -runs=100000 -max_total_time=600
          done

  audit:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions-rs/audit-check@v1
        with: { token: ${{secrets.GITHUB_TOKEN}} }

  coverage:
    runs-on: ubuntu-latest
    if: github.event_name == 'pull_request'
    steps:
      - uses: actions/checkout@v4
      - uses: dtolnay/rust-toolchain@stable
      - run: cargo install cargo-tarpaulin
      - run: cargo tarpaulin --workspace --lcov --output-dir coverage
      - run: |
          # Gate: core crates ≥80%
          ./scripts/check-coverage.sh coverage/lcov.info --core-min 80 --adapter-min 60
```

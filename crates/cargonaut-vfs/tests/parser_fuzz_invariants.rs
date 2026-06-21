// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Feature 063 (US1) — always-on randomized invariant gate for the untrusted-
//! input parsers. Runs in normal `cargo test` (stable, no nightly/cargo-fuzz),
//! so every PR is protected. The coverage-guided `cargo-fuzz` targets in `fuzz/`
//! complement this for deeper local / nightly fuzzing.
//!
//! Invariant (FR-001): `VfsPath::parse`, `ModeSpec::parse`, `parse_owner` MUST
//! return `Ok`/`Err` on ANY input — never panic. proptest reports a panic as a
//! failing case with the minimal reproducing input.

use cargonaut_vfs::{parse_owner, ModeSpec, VfsPath};
use proptest::prelude::*;

proptest! {
    // ≥ 1000 cases per parser (SC-001). Two generators each: arbitrary unicode
    // strings, and arbitrary byte buffers decoded lossily (covers invalid UTF-8,
    // NULs, control chars).
    #![proptest_config(ProptestConfig::with_cases(1500))]

    #[test]
    fn vfspath_parse_never_panics(s in ".*") {
        let _ = VfsPath::parse(&s);
    }

    #[test]
    fn vfspath_parse_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..512)) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = VfsPath::parse(&s);
    }

    #[test]
    fn modespec_parse_never_panics(s in ".*") {
        let _ = ModeSpec::parse(&s);
    }

    #[test]
    fn modespec_parse_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = ModeSpec::parse(&s);
    }

    #[test]
    fn parse_owner_never_panics(s in ".*") {
        let _ = parse_owner(&s);
    }

    #[test]
    fn parse_owner_bytes_never_panics(bytes in proptest::collection::vec(any::<u8>(), 0..64)) {
        let s = String::from_utf8_lossy(&bytes);
        let _ = parse_owner(&s);
    }

    // Roundtrip (FR-003): a path that parses must re-parse from its own rendered
    // form. We assert re-parse succeeds (display() is the canonical rendering);
    // strict structural equality is intentionally not asserted because display()
    // may normalize.
    #[test]
    fn vfspath_display_reparses(s in ".*") {
        if let Ok(p) = VfsPath::parse(&s) {
            let rendered = p.display();
            prop_assert!(
                VfsPath::parse(&rendered).is_ok(),
                "rendered VfsPath failed to re-parse: {rendered:?}"
            );
        }
    }
}

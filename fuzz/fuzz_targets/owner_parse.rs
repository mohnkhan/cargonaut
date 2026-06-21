// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0
//! Feature 063: coverage-guided fuzz target for `parse_owner` (FR-004).
//! Invariant: parsing arbitrary bytes never panics.
#![no_main]

use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: &[u8]| {
    if let Ok(s) = std::str::from_utf8(data) {
        let _ = cargonaut_vfs::parse_owner(s);
    }
});

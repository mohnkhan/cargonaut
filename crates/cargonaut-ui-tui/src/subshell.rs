// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! Persistent subshell panel state (Feature 054, FR-001..FR-015).

// =====================================================================
// Tests (T006 red → green)
// =====================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // Compile-time proof that SubshellState has the required fields.
    // This will fail to compile until T007 adds SubshellState.
    #[allow(dead_code)]
    fn _assert_subshell_state_fields(s: &SubshellState) {
        let _dead: bool = s.dead;
        let _offset: u16 = s.scroll_offset;
    }

    #[test]
    fn subshell_state_struct_fields() {
        let _: fn(&SubshellState) = _assert_subshell_state_fields;
    }
}

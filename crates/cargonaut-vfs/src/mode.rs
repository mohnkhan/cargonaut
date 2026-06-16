// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! chmod mode parsing (Feature 043).
//!
//! A pure, I/O-free representation of a permission-change request, parsed once
//! and applied per file. Octal forms (`644`, `0755`) are absolute; symbolic
//! forms (`u+x`, `go-w`, `a=r`, comma-separated) are applied relative to each
//! file's current mode. This is the SC-004 invalid-input gate — malformed input
//! never reaches the filesystem.

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn octal_is_absolute_ignoring_current() {
        assert_eq!(ModeSpec::parse("644").unwrap().apply(0o777), 0o644);
        assert_eq!(ModeSpec::parse("0755").unwrap().apply(0o000), 0o755);
        assert_eq!(ModeSpec::parse("600").unwrap().apply(0o123), 0o600);
    }

    #[test]
    fn symbolic_is_relative_to_current() {
        assert_eq!(ModeSpec::parse("u+x").unwrap().apply(0o644), 0o744);
        assert_eq!(ModeSpec::parse("go-w").unwrap().apply(0o666), 0o644);
        assert_eq!(ModeSpec::parse("a=r").unwrap().apply(0o751), 0o444);
        assert_eq!(ModeSpec::parse("u+x,g+x").unwrap().apply(0o644), 0o754);
        // omitted "who" defaults to all
        assert_eq!(ModeSpec::parse("+x").unwrap().apply(0o644), 0o755);
    }

    #[test]
    fn invalid_input_is_rejected() {
        assert_eq!(ModeSpec::parse(""), Err(ModeError::Empty));
        assert_eq!(ModeSpec::parse("   "), Err(ModeError::Empty));
        assert_eq!(ModeSpec::parse("999"), Err(ModeError::BadOctal));
        assert_eq!(ModeSpec::parse("8"), Err(ModeError::BadOctal));
        assert_eq!(ModeSpec::parse("12345"), Err(ModeError::BadOctal));
        assert_eq!(ModeSpec::parse("xyz"), Err(ModeError::BadSymbolic));
        assert_eq!(ModeSpec::parse("u?x"), Err(ModeError::BadSymbolic));
        assert_eq!(ModeSpec::parse("q+x"), Err(ModeError::BadSymbolic));
    }
}

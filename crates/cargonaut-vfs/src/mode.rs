// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! chmod mode parsing (Feature 043).
//!
//! A pure, I/O-free representation of a permission-change request, parsed once
//! and applied per file. Octal forms (`644`, `0755`) are absolute; symbolic
//! forms (`u+x`, `go-w`, `a=r`, comma-separated) are applied relative to each
//! file's current mode. This is the SC-004 invalid-input gate — malformed input
//! never reaches the filesystem.

/// A parsed, validated chmod request — pure, no I/O.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ModeSpec {
    /// Absolute octal permission bits (low 9 bits), e.g. `0o644`.
    Octal(u32),
    /// One or more symbolic clauses applied left-to-right, relative to the
    /// file's current mode.
    Symbolic(Vec<SymClause>),
}

/// One symbolic clause, e.g. `u+x` or `go-w` or `a=r`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SymClause {
    /// Which permission groups it affects: bit 2 = user, bit 1 = group, bit 0 = other.
    who: u8,
    /// The operator.
    op: Op,
    /// The rwx permission mask (r=4, w=2, x=1) the operator applies.
    perms: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Op {
    /// `+` — set the named bits.
    Add,
    /// `-` — clear the named bits.
    Remove,
    /// `=` — replace that group's bits.
    Set,
}

/// Why a mode string could not be parsed (→ `AppError::BadAttr`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModeError {
    /// Empty or whitespace-only input.
    Empty,
    /// Looks octal (leading digit) but is malformed (bad digit or length).
    BadOctal,
    /// Malformed symbolic clause.
    BadSymbolic,
}

const WHO_USER: u8 = 0b100;
const WHO_GROUP: u8 = 0b010;
const WHO_OTHER: u8 = 0b001;
const WHO_ALL: u8 = WHO_USER | WHO_GROUP | WHO_OTHER;

impl ModeSpec {
    /// Parse a chmod request. Input starting with a digit is treated as octal
    /// (3–4 octal digits); otherwise it is parsed as comma-separated symbolic
    /// clauses (`[ugoa]*[+-=][rwx]*`).
    pub fn parse(input: &str) -> Result<ModeSpec, ModeError> {
        let s = input.trim();
        if s.is_empty() {
            return Err(ModeError::Empty);
        }
        if s.chars().next().unwrap().is_ascii_digit() {
            // Octal: 3 or 4 digits, each 0-7.
            if !(3..=4).contains(&s.len()) || !s.bytes().all(|b| (b'0'..=b'7').contains(&b)) {
                return Err(ModeError::BadOctal);
            }
            let bits = u32::from_str_radix(s, 8).map_err(|_| ModeError::BadOctal)?;
            return Ok(ModeSpec::Octal(bits & 0o7777));
        }
        let mut clauses = Vec::new();
        for part in s.split(',') {
            clauses.push(parse_clause(part)?);
        }
        Ok(ModeSpec::Symbolic(clauses))
    }

    /// Resolve to concrete permission bits. `Octal` ignores `current`; `Symbolic`
    /// applies each clause to `current` (low 9 bits) in order.
    pub fn apply(&self, current_bits: u32) -> u32 {
        match self {
            ModeSpec::Octal(bits) => *bits,
            ModeSpec::Symbolic(clauses) => {
                let mut bits = current_bits & 0o777;
                for c in clauses {
                    for (who_bit, shift) in [(WHO_USER, 6), (WHO_GROUP, 3), (WHO_OTHER, 0)] {
                        if c.who & who_bit == 0 {
                            continue;
                        }
                        let cur = (bits >> shift) & 0o7;
                        let next = match c.op {
                            Op::Add => cur | c.perms,
                            Op::Remove => cur & !c.perms,
                            Op::Set => c.perms,
                        };
                        bits = (bits & !(0o7 << shift)) | (next << shift);
                    }
                }
                bits
            }
        }
    }
}

fn parse_clause(part: &str) -> Result<SymClause, ModeError> {
    let mut who: u8 = 0;
    let mut chars = part.chars().peekable();
    while let Some(&c) = chars.peek() {
        match c {
            'u' => who |= WHO_USER,
            'g' => who |= WHO_GROUP,
            'o' => who |= WHO_OTHER,
            'a' => who |= WHO_ALL,
            _ => break,
        }
        chars.next();
    }
    if who == 0 {
        who = WHO_ALL; // omitted "who" => all
    }
    let op = match chars.next() {
        Some('+') => Op::Add,
        Some('-') => Op::Remove,
        Some('=') => Op::Set,
        _ => return Err(ModeError::BadSymbolic),
    };
    let mut perms = 0u32;
    for c in chars {
        match c {
            'r' => perms |= 0o4,
            'w' => perms |= 0o2,
            'x' => perms |= 0o1,
            _ => return Err(ModeError::BadSymbolic),
        }
    }
    Ok(SymClause { who, op, perms })
}

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

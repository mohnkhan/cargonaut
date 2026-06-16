// Copyright (c) 2024-2026 Mohiuddin Khan Inamdar.
// SPDX-License-Identifier: MIT OR Apache-2.0

//! chown owner-string parsing (Feature 043).
//!
//! Parses a `user`, `:group`, or `user:group` spec where each side is either a
//! numeric id or a name resolved via `nix` (no `unsafe`). Returns the
//! `(uid, gid)` pair to pass to [`crate::VfsBackend::chown`]; `None` for an
//! omitted side leaves that field unchanged.

/// Why an owner spec could not be parsed/resolved (→ `AppError::BadAttr`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OwnerError {
    /// Neither a user nor a group was given.
    Empty,
    /// The user name does not resolve to an account.
    UnknownUser(String),
    /// The group name does not resolve to a group.
    UnknownGroup(String),
}

/// Parse `user[:group]` / `:group` into `(uid, gid)`. Each side may be a numeric
/// id or a name (resolved via the system user/group database). An omitted side
/// is `None` (leave unchanged). At least one side must be present.
pub fn parse_owner(spec: &str) -> Result<(Option<u32>, Option<u32>), OwnerError> {
    let spec = spec.trim();
    let (user_part, group_part) = match spec.split_once(':') {
        Some((u, g)) => (u.trim(), Some(g.trim())),
        None => (spec, None),
    };
    let uid = resolve_user(user_part)?;
    let gid = match group_part {
        Some(g) => resolve_group(g)?,
        None => None,
    };
    if uid.is_none() && gid.is_none() {
        return Err(OwnerError::Empty);
    }
    Ok((uid, gid))
}

fn resolve_user(s: &str) -> Result<Option<u32>, OwnerError> {
    if s.is_empty() {
        return Ok(None);
    }
    if let Ok(n) = s.parse::<u32>() {
        return Ok(Some(n));
    }
    match nix::unistd::User::from_name(s) {
        Ok(Some(u)) => Ok(Some(u.uid.as_raw())),
        _ => Err(OwnerError::UnknownUser(s.to_string())),
    }
}

fn resolve_group(s: &str) -> Result<Option<u32>, OwnerError> {
    if s.is_empty() {
        return Ok(None);
    }
    if let Ok(n) = s.parse::<u32>() {
        return Ok(Some(n));
    }
    match nix::unistd::Group::from_name(s) {
        Ok(Some(g)) => Ok(Some(g.gid.as_raw())),
        _ => Err(OwnerError::UnknownGroup(s.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn numeric_both_sides() {
        assert_eq!(parse_owner("1000:1000"), Ok((Some(1000), Some(1000))));
    }

    #[test]
    fn group_only_and_user_only() {
        assert_eq!(parse_owner(":1000"), Ok((None, Some(1000))));
        assert_eq!(parse_owner("0"), Ok((Some(0), None)));
    }

    #[test]
    fn empty_is_error() {
        assert_eq!(parse_owner(""), Err(OwnerError::Empty));
        assert_eq!(parse_owner(":"), Err(OwnerError::Empty));
        assert_eq!(parse_owner("   "), Err(OwnerError::Empty));
    }

    #[test]
    fn unknown_name_is_error() {
        assert!(matches!(
            parse_owner("no_such_user_xyzzy_42"),
            Err(OwnerError::UnknownUser(_))
        ));
        assert!(matches!(
            parse_owner(":no_such_group_xyzzy_42"),
            Err(OwnerError::UnknownGroup(_))
        ));
    }
}

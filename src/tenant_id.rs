// SPDX-License-Identifier: MIT
//! A validated tenant identifier.
//!
//! A tenant id reaches the file system (`tenant_<id>/service_<uuid>.mem`), so it is parsed once, at
//! the wire boundary, and only a parsed [`TenantId`] is used to build a path. A string that holds a
//! separator, a dot or anything else outside `[A-Za-z0-9_-]` can not become one.

use std::fmt;

/// The longest tenant id accepted.
pub const MAX_TENANT_ID_LEN: usize = 64;

/// A tenant id that is safe to use as one file-name component.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct TenantId(String);

/// Why a string is not a valid tenant id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InvalidTenantId {
    /// The string is empty.
    Empty,
    /// The string is longer than [`MAX_TENANT_ID_LEN`] bytes.
    TooLong,
    /// The string holds a character outside `[A-Za-z0-9_-]`.
    BadCharacter,
}

impl fmt::Display for InvalidTenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Empty => write!(f, "tenant id is empty"),
            Self::TooLong => write!(f, "tenant id is longer than {MAX_TENANT_ID_LEN} bytes"),
            Self::BadCharacter => write!(f, "tenant id may only use A-Z, a-z, 0-9, '_' and '-'"),
        }
    }
}

impl std::error::Error for InvalidTenantId {}

impl TenantId {
    /// Parse a tenant id: 1 to 64 characters from `[A-Za-z0-9_-]`.
    pub fn parse(s: &str) -> Result<Self, InvalidTenantId> {
        if s.is_empty() {
            return Err(InvalidTenantId::Empty);
        }
        if s.len() > MAX_TENANT_ID_LEN {
            return Err(InvalidTenantId::TooLong);
        }
        if !s
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b == b'_' || b == b'-')
        {
            return Err(InvalidTenantId::BadCharacter);
        }
        Ok(Self(s.to_owned()))
    }

    /// The id as a string.
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The directory that holds this tenant's service files. Always one path component.
    pub fn dir_name(&self) -> String {
        format!("tenant_{}", self.0)
    }
}

impl fmt::Display for TenantId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_ordinary_ids() {
        for ok in ["a", "my_tenant", "Org-42", "t_0123abcd", &"x".repeat(64)] {
            assert_eq!(TenantId::parse(ok).unwrap().as_str(), ok);
        }
    }

    #[test]
    fn rejects_path_tricks() {
        for bad in [
            "..",
            ".",
            "a/../../x",
            "a\\..\\x",
            "/etc",
            "C:",
            "a b",
            "a.b",
            "a\0b",
            "a\nb",
            "é",
            "a:b",
        ] {
            assert_eq!(
                TenantId::parse(bad),
                Err(InvalidTenantId::BadCharacter),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn rejects_empty_and_long() {
        assert_eq!(TenantId::parse(""), Err(InvalidTenantId::Empty));
        assert_eq!(
            TenantId::parse(&"x".repeat(65)),
            Err(InvalidTenantId::TooLong)
        );
    }

    #[test]
    fn dir_name_is_one_component() {
        let id = TenantId::parse("acme-1").unwrap();
        assert_eq!(id.dir_name(), "tenant_acme-1");
        assert!(!id.dir_name().contains(['/', '\\']));
    }
}

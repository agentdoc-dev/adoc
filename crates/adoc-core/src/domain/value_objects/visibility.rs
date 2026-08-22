//! Visibility classification value object (E1.1.T3, ADR-0058 §3).
//!
//! The authored per-object visibility vocabulary is the closed set
//! `public | internal | restricted`. Absence means `public` by definition and
//! is neither serialized nor hashed; an authored value is typed, serialized,
//! and hash-included. An invalid value fails closed with
//! `schema.visibility_invalid` — never a silent default.

use std::collections::BTreeMap;
use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A visibility classification with constructor-asserted validity.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Visibility {
    Public,
    Internal,
    Restricted,
}

/// Why a visibility string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VisibilityError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but not one of the canonical visibilities.
    Invalid(String),
}

/// The canonical closed set, rendered for diagnostics.
pub(crate) const VISIBILITY_CLOSED_SET_HELP: &str = "public, internal, restricted";

impl Visibility {
    /// Parse a visibility from a string slice. ASCII-trims, then matches the
    /// canonical lowercase set; empty input is [`VisibilityError::Missing`]
    /// and any other spelling (including miscased) is
    /// [`VisibilityError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, VisibilityError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(VisibilityError::Missing);
        }
        match trimmed {
            "public" => Ok(Self::Public),
            "internal" => Ok(Self::Internal),
            "restricted" => Ok(Self::Restricted),
            _ => Err(VisibilityError::Invalid(trimmed.to_string())),
        }
    }

    /// The canonical lowercase rendering of this visibility.
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Public => "public",
            Self::Internal => "internal",
            Self::Restricted => "restricted",
        }
    }
}

impl fmt::Display for Visibility {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Parse an authored `field_visibility` value — a comma-separated list of
/// `field=visibility` entries (e.g. `owner=internal, cost=restricted`) —
/// into a field → canonical-visibility map. Field names are carried, never
/// checked against the kind's schema (carriage only; enforcement is E6).
/// Any entry without a `=`, with an empty field name, or with an invalid
/// visibility fails the whole value.
pub(crate) fn parse_field_visibility(
    value: &str,
) -> Result<BTreeMap<String, String>, VisibilityError> {
    let mut entries = BTreeMap::new();
    for entry in value.split(',') {
        let entry = trim_ascii_edges(entry);
        if entry.is_empty() {
            return Err(VisibilityError::Invalid(value.trim().to_string()));
        }
        let Some((field, visibility)) = entry.split_once('=') else {
            return Err(VisibilityError::Invalid(entry.to_string()));
        };
        let field = trim_ascii_edges(field);
        if field.is_empty() {
            return Err(VisibilityError::Invalid(entry.to_string()));
        }
        let visibility = Visibility::try_new(visibility).map_err(|_| match visibility.trim() {
            "" => VisibilityError::Invalid(entry.to_string()),
            other => VisibilityError::Invalid(other.to_string()),
        })?;
        entries.insert(field.to_string(), visibility.as_str().to_string());
    }
    if entries.is_empty() {
        return Err(VisibilityError::Missing);
    }
    Ok(entries)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visibility_accepts_only_canonical_values() {
        for value in ["public", "internal", "restricted"] {
            let visibility = Visibility::try_new(value).expect("canonical visibility");
            assert_eq!(visibility.as_str(), value);
        }
    }

    #[test]
    fn visibility_rejects_empty_unknown_and_miscased_values() {
        assert_eq!(Visibility::try_new(" \t "), Err(VisibilityError::Missing));
        assert_eq!(
            Visibility::try_new("secret"),
            Err(VisibilityError::Invalid("secret".to_string()))
        );
        assert_eq!(
            Visibility::try_new("Public"),
            Err(VisibilityError::Invalid("Public".to_string()))
        );
    }

    #[test]
    fn field_visibility_parses_comma_separated_entries() {
        let entries =
            parse_field_visibility("owner=internal, cost=restricted").expect("valid entries");
        assert_eq!(
            entries,
            BTreeMap::from([
                ("owner".to_string(), "internal".to_string()),
                ("cost".to_string(), "restricted".to_string()),
            ])
        );
    }

    #[test]
    fn field_visibility_rejects_malformed_entries() {
        assert!(parse_field_visibility("").is_err());
        assert!(parse_field_visibility("owner").is_err());
        assert!(parse_field_visibility("=internal").is_err());
        assert!(parse_field_visibility("owner=secret").is_err());
        assert!(parse_field_visibility("owner=internal,").is_err());
    }
}

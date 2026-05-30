//! `approved_by` value object used by the `policy` Knowledge Object.
//!
//! Introduced in V5.4. Constructed only via [`ApprovedBy::try_new`]; the
//! accepted grammar is any non-empty ASCII-trimmed string — the same rule as
//! `claim`'s `Owner` field. An approver name may contain spaces, punctuation,
//! or mixed case (e.g. `"Alice Smith"`, `"security-team"`).

use std::fmt;

use crate::domain::values::NonEmptyText;

/// An approver name with constructor-asserted non-emptiness.
///
/// Once constructed the inner string is stored in its ASCII-trimmed form and
/// is guaranteed to be non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ApprovedBy(String);

impl ApprovedBy {
    /// Parse an approver name from a string slice. ASCII-trims first; empty or
    /// whitespace-only input returns `None`.
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|text| Self(text.as_str().to_string()))
    }

    /// The validated approver name string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ApprovedBy {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn approved_by_accepts_non_empty_values() {
        for value in ["Alice Smith", "security-team", "bob", "CISO"] {
            let approved_by = ApprovedBy::try_new(value).expect("valid approver name");
            assert_eq!(approved_by.as_str(), value);
        }
    }

    #[test]
    fn approved_by_trims_ascii_edges() {
        let approved_by = ApprovedBy::try_new("  Alice Smith  ").expect("valid after trim");
        assert_eq!(approved_by.as_str(), "Alice Smith");
    }

    #[test]
    fn approved_by_rejects_empty_and_whitespace_only() {
        assert!(ApprovedBy::try_new("").is_none());
        assert!(ApprovedBy::try_new("   ").is_none());
        assert!(ApprovedBy::try_new(" \t ").is_none());
    }

    #[test]
    fn approved_by_display_round_trips_through_try_new() {
        for value in ["Alice Smith", "security-team", "bob"] {
            let approved_by = ApprovedBy::try_new(value).expect("valid approver name");
            let rendered = approved_by.to_string();
            assert_eq!(ApprovedBy::try_new(&rendered), Some(approved_by));
        }
    }
}

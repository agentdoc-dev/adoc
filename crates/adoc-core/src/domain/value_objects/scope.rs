//! `scope` value object used by the `agent_instruction` Knowledge Object.
//!
//! Introduced in V5.5. Constructed only via [`Scope::try_new`]; the accepted
//! grammar is any non-empty ASCII-trimmed string (a glob pattern, e.g.
//! `docs/auth/*`). Presence-only validation — no glob syntax check.

use std::fmt;

use crate::domain::values::NonEmptyText;

/// An agent-instruction scope glob with constructor-asserted non-emptiness.
///
/// Once constructed the inner string is stored in its ASCII-trimmed form and
/// is guaranteed to be non-empty.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Scope(String);

impl Scope {
    /// Parse a scope from a string slice. ASCII-trims first; empty or
    /// whitespace-only input returns `None`.
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|text| Self(text.as_str().to_string()))
    }

    /// The validated scope string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Scope {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scope_accepts_non_empty_values() {
        for value in ["docs/auth/*", "src/**/*.rs", "policy", "*"] {
            let scope = Scope::try_new(value).expect("valid scope");
            assert_eq!(scope.as_str(), value);
        }
    }

    #[test]
    fn scope_trims_ascii_edges() {
        let scope = Scope::try_new("  docs/auth/*  ").expect("valid after trim");
        assert_eq!(scope.as_str(), "docs/auth/*");
    }

    #[test]
    fn scope_rejects_empty_and_whitespace_only() {
        assert!(Scope::try_new("").is_none());
        assert!(Scope::try_new("   ").is_none());
        assert!(Scope::try_new(" \t ").is_none());
    }

    #[test]
    fn scope_display_round_trips_through_try_new() {
        for value in ["docs/auth/*", "src/**/*.rs"] {
            let scope = Scope::try_new(value).expect("valid scope");
            let rendered = scope.to_string();
            assert_eq!(Scope::try_new(&rendered), Some(scope));
        }
    }
}

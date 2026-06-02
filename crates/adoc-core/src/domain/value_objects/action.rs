//! `allowed_action` and `forbidden_action` value objects used by the
//! `agent_instruction` Knowledge Object.
//!
//! Introduced in V5.5. Both are opaque string newtypes with constructor-
//! asserted non-emptiness. The validator does not enumerate a fixed vocabulary —
//! any non-empty ASCII-trimmed string is a valid action name.

use std::fmt;

use crate::domain::values::NonEmptyText;

/// An allowed action name with constructor-asserted non-emptiness.
///
/// Once constructed the inner string is stored in its ASCII-trimmed form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AllowedAction(String);

impl AllowedAction {
    /// Parse an allowed action from a string slice. ASCII-trims first; empty
    /// or whitespace-only input returns `None`.
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|text| Self(text.as_str().to_string()))
    }

    /// The validated action name string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for AllowedAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A forbidden action name with constructor-asserted non-emptiness.
///
/// Once constructed the inner string is stored in its ASCII-trimmed form.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct ForbiddenAction(String);

impl ForbiddenAction {
    /// Parse a forbidden action from a string slice. ASCII-trims first; empty
    /// or whitespace-only input returns `None`.
    pub(crate) fn try_new(value: &str) -> Option<Self> {
        NonEmptyText::try_new(value).map(|text| Self(text.as_str().to_string()))
    }

    /// The validated action name string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for ForbiddenAction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn allowed_action_accepts_non_empty_values() {
        for value in ["summarize", "cite", "suggest_edits", "read-docs"] {
            let action = AllowedAction::try_new(value).expect("valid allowed action");
            assert_eq!(action.as_str(), value);
        }
    }

    #[test]
    fn allowed_action_trims_ascii_edges() {
        let action = AllowedAction::try_new("  summarize  ").expect("valid after trim");
        assert_eq!(action.as_str(), "summarize");
    }

    #[test]
    fn allowed_action_rejects_empty_and_whitespace_only() {
        assert!(AllowedAction::try_new("").is_none());
        assert!(AllowedAction::try_new("   ").is_none());
        assert!(AllowedAction::try_new(" \t ").is_none());
    }

    #[test]
    fn allowed_action_display_round_trips() {
        let action = AllowedAction::try_new("cite").expect("valid");
        assert_eq!(action.to_string(), "cite");
    }

    #[test]
    fn forbidden_action_accepts_non_empty_values() {
        for value in ["execute_shell", "access_secrets", "modify_auth_code"] {
            let action = ForbiddenAction::try_new(value).expect("valid forbidden action");
            assert_eq!(action.as_str(), value);
        }
    }

    #[test]
    fn forbidden_action_trims_ascii_edges() {
        let action = ForbiddenAction::try_new("  execute_shell  ").expect("valid after trim");
        assert_eq!(action.as_str(), "execute_shell");
    }

    #[test]
    fn forbidden_action_rejects_empty_and_whitespace_only() {
        assert!(ForbiddenAction::try_new("").is_none());
        assert!(ForbiddenAction::try_new("   ").is_none());
    }

    #[test]
    fn forbidden_action_display_round_trips() {
        let action = ForbiddenAction::try_new("access_secrets").expect("valid");
        assert_eq!(action.to_string(), "access_secrets");
    }
}

//! Sandbox name value object used by the `example` Knowledge Object and
//! (later) other Knowledge Objects that reference an execution environment.
//!
//! Introduced in V5.3. Constructed only via [`SandboxName::try_new`]; the
//! accepted grammar is an ASCII-trimmed token whose first character is a
//! lowercase letter `a-z` and whose remaining characters are each one of:
//! lowercase letter, ASCII digit, `_`, `+`, `-`, `:`. The colon acts as an
//! optional namespace separator (e.g. `docker:node-test`, `python:bookworm`).
//! Dots (`.`) are explicitly disallowed.
//! Examples: `node-test`, `python:bookworm`, `docker:node-test`.

// Not yet referenced by any aggregate; suppressed until the V5.3 wiring step.
#![allow(dead_code)]

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A sandbox name with constructor-asserted validity.
///
/// Once constructed the inner string satisfies the grammar
/// `[a-z][a-z0-9_+:-]*` and is stored in its trimmed form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SandboxName(String);

/// Why a sandbox name string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SandboxNameError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but did not match the grammar `[a-z][a-z0-9_+:-]*`.
    Invalid(String),
}

impl SandboxName {
    /// Parse a sandbox name from a string slice. ASCII-trims first, then
    /// validates the grammar `[a-z][a-z0-9_+:-]*`; empty input (after trim)
    /// is [`SandboxNameError::Missing`] and any other violation is
    /// [`SandboxNameError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, SandboxNameError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(SandboxNameError::Missing);
        }
        let mut chars = trimmed.chars();
        let first = chars.next().expect("non-empty after trim check");
        let first_ok = first.is_ascii_lowercase();
        let rest_ok = chars.all(|c| {
            c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '+' | '-' | ':')
        });
        if first_ok && rest_ok {
            Ok(Self(trimmed.to_string()))
        } else {
            Err(SandboxNameError::Invalid(trimmed.to_string()))
        }
    }

    /// The validated sandbox name string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for SandboxName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sandbox_name_accepts_valid_tokens() {
        for value in ["node-test", "python:bookworm", "docker:node-test"] {
            let name = SandboxName::try_new(value).expect("valid sandbox name");
            assert_eq!(name.as_str(), value);
        }
    }

    #[test]
    fn sandbox_name_trims_ascii_edges() {
        let name = SandboxName::try_new("  node-test  ").expect("valid sandbox name after trim");
        assert_eq!(name.as_str(), "node-test");
    }

    #[test]
    fn sandbox_name_rejects_empty_and_whitespace_only() {
        assert_eq!(SandboxName::try_new(""), Err(SandboxNameError::Missing));
        assert_eq!(SandboxName::try_new("   "), Err(SandboxNameError::Missing));
        assert_eq!(SandboxName::try_new(" \t "), Err(SandboxNameError::Missing));
    }

    #[test]
    fn sandbox_name_rejects_leading_colon() {
        assert_eq!(
            SandboxName::try_new(":x"),
            Err(SandboxNameError::Invalid(":x".to_string()))
        );
    }

    #[test]
    fn sandbox_name_rejects_leading_digit() {
        assert_eq!(
            SandboxName::try_new("3d"),
            Err(SandboxNameError::Invalid("3d".to_string()))
        );
    }

    #[test]
    fn sandbox_name_rejects_uppercase() {
        assert_eq!(
            SandboxName::try_new("Node"),
            Err(SandboxNameError::Invalid("Node".to_string()))
        );
    }

    #[test]
    fn sandbox_name_rejects_spaces_inside() {
        assert_eq!(
            SandboxName::try_new("node test"),
            Err(SandboxNameError::Invalid("node test".to_string()))
        );
    }

    #[test]
    fn sandbox_name_display_round_trips_through_try_new() {
        for value in ["node-test", "python:bookworm", "docker:node-test"] {
            let name = SandboxName::try_new(value).expect("valid sandbox name");
            let rendered = name.to_string();
            assert_eq!(SandboxName::try_new(&rendered), Ok(name));
        }
    }

    #[test]
    fn sandbox_name_rejects_dot() {
        assert_eq!(
            SandboxName::try_new("python:3.12"),
            Err(SandboxNameError::Invalid("python:3.12".to_string()))
        );
        assert_eq!(
            SandboxName::try_new("node.test"),
            Err(SandboxNameError::Invalid("node.test".to_string()))
        );
    }
}

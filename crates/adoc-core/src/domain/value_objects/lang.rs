//! Language token value object used by the `example` Knowledge Object and
//! (later) other Knowledge Objects that reference a source-code language.
//!
//! Introduced in V5.3. Constructed only via [`Lang::try_new`]; the accepted
//! grammar is an ASCII-trimmed token whose first character is a lowercase
//! letter `a-z` and whose remaining characters are each one of: lowercase
//! letter, ASCII digit, `_`, `+`, `-`. Examples: `ts`, `python`, `c++`,
//! `objective-c`, `node_18`.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A language token with constructor-asserted validity.
///
/// Once constructed the inner string satisfies the grammar
/// `[a-z][a-z0-9_+-]*` and is stored in its trimmed form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Lang(String);

/// Why a language string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LangError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but did not match the grammar `[a-z][a-z0-9_+-]*`.
    Invalid(String),
}

impl Lang {
    /// Parse a language token from a string slice. ASCII-trims first, then
    /// validates the grammar `[a-z][a-z0-9_+-]*`; empty input (after trim)
    /// is [`LangError::Missing`] and any other violation is
    /// [`LangError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, LangError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(LangError::Missing);
        }
        let mut chars = trimmed.chars();
        let first = chars.next().expect("non-empty after trim check");
        let first_ok = first.is_ascii_lowercase();
        let rest_ok = chars
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || matches!(c, '_' | '+' | '-'));
        if first_ok && rest_ok {
            Ok(Self(trimmed.to_string()))
        } else {
            Err(LangError::Invalid(trimmed.to_string()))
        }
    }

    /// The validated language token string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for Lang {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lang_accepts_valid_tokens() {
        for value in ["ts", "python", "c++", "objective-c", "node_18"] {
            let lang = Lang::try_new(value).expect("valid lang token");
            assert_eq!(lang.as_str(), value);
        }
    }

    #[test]
    fn lang_trims_ascii_edges() {
        let lang = Lang::try_new("  ts  ").expect("valid lang after trim");
        assert_eq!(lang.as_str(), "ts");
    }

    #[test]
    fn lang_rejects_empty_and_whitespace_only() {
        assert_eq!(Lang::try_new(""), Err(LangError::Missing));
        assert_eq!(Lang::try_new("   "), Err(LangError::Missing));
        assert_eq!(Lang::try_new(" \t "), Err(LangError::Missing));
    }

    #[test]
    fn lang_rejects_leading_digit() {
        assert_eq!(
            Lang::try_new("3d"),
            Err(LangError::Invalid("3d".to_string()))
        );
    }

    #[test]
    fn lang_rejects_uppercase() {
        assert_eq!(
            Lang::try_new("TS"),
            Err(LangError::Invalid("TS".to_string()))
        );
    }

    #[test]
    fn lang_rejects_leading_separator() {
        assert_eq!(
            Lang::try_new("-x"),
            Err(LangError::Invalid("-x".to_string()))
        );
    }

    #[test]
    fn lang_rejects_spaces_inside() {
        assert_eq!(
            Lang::try_new("c sharp"),
            Err(LangError::Invalid("c sharp".to_string()))
        );
    }

    #[test]
    fn lang_display_round_trips_through_try_new() {
        for value in ["ts", "python", "c++", "objective-c", "node_18"] {
            let lang = Lang::try_new(value).expect("valid lang token");
            let rendered = lang.to_string();
            assert_eq!(Lang::try_new(&rendered), Ok(lang));
        }
    }
}

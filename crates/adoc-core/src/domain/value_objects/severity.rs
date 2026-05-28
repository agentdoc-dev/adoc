//! Shared severity value object used by `constraint`, `warning`, and (later)
//! `contradiction` Knowledge Objects.
//!
//! Extracted in V5.1 from `warning`'s former private `WarningSeverity` enum so
//! that severity means the same thing on every kind that carries it (ADR-0024).
//! Constructed only via [`Severity::try_new`]; the accepted grammar is the
//! lowercase canonical set `low | medium | high | critical`, ASCII-trimmed.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A severity level with constructor-asserted validity.
///
/// Once constructed the value is total — every variant maps to exactly one
/// canonical lowercase string and back.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Severity {
    Low,
    Medium,
    High,
    Critical,
}

/// Why a severity string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SeverityError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but not one of the canonical severities.
    Invalid(String),
}

impl Severity {
    /// Parse a severity from a string slice. ASCII-trims, then matches the
    /// canonical lowercase set; empty input is [`SeverityError::Missing`] and
    /// any other spelling (including miscased) is [`SeverityError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, SeverityError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(SeverityError::Missing);
        }
        match trimmed {
            "low" => Ok(Self::Low),
            "medium" => Ok(Self::Medium),
            "high" => Ok(Self::High),
            "critical" => Ok(Self::Critical),
            _ => Err(SeverityError::Invalid(trimmed.to_string())),
        }
    }

    /// The canonical lowercase rendering of this severity.
    pub(crate) fn as_str(&self) -> &str {
        match self {
            Self::Low => "low",
            Self::Medium => "medium",
            Self::High => "high",
            Self::Critical => "critical",
        }
    }
}

impl fmt::Display for Severity {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_accepts_only_canonical_values() {
        for value in ["low", "medium", "high", "critical"] {
            let severity = Severity::try_new(value).expect("canonical severity");
            assert_eq!(severity.as_str(), value);
        }
    }

    #[test]
    fn severity_trims_ascii_edges_for_valid_values() {
        let severity = Severity::try_new("  critical  ").expect("valid severity");
        assert_eq!(severity.as_str(), "critical");
    }

    #[test]
    fn severity_rejects_empty_unknown_and_miscased_values() {
        assert_eq!(Severity::try_new(" \t "), Err(SeverityError::Missing));
        assert_eq!(
            Severity::try_new("panic"),
            Err(SeverityError::Invalid("panic".to_string()))
        );
        assert_eq!(
            Severity::try_new("Critical"),
            Err(SeverityError::Invalid("Critical".to_string()))
        );
        assert_eq!(
            Severity::try_new("HIGH"),
            Err(SeverityError::Invalid("HIGH".to_string()))
        );
    }

    #[test]
    fn severity_display_round_trips_through_try_new() {
        for severity in [
            Severity::Low,
            Severity::Medium,
            Severity::High,
            Severity::Critical,
        ] {
            let rendered = severity.to_string();
            assert_eq!(Severity::try_new(&rendered), Ok(severity));
        }
    }
}

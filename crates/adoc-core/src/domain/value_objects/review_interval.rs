//! `review_interval` value object used by the `policy` Knowledge Object.
//!
//! Introduced in V5.4. Constructed only via [`ReviewInterval::try_new`]; the
//! accepted grammar is one or more ASCII digits followed by a literal `d`
//! (e.g. `90d`, `365d`, `1d`). The token is stored in its ASCII-trimmed form.
//! Uppercase (`90D`), bare numbers (`90`), spelled-out suffixes (`90days`), and
//! embedded spaces (`9 0d`) are all rejected.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A review interval with constructor-asserted validity.
///
/// Once constructed the token satisfies the grammar `[0-9]+d`, is stored in
/// its trimmed form, and its numeric value is known to fit in `u32`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ReviewInterval {
    token: String,
    days: u32,
}

/// Why a review interval string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReviewIntervalError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but did not match the grammar `[0-9]+d`.
    Invalid(String),
}

impl ReviewInterval {
    /// Parse a review interval from a string slice. ASCII-trims first; empty
    /// input is [`ReviewIntervalError::Missing`]; any value that does not
    /// match `[0-9]+d` (one or more ASCII digits followed by a single literal
    /// `d`) is [`ReviewIntervalError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, ReviewIntervalError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(ReviewIntervalError::Missing);
        }
        if !Self::is_valid_grammar(trimmed) {
            return Err(ReviewIntervalError::Invalid(trimmed.to_string()));
        }

        let days = trimmed[..trimmed.len() - 1]
            .parse()
            .map_err(|_| ReviewIntervalError::Invalid(trimmed.to_string()))?;
        Ok(Self {
            token: trimmed.to_string(),
            days,
        })
    }

    /// The validated review interval token string.
    pub(crate) fn as_str(&self) -> &str {
        &self.token
    }

    /// The number of days encoded by this interval.
    ///
    pub(crate) fn days(&self) -> u32 {
        self.days
    }

    /// Validates the grammar `[0-9]+d`: one or more ASCII digits then exactly
    /// one lowercase `d`, with nothing else.
    fn is_valid_grammar(s: &str) -> bool {
        let bytes = s.as_bytes();
        // Must end with lowercase 'd' and have at least one digit before it.
        match bytes.split_last() {
            Some((&b'd', digits)) if !digits.is_empty() => {
                digits.iter().all(|b| b.is_ascii_digit())
            }
            _ => false,
        }
    }
}

impl fmt::Display for ReviewInterval {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn review_interval_accepts_valid_tokens() {
        for value in ["90d", "365d", "1d"] {
            let interval = ReviewInterval::try_new(value).expect("valid review interval");
            assert_eq!(interval.as_str(), value);
        }
    }

    #[test]
    fn review_interval_trims_ascii_edges() {
        let interval =
            ReviewInterval::try_new("  90d  ").expect("valid review interval after trim");
        assert_eq!(interval.as_str(), "90d");
    }

    #[test]
    fn review_interval_rejects_empty_and_whitespace_only() {
        assert_eq!(
            ReviewInterval::try_new(""),
            Err(ReviewIntervalError::Missing)
        );
        assert_eq!(
            ReviewInterval::try_new("   "),
            Err(ReviewIntervalError::Missing)
        );
        assert_eq!(
            ReviewInterval::try_new(" \t "),
            Err(ReviewIntervalError::Missing)
        );
    }

    #[test]
    fn review_interval_rejects_bare_d() {
        assert_eq!(
            ReviewInterval::try_new("d"),
            Err(ReviewIntervalError::Invalid("d".to_string()))
        );
    }

    #[test]
    fn review_interval_rejects_bare_number() {
        assert_eq!(
            ReviewInterval::try_new("90"),
            Err(ReviewIntervalError::Invalid("90".to_string()))
        );
    }

    #[test]
    fn review_interval_rejects_spelled_out_suffix() {
        assert_eq!(
            ReviewInterval::try_new("90days"),
            Err(ReviewIntervalError::Invalid("90days".to_string()))
        );
    }

    #[test]
    fn review_interval_rejects_uppercase_d() {
        assert_eq!(
            ReviewInterval::try_new("90D"),
            Err(ReviewIntervalError::Invalid("90D".to_string()))
        );
    }

    #[test]
    fn review_interval_rejects_embedded_space() {
        assert_eq!(
            ReviewInterval::try_new("9 0d"),
            Err(ReviewIntervalError::Invalid("9 0d".to_string()))
        );
    }

    #[test]
    fn review_interval_display_round_trips_through_try_new() {
        for value in ["90d", "365d", "1d"] {
            let interval = ReviewInterval::try_new(value).expect("valid review interval");
            let rendered = interval.to_string();
            assert_eq!(ReviewInterval::try_new(&rendered), Ok(interval));
        }
    }

    #[test]
    fn days_returns_numeric_value_of_interval() {
        assert_eq!(ReviewInterval::try_new("90d").expect("valid").days(), 90);
        assert_eq!(ReviewInterval::try_new("1d").expect("valid").days(), 1);
        assert_eq!(ReviewInterval::try_new("365d").expect("valid").days(), 365);
    }

    #[test]
    fn review_interval_rejects_u32_overflow() {
        assert_eq!(
            ReviewInterval::try_new("4294967296d"),
            Err(ReviewIntervalError::Invalid("4294967296d".to_string()))
        );
    }

    #[test]
    fn review_interval_accepts_u32_maximum() {
        let interval = ReviewInterval::try_new("4294967295d").expect("valid u32 maximum");
        assert_eq!(interval.as_str(), "4294967295d");
        assert_eq!(interval.days(), u32::MAX);
    }
}

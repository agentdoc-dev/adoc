//! `sample_size` value object used by the `observation` Knowledge Object.
//!
//! Introduced in V6.5.2. Constructed only via [`SampleSize::try_new`]; the
//! accepted grammar is one or more ASCII digits denoting a positive integer
//! (e.g. `37`, `1200`). Zero (`0`, `000`), signs (`-3`, `+3`), and any
//! non-digit characters are rejected. The value is stored in canonical
//! decimal form (leading zeros dropped, e.g. `00037` → `37`) so that
//! cosmetically different authored tokens hash identically.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A positive sample size with constructor-asserted validity.
///
/// Once constructed the inner string is the canonical decimal rendering of
/// a positive `u64` (no leading zeros).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SampleSize(String);

/// Why a sample size string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum SampleSizeError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but was not a positive integer.
    Invalid(String),
}

impl SampleSize {
    /// Parse a sample size from a string slice. ASCII-trims first; empty
    /// input is [`SampleSizeError::Missing`]; any value that is not a run of
    /// ASCII digits with a positive `u64` value is
    /// [`SampleSizeError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, SampleSizeError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(SampleSizeError::Missing);
        }
        let parsed = trimmed
            .bytes()
            .all(|b| b.is_ascii_digit())
            .then(|| trimmed.parse::<u64>().ok())
            .flatten()
            .filter(|&n| n > 0);
        match parsed {
            // Canonicalize so leading-zero variants hash identically.
            Some(n) => Ok(Self(n.to_string())),
            None => Err(SampleSizeError::Invalid(trimmed.to_string())),
        }
    }

    /// The validated sample size token string.
    pub(crate) fn as_str(&self) -> &str {
        &self.0
    }

    /// The numeric sample size.
    ///
    /// The constructor guarantees the inner string is a run of ASCII digits
    /// that parses as a positive `u64`, so the `.expect` documents the
    /// invariant rather than silently propagating a logic error.
    #[cfg(test)]
    pub(crate) fn value(&self) -> u64 {
        self.0
            .parse::<u64>()
            .expect("SampleSize inner string is always a positive u64")
    }
}

impl fmt::Display for SampleSize {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sample_size_accepts_positive_integers() {
        for value in ["1", "37", "1200"] {
            let sample_size = SampleSize::try_new(value).expect("valid sample size");
            assert_eq!(sample_size.as_str(), value);
        }
    }

    #[test]
    fn sample_size_trims_ascii_edges() {
        let sample_size = SampleSize::try_new("  37  ").expect("valid sample size after trim");
        assert_eq!(sample_size.as_str(), "37");
    }

    #[test]
    fn sample_size_rejects_empty_and_whitespace_only() {
        assert_eq!(SampleSize::try_new(""), Err(SampleSizeError::Missing));
        assert_eq!(SampleSize::try_new("   "), Err(SampleSizeError::Missing));
    }

    #[test]
    fn sample_size_canonicalizes_leading_zeros() {
        let sample_size = SampleSize::try_new("00037").expect("valid sample size");
        assert_eq!(sample_size.as_str(), "37");
        let sample_size = SampleSize::try_new("010").expect("valid sample size");
        assert_eq!(sample_size.as_str(), "10");
    }

    #[test]
    fn sample_size_rejects_zero() {
        assert_eq!(
            SampleSize::try_new("0"),
            Err(SampleSizeError::Invalid("0".to_string()))
        );
        assert_eq!(
            SampleSize::try_new("000"),
            Err(SampleSizeError::Invalid("000".to_string()))
        );
    }

    #[test]
    fn sample_size_rejects_negative_numbers() {
        assert_eq!(
            SampleSize::try_new("-3"),
            Err(SampleSizeError::Invalid("-3".to_string()))
        );
    }

    #[test]
    fn sample_size_rejects_non_digit_input() {
        for value in ["+3", "3.5", "many", "3 7"] {
            assert_eq!(
                SampleSize::try_new(value),
                Err(SampleSizeError::Invalid(value.to_string()))
            );
        }
    }

    #[test]
    fn sample_size_rejects_values_beyond_u64() {
        let value = "99999999999999999999999999";
        assert_eq!(
            SampleSize::try_new(value),
            Err(SampleSizeError::Invalid(value.to_string()))
        );
    }

    #[test]
    fn sample_size_display_round_trips_through_try_new() {
        let sample_size = SampleSize::try_new("37").expect("valid sample size");
        let rendered = sample_size.to_string();
        assert_eq!(SampleSize::try_new(&rendered), Ok(sample_size));
    }

    #[test]
    fn value_returns_numeric_sample_size() {
        assert_eq!(SampleSize::try_new("37").expect("valid").value(), 37);
        assert_eq!(SampleSize::try_new("1").expect("valid").value(), 1);
    }
}

//! `effective_date` value object used by the `policy` Knowledge Object.
//!
//! Introduced in V5.4. Constructed only via [`EffectiveDate::try_new`]; the
//! accepted format is an ISO 8601 calendar date in `YYYY-MM-DD` form
//! (e.g. `2026-04-01`). The inner [`chrono::NaiveDate`] is the parsed value,
//! enabling `<= today` comparisons in validation rules.

use std::fmt;

use chrono::NaiveDate;

use crate::domain::values::trim_ascii_edges;

/// An effective date with constructor-asserted validity.
///
/// Once constructed the inner value is the parsed [`NaiveDate`]; the canonical
/// string form is its `YYYY-MM-DD` rendering, which is what [`fmt::Display`]
/// and [`EffectiveDate::as_str`] produce. The canonical string is stored
/// alongside the date so that `as_str()` returns a `&str` without allocation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct EffectiveDate {
    date: NaiveDate,
    canonical: String,
}

/// Why an effective date string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum EffectiveDateError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but did not parse as a `YYYY-MM-DD` date.
    Invalid(String),
}

impl EffectiveDate {
    /// Parse an effective date from a string slice. ASCII-trims first; empty
    /// input is [`EffectiveDateError::Missing`]; any value that does not
    /// conform to `YYYY-MM-DD` is [`EffectiveDateError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, EffectiveDateError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(EffectiveDateError::Missing);
        }
        NaiveDate::parse_from_str(trimmed, "%Y-%m-%d")
            .map(|date| Self {
                canonical: date.format("%Y-%m-%d").to_string(),
                date,
            })
            .map_err(|_| EffectiveDateError::Invalid(trimmed.to_string()))
    }

    /// The canonical `YYYY-MM-DD` string representation, borrowed without
    /// allocation. Identical to the [`fmt::Display`] output.
    pub(crate) fn as_str(&self) -> &str {
        &self.canonical
    }

    /// The inner parsed date value, enabling `<= today` comparisons without
    /// re-parsing the canonical string.
    pub(crate) fn date(&self) -> NaiveDate {
        self.date
    }
}

impl fmt::Display for EffectiveDate {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.canonical)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn effective_date_accepts_valid_iso_date() {
        let date = EffectiveDate::try_new("2026-04-01").expect("valid date");
        assert_eq!(date.to_string(), "2026-04-01");
        assert_eq!(date.as_str(), "2026-04-01");
    }

    #[test]
    fn effective_date_returns_inner_naive_date_via_date_accessor() {
        let date = EffectiveDate::try_new("2026-04-01").unwrap();
        assert_eq!(date.date(), NaiveDate::from_ymd_opt(2026, 4, 1).unwrap());
    }

    #[test]
    fn effective_date_trims_ascii_edges() {
        let date = EffectiveDate::try_new("  2026-04-01  ").expect("valid date after trim");
        assert_eq!(date.to_string(), "2026-04-01");
    }

    #[test]
    fn effective_date_rejects_empty_input() {
        assert_eq!(EffectiveDate::try_new(""), Err(EffectiveDateError::Missing));
        assert_eq!(
            EffectiveDate::try_new("   "),
            Err(EffectiveDateError::Missing)
        );
        assert_eq!(
            EffectiveDate::try_new(" \t "),
            Err(EffectiveDateError::Missing)
        );
    }

    #[test]
    fn effective_date_rejects_non_date_strings() {
        assert_eq!(
            EffectiveDate::try_new("not-a-date"),
            Err(EffectiveDateError::Invalid("not-a-date".to_string()))
        );
    }

    #[test]
    fn effective_date_rejects_out_of_range_month() {
        assert_eq!(
            EffectiveDate::try_new("2026-13-01"),
            Err(EffectiveDateError::Invalid("2026-13-01".to_string()))
        );
    }

    #[test]
    fn effective_date_rejects_wrong_date_format() {
        assert_eq!(
            EffectiveDate::try_new("04-01-2026"),
            Err(EffectiveDateError::Invalid("04-01-2026".to_string()))
        );
    }

    #[test]
    fn effective_date_display_round_trips_through_try_new() {
        let date = EffectiveDate::try_new("2026-04-01").expect("valid date");
        let rendered = date.to_string();
        assert_eq!(
            EffectiveDate::try_new(&rendered),
            Ok(date),
            "Display output must re-parse to the same value"
        );
    }
}

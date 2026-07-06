//! Shared lifecycle status for Knowledge Object kinds whose closed status set
//! is exactly `draft | verified | deprecated` (`example`, `procedure`, `api`).
//!
//! Extracted per the ADR-0024 template (shared value object once the same
//! closed set recurs across kinds): the parse grammar is unchanged and each
//! aggregate maps [`LifecycleStatusError`] into its own error variant so
//! per-kind diagnostic codes, help text, and requiredness stay where they are.
//! Kinds with different status sets (claim, decision, observation, policy,
//! contradiction) keep their own enums — see CONTEXT.md **LifecycleStatus**.

use crate::domain::values::trim_ascii_edges;

const VERIFIED_STATUS: &str = "verified";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum LifecycleStatus {
    Draft,
    Verified,
    Deprecated,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum LifecycleStatusError {
    /// The value was absent or blank after trimming.
    Missing,
    /// The trimmed value is not a member of the closed set.
    Invalid(String),
}

impl LifecycleStatus {
    pub(crate) fn try_new(value: &str) -> Result<Self, LifecycleStatusError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(LifecycleStatusError::Missing);
        }
        match trimmed {
            "draft" => Ok(Self::Draft),
            VERIFIED_STATUS => Ok(Self::Verified),
            "deprecated" => Ok(Self::Deprecated),
            _ => Err(LifecycleStatusError::Invalid(trimmed.to_string())),
        }
    }

    pub(crate) fn as_str(&self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Verified => VERIFIED_STATUS,
            Self::Deprecated => "deprecated",
        }
    }

    pub(crate) fn is_verified(&self) -> bool {
        matches!(self, Self::Verified)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_closed_set_and_trims() {
        assert_eq!(
            LifecycleStatus::try_new("  draft  ").expect("draft"),
            LifecycleStatus::Draft
        );
        assert_eq!(
            LifecycleStatus::try_new("verified").expect("verified"),
            LifecycleStatus::Verified
        );
        assert_eq!(
            LifecycleStatus::try_new("deprecated").expect("deprecated"),
            LifecycleStatus::Deprecated
        );
    }

    #[test]
    fn try_new_rejects_blank_as_missing() {
        assert_eq!(
            LifecycleStatus::try_new("  "),
            Err(LifecycleStatusError::Missing)
        );
        assert_eq!(
            LifecycleStatus::try_new(""),
            Err(LifecycleStatusError::Missing)
        );
    }

    #[test]
    fn try_new_rejects_unknown_values_case_sensitively() {
        assert_eq!(
            LifecycleStatus::try_new("active"),
            Err(LifecycleStatusError::Invalid("active".to_string()))
        );
        assert_eq!(
            LifecycleStatus::try_new("Verified"),
            Err(LifecycleStatusError::Invalid("Verified".to_string()))
        );
    }

    #[test]
    fn round_trips_as_str() {
        for status in [
            LifecycleStatus::Draft,
            LifecycleStatus::Verified,
            LifecycleStatus::Deprecated,
        ] {
            assert_eq!(
                LifecycleStatus::try_new(status.as_str()).expect("round trip"),
                status
            );
        }
        assert!(LifecycleStatus::Verified.is_verified());
        assert!(!LifecycleStatus::Draft.is_verified());
    }
}

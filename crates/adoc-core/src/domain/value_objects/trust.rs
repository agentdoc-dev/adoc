//! `trust` value object used by the `agent_instruction` Knowledge Object.
//!
//! Introduced in V5.5. Constructed only via [`Trust::try_new`]; the accepted
//! grammar is the lowercase canonical set
//! `informal | team | authoritative | regulated | system`, ASCII-trimmed.
//! Variants are declared in ascending authority order so that `derive(Ord)`
//! gives a natural "trust upgrade" comparison: `after > before`.

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// A trust level with constructor-asserted validity.
///
/// Variants are ordered from lowest (`Informal`) to highest (`System`)
/// authority. A "trust upgrade" is any change where `after > before`.
///
/// Once constructed the value is total — every variant maps to exactly one
/// canonical lowercase string and back.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Trust {
    Informal,
    Team,
    Authoritative,
    Regulated,
    System,
}

/// Why a trust string failed to parse.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum TrustError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but not one of the canonical trust levels.
    Invalid(String),
}

impl Trust {
    /// Parse a trust level from a string slice. ASCII-trims, then matches the
    /// canonical lowercase set; empty input is [`TrustError::Missing`] and any
    /// other spelling (including miscased) is [`TrustError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, TrustError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(TrustError::Missing);
        }
        match trimmed {
            "informal" => Ok(Self::Informal),
            "team" => Ok(Self::Team),
            "authoritative" => Ok(Self::Authoritative),
            "regulated" => Ok(Self::Regulated),
            "system" => Ok(Self::System),
            _ => Err(TrustError::Invalid(trimmed.to_string())),
        }
    }

    /// The canonical lowercase rendering of this trust level.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Informal => "informal",
            Self::Team => "team",
            Self::Authoritative => "authoritative",
            Self::Regulated => "regulated",
            Self::System => "system",
        }
    }
}

impl fmt::Display for Trust {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn trust_accepts_only_canonical_values() {
        for value in ["informal", "team", "authoritative", "regulated", "system"] {
            let trust = Trust::try_new(value).expect("canonical trust");
            assert_eq!(trust.as_str(), value);
        }
    }

    #[test]
    fn trust_trims_ascii_edges_for_valid_values() {
        let trust = Trust::try_new("  team  ").expect("valid trust");
        assert_eq!(trust.as_str(), "team");
    }

    #[test]
    fn trust_rejects_empty_unknown_and_miscased_values() {
        assert_eq!(Trust::try_new(" \t "), Err(TrustError::Missing));
        assert_eq!(
            Trust::try_new("internal"),
            Err(TrustError::Invalid("internal".to_string()))
        );
        assert_eq!(
            Trust::try_new("Team"),
            Err(TrustError::Invalid("Team".to_string()))
        );
        assert_eq!(
            Trust::try_new("SYSTEM"),
            Err(TrustError::Invalid("SYSTEM".to_string()))
        );
    }

    #[test]
    fn trust_display_round_trips_through_try_new() {
        for trust in [
            Trust::Informal,
            Trust::Team,
            Trust::Authoritative,
            Trust::Regulated,
            Trust::System,
        ] {
            let rendered = trust.to_string();
            assert_eq!(Trust::try_new(&rendered), Ok(trust));
        }
    }

    #[test]
    fn trust_ordering_is_ascending_authority() {
        assert!(Trust::Informal < Trust::Team);
        assert!(Trust::Team < Trust::Authoritative);
        assert!(Trust::Authoritative < Trust::Regulated);
        assert!(Trust::Regulated < Trust::System);
    }

    #[test]
    fn trust_upgrade_detected_by_gt_comparison() {
        // A trust upgrade means `after > before`.
        let before = Trust::Team;
        let after = Trust::Authoritative;
        assert!(after > before, "trust upgrade: authoritative > team");
    }

    #[test]
    fn trust_downgrade_detected_by_lt_comparison() {
        let before = Trust::System;
        let after = Trust::Informal;
        assert!(after < before, "trust downgrade: informal < system");
    }

    #[test]
    fn trust_same_level_is_not_upgrade() {
        // Same level on both sides compares Equal, so the upgrade test
        // (`after > before`) is false.
        let before = Trust::Team;
        let after = Trust::Team;
        assert_eq!(after.cmp(&before), std::cmp::Ordering::Equal);
    }
}

//! Lifecycle status for `contradiction` Knowledge Objects (V5.6, ADR-0026).
//!
//! Three states: `unresolved` (active — a known conflict awaiting decision),
//! `resolved` (the conflict has been addressed), and `dismissed` (the conflict
//! was judged non-applicable and explicitly dropped).

use std::fmt;

use crate::domain::values::trim_ascii_edges;

/// Lifecycle status of a `contradiction` Knowledge Object.
///
/// An `unresolved` contradiction is considered **active**: agents should surface
/// it when answering about any cited claim. `resolved` and `dismissed` are
/// terminal states.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ContradictionStatus {
    Unresolved,
    Resolved,
    Dismissed,
}

/// Why a contradiction status string failed to parse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContradictionStatusError {
    /// The value was empty or contained only ASCII whitespace.
    Missing,
    /// The value was non-empty but not one of the canonical statuses.
    Invalid(String),
}

impl ContradictionStatus {
    /// Parse a contradiction status from a string slice. ASCII-trims, then
    /// matches the canonical lowercase set. Empty input yields
    /// [`ContradictionStatusError::Missing`]; any unrecognised spelling yields
    /// [`ContradictionStatusError::Invalid`].
    pub(crate) fn try_new(value: &str) -> Result<Self, ContradictionStatusError> {
        let trimmed = trim_ascii_edges(value);
        if trimmed.is_empty() {
            return Err(ContradictionStatusError::Missing);
        }
        match trimmed {
            "unresolved" => Ok(Self::Unresolved),
            "resolved" => Ok(Self::Resolved),
            "dismissed" => Ok(Self::Dismissed),
            _ => Err(ContradictionStatusError::Invalid(trimmed.to_string())),
        }
    }

    /// The canonical lowercase wire string for this status.
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::Unresolved => "unresolved",
            Self::Resolved => "resolved",
            Self::Dismissed => "dismissed",
        }
    }

    /// `true` only for `Unresolved` — the only state where the contradiction
    /// still requires attention from agents and triggers the
    /// `schema.claim_contradicted_by_unresolved` diagnostic and the HTML badge.
    pub(crate) fn is_active(self) -> bool {
        matches!(self, Self::Unresolved)
    }
}

impl fmt::Display for ContradictionStatus {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn try_new_accepts_canonical_values() {
        for (raw, expected) in [
            ("unresolved", ContradictionStatus::Unresolved),
            ("resolved", ContradictionStatus::Resolved),
            ("dismissed", ContradictionStatus::Dismissed),
        ] {
            let status = ContradictionStatus::try_new(raw).expect("canonical value");
            assert_eq!(status, expected);
            assert_eq!(status.as_str(), raw);
        }
    }

    #[test]
    fn try_new_trims_ascii_edges() {
        let status = ContradictionStatus::try_new("  unresolved  ").expect("trimmed");
        assert_eq!(status, ContradictionStatus::Unresolved);
    }

    #[test]
    fn try_new_rejects_empty_and_whitespace() {
        assert_eq!(
            ContradictionStatus::try_new(""),
            Err(ContradictionStatusError::Missing)
        );
        assert_eq!(
            ContradictionStatus::try_new("  "),
            Err(ContradictionStatusError::Missing)
        );
    }

    #[test]
    fn try_new_rejects_unknown_values() {
        assert_eq!(
            ContradictionStatus::try_new("open"),
            Err(ContradictionStatusError::Invalid("open".to_string()))
        );
    }

    #[test]
    fn is_active_returns_true_only_for_unresolved() {
        assert!(ContradictionStatus::Unresolved.is_active());
        assert!(!ContradictionStatus::Resolved.is_active());
        assert!(!ContradictionStatus::Dismissed.is_active());
    }

    #[test]
    fn display_round_trips_through_try_new() {
        for status in [
            ContradictionStatus::Unresolved,
            ContradictionStatus::Resolved,
            ContradictionStatus::Dismissed,
        ] {
            let rendered = status.to_string();
            assert_eq!(ContradictionStatus::try_new(&rendered), Ok(status));
        }
    }
}

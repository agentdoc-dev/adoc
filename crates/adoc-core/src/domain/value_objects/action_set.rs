//! `DisjointActionSets` value object — the only valid pair of allowed/forbidden
//! action lists for an `agent_instruction` Knowledge Object.
//!
//! Introduced in V5.5. The sole path to a valid pair is
//! [`DisjointActionSets::try_new`]; it rejects any overlap between the two
//! lists. The [`OverlapError`] carries the overlapping names in sorted order
//! so the aggregate can name them in the diagnostic message.

use std::collections::BTreeSet;
use std::fmt;

use super::action::{AllowedAction, ForbiddenAction};

/// A validated pair of disjoint allowed/forbidden action lists.
///
/// Construction via [`DisjointActionSets::try_new`] is the only way to obtain
/// a value of this type; the invariant "no shared member" holds for any live
/// instance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct DisjointActionSets {
    allowed: Vec<AllowedAction>,
    forbidden: Vec<ForbiddenAction>,
}

/// Returned by [`DisjointActionSets::try_new`] when allowed and forbidden sets
/// share at least one action name.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OverlapError {
    /// The overlapping action names, sorted ascending.
    pub(crate) overlapping: Vec<String>,
}

impl fmt::Display for OverlapError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "allowed and forbidden actions overlap: {}",
            self.overlapping.join(", ")
        )
    }
}

impl DisjointActionSets {
    /// Construct a validated pair of disjoint action lists.
    ///
    /// Returns `Err(OverlapError)` when any action name appears in both lists.
    /// The `overlapping` field of the error carries the shared names in sorted
    /// order so the aggregate can include them verbatim in its diagnostic.
    pub(crate) fn try_new(
        allowed: Vec<AllowedAction>,
        forbidden: Vec<ForbiddenAction>,
    ) -> Result<Self, OverlapError> {
        let allowed_set: BTreeSet<&str> = allowed.iter().map(AllowedAction::as_str).collect();
        let forbidden_set: BTreeSet<&str> = forbidden.iter().map(ForbiddenAction::as_str).collect();
        let overlap: Vec<String> = allowed_set
            .intersection(&forbidden_set)
            .map(|s| (*s).to_string())
            .collect();
        if !overlap.is_empty() {
            return Err(OverlapError {
                overlapping: overlap,
            });
        }
        Ok(Self { allowed, forbidden })
    }

    /// The validated allowed action list.
    pub(crate) fn allowed(&self) -> &[AllowedAction] {
        &self.allowed
    }

    /// The validated forbidden action list.
    pub(crate) fn forbidden(&self) -> &[ForbiddenAction] {
        &self.forbidden
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn allowed(names: &[&str]) -> Vec<AllowedAction> {
        names
            .iter()
            .filter_map(|s| AllowedAction::try_new(s))
            .collect()
    }

    fn forbidden(names: &[&str]) -> Vec<ForbiddenAction> {
        names
            .iter()
            .filter_map(|s| ForbiddenAction::try_new(s))
            .collect()
    }

    #[test]
    fn disjoint_sets_accepted() {
        let sets = DisjointActionSets::try_new(
            allowed(&["summarize", "cite"]),
            forbidden(&["execute_shell", "access_secrets"]),
        )
        .expect("disjoint sets");
        assert_eq!(sets.allowed().len(), 2);
        assert_eq!(sets.forbidden().len(), 2);
    }

    #[test]
    fn overlapping_sets_rejected_with_sorted_overlap() {
        let err = DisjointActionSets::try_new(
            allowed(&["cite", "summarize", "execute_shell"]),
            forbidden(&["execute_shell", "access_secrets", "cite"]),
        )
        .expect_err("overlapping sets");
        // Overlap must be sorted ascending.
        assert_eq!(err.overlapping, vec!["cite", "execute_shell"]);
    }

    #[test]
    fn single_overlap_carries_one_name() {
        let err = DisjointActionSets::try_new(
            allowed(&["summarize", "cite"]),
            forbidden(&["execute_shell", "cite"]),
        )
        .expect_err("single overlap");
        assert_eq!(err.overlapping, vec!["cite"]);
    }

    #[test]
    fn empty_allowed_and_non_overlapping_forbidden_accepted() {
        let sets =
            DisjointActionSets::try_new(allowed(&[]), forbidden(&["execute_shell"])).expect("ok");
        assert!(sets.allowed().is_empty());
        assert_eq!(sets.forbidden().len(), 1);
    }

    #[test]
    fn overlap_error_display_names_actions() {
        let err = OverlapError {
            overlapping: vec!["cite".to_string(), "summarize".to_string()],
        };
        let display = err.to_string();
        assert!(display.contains("cite"));
        assert!(display.contains("summarize"));
    }
}

//! `ContradictionClaims` value object — a sorted, deduplicated, non-empty list
//! of claim [`ObjectId`]s with a minimum arity of 2 (V5.6, ADR-0026).
//!
//! A contradiction must reference at least two distinct claims to be
//! meaningful; the arity check is enforced here at construction time.

use crate::domain::identity::ObjectId;
use crate::domain::values::NonEmpty;

/// A non-empty, sorted, deduplicated list of [`ObjectId`]s referencing at
/// least two distinct `claim` objects.
///
/// Arity ≥ 2 is enforced by [`ContradictionClaims::try_new`]; after
/// construction, the internal list is stable (sorted by `ObjectId`'s `Ord`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ContradictionClaims(NonEmpty<ObjectId>);

/// Why a `ContradictionClaims` value failed to be constructed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ContradictionClaimsError {
    /// After deduplication, fewer than two distinct claim IDs were provided.
    TooFew,
}

impl ContradictionClaims {
    /// Construct from a (possibly unordered, possibly duplicate) `Vec<ObjectId>`.
    ///
    /// The list is sorted and deduplicated; if after deduplication fewer than
    /// two distinct IDs remain, [`ContradictionClaimsError::TooFew`] is
    /// returned.
    pub(crate) fn try_new(mut ids: Vec<ObjectId>) -> Result<Self, ContradictionClaimsError> {
        ids.sort();
        ids.dedup();
        if ids.len() < 2 {
            return Err(ContradictionClaimsError::TooFew);
        }
        let non_empty =
            NonEmpty::from_vec(ids).expect("ids.len() >= 2 guarantees non-empty post-dedup");
        Ok(Self(non_empty))
    }

    /// The sorted, deduplicated claim IDs.
    pub(crate) fn as_slice(&self) -> &[ObjectId] {
        self.0.as_slice()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> ObjectId {
        ObjectId::new(s).expect("valid object id")
    }

    #[test]
    fn try_new_accepts_exactly_two_distinct_ids() {
        let claims =
            ContradictionClaims::try_new(vec![id("auth.a"), id("auth.b")]).expect("valid claims");
        assert_eq!(claims.as_slice().len(), 2);
    }

    #[test]
    fn try_new_sorts_ids() {
        let claims =
            ContradictionClaims::try_new(vec![id("auth.z"), id("auth.a")]).expect("valid claims");
        assert_eq!(claims.as_slice()[0].as_str(), "auth.a");
        assert_eq!(claims.as_slice()[1].as_str(), "auth.z");
    }

    #[test]
    fn try_new_deduplicates_ids() {
        let claims = ContradictionClaims::try_new(vec![id("auth.a"), id("auth.b"), id("auth.a")])
            .expect("valid after dedup");
        assert_eq!(claims.as_slice().len(), 2);
    }

    #[test]
    fn try_new_rejects_empty_list() {
        assert_eq!(
            ContradictionClaims::try_new(Vec::new()),
            Err(ContradictionClaimsError::TooFew)
        );
    }

    #[test]
    fn try_new_rejects_single_id() {
        assert_eq!(
            ContradictionClaims::try_new(vec![id("auth.a")]),
            Err(ContradictionClaimsError::TooFew)
        );
    }

    #[test]
    fn try_new_rejects_two_identical_ids_after_dedup() {
        assert_eq!(
            ContradictionClaims::try_new(vec![id("auth.a"), id("auth.a")]),
            Err(ContradictionClaimsError::TooFew)
        );
    }

    #[test]
    fn try_new_accepts_more_than_two_ids() {
        let claims = ContradictionClaims::try_new(vec![id("auth.c"), id("auth.a"), id("auth.b")])
            .expect("valid with three ids");
        assert_eq!(claims.as_slice().len(), 3);
        // Sorted order.
        assert_eq!(claims.as_slice()[0].as_str(), "auth.a");
        assert_eq!(claims.as_slice()[1].as_str(), "auth.b");
        assert_eq!(claims.as_slice()[2].as_str(), "auth.c");
    }
}

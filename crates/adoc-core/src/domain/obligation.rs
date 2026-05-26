//! Shared `ProofObligation` value object.
//!
//! Promoted from `domain/patch/mod.rs` in V3.4 so both the V2 patch
//! validation surface (`domain/patch/`) and the V3 review surface
//! (`domain/review/`) can speak the same vocabulary about proof. See
//! ADR-0020.
//!
//! The wire shape (`{ object_id, reason, required_evidence }`) is the
//! contract embedded in `adoc.patch.check.v0` and `adoc.review.v0`. Field
//! values are populated by the respective trigger logic — V2 by
//! `PatchValidator`, V3 by `domain/review/obligation_rules.rs`.

use serde::Serialize;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofObligation {
    pub object_id: String,
    pub reason: String,
    pub required_evidence: Vec<String>,
}

impl ProofObligation {
    /// Merge an iterator of obligations into a deterministic deduplicated
    /// vector.
    ///
    /// Duplicates are detected by `(object_id, reason)`; first occurrence
    /// wins, so callers control tie resolution by ordering their iterators
    /// (V3 review aggregates diff-driven obligations ahead of impact-driven
    /// ones; the patch-composition envelope chains session obligations ahead
    /// of patch-check obligations). The result is sorted by
    /// `(object_id, reason)` so JSON output is byte-stable across runs and
    /// across machines.
    pub fn merge_dedup_sorted<I>(obligations: I) -> Vec<Self>
    where
        I: IntoIterator<Item = Self>,
    {
        let mut deduped: Vec<Self> = Vec::new();
        for obligation in obligations {
            if !deduped.iter().any(|existing| {
                existing.object_id == obligation.object_id && existing.reason == obligation.reason
            }) {
                deduped.push(obligation);
            }
        }
        deduped.sort_by(|a, b| {
            (a.object_id.as_str(), a.reason.as_str())
                .cmp(&(b.object_id.as_str(), b.reason.as_str()))
        });
        deduped
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ob(id: &str, reason: &str, evidence: &[&str]) -> ProofObligation {
        ProofObligation {
            object_id: id.to_string(),
            reason: reason.to_string(),
            required_evidence: evidence.iter().map(|e| (*e).to_string()).collect(),
        }
    }

    #[test]
    fn merge_dedup_sorted_returns_empty_for_empty_input() {
        let out = ProofObligation::merge_dedup_sorted(std::iter::empty());
        assert!(out.is_empty());
    }

    #[test]
    fn merge_dedup_sorted_sorts_by_object_id_then_reason() {
        let out = ProofObligation::merge_dedup_sorted([
            ob("billing.tax", "re-verify", &["source"]),
            ob("billing.credits", "stale verified claim", &[]),
            ob("billing.credits", "re-verify", &["source"]),
        ]);

        assert_eq!(
            out.iter()
                .map(|o| (o.object_id.as_str(), o.reason.as_str()))
                .collect::<Vec<_>>(),
            vec![
                ("billing.credits", "re-verify"),
                ("billing.credits", "stale verified claim"),
                ("billing.tax", "re-verify"),
            ]
        );
    }

    #[test]
    fn merge_dedup_sorted_drops_later_duplicates_keeping_first_required_evidence() {
        let out = ProofObligation::merge_dedup_sorted([
            ob("billing.credits", "re-verify", &["source", "test"]),
            ob("billing.credits", "re-verify", &["reviewed_by"]),
        ]);

        assert_eq!(out.len(), 1);
        assert_eq!(out[0].required_evidence, vec!["source", "test"]);
    }

    #[test]
    fn merge_dedup_sorted_preserves_distinct_reasons_for_same_object() {
        let out = ProofObligation::merge_dedup_sorted([
            ob("billing.credits", "re-verify", &["source"]),
            ob("billing.credits", "owner reassignment", &[]),
        ]);

        assert_eq!(out.len(), 2);
        assert_eq!(out[0].reason, "owner reassignment");
        assert_eq!(out[1].reason, "re-verify");
    }
}

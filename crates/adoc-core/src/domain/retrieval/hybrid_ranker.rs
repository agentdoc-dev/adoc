use std::collections::{BTreeMap, BTreeSet};

use crate::domain::retrieval::lexical_index::LexicalSearchHit;
use crate::domain::retrieval::vector_index::VectorHit;

const RRF_K: f64 = 60.0;

#[derive(Debug, Clone)]
pub(crate) struct HybridRanker;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct HybridRankedHit {
    pub(crate) id: String,
    pub(crate) rrf_score: f64,
    pub(crate) lexical_rank: Option<u32>,
    pub(crate) vector_rank: Option<u32>,
}

impl HybridRanker {
    pub(crate) fn rank(
        &self,
        query_text: &str,
        candidate_ids: &[&str],
        lexical_hits: &[LexicalSearchHit],
        vector_hits: &[VectorHit],
        top: usize,
    ) -> Vec<HybridRankedHit> {
        if top == 0 || candidate_ids.is_empty() {
            return Vec::new();
        }

        let candidates: BTreeSet<&str> = candidate_ids.iter().copied().collect();
        let lexical_by_id: BTreeMap<&str, &LexicalSearchHit> = lexical_hits
            .iter()
            .filter(|hit| candidates.contains(hit.id.as_str()))
            .map(|hit| (hit.id.as_str(), hit))
            .collect();
        let vector_by_id: BTreeMap<&str, &VectorHit> = vector_hits
            .iter()
            .filter(|hit| candidates.contains(hit.id.as_str()))
            .map(|hit| (hit.id.as_str(), hit))
            .collect();

        let mut scored = BTreeMap::<String, HybridRankedHit>::new();
        for hit in lexical_by_id.values() {
            let entry = scored
                .entry(hit.id.clone())
                .or_insert_with(|| HybridRankedHit::new(hit.id.clone()));
            entry.rrf_score += rrf_component(hit.lexical_rank);
            entry.lexical_rank = Some(hit.lexical_rank);
        }
        for hit in vector_by_id.values() {
            let entry = scored
                .entry(hit.id.clone())
                .or_insert_with(|| HybridRankedHit::new(hit.id.clone()));
            entry.rrf_score += rrf_component(hit.vector_rank);
            entry.vector_rank = Some(hit.vector_rank);
        }

        let mut ranked: Vec<_> = scored.into_values().collect();
        ranked.sort_by(|left, right| {
            right
                .rrf_score
                .total_cmp(&left.rrf_score)
                .then_with(|| left.id.cmp(&right.id))
        });

        let pinned = self.pinned_candidate_ids(query_text, candidate_ids);
        let mut seen = BTreeSet::new();
        let mut results = Vec::new();
        for id in pinned {
            if seen.insert(id.clone()) {
                let lexical_rank = lexical_by_id.get(id.as_str()).map(|hit| hit.lexical_rank);
                let vector_rank = vector_by_id.get(id.as_str()).map(|hit| hit.vector_rank);
                let rrf_score = lexical_rank.map(rrf_component).unwrap_or(0.0)
                    + vector_rank.map(rrf_component).unwrap_or(0.0);
                results.push(HybridRankedHit {
                    id,
                    rrf_score,
                    lexical_rank,
                    vector_rank,
                });
            }
            if results.len() >= top {
                return results;
            }
        }

        for hit in ranked {
            if seen.insert(hit.id.clone()) {
                results.push(hit);
            }
            if results.len() >= top {
                break;
            }
        }

        results
    }

    /// Returns Object ID prefix matches before scored hits.
    ///
    /// Pinned matches use a two-stage deterministic order: shorter matching IDs
    /// first, then lexicographic order for IDs with equal length.
    pub(crate) fn pinned_candidate_ids(
        &self,
        query_text: &str,
        candidate_ids: &[&str],
    ) -> Vec<String> {
        if query_text.is_empty() {
            return Vec::new();
        }

        let mut pinned_ids: Vec<_> = candidate_ids
            .iter()
            .copied()
            .filter(|id| id.starts_with(query_text))
            .map(str::to_string)
            .collect();
        pinned_ids
            .sort_by(|left, right| left.len().cmp(&right.len()).then_with(|| left.cmp(right)));
        pinned_ids
    }
}

impl HybridRankedHit {
    fn new(id: String) -> Self {
        Self {
            id,
            rrf_score: 0.0,
            lexical_rank: None,
            vector_rank: None,
        }
    }
}

fn rrf_component(rank: u32) -> f64 {
    1.0 / (RRF_K + f64::from(rank))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::retrieval::lexical_index::LexicalSearchHit;
    use crate::domain::retrieval::vector_index::VectorHit;

    fn lexical(id: &str, rank: u32) -> LexicalSearchHit {
        LexicalSearchHit {
            id: id.to_string(),
            lexical_rank: rank,
            score: 1.0,
        }
    }

    fn vector(id: &str, rank: u32) -> VectorHit {
        VectorHit {
            id: id.to_string(),
            vector_rank: rank,
            cosine_score: 1.0,
        }
    }

    #[test]
    fn fuses_disjoint_non_empty_lists_with_rrf_scores() {
        let ranker = HybridRanker;

        let hits = ranker.rank(
            "credit ledger",
            &["billing.lexical", "billing.semantic"],
            &[lexical("billing.lexical", 1)],
            &[vector("billing.semantic", 1)],
            10,
        );

        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].id, "billing.lexical");
        assert_eq!(hits[0].rrf_score, 1.0 / 61.0);
        assert_eq!(hits[0].lexical_rank, Some(1));
        assert_eq!(hits[0].vector_rank, None);
        assert_eq!(hits[1].id, "billing.semantic");
        assert_eq!(hits[1].rrf_score, 1.0 / 61.0);
        assert_eq!(hits[1].lexical_rank, None);
        assert_eq!(hits[1].vector_rank, Some(1));
    }

    #[test]
    fn breaks_rrf_score_ties_by_ascending_object_id() {
        let ranker = HybridRanker;

        let hits = ranker.rank(
            "same",
            &["zeta.same", "alpha.same"],
            &[lexical("zeta.same", 1)],
            &[vector("alpha.same", 1)],
            10,
        );

        let ids: Vec<_> = hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(ids, ["alpha.same", "zeta.same"]);
    }

    #[test]
    fn pins_id_prefix_matches_by_length_then_lex_before_fused_hits() {
        let ranker = HybridRanker;

        let hits = ranker.rank(
            "billing.credit",
            &[
                "support.heavy",
                "billing.credits.b",
                "billing.credit",
                "billing.credits.a",
                "billing.credits",
            ],
            &[lexical("support.heavy", 1)],
            &[vector("support.heavy", 1)],
            10,
        );

        let ids: Vec<_> = hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "billing.credit",
                "billing.credits",
                "billing.credits.a",
                "billing.credits.b",
                "support.heavy"
            ]
        );
        assert_eq!(hits[0].rrf_score, 0.0);
        assert_eq!(hits[0].lexical_rank, None);
        assert_eq!(hits[0].vector_rank, None);
    }
}

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
    /// Fuses lexical and vector ranks with RRF over `candidate_ids`.
    ///
    /// `pinnable_ids` is the subset eligible for Object ID prefix pinning —
    /// Knowledge Object ids only (ADR-0040): prose block ids ride
    /// `candidate_ids` for fusion but must never pin, or a page-id-prefix
    /// query would pin that page's blocks above scored results.
    pub(crate) fn rank(
        &self,
        query_text: &str,
        candidate_ids: &[&str],
        pinnable_ids: &[&str],
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

        let pinned_hits: Vec<_> = self
            .pinned_candidate_ids(query_text, pinnable_ids)
            .into_iter()
            .map(|id| {
                let lexical_rank = lexical_by_id.get(id.as_str()).map(|hit| hit.lexical_rank);
                let vector_rank = vector_by_id.get(id.as_str()).map(|hit| hit.vector_rank);
                HybridRankedHit {
                    rrf_score: lexical_rank.map(rrf_component).unwrap_or(0.0)
                        + vector_rank.map(rrf_component).unwrap_or(0.0),
                    id,
                    lexical_rank,
                    vector_rank,
                }
            })
            .collect();
        merge_pinned_then_scored(pinned_hits, ranked, |hit| hit.id.as_str(), top)
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

/// The single merge policy for pinned and scored hits (ADR-0040): pins first
/// (deduped), then scored items until `top` non-pinned items are taken. `top`
/// bounds the scored budget only — pinned ids ride above it and can never
/// displace a scored result, so results may exceed `top` by the pin count.
pub(crate) fn merge_pinned_then_scored<T>(
    pinned: Vec<T>,
    scored: impl IntoIterator<Item = T>,
    id_of: impl Fn(&T) -> &str,
    top: usize,
) -> Vec<T> {
    let mut seen = BTreeSet::new();
    let mut results = Vec::new();
    for item in pinned {
        if seen.insert(id_of(&item).to_string()) {
            results.push(item);
        }
    }
    let mut scored_taken = 0;
    for item in scored {
        if scored_taken >= top {
            break;
        }
        if seen.insert(id_of(&item).to_string()) {
            results.push(item);
            scored_taken += 1;
        }
    }
    results
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

        let candidates = [
            "support.heavy",
            "billing.credits.b",
            "billing.credit",
            "billing.credits.a",
            "billing.credits",
        ];
        let hits = ranker.rank(
            "billing.credit",
            &candidates,
            &candidates,
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

    /// V1.7.1 review follow-up: `top` bounds scored hits only. A pinned id
    /// rides above the budget and must never displace a scored result.
    #[test]
    fn pinned_ids_ride_above_the_scored_budget() {
        let ranker = HybridRanker;

        let hits = ranker.rank(
            "billing.credit",
            &["billing.credit", "guides.page#block-0001"],
            &["billing.credit"],
            &[lexical("guides.page#block-0001", 1)],
            &[],
            1,
        );

        let ids: Vec<_> = hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(
            ids,
            ["billing.credit", "guides.page#block-0001"],
            "the pin must not consume the single scored slot"
        );
    }

    #[test]
    fn prose_ids_outside_the_pinnable_set_never_pin() {
        let ranker = HybridRanker;

        // A page-id-prefix query with a prose block id in the fusion pool:
        // the prose id must rank by score, never pin (ADR-0040).
        let hits = ranker.rank(
            "guides.getting-started",
            &["guides.getting-started#block-0007", "billing.credits"],
            &["billing.credits"],
            &[
                lexical("billing.credits", 1),
                lexical("guides.getting-started#block-0007", 2),
            ],
            &[],
            10,
        );

        let ids: Vec<_> = hits.iter().map(|hit| hit.id.as_str()).collect();
        assert_eq!(
            ids,
            ["billing.credits", "guides.getting-started#block-0007"]
        );
        assert!(
            hits[1].rrf_score > 0.0,
            "prose hit is scored, not pinned with a zero score"
        );
    }
}

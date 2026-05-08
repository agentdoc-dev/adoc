use std::collections::BTreeSet;

#[derive(Debug, Clone)]
pub(crate) struct VectorIndex {
    entries: Vec<VectorEntry>,
}

#[derive(Debug, Clone)]
struct VectorEntry {
    id: String,
    vector: Vec<f32>,
    norm: f32,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct VectorHit {
    pub(crate) id: String,
    pub(crate) vector_rank: u32,
    pub(crate) cosine_score: f32,
}

impl VectorIndex {
    pub(crate) fn new(items: Vec<(String, Vec<f32>)>) -> Self {
        let entries = items
            .into_iter()
            .map(|(id, vector)| {
                let norm = vector_norm(&vector);
                VectorEntry { id, vector, norm }
            })
            .collect();
        Self { entries }
    }

    #[cfg(test)]
    pub(crate) fn rank(&self, query: &[f32], top: usize) -> Vec<VectorHit> {
        self.rank_among(
            query,
            self.entries.iter().map(|entry| entry.id.as_str()),
            top,
        )
    }

    pub(crate) fn rank_among<'a, I>(
        &self,
        query: &[f32],
        candidates: I,
        top: usize,
    ) -> Vec<VectorHit>
    where
        I: IntoIterator<Item = &'a str>,
    {
        let query_norm = vector_norm(query);
        if query_norm == 0.0 || top == 0 {
            return Vec::new();
        }

        let allowed: BTreeSet<&str> = candidates.into_iter().collect();

        let mut hits: Vec<VectorHit> = self
            .entries
            .iter()
            .filter(|entry| allowed.contains(entry.id.as_str()))
            .filter(|entry| entry.norm > 0.0 && entry.vector.len() == query.len())
            .map(|entry| {
                let dot: f32 = entry.vector.iter().zip(query).map(|(a, b)| a * b).sum();
                let cosine_score = dot / (entry.norm * query_norm);
                VectorHit {
                    id: entry.id.clone(),
                    vector_rank: 0,
                    cosine_score,
                }
            })
            .filter(|hit| hit.cosine_score.is_finite())
            .collect();

        hits.sort_by(|left, right| {
            right
                .cosine_score
                .total_cmp(&left.cosine_score)
                .then_with(|| left.id.cmp(&right.id))
        });
        hits.truncate(top);
        for (index, hit) in hits.iter_mut().enumerate() {
            hit.vector_rank = (index + 1) as u32;
        }
        hits
    }
}

fn vector_norm(vector: &[f32]) -> f32 {
    vector.iter().map(|value| value * value).sum::<f32>().sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id(s: &str) -> String {
        s.to_string()
    }

    #[test]
    fn ranks_by_descending_cosine_and_assigns_vector_ranks() {
        let entries = vec![
            (id("billing.refunds"), vec![1.0, 0.0, 0.0]),
            (id("billing.credits"), vec![0.0, 1.0, 0.0]),
            (id("ops.dlq"), vec![0.0, 0.0, 1.0]),
        ];
        let index = VectorIndex::new(entries);

        let hits = index.rank(&[0.1, 0.9, 0.0], 3);

        assert_eq!(hits[0].id, "billing.credits");
        assert_eq!(hits[0].vector_rank, 1);
        assert!(hits[0].cosine_score > hits[1].cosine_score);
    }

    #[test]
    fn breaks_cosine_ties_by_ascending_object_id() {
        let entries = vec![
            (id("zeta.same"), vec![1.0, 0.0]),
            (id("alpha.same"), vec![1.0, 0.0]),
        ];
        let hits = VectorIndex::new(entries).rank(&[1.0, 0.0], 10);
        assert_eq!(hits[0].id, "alpha.same");
        assert_eq!(hits[1].id, "zeta.same");
    }

    #[test]
    fn truncates_results_to_top_n() {
        let entries: Vec<_> = (0..10)
            .map(|i| (format!("obj.{i}"), vec![1.0, i as f32]))
            .collect();
        let hits = VectorIndex::new(entries).rank(&[1.0, 0.0], 3);
        assert_eq!(hits.len(), 3);
    }

    #[test]
    fn rank_among_filters_then_ranks() {
        let entries = vec![
            (id("a.one"), vec![1.0, 0.0]),
            (id("b.two"), vec![1.0, 0.0]),
            (id("c.thr"), vec![1.0, 0.0]),
        ];
        let index = VectorIndex::new(entries);
        let hits = index.rank_among(&[1.0, 0.0], ["a.one", "c.thr"].iter().copied(), 5);
        let ids: Vec<_> = hits.iter().map(|h| h.id.as_str()).collect();
        assert_eq!(ids, ["a.one", "c.thr"]);
    }

    #[test]
    fn empty_index_returns_empty_hits() {
        let index = VectorIndex::new(Vec::new());
        assert!(index.rank(&[1.0], 5).is_empty());
    }

    #[test]
    fn zero_norm_vectors_are_skipped_not_nan() {
        let entries = vec![(id("billing.zero"), vec![0.0, 0.0])];
        let hits = VectorIndex::new(entries).rank(&[1.0, 0.0], 1);
        assert!(hits.is_empty());
    }
}

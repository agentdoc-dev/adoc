use std::collections::{BTreeMap, BTreeSet};

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::retrieval::metadata;

const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LexicalSearchHit {
    pub(crate) id: String,
    pub(crate) lexical_rank: u32,
    pub(crate) score: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LexicalIndex {
    documents: Vec<IndexedDocument>,
    document_count: usize,
    average_document_len: f64,
    document_frequencies: BTreeMap<String, usize>,
}

#[derive(Debug, Clone)]
struct IndexedDocument {
    id: String,
    term_frequencies: BTreeMap<String, usize>,
    token_count: usize,
}

impl LexicalIndex {
    pub(crate) fn from_objects<'a>(
        objects: impl IntoIterator<Item = &'a GraphKnowledgeObjectNode>,
    ) -> Self {
        let mut documents = Vec::new();

        for object in objects {
            let tokens = indexed_tokens(object);
            let token_count = tokens.len();

            let mut term_frequencies = BTreeMap::new();

            for token in tokens {
                *term_frequencies.entry(token).or_insert(0) += 1;
            }

            documents.push(IndexedDocument {
                id: object.id.clone(),
                term_frequencies,
                token_count,
            });
        }

        let document_count = documents.len();
        let average_document_len = if document_count == 0 {
            0.0
        } else {
            documents
                .iter()
                .map(|document| document.token_count)
                .sum::<usize>() as f64
                / document_count as f64
        };
        let document_frequencies = document_frequencies_for_documents(&documents);

        Self {
            documents,
            document_count,
            average_document_len,
            document_frequencies,
        }
    }

    #[cfg(test)]
    pub(crate) fn search(&self, query: &str) -> Vec<LexicalSearchHit> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() || self.documents.is_empty() {
            return Vec::new();
        }

        self.search_documents(query_terms, self.documents.iter().collect())
    }

    pub(crate) fn search_candidates<'a>(
        &self,
        query: &str,
        candidate_ids: impl IntoIterator<Item = &'a str>,
    ) -> Vec<LexicalSearchHit> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() || self.documents.is_empty() {
            return Vec::new();
        }

        let candidate_ids: BTreeSet<&str> = candidate_ids.into_iter().collect();
        if candidate_ids.is_empty() {
            return Vec::new();
        }

        let documents = self
            .documents
            .iter()
            .filter(|document| candidate_ids.contains(document.id.as_str()))
            .collect();

        self.search_documents(query_terms, documents)
    }

    fn search_documents(
        &self,
        query_terms: Vec<String>,
        documents: Vec<&IndexedDocument>,
    ) -> Vec<LexicalSearchHit> {
        if documents.is_empty() {
            return Vec::new();
        }

        let mut hits: Vec<_> = documents
            .iter()
            .filter_map(|document| {
                let score = score_document(
                    document,
                    &query_terms,
                    self.document_count,
                    self.average_document_len,
                    &self.document_frequencies,
                );
                (score > 0.0).then(|| LexicalSearchHit {
                    id: document.id.clone(),
                    lexical_rank: 0,
                    score,
                })
            })
            .collect();

        hits.sort_by(|left, right| {
            right
                .score
                .total_cmp(&left.score)
                .then_with(|| left.id.cmp(&right.id))
        });

        for (index, hit) in hits.iter_mut().enumerate() {
            hit.lexical_rank = (index + 1) as u32;
        }

        hits
    }
}

fn document_frequencies_for_documents(documents: &[IndexedDocument]) -> BTreeMap<String, usize> {
    let mut document_frequencies = BTreeMap::new();

    for document in documents {
        for term in document.term_frequencies.keys() {
            *document_frequencies.entry(term.clone()).or_insert(0) += 1;
        }
    }

    document_frequencies
}

fn score_document(
    document: &IndexedDocument,
    query_terms: &[String],
    document_count: usize,
    average_document_len: f64,
    document_frequencies: &BTreeMap<String, usize>,
) -> f64 {
    query_terms
        .iter()
        .map(|term| {
            score_term(
                document,
                term,
                document_count,
                average_document_len,
                document_frequencies,
            )
        })
        .sum()
}

fn score_term(
    document: &IndexedDocument,
    term: &str,
    document_count: usize,
    average_document_len: f64,
    document_frequencies: &BTreeMap<String, usize>,
) -> f64 {
    let Some(&term_frequency) = document.term_frequencies.get(term) else {
        return 0.0;
    };
    let Some(&document_frequency) = document_frequencies.get(term) else {
        return 0.0;
    };

    let document_count = document_count as f64;
    let term_frequency = term_frequency as f64;
    let document_frequency = document_frequency as f64;
    let document_len = document.token_count as f64;
    let length_norm = if average_document_len == 0.0 {
        0.0
    } else {
        document_len / average_document_len
    };
    let idf = ((document_count - document_frequency + 0.5) / (document_frequency + 0.5) + 1.0).ln();
    let denominator = term_frequency + BM25_K1 * (1.0 - BM25_B + BM25_B * length_norm);

    idf * (term_frequency * (BM25_K1 + 1.0)) / denominator
}

fn indexed_tokens(object: &GraphKnowledgeObjectNode) -> Vec<String> {
    let mut tokens = tokenize(&object.body);
    tokens.extend(tokenize(&object.id));
    tokens.extend(tokenize(&object.kind));
    for value in metadata::indexed_field_values(object) {
        tokens.extend(tokenize(value));
    }
    tokens
}

fn tokenize(text: &str) -> Vec<String> {
    let mut tokens = Vec::new();
    let mut current = String::new();

    for character in text.chars() {
        if character.is_alphanumeric() {
            current.extend(character.to_lowercase());
        } else if !current.is_empty() {
            tokens.push(std::mem::take(&mut current));
        }
    }

    if !current.is_empty() {
        tokens.push(current);
    }

    tokens
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;
    use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};

    fn object(id: &str, kind: &str, owner: Option<&str>, body: &str) -> GraphKnowledgeObjectNode {
        let mut fields = BTreeMap::new();
        if let Some(owner) = owner {
            fields.insert("owner".to_string(), owner.to_string());
        }

        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: kind.to_string(),
            content_hash: format!("sha256:{id}"),
            status: None,
            body: body.to_string(),
            page_id: "team.page".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields,
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
        }
    }

    #[test]
    fn lexical_index_ranks_body_term_by_bm25_relevance() {
        let objects = vec![
            object(
                "billing.credits.depth",
                "claim",
                None,
                "credits credits credits ledger",
            ),
            object("billing.credits.single", "claim", None, "credits"),
            object(
                "billing.refunds",
                "claim",
                None,
                "refunds use manual review",
            ),
        ];

        let hits = LexicalIndex::from_objects(&objects).search("credits");

        assert_eq!(
            hit_ids(&hits),
            ["billing.credits.depth", "billing.credits.single"]
        );
        assert_eq!(hits[0].lexical_rank, 1);
        assert!(hits[0].score > hits[1].score);
    }

    #[test]
    fn lexical_index_returns_empty_for_no_hit_and_empty_query() {
        let objects = vec![object(
            "billing.credits",
            "claim",
            None,
            "credits decrement after payment",
        )];
        let index = LexicalIndex::from_objects(&objects);

        assert!(index.search("chargebacks").is_empty());
        assert!(index.search(" ... \n\t ").is_empty());
    }

    #[test]
    fn lexical_index_orders_score_ties_by_object_id_and_assigns_final_ranks() {
        let objects = vec![
            object("zeta.object", "claim", None, "same term"),
            object("alpha.object", "claim", None, "same term"),
        ];

        let hits = LexicalIndex::from_objects(&objects).search("same");

        assert_eq!(hit_ids(&hits), ["alpha.object", "zeta.object"]);
        assert_eq!(hits[0].lexical_rank, 1);
        assert_eq!(hits[1].lexical_rank, 2);
    }

    #[test]
    fn lexical_index_indexes_body_id_kind_and_owner_fields() {
        let objects = vec![
            object(
                "billing-policy",
                "claim",
                Some("team-platform"),
                "approved refunds",
            ),
            object(
                "finance.audit",
                "decision",
                Some("team-ledger"),
                "monthly close",
            ),
        ];
        let index = LexicalIndex::from_objects(&objects);

        assert_eq!(hit_ids(&index.search("approved")), ["billing-policy"]);
        assert_eq!(hit_ids(&index.search("billing")), ["billing-policy"]);
        assert_eq!(hit_ids(&index.search("decision")), ["finance.audit"]);
        assert_eq!(hit_ids(&index.search("ledger")), ["finance.audit"]);
    }

    #[test]
    fn lexical_index_uses_global_corpus_stats_when_searching_candidates() {
        let mut objects = vec![
            object(
                "global.alpha-heavy",
                "claim",
                None,
                "alpha alpha alpha alpha alpha alpha alpha alpha alpha alpha",
            ),
            object("global.beta-light", "claim", None, "beta"),
        ];
        for index in 0..10 {
            objects.push(object(
                &format!("outside.alpha-{index}"),
                "claim",
                None,
                "alpha",
            ));
        }
        let index = LexicalIndex::from_objects(&objects);

        let hits =
            index.search_candidates("alpha beta", ["global.alpha-heavy", "global.beta-light"]);

        assert_eq!(hit_ids(&hits), ["global.beta-light", "global.alpha-heavy"]);
    }

    fn hit_ids(hits: &[LexicalSearchHit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.id.as_str()).collect()
    }
}

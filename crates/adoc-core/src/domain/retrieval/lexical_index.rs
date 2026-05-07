use std::collections::{BTreeMap, BTreeSet};

use crate::domain::artifact::AgentJsonObject;

const BM25_K1: f64 = 1.2;
const BM25_B: f64 = 0.75;
const OWNER_FIELD: &str = "owner";

#[derive(Debug, Clone, PartialEq)]
pub(crate) struct LexicalSearchHit {
    pub(crate) id: String,
    pub(crate) lexical_rank: u32,
    pub(crate) score: f64,
}

#[derive(Debug, Clone, Default)]
pub(crate) struct LexicalIndex {
    documents: Vec<IndexedDocument>,
    document_frequencies: BTreeMap<String, usize>,
    average_document_len: f64,
}

#[derive(Debug, Clone)]
struct IndexedDocument {
    id: String,
    term_frequencies: BTreeMap<String, usize>,
    token_count: usize,
}

impl LexicalIndex {
    pub(crate) fn from_objects<'a>(objects: impl IntoIterator<Item = &'a AgentJsonObject>) -> Self {
        let mut documents = Vec::new();
        let mut document_frequencies = BTreeMap::new();
        let mut total_document_len = 0usize;

        for object in objects {
            let tokens = indexed_tokens(object);
            let token_count = tokens.len();
            total_document_len += token_count;

            let mut term_frequencies = BTreeMap::new();
            let mut seen_terms = BTreeSet::new();

            for token in tokens {
                *term_frequencies.entry(token.clone()).or_insert(0) += 1;
                seen_terms.insert(token);
            }

            for term in seen_terms {
                *document_frequencies.entry(term).or_insert(0) += 1;
            }

            documents.push(IndexedDocument {
                id: object.id.clone(),
                term_frequencies,
                token_count,
            });
        }

        let average_document_len = if documents.is_empty() {
            0.0
        } else {
            total_document_len as f64 / documents.len() as f64
        };

        Self {
            documents,
            document_frequencies,
            average_document_len,
        }
    }

    pub(crate) fn search(&self, query: &str) -> Vec<LexicalSearchHit> {
        let query_terms = tokenize(query);
        if query_terms.is_empty() || self.documents.is_empty() {
            return Vec::new();
        }

        let mut hits: Vec<_> = self
            .documents
            .iter()
            .filter_map(|document| {
                let score = self.score_document(document, &query_terms);
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

    fn score_document(&self, document: &IndexedDocument, query_terms: &[String]) -> f64 {
        query_terms
            .iter()
            .map(|term| self.score_term(document, term))
            .sum()
    }

    fn score_term(&self, document: &IndexedDocument, term: &str) -> f64 {
        let Some(&term_frequency) = document.term_frequencies.get(term) else {
            return 0.0;
        };
        let Some(&document_frequency) = self.document_frequencies.get(term) else {
            return 0.0;
        };

        let document_count = self.documents.len() as f64;
        let term_frequency = term_frequency as f64;
        let document_frequency = document_frequency as f64;
        let document_len = document.token_count as f64;
        let length_norm = if self.average_document_len == 0.0 {
            0.0
        } else {
            document_len / self.average_document_len
        };
        let idf =
            ((document_count - document_frequency + 0.5) / (document_frequency + 0.5) + 1.0).ln();
        let denominator = term_frequency + BM25_K1 * (1.0 - BM25_B + BM25_B * length_norm);

        idf * (term_frequency * (BM25_K1 + 1.0)) / denominator
    }
}

fn indexed_tokens(object: &AgentJsonObject) -> Vec<String> {
    let mut tokens = tokenize(&object.body);
    tokens.extend(tokenize(&object.id));
    tokens.extend(tokenize(&object.kind));
    if let Some(owner) = object.fields.get(OWNER_FIELD) {
        tokens.extend(tokenize(owner));
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
    use crate::domain::artifact::{AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan};

    fn object(id: &str, kind: &str, owner: Option<&str>, body: &str) -> AgentJsonObject {
        let mut fields = BTreeMap::new();
        if let Some(owner) = owner {
            fields.insert("owner".to_string(), owner.to_string());
        }

        AgentJsonObject {
            id: id.to_string(),
            kind: kind.to_string(),
            status: None,
            body: body.to_string(),
            page_id: "team.page".to_string(),
            source_span: AgentJsonSourceSpan {
                path: "docs/team.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields,
            relations: AgentJsonRelations::default(),
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

    fn hit_ids(hits: &[LexicalSearchHit]) -> Vec<&str> {
        hits.iter().map(|hit| hit.id.as_str()).collect()
    }
}

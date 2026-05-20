use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations};
use crate::domain::retrieval::metadata;

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetrievalRecord {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verified_at: Option<String>,
    pub body: String,
    pub source: RetrievalSource,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub evidence: BTreeMap<String, String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, String>,
    pub relations: RetrievalRelations,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub search_match: Option<RetrievalMatch>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct RetrievalMatch {
    pub mode: SearchMode,
    pub result_rank: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rrf_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub vector_rank: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cosine_score: Option<f32>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct RetrievalRelations {
    pub depends_on: Vec<String>,
    pub supersedes: Vec<String>,
    pub related_to: Vec<String>,
}

impl RetrievalRelations {
    pub fn is_empty(&self) -> bool {
        self.depends_on.is_empty() && self.supersedes.is_empty() && self.related_to.is_empty()
    }

    pub(crate) fn from_graph(relations: &GraphRelations) -> Self {
        Self {
            depends_on: relations.depends_on.clone(),
            supersedes: relations.supersedes.clone(),
            related_to: relations.related_to.clone(),
        }
    }

    pub(crate) fn iter_targets(&self) -> impl Iterator<Item = &str> {
        let mut targets: Vec<&str> = self
            .depends_on
            .iter()
            .chain(self.supersedes.iter())
            .chain(self.related_to.iter())
            .map(String::as_str)
            .collect();
        targets.sort_unstable();
        targets.dedup();
        targets.into_iter()
    }
}

impl RetrievalMatch {
    pub fn lexical(result_rank: u32, lexical_rank: Option<u32>) -> Self {
        Self {
            mode: SearchMode::Lexical,
            result_rank,
            rrf_score: None,
            lexical_rank,
            vector_rank: None,
            cosine_score: None,
        }
    }

    pub fn semantic(result_rank: u32, vector_rank: u32, cosine_score: f32) -> Self {
        Self {
            mode: SearchMode::Semantic,
            result_rank,
            rrf_score: None,
            lexical_rank: None,
            vector_rank: Some(vector_rank),
            cosine_score: Some(cosine_score),
        }
    }

    pub fn hybrid(
        result_rank: u32,
        rrf_score: f64,
        lexical_rank: Option<u32>,
        vector_rank: Option<u32>,
    ) -> Self {
        Self {
            mode: SearchMode::Hybrid,
            result_rank,
            rrf_score: Some(rrf_score),
            lexical_rank,
            vector_rank,
            cosine_score: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Hybrid,
    Lexical,
    Semantic,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievalSource {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

impl From<&GraphKnowledgeObjectNode> for RetrievalRecord {
    fn from(object: &GraphKnowledgeObjectNode) -> Self {
        Self::from_object_with_optional_match(object, None)
    }
}

impl RetrievalRecord {
    pub(crate) fn from_object_with_match(
        object: &GraphKnowledgeObjectNode,
        search_match: RetrievalMatch,
    ) -> Self {
        Self::from_object_with_optional_match(object, Some(search_match))
    }

    fn from_object_with_optional_match(
        object: &GraphKnowledgeObjectNode,
        search_match: Option<RetrievalMatch>,
    ) -> Self {
        Self {
            id: object.id.clone(),
            kind: object.kind.clone(),
            status: object.status.clone(),
            content_hash: object.content_hash.clone(),
            owner: metadata::owner(object).map(str::to_string),
            verified_at: metadata::verified_at(object).map(str::to_string),
            body: object.body.clone(),
            source: RetrievalSource {
                path: object.source_span.path.clone(),
                line: object.source_span.line,
                column: object.source_span.column,
            },
            evidence: metadata::evidence_fields(&object.fields),
            fields: metadata::generic_fields(&object.fields),
            relations: RetrievalRelations::from_graph(&object.relations),
            search_match,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::{GraphRelations, GraphSourceSpan};

    #[test]
    fn retrieval_record_projects_graph_object_metadata() {
        let object = GraphKnowledgeObjectNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            content_hash: "sha256:content".to_string(),
            body: "Credits are verified.".to_string(),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "billing.adoc".to_string(),
                line: 5,
                column: 1,
            },
            fields: BTreeMap::from([
                ("owner".to_string(), "team-billing".to_string()),
                ("reviewed_by".to_string(), "qa-team".to_string()),
                ("source".to_string(), "ledger".to_string()),
                ("status".to_string(), "domain-extra".to_string()),
                ("test".to_string(), "cargo test billing".to_string()),
                ("verified_at".to_string(), "2026-05-05".to_string()),
            ]),
            relations: GraphRelations {
                depends_on: vec!["billing.ledger".to_string()],
                supersedes: Vec::new(),
                related_to: Vec::new(),
            },
        };

        let record = RetrievalRecord::from(&object);

        assert_eq!(record.owner.as_deref(), Some("team-billing"));
        assert_eq!(record.verified_at.as_deref(), Some("2026-05-05"));
        assert_eq!(
            record.evidence.get("source").map(String::as_str),
            Some("ledger")
        );
        assert_eq!(
            record.evidence.get("reviewed_by").map(String::as_str),
            Some("qa-team")
        );
        assert_eq!(
            record.fields.get("status").map(String::as_str),
            Some("domain-extra")
        );
        assert_eq!(record.relations.depends_on, ["billing.ledger"]);
    }
}

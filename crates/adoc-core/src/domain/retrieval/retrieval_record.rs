use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::artifact::{AgentJsonObject, AgentJsonRelations};

const OWNER_FIELD: &str = "owner";
const VERIFIED_AT_FIELD: &str = "verified_at";
const EVIDENCE_FIELDS: [&str; 3] = ["source", "test", "reviewed_by"];

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievalRecord {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
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
    pub relations: AgentJsonRelations,
    #[serde(rename = "match", skip_serializing_if = "Option::is_none")]
    pub search_match: Option<RetrievalMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievalMatch {
    pub mode: SearchMode,
    pub result_rank: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub lexical_rank: Option<u32>,
}

impl RetrievalMatch {
    pub fn lexical(result_rank: u32, lexical_rank: Option<u32>) -> Self {
        Self {
            mode: SearchMode::Lexical,
            result_rank,
            lexical_rank,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SearchMode {
    Lexical,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct RetrievalSource {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

impl From<&AgentJsonObject> for RetrievalRecord {
    fn from(object: &AgentJsonObject) -> Self {
        Self::from_object_with_optional_match(object, None)
    }
}

impl RetrievalRecord {
    pub fn from_object_with_match(object: &AgentJsonObject, search_match: RetrievalMatch) -> Self {
        Self::from_object_with_optional_match(object, Some(search_match))
    }

    fn from_object_with_optional_match(
        object: &AgentJsonObject,
        search_match: Option<RetrievalMatch>,
    ) -> Self {
        let mut evidence = BTreeMap::new();
        let mut fields = BTreeMap::new();

        for (key, value) in &object.fields {
            if EVIDENCE_FIELDS.contains(&key.as_str()) {
                evidence.insert(key.clone(), value.clone());
            } else if key != OWNER_FIELD && key != VERIFIED_AT_FIELD {
                fields.insert(key.clone(), value.clone());
            }
        }

        Self {
            id: object.id.clone(),
            kind: object.kind.clone(),
            status: object.status.clone(),
            owner: object.fields.get(OWNER_FIELD).cloned(),
            verified_at: object.fields.get(VERIFIED_AT_FIELD).cloned(),
            body: object.body.clone(),
            source: RetrievalSource {
                path: object.source_span.path.clone(),
                line: object.source_span.line,
                column: object.source_span.column,
            },
            evidence,
            fields,
            relations: object.relations.clone(),
            search_match,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::artifact::{AgentJsonRelations, AgentJsonSourceSpan};

    #[test]
    fn retrieval_record_projects_agent_object_metadata() {
        let object = AgentJsonObject {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            body: "Credits are verified.".to_string(),
            page_id: "team.billing".to_string(),
            source_span: AgentJsonSourceSpan {
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
            relations: AgentJsonRelations {
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

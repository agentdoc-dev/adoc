use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentJsonDocument {
    pub schema_version: String,
    pub pages: Vec<AgentJsonPage>,
    pub objects: Vec<AgentJsonObject>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AgentJsonDocument {
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentJsonPage {
    pub id: String,
    pub title: Option<String>,
    pub source_path: String,
}

impl From<&PageAst> for AgentJsonPage {
    fn from(page: &PageAst) -> Self {
        Self {
            id: page.id.as_str().to_string(),
            title: page.title.clone(),
            source_path: page.source_path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentJsonObject {
    pub id: String,
    pub kind: String,
    /// Kind-primary normalized discriminant. For v0 this is the claim status,
    /// decision status, or warning severity. Objects without such a
    /// discriminant omit the field from serialized agent JSON.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub body: String,
    pub page_id: String,
    pub source_span: AgentJsonSourceSpan,
    pub fields: BTreeMap<String, String>,
    pub relations: AgentJsonRelations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AgentJsonSourceSpan {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Default)]
pub struct AgentJsonRelations {
    pub depends_on: Vec<String>,
    pub supersedes: Vec<String>,
    pub related_to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchArtifactDocument {
    pub schema_version: String,
    pub model: SearchModelHeader,
    pub agent_artifact_hash: String,
    pub embeddings: Vec<SearchEmbedding>,
}

impl SearchArtifactDocument {
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchModelHeader {
    pub id: String,
    pub provider: String,
    pub dim: usize,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct SearchEmbedding {
    pub id: String,
    pub content_hash: String,
    pub vector: Vec<f32>,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn agent_json_object_serializes_with_v0_2_shape() {
        let obj = AgentJsonObject {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            body: "The system credits users automatically.".to_string(),
            page_id: "team.guide".to_string(),
            source_span: AgentJsonSourceSpan {
                path: "sample.adoc".to_string(),
                line: 5,
                column: 1,
            },
            fields: BTreeMap::from([("owner".to_string(), "team-a".to_string())]),
            relations: AgentJsonRelations::default(),
        };

        let value = serde_json::to_value(&obj).expect("serialization must succeed");

        assert_eq!(value["id"], "billing.credits");
        assert_eq!(value["kind"], "claim");
        assert_eq!(value["status"], "verified");
        assert_eq!(value["body"], "The system credits users automatically.");
        assert_eq!(value["page_id"], "team.guide");
        assert_eq!(value["source_span"]["path"], "sample.adoc");
        assert_eq!(value["source_span"]["line"], 5);
        assert_eq!(value["source_span"]["column"], 1);
        assert_eq!(value["fields"]["owner"], "team-a");
        assert_eq!(value["relations"]["depends_on"], serde_json::json!([]));
        assert_eq!(value["relations"]["supersedes"], serde_json::json!([]));
        assert_eq!(value["relations"]["related_to"], serde_json::json!([]));
    }

    #[test]
    fn agent_json_object_omits_status_when_absent() {
        let obj = AgentJsonObject {
            id: "billing.note".to_string(),
            kind: "note".to_string(),
            status: None,
            body: "Freeform object without a kind-primary discriminant.".to_string(),
            page_id: "team.guide".to_string(),
            source_span: AgentJsonSourceSpan {
                path: "sample.adoc".to_string(),
                line: 5,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
        };

        let value = serde_json::to_value(&obj).expect("serialization must succeed");

        assert!(
            value.get("status").is_none(),
            "absent status must be omitted from agent JSON"
        );
    }

    #[test]
    fn search_artifact_serializes_with_v1_3_shape() {
        let artifact = SearchArtifactDocument {
            schema_version: "adoc.search.v0".to_string(),
            model: SearchModelHeader {
                id: "bge-small-en-v1.5".to_string(),
                provider: "fastembed".to_string(),
                dim: 384,
            },
            agent_artifact_hash: "sha256:agent".to_string(),
            embeddings: vec![SearchEmbedding {
                id: "billing.credits".to_string(),
                content_hash: "sha256:content".to_string(),
                vector: vec![0.25, -0.5],
            }],
        };

        let value = serde_json::to_value(&artifact).expect("search artifact serializes");

        assert_eq!(value["schema_version"], "adoc.search.v0");
        assert_eq!(value["model"]["id"], "bge-small-en-v1.5");
        assert_eq!(value["model"]["provider"], "fastembed");
        assert_eq!(value["model"]["dim"], 384);
        assert_eq!(value["agent_artifact_hash"], "sha256:agent");
        assert_eq!(value["embeddings"][0]["id"], "billing.credits");
        assert_eq!(value["embeddings"][0]["content_hash"], "sha256:content");
        assert_eq!(
            value["embeddings"][0]["vector"],
            serde_json::json!([0.25, -0.5])
        );
    }
}

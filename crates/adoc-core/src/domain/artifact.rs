use std::collections::BTreeMap;

use serde::Serialize;

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentJsonObject {
    pub id: String,
    pub kind: String,
    /// Kind-primary normalized discriminant. For v0 this is the claim status,
    /// decision status, or warning severity.
    pub status: String,
    pub body: String,
    pub page_id: String,
    pub source_span: AgentJsonSourceSpan,
    pub fields: BTreeMap<String, String>,
    pub relations: AgentJsonRelations,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentJsonSourceSpan {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Default)]
pub struct AgentJsonRelations {
    pub depends_on: Vec<String>,
    pub supersedes: Vec<String>,
    pub related_to: Vec<String>,
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
            status: "verified".to_string(),
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
}

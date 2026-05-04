use serde::Serialize;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentJsonObject {}

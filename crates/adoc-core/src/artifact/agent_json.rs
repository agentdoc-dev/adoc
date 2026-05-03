use serde::Serialize;

use crate::ast::PageAst;
use crate::diagnostic::Diagnostic;

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentJsonDocument {
    pub schema_version: String,
    pub pages: Vec<AgentJsonPage>,
    pub objects: Vec<AgentJsonObject>,
    pub diagnostics: Vec<Diagnostic>,
}

impl AgentJsonDocument {
    pub fn from_pages_and_diagnostics(pages: &[PageAst], diagnostics: &[Diagnostic]) -> Self {
        Self {
            schema_version: "adoc.agent.v0".to_string(),
            pages: pages.iter().map(AgentJsonPage::from).collect(),
            objects: Vec::new(),
            diagnostics: diagnostics.to_vec(),
        }
    }

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
            id: page.id.clone(),
            title: page.title.clone(),
            source_path: page.source_path.display().to_string(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AgentJsonObject {}

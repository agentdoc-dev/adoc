use crate::domain::artifact::{AgentJsonDocument, AgentJsonObject, AgentJsonPage};
use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;
use crate::domain::ports::artifact_writer::ArtifactWriter;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct AgentJsonArtifact;

impl ArtifactWriter for AgentJsonArtifact {
    type Output = AgentJsonDocument;
    fn build(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> AgentJsonDocument {
        AgentJsonDocument {
            schema_version: "adoc.agent.v0".to_string(),
            pages: pages.iter().map(AgentJsonPage::from).collect(),
            objects: Vec::new(),
            diagnostics: diagnostics.to_vec(),
        }
    }
}

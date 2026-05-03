pub(crate) mod agent_json;

use crate::ast::PageAst;
use crate::diagnostic::Diagnostic;

pub(crate) use agent_json::AgentJsonArtifact;
pub use agent_json::AgentJsonDocument;

/// Output port for compiler artifacts that aren't a single rendered string —
/// today only the agent JSON document, which has its own typed shape so that
/// the CLI can serialize it via serde at the file-write boundary.
pub(crate) trait ArtifactWriter {
    fn write(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> AgentJsonDocument;
}

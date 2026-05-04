pub(crate) mod agent_json;

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;

pub(crate) use agent_json::AgentJsonArtifact;
pub use agent_json::AgentJsonDocument;

/// Output port for compiler artifacts that aren't a single rendered string.
///
/// Each adapter declares its own `Output` shape — agent JSON today, and any
/// future structured artifact (RDF graph, Markdown index) — so adding a new
/// format is a new adapter rather than another edit to `compile.rs` or this
/// trait. Static dispatch is preserved per ADR-0006: callers pick the
/// concrete adapter at the call site.
pub(crate) trait ArtifactWriter {
    type Output;
    fn build(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> Self::Output;
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::PageAst;
    use crate::domain::identity::PageId;

    /// Stub adapter declaring a non-`AgentJsonDocument` Output. Compiling and
    /// running this proves the trait is genuinely format-agnostic.
    struct CountingArtifact;

    impl ArtifactWriter for CountingArtifact {
        type Output = String;
        fn build(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> String {
            format!(
                "{} page(s), {} diagnostic(s)",
                pages.len(),
                diagnostics.len()
            )
        }
    }

    fn page(id: &str) -> PageAst {
        PageAst {
            id: PageId::from_string(id).expect("test page id is valid"),
            title: None,
            source_path: PathBuf::from(format!("{id}.adoc")),
            blocks: Vec::new(),
        }
    }

    #[test]
    fn artifact_writer_supports_distinct_output_types() {
        let pages = vec![page("team.a"), page("team.b")];
        let diagnostics: Vec<Diagnostic> = Vec::new();

        let summary = CountingArtifact.build(&pages, &diagnostics);

        assert_eq!(summary, "2 page(s), 0 diagnostic(s)");
    }
}

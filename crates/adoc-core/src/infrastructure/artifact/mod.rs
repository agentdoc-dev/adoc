pub(crate) mod agent_json;
pub(crate) mod graph_json;
pub(crate) mod search_json;

pub(crate) use agent_json::AgentJsonArtifact;
pub(crate) use graph_json::GraphJsonArtifact;
pub(crate) use search_json::SearchJsonArtifact;

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::ast::PageAst;
    use crate::domain::diagnostic::Diagnostic;
    use crate::domain::identity::PageId;
    use crate::domain::ports::artifact_writer::ArtifactWriter;

    /// Stub adapter declaring a non-`AgentJsonDocument` Output. Compiling and
    /// running this proves the trait is genuinely format-agnostic.
    struct CountingArtifact;

    impl ArtifactWriter<[PageAst]> for CountingArtifact {
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

    #[test]
    fn graph_json_artifact_writes_from_agent_document_through_same_writer_port() {
        use crate::domain::artifact::AgentJsonDocument;
        use crate::infrastructure::artifact::GraphJsonArtifact;

        let agent_document = AgentJsonDocument {
            schema_version: "adoc.agent.v0".to_string(),
            pages: Vec::new(),
            objects: Vec::new(),
            diagnostics: Vec::new(),
        };

        let graph_document = GraphJsonArtifact.build(&agent_document, &[]);

        assert_eq!(graph_document.schema_version, "adoc.graph.v0");
        assert!(graph_document.nodes.is_empty());
        assert!(graph_document.edges.is_empty());
    }
}

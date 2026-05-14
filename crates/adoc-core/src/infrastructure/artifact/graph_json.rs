use std::fs;
use std::io;
use std::path::Path;

use crate::application::hashing::sha256_prefixed;
use crate::domain::artifact::{AgentJsonDocument, AgentJsonRelations};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{GraphArtifactDocument, GraphEdge, GraphNode, GraphRelationKind};
use crate::domain::ports::{artifact_reader::ArtifactReader, artifact_writer::ArtifactWriter};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphJsonArtifact;

pub(crate) const SUPPORTED_GRAPH_SCHEMA_VERSION: &str = "adoc.graph.v0";

pub(crate) fn read_graph_artifact_document(
    path: &Path,
) -> Result<GraphArtifactDocument, Vec<Diagnostic>> {
    let contents = match fs::read_to_string(path) {
        Ok(contents) => contents,
        Err(error) => return Err(vec![read_error_diagnostic(path, error)]),
    };
    let document = match serde_json::from_str::<GraphArtifactDocument>(&contents) {
        Ok(document) => document,
        Err(error) => {
            return Err(vec![
                Diagnostic::error(
                    DiagnosticCode::IoArtifactMalformed,
                    format!("Artifact '{}' is malformed: {error}", path.display()),
                )
                .with_help("Rebuild docs.graph.json from the source workspace."),
            ]);
        }
    };

    if document.schema_version != SUPPORTED_GRAPH_SCHEMA_VERSION {
        return Err(vec![
            Diagnostic::error(
                DiagnosticCode::SchemaUnsupportedVersion,
                format!(
                    "Artifact '{}' uses unsupported schema_version '{}'.",
                    path.display(),
                    document.schema_version
                ),
            )
            .with_help(format!(
                "Expected schema_version '{}'.",
                SUPPORTED_GRAPH_SCHEMA_VERSION
            )),
        ]);
    }

    Ok(document)
}

impl ArtifactWriter<AgentJsonDocument> for GraphJsonArtifact {
    type Output = GraphArtifactDocument;

    fn build(
        &self,
        agent_json: &AgentJsonDocument,
        _diagnostics: &[Diagnostic],
    ) -> GraphArtifactDocument {
        let agent_json_text = agent_json
            .to_pretty_json()
            .expect("agent artifact serialization should not fail");
        let mut nodes: Vec<_> = agent_json
            .objects
            .iter()
            .map(|object| GraphNode {
                id: object.id.clone(),
                kind: object.kind.clone(),
                status: object.status.clone(),
                page_id: object.page_id.clone(),
            })
            .collect();
        nodes.sort();

        let mut edges = Vec::new();
        for object in &agent_json.objects {
            push_relation_edges(
                &mut edges,
                &object.id,
                GraphRelationKind::DependsOn,
                &object.relations,
            );
            push_relation_edges(
                &mut edges,
                &object.id,
                GraphRelationKind::Supersedes,
                &object.relations,
            );
            push_relation_edges(
                &mut edges,
                &object.id,
                GraphRelationKind::RelatedTo,
                &object.relations,
            );
        }
        edges.sort();

        GraphArtifactDocument {
            schema_version: SUPPORTED_GRAPH_SCHEMA_VERSION.to_string(),
            agent_artifact_hash: sha256_prefixed(agent_json_text.as_bytes()),
            nodes,
            edges,
        }
    }
}

impl ArtifactReader for GraphJsonArtifact {
    type Output = GraphArtifactDocument;

    fn read(&self, path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
        read_graph_artifact_document(path)
    }
}

fn push_relation_edges(
    edges: &mut Vec<GraphEdge>,
    source: &str,
    relation: GraphRelationKind,
    relations: &AgentJsonRelations,
) {
    let targets = match relation {
        GraphRelationKind::DependsOn => &relations.depends_on,
        GraphRelationKind::Supersedes => &relations.supersedes,
        GraphRelationKind::RelatedTo => &relations.related_to,
    };
    for target in targets {
        edges.push(GraphEdge {
            source: source.to_string(),
            target: target.clone(),
            relation,
        });
    }
}

fn read_error_diagnostic(path: &Path, error: io::Error) -> Diagnostic {
    let code = if error.kind() == io::ErrorKind::NotFound {
        DiagnosticCode::IoArtifactMissing
    } else {
        DiagnosticCode::IoArtifactUnreadable
    };
    Diagnostic::error(
        code,
        format!("Unable to read artifact '{}': {error}", path.display()),
    )
}

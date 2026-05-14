use std::path::{Path, PathBuf};

use crate::application::hashing::sha256_prefixed;
use crate::domain::artifact::AgentJsonDocument;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::graph::{
    GraphArtifactDocument, GraphIndex, GraphTraversalEdge, GraphTraversalNode, GraphTraversalQuery,
    GraphTraversalResult,
};
use crate::domain::ports::artifact_reader::ArtifactReader;

pub const GRAPH_TRAVERSAL_SCHEMA_VERSION: &str = "adoc.graph.traversal.v0";

#[derive(Debug, Clone)]
pub struct GraphInput {
    pub agent_artifact_path: PathBuf,
    pub graph_artifact_path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct GraphLoadResult {
    pub session: Option<GraphSession>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone)]
pub struct GraphSession {
    index: GraphIndex,
}

impl GraphSession {
    pub(crate) fn related_candidate_ids(
        &self,
        query: GraphTraversalQuery,
    ) -> Result<std::collections::BTreeSet<String>, Vec<Diagnostic>> {
        self.index.related_candidate_ids(query)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, serde::Serialize)]
pub struct GraphTraversalEnvelope {
    pub schema_version: &'static str,
    pub root: String,
    pub nodes: Vec<GraphTraversalNode>,
    pub edges: Vec<GraphTraversalEdge>,
    pub diagnostics: Vec<Diagnostic>,
}

impl GraphTraversalEnvelope {
    pub fn new(
        root: String,
        nodes: Vec<GraphTraversalNode>,
        edges: Vec<GraphTraversalEdge>,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            schema_version: GRAPH_TRAVERSAL_SCHEMA_VERSION,
            root,
            nodes,
            edges,
            diagnostics,
        }
    }
}

impl From<GraphTraversalResult> for GraphTraversalEnvelope {
    fn from(result: GraphTraversalResult) -> Self {
        Self::new(result.root, result.nodes, result.edges, result.diagnostics)
    }
}

pub(crate) fn load_graph_session_with_readers<A, G>(
    input: GraphInput,
    agent_reader: &A,
    graph_reader: &G,
) -> GraphLoadResult
where
    A: ArtifactReader<Output = AgentJsonDocument>,
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let agent_document = match agent_reader.read(&input.agent_artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return GraphLoadResult {
                session: None,
                diagnostics,
            };
        }
    };
    let canonical_agent_json = agent_document
        .to_pretty_json()
        .expect("agent artifact serialization should not fail");
    load_graph_session_from_canonical_agent(
        canonical_agent_json.as_bytes(),
        &input.graph_artifact_path,
        graph_reader,
    )
}

pub(crate) fn load_graph_session_from_canonical_agent<G>(
    canonical_agent_json: &[u8],
    graph_artifact_path: &Path,
    graph_reader: &G,
) -> GraphLoadResult
where
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let graph_document = match graph_reader.read(graph_artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return GraphLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    let mut diagnostics = Vec::new();
    let actual_hash = sha256_prefixed(canonical_agent_json);
    if graph_document.agent_artifact_hash != actual_hash {
        diagnostics.push(Diagnostic::warning(
            DiagnosticCode::GraphHashDrift,
            format!(
                "Graph artifact `{}` references agent_artifact_hash `{}` but the loaded agent artifact hashes to `{}`.",
                graph_artifact_path.display(),
                graph_document.agent_artifact_hash,
                actual_hash,
            ),
        ));
    }

    let index = match GraphIndex::from_document(graph_document) {
        Ok(index) => index,
        Err(mut graph_diagnostics) => {
            diagnostics.append(&mut graph_diagnostics);
            return GraphLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    GraphLoadResult {
        session: Some(GraphSession { index }),
        diagnostics,
    }
}

pub fn traverse_graph(session: &GraphSession, query: GraphTraversalQuery) -> GraphTraversalResult {
    session.index.traverse(query)
}

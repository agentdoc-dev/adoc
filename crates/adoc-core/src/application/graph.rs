use std::path::PathBuf;

use crate::domain::diagnostic::Diagnostic;
use crate::domain::graph::{
    GraphArtifactDocument, GraphIndex, GraphKnowledgeObjectNode, GraphProseBlock,
    GraphTraversalEdge, GraphTraversalNode, GraphTraversalQuery, GraphTraversalResult,
};
use crate::domain::identity::ObjectId;
use crate::domain::ports::artifact_reader::ArtifactReader;

pub const GRAPH_TRAVERSAL_SCHEMA_VERSION: &str = "adoc.graph.traversal.v0";

#[derive(Debug, Clone)]
pub struct GraphInput {
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
    pub(crate) fn new(index: GraphIndex) -> Self {
        Self { index }
    }

    pub(crate) fn related_candidate_ids(
        &self,
        query: GraphTraversalQuery,
    ) -> Result<std::collections::BTreeSet<String>, Vec<Diagnostic>> {
        self.index.related_candidate_ids(query)
    }

    pub(crate) fn object(&self, id: &ObjectId) -> Option<&GraphKnowledgeObjectNode> {
        self.index.object(id)
    }

    pub(crate) fn objects(&self) -> impl Iterator<Item = &GraphKnowledgeObjectNode> {
        self.index.objects()
    }

    pub(crate) fn related_statuses<'a>(
        &self,
        targets: impl IntoIterator<Item = &'a str>,
    ) -> std::collections::BTreeMap<String, Option<String>> {
        self.index.related_statuses(targets)
    }

    pub(crate) fn prose_block_count(&self) -> usize {
        self.index.prose_block_count()
    }

    // V1.7.1: consumed by the prose retrieval corpus in the next commit.
    #[allow(dead_code)]
    pub(crate) fn prose_block(&self, id: &str) -> Option<&GraphProseBlock> {
        self.index.prose_block(id)
    }

    #[allow(dead_code)]
    pub(crate) fn prose_blocks(&self) -> impl Iterator<Item = &GraphProseBlock> {
        self.index.prose_blocks()
    }

    pub(crate) fn has_markdown_pages(&self) -> bool {
        self.index.has_markdown_pages()
    }

    pub(crate) fn knowledge_object_count(&self) -> usize {
        self.index.knowledge_object_count()
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

pub(crate) fn load_graph_session_with_readers<G>(
    input: GraphInput,
    graph_reader: &G,
) -> GraphLoadResult
where
    G: ArtifactReader<Output = GraphArtifactDocument>,
{
    let graph_document = match graph_reader.read(&input.graph_artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => {
            return GraphLoadResult {
                session: None,
                diagnostics,
            };
        }
    };

    let mut diagnostics = graph_document.diagnostics.clone();

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
        session: Some(GraphSession::new(index)),
        diagnostics,
    }
}

pub fn traverse_graph(session: &GraphSession, query: GraphTraversalQuery) -> GraphTraversalResult {
    session.index.traverse(query)
}

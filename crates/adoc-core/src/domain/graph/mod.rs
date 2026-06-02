use std::collections::{BTreeMap, BTreeSet, VecDeque, btree_map::Entry};
use std::fmt;

use serde::{Deserialize, Serialize};

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::identity::{OBJECT_ID_GRAMMAR_HELP, ObjectId};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphRelationKind {
    DependsOn,
    Supersedes,
    RelatedTo,
}

impl GraphRelationKind {
    pub(crate) const ALL: [Self; 3] = [Self::DependsOn, Self::Supersedes, Self::RelatedTo];

    pub fn as_str(self) -> &'static str {
        match self {
            Self::DependsOn => "depends_on",
            Self::Supersedes => "supersedes",
            Self::RelatedTo => "related_to",
        }
    }
}

impl fmt::Display for GraphRelationKind {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GraphDirection {
    Outgoing,
    Incoming,
    #[default]
    Both,
}

impl GraphDirection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Outgoing => "outgoing",
            Self::Incoming => "incoming",
            Self::Both => "both",
        }
    }

    fn includes_outgoing(self) -> bool {
        matches!(self, Self::Outgoing | Self::Both)
    }

    fn includes_incoming(self) -> bool {
        matches!(self, Self::Incoming | Self::Both)
    }
}

impl fmt::Display for GraphDirection {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub(crate) struct GraphArtifactDocument {
    pub(crate) schema_version: String,
    pub(crate) nodes: Vec<GraphNode>,
    pub(crate) edges: Vec<GraphEdge>,
    pub(crate) diagnostics: Vec<Diagnostic>,
}

impl GraphArtifactDocument {
    pub(crate) fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum GraphNode {
    Page(GraphPageNode),
    Heading(GraphBlockNode),
    Paragraph(GraphBlockNode),
    List(GraphBlockNode),
    CodeBlock(GraphBlockNode),
    KnowledgeObject(GraphKnowledgeObjectNode),
}

impl GraphNode {
    pub(crate) fn as_knowledge_object(&self) -> Option<&GraphKnowledgeObjectNode> {
        match self {
            Self::KnowledgeObject(node) => Some(node),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphPageNode {
    pub(crate) id: String,
    pub(crate) order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) title: Option<String>,
    pub(crate) source_path: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphBlockNode {
    pub(crate) id: String,
    pub(crate) page_id: String,
    pub(crate) order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) code: Option<String>,
    #[serde(skip_serializing_if = "Vec::is_empty", default)]
    pub(crate) items: Vec<String>,
    pub(crate) source_span: GraphSourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphKnowledgeObjectNode {
    pub(crate) id: String,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    pub(crate) body: String,
    pub(crate) page_id: String,
    pub(crate) source_span: GraphSourceSpan,
    pub(crate) fields: BTreeMap<String, String>,
    pub(crate) relations: GraphRelations,
    /// V3.3 opt-in source-path impact list. Repo-relative paths, sorted and
    /// deduplicated at parse time. Skipped when empty so existing fixtures
    /// without `impacts` remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) impacts: Vec<String>,
    /// V5.4 policy approver list. Populated for `policy` nodes only; skipped
    /// when empty so fixtures for other kinds remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) approved_by: Vec<String>,
    /// V5.5 agent_instruction allowed action list. Populated for
    /// `agent_instruction` nodes only; skipped when empty so fixtures for
    /// other kinds remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) allowed_actions: Vec<String>,
    /// V5.5 agent_instruction forbidden action list. Populated for
    /// `agent_instruction` nodes only; skipped when empty so fixtures for
    /// other kinds remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) forbidden_actions: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphSourceSpan {
    pub(crate) path: String,
    pub(crate) line: u32,
    pub(crate) column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, Default)]
pub(crate) struct GraphRelations {
    pub(crate) depends_on: Vec<String>,
    pub(crate) supersedes: Vec<String>,
    pub(crate) related_to: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphEdge {
    pub(crate) kind: GraphEdgeKind,
    pub(crate) source: String,
    pub(crate) target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) relation: Option<GraphRelationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) order: Option<u32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(crate) enum GraphEdgeKind {
    Contains,
    Relation,
    Reference,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphTraversalQuery {
    pub root_id: String,
    pub direction: GraphDirection,
    /// Empty means all supported relation kinds.
    pub relations: Vec<GraphRelationKind>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphTraversalResult {
    pub root: String,
    pub nodes: Vec<GraphTraversalNode>,
    pub edges: Vec<GraphTraversalEdge>,
    pub diagnostics: Vec<Diagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphTraversalNode {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub page_id: String,
    pub distance: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct GraphTraversalEdge {
    pub source: String,
    pub target: String,
    pub relation: GraphRelationKind,
    pub revisit: bool,
}

#[derive(Debug, Clone)]
pub(crate) struct GraphIndex {
    nodes: BTreeMap<ObjectId, GraphKnowledgeObjectNode>,
    page_ids: BTreeSet<String>,
    edges: Vec<GraphEdge>,
    outgoing: BTreeMap<ObjectId, Vec<usize>>,
    incoming: BTreeMap<ObjectId, Vec<usize>>,
    /// Count of prose-block nodes (Heading, Paragraph, List, CodeBlock) in the
    /// loaded artifact. Used by V4.3 retrieval to detect prose-only projects
    /// and emit the migration hint diagnostic. Not serialized.
    prose_block_count: usize,
    /// `true` when at least one `Page` node in the loaded artifact has a
    /// `source_path` ending in `.md`. Gates the migration hint: an
    /// `.adoc`-only project (prose, no typed Knowledge Objects) must NOT
    /// be told to migrate `.md` files that do not exist. Not serialized.
    has_markdown_pages: bool,
}

impl GraphIndex {
    pub(crate) fn from_document(document: GraphArtifactDocument) -> Result<Self, Vec<Diagnostic>> {
        let mut nodes = BTreeMap::new();
        let mut page_ids = BTreeSet::new();
        let mut diagnostics = Vec::new();
        let mut prose_block_count: usize = 0;
        let mut has_markdown_pages = false;

        for node in document.nodes {
            if let GraphNode::Page(page) = &node {
                page_ids.insert(page.id.clone());
                if page.source_path.ends_with(".md") {
                    has_markdown_pages = true;
                }
            }
            if matches!(
                node,
                GraphNode::Heading(_)
                    | GraphNode::Paragraph(_)
                    | GraphNode::List(_)
                    | GraphNode::CodeBlock(_)
            ) {
                prose_block_count += 1;
            }
            let Some(knowledge_object) = node.as_knowledge_object().cloned() else {
                continue;
            };
            let id_text = knowledge_object.id.clone();
            if let Some(diagnostic) = content_hash_diagnostic(&knowledge_object) {
                diagnostics.push(diagnostic);
                continue;
            }
            let node_id = match ObjectId::new(id_text.clone()) {
                Ok(node_id) => node_id,
                Err(_) => {
                    diagnostics.push(invalid_object_id_diagnostic(id_text));
                    continue;
                }
            };
            match nodes.entry(node_id) {
                Entry::Vacant(entry) => {
                    entry.insert(knowledge_object);
                }
                Entry::Occupied(_) => {
                    diagnostics.push(
                        Diagnostic::error(
                            DiagnosticCode::IdDuplicateInArtifact,
                            format!("duplicate Object ID `{id_text}` in graph artifact"),
                        )
                        .with_object_id(id_text)
                        .with_help("Rebuild docs.graph.json from validated AgentDoc Source."),
                    );
                }
            }
        }

        let mut edges: Vec<_> = document
            .edges
            .into_iter()
            .filter(|edge| edge.kind == GraphEdgeKind::Relation)
            .collect();
        edges.sort();
        let mut outgoing: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();
        let mut incoming: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();

        for (index, edge) in edges.iter().enumerate() {
            if edge.relation.is_none() {
                diagnostics.push(
                    Diagnostic::error(
                        DiagnosticCode::IoArtifactMalformed,
                        format!(
                            "Graph relation edge `{}` -> `{}` is missing a relation kind.",
                            edge.source, edge.target
                        ),
                    )
                    .with_help("Rebuild docs.graph.json from validated AgentDoc Source."),
                );
                continue;
            }
            let source_id = match ObjectId::new(edge.source.clone()) {
                Ok(id) => id,
                Err(_) => {
                    diagnostics.push(invalid_object_id_diagnostic(edge.source.clone()));
                    continue;
                }
            };
            let target_id = match ObjectId::new(edge.target.clone()) {
                Ok(id) => id,
                Err(_) => {
                    diagnostics.push(invalid_object_id_diagnostic(edge.target.clone()));
                    continue;
                }
            };
            if !nodes.contains_key(&source_id) {
                diagnostics.push(missing_graph_object_diagnostic(edge.source.clone()));
                continue;
            }
            if !nodes.contains_key(&target_id) {
                diagnostics.push(missing_graph_object_diagnostic(edge.target.clone()));
                continue;
            }
            outgoing.entry(source_id).or_default().push(index);
            incoming.entry(target_id).or_default().push(index);
        }

        if diagnostics.is_empty() {
            Ok(Self {
                nodes,
                page_ids,
                edges,
                outgoing,
                incoming,
                prose_block_count,
                has_markdown_pages,
            })
        } else {
            Err(diagnostics)
        }
    }

    pub(crate) fn prose_block_count(&self) -> usize {
        self.prose_block_count
    }

    pub(crate) fn has_markdown_pages(&self) -> bool {
        self.has_markdown_pages
    }

    pub(crate) fn knowledge_object_count(&self) -> usize {
        self.nodes.len()
    }

    pub(crate) fn traverse(&self, query: GraphTraversalQuery) -> GraphTraversalResult {
        let root = query.root_id;
        let root_id = match ObjectId::new(root.clone()) {
            Ok(root_id) => root_id,
            Err(_) => {
                return GraphTraversalResult {
                    root: root.clone(),
                    nodes: Vec::new(),
                    edges: Vec::new(),
                    diagnostics: vec![invalid_object_id_diagnostic(root)],
                };
            }
        };

        let Some(root_node) = self.nodes.get(&root_id) else {
            return GraphTraversalResult {
                root: root.clone(),
                nodes: Vec::new(),
                edges: Vec::new(),
                diagnostics: vec![missing_graph_object_diagnostic(root)],
            };
        };

        let relation_set = relation_set(&query.relations);
        let mut visited: BTreeMap<ObjectId, u32> = BTreeMap::from([(root_id.clone(), 0)]);
        let mut queue = VecDeque::from([(root_id, 0)]);
        let mut emitted_edges = BTreeSet::new();
        let mut nodes = vec![traversal_node(root_node, 0)];
        let mut edges = Vec::new();

        while let Some((current, distance)) = queue.pop_front() {
            for edge_index in self.incident_edge_indices(&current, query.direction, &relation_set) {
                let edge = &self.edges[edge_index];
                let Some(relation) = edge.relation else {
                    continue;
                };
                let edge_key = (edge.source.clone(), relation, edge.target.clone());
                if !emitted_edges.insert(edge_key) {
                    continue;
                }

                let Some(neighbor) = self.neighbor_for_edge(&current, edge, query.direction) else {
                    continue;
                };
                let revisit = visited.contains_key(&neighbor);
                edges.push(GraphTraversalEdge {
                    source: edge.source.clone(),
                    target: edge.target.clone(),
                    relation,
                    revisit,
                });

                if revisit {
                    continue;
                }

                let next_distance = distance + 1;
                visited.insert(neighbor.clone(), next_distance);
                if let Some(node) = self.nodes.get(&neighbor) {
                    nodes.push(traversal_node(node, next_distance));
                    queue.push_back((neighbor, next_distance));
                }
            }
        }

        GraphTraversalResult {
            root,
            nodes,
            edges,
            diagnostics: Vec::new(),
        }
    }

    pub(crate) fn object(&self, id: &ObjectId) -> Option<&GraphKnowledgeObjectNode> {
        self.nodes.get(id)
    }

    pub(crate) fn objects(&self) -> impl Iterator<Item = &GraphKnowledgeObjectNode> {
        self.nodes.values()
    }

    pub(crate) fn contains_object(&self, id: &ObjectId) -> bool {
        self.nodes.contains_key(id)
    }

    pub(crate) fn page_exists(&self, page_id: &str) -> bool {
        self.page_ids.contains(page_id)
    }

    pub(crate) fn object_page_id(&self, id: &ObjectId) -> Option<&str> {
        self.nodes.get(id).map(|object| object.page_id.as_str())
    }

    pub(crate) fn related_statuses<'a>(
        &self,
        targets: impl IntoIterator<Item = &'a str>,
    ) -> BTreeMap<String, Option<String>> {
        let mut statuses = BTreeMap::new();

        for target in targets {
            let status = ObjectId::new(target)
                .ok()
                .and_then(|target_id| self.nodes.get(&target_id))
                .and_then(|object| object.status.clone());
            statuses.insert(target.to_string(), status);
        }

        statuses
    }

    pub(crate) fn related_candidate_ids(
        &self,
        query: GraphTraversalQuery,
    ) -> Result<BTreeSet<String>, Vec<Diagnostic>> {
        let root = query.root_id.clone();
        let traversal = self.traverse(query);
        if !traversal.diagnostics.is_empty() {
            return Err(traversal.diagnostics);
        }
        Ok(traversal
            .nodes
            .into_iter()
            .filter(|node| node.id != root)
            .map(|node| node.id)
            .collect())
    }

    fn incident_edge_indices(
        &self,
        current: &ObjectId,
        direction: GraphDirection,
        relations: &BTreeSet<GraphRelationKind>,
    ) -> Vec<usize> {
        let mut edge_indices = Vec::new();
        if direction.includes_outgoing()
            && let Some(outgoing) = self.outgoing.get(current)
        {
            edge_indices.extend(outgoing.iter().copied());
        }
        if direction.includes_incoming()
            && let Some(incoming) = self.incoming.get(current)
        {
            edge_indices.extend(incoming.iter().copied());
        }
        edge_indices.sort_unstable();
        edge_indices.dedup();
        edge_indices
            .into_iter()
            .filter(|index| {
                self.edges[*index]
                    .relation
                    .is_some_and(|relation| relations.contains(&relation))
            })
            .collect()
    }

    fn neighbor_for_edge(
        &self,
        current: &ObjectId,
        edge: &GraphEdge,
        direction: GraphDirection,
    ) -> Option<ObjectId> {
        if direction.includes_outgoing() && edge.source == current.as_str() {
            return Some(ObjectId::new_unchecked(edge.target.clone()));
        }
        if direction.includes_incoming() && edge.target == current.as_str() {
            return Some(ObjectId::new_unchecked(edge.source.clone()));
        }
        None
    }
}

fn relation_set(relations: &[GraphRelationKind]) -> BTreeSet<GraphRelationKind> {
    if relations.is_empty() {
        GraphRelationKind::ALL.into_iter().collect()
    } else {
        relations.iter().copied().collect()
    }
}

fn traversal_node(node: &GraphKnowledgeObjectNode, distance: u32) -> GraphTraversalNode {
    GraphTraversalNode {
        id: node.id.clone(),
        kind: node.kind.clone(),
        status: node.status.clone(),
        page_id: node.page_id.clone(),
        distance,
    }
}

fn invalid_object_id_diagnostic(id: impl Into<String>) -> Diagnostic {
    let id = id.into();
    Diagnostic::error(
        DiagnosticCode::IdInvalid,
        format!("Object ID `{id}` is invalid."),
    )
    .with_object_id(id)
    .with_help(OBJECT_ID_GRAMMAR_HELP)
}

fn missing_graph_object_diagnostic(id: impl Into<String>) -> Diagnostic {
    let id = id.into();
    Diagnostic::error(
        DiagnosticCode::GraphObjectNotFound,
        format!("Object ID `{id}` was not found in the graph artifact."),
    )
    .with_object_id(id)
    .with_help("Run `adoc build` if the source was changed after the graph artifact was generated.")
}

fn content_hash_diagnostic(node: &GraphKnowledgeObjectNode) -> Option<Diagnostic> {
    if node.content_hash.trim().is_empty() {
        return Some(
            Diagnostic::error(
                DiagnosticCode::IoArtifactMalformed,
                format!(
                    "Graph Knowledge Object `{}` is missing content_hash.",
                    node.id
                ),
            )
            .with_object_id(&node.id)
            .with_help("Rebuild docs.graph.json from validated AgentDoc Source."),
        );
    }

    let Some(suffix) = node.content_hash.strip_prefix("sha256:") else {
        return Some(invalid_content_hash_diagnostic(node));
    };
    if suffix.trim().is_empty() {
        return Some(invalid_content_hash_diagnostic(node));
    }

    None
}

fn invalid_content_hash_diagnostic(node: &GraphKnowledgeObjectNode) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::IoArtifactMalformed,
        format!(
            "Graph Knowledge Object `{}` has invalid content_hash `{}`.",
            node.id, node.content_hash
        ),
    )
    .with_object_id(&node.id)
    .with_help("Graph Artifact v2 content_hash values must start with `sha256:` and include a non-empty suffix.")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn graph_document(content_hash: Option<&str>) -> GraphArtifactDocument {
        GraphArtifactDocument {
            schema_version: "adoc.graph.v3".to_string(),
            nodes: vec![
                GraphNode::Page(GraphPageNode {
                    id: "team.page".to_string(),
                    order: 0,
                    title: None,
                    source_path: "docs/team.adoc".to_string(),
                }),
                GraphNode::KnowledgeObject(GraphKnowledgeObjectNode {
                    id: "billing.credits".to_string(),
                    kind: "claim".to_string(),
                    content_hash: content_hash.unwrap_or_default().to_string(),
                    status: Some("draft".to_string()),
                    body: "Credits apply after payment.".to_string(),
                    page_id: "team.page".to_string(),
                    source_span: GraphSourceSpan {
                        path: "docs/team.adoc".to_string(),
                        line: 1,
                        column: 1,
                    },
                    fields: BTreeMap::new(),
                    relations: GraphRelations::default(),
                    impacts: Vec::new(),
                    approved_by: Vec::new(),
                    allowed_actions: Vec::new(),
                    forbidden_actions: Vec::new(),
                }),
            ],
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn from_document_rejects_missing_content_hash_without_adapter() {
        let diagnostics =
            GraphIndex::from_document(graph_document(None)).expect_err("missing hash fails");

        assert_eq!(diagnostics[0].code, DiagnosticCode::IoArtifactMalformed);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
        assert!(diagnostics[0].message.contains("missing content_hash"));
    }

    #[test]
    fn from_document_rejects_empty_content_hash_without_adapter() {
        let diagnostics =
            GraphIndex::from_document(graph_document(Some(" \t "))).expect_err("empty hash fails");

        assert_eq!(diagnostics[0].code, DiagnosticCode::IoArtifactMalformed);
        assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
        assert!(diagnostics[0].message.contains("missing content_hash"));
    }

    #[test]
    fn from_document_rejects_unprefixed_or_empty_sha256_content_hash() {
        for hash in ["content", "sha256:"] {
            let diagnostics =
                GraphIndex::from_document(graph_document(Some(hash))).expect_err("hash fails");

            assert_eq!(diagnostics[0].code, DiagnosticCode::IoArtifactMalformed);
            assert_eq!(diagnostics[0].object_id.as_deref(), Some("billing.credits"));
            assert!(diagnostics[0].message.contains("invalid content_hash"));
        }
    }

    #[test]
    fn from_document_accepts_sha256_prefixed_content_hash_with_fake_suffix() {
        let graph = GraphIndex::from_document(graph_document(Some("sha256:billing.credits")))
            .expect("fake but prefixed hash is accepted");

        assert!(graph.contains_object(&ObjectId::new_unchecked("billing.credits".to_string())));
    }

    fn prose_only_document(source_paths: &[&str]) -> GraphArtifactDocument {
        let nodes = source_paths
            .iter()
            .enumerate()
            .map(|(i, path)| {
                GraphNode::Page(GraphPageNode {
                    id: format!("page.{i}"),
                    order: i as u32,
                    title: None,
                    source_path: (*path).to_string(),
                })
            })
            .collect();
        GraphArtifactDocument {
            schema_version: "adoc.graph.v3".to_string(),
            nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn from_document_sets_has_markdown_pages_when_md_page_present() {
        let graph =
            GraphIndex::from_document(prose_only_document(&["docs/team.adoc", "docs/guide.md"]))
                .expect("indexes without errors");

        assert!(
            graph.has_markdown_pages(),
            "has_markdown_pages must be true when at least one .md page is present"
        );
    }

    #[test]
    fn from_document_clears_has_markdown_pages_for_adoc_only_pages() {
        let graph =
            GraphIndex::from_document(prose_only_document(&["docs/team.adoc", "docs/ref.adoc"]))
                .expect("indexes without errors");

        assert!(
            !graph.has_markdown_pages(),
            "has_markdown_pages must be false when all pages are .adoc"
        );
    }
}

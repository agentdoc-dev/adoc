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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GraphArtifactDocument {
    pub schema_version: String,
    pub agent_artifact_hash: String,
    pub nodes: Vec<GraphNode>,
    pub edges: Vec<GraphEdge>,
}

impl GraphArtifactDocument {
    pub fn to_pretty_json(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string_pretty(self)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphNode {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<String>,
    pub page_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GraphEdge {
    pub source: String,
    pub target: String,
    pub relation: GraphRelationKind,
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
    nodes: BTreeMap<ObjectId, GraphNode>,
    edges: Vec<GraphEdge>,
    outgoing: BTreeMap<ObjectId, Vec<usize>>,
    incoming: BTreeMap<ObjectId, Vec<usize>>,
}

impl GraphIndex {
    pub(crate) fn from_document(document: GraphArtifactDocument) -> Result<Self, Vec<Diagnostic>> {
        let mut nodes = BTreeMap::new();
        let mut diagnostics = Vec::new();

        for node in document.nodes {
            let id_text = node.id.clone();
            let node_id = match ObjectId::new(id_text.clone()) {
                Ok(node_id) => node_id,
                Err(_) => {
                    diagnostics.push(invalid_object_id_diagnostic(id_text));
                    continue;
                }
            };
            match nodes.entry(node_id) {
                Entry::Vacant(entry) => {
                    entry.insert(node);
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

        let mut edges = document.edges;
        edges.sort();
        let mut outgoing: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();
        let mut incoming: BTreeMap<ObjectId, Vec<usize>> = BTreeMap::new();

        for (index, edge) in edges.iter().enumerate() {
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
                edges,
                outgoing,
                incoming,
            })
        } else {
            Err(diagnostics)
        }
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
                let edge_key = (edge.source.clone(), edge.relation, edge.target.clone());
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
                    relation: edge.relation,
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
            .filter(|index| relations.contains(&self.edges[*index].relation))
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

fn traversal_node(node: &GraphNode, distance: u32) -> GraphTraversalNode {
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

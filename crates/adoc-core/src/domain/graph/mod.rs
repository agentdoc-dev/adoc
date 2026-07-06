use std::borrow::Cow;
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

// `GraphKnowledgeObjectNode` is large by design (carries all graph-node fields
// inline for zero-copy serde). Boxing here would add indirection on every graph
// traversal; the size asymmetry is acceptable per the `adoc.graph.v4` contract.
#[allow(clippy::large_enum_variant)]
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

    /// V1.7.2: the prose counterpart of [`Self::as_knowledge_object`] —
    /// pairs the block payload with its variant's [`ProseBlockKind`].
    pub(crate) fn as_prose_block(&self) -> Option<(ProseBlockKind, &GraphBlockNode)> {
        match self {
            Self::Heading(block) => Some((ProseBlockKind::Heading, block)),
            Self::Paragraph(block) => Some((ProseBlockKind::Paragraph, block)),
            Self::List(block) => Some((ProseBlockKind::List, block)),
            Self::CodeBlock(block) => Some((ProseBlockKind::CodeBlock, block)),
            Self::Page(_) | Self::KnowledgeObject(_) => None,
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

/// V1.7.1 (ADR-0040): the closed prose block kind set, mirroring the four
/// prose `GraphNode` variants. Serialized into `adoc.retrieval.v1` prose
/// records as `block_kind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProseBlockKind {
    Heading,
    Paragraph,
    List,
    CodeBlock,
}

impl ProseBlockKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Heading => "heading",
            Self::Paragraph => "paragraph",
            Self::List => "list",
            Self::CodeBlock => "code_block",
        }
    }

    /// Selects a block's canonical searchable text from its payload parts:
    /// `text` for headings and paragraphs, `code` for code blocks,
    /// newline-joined `items` for lists (a list's `text` is its
    /// `ordered`/`unordered` marker, not content). Single source of truth
    /// for both retained prose blocks and raw artifact nodes (V1.7.2).
    pub(crate) fn content_text_from<'a>(
        self,
        text: Option<&'a str>,
        code: Option<&'a str>,
        items: &'a [String],
    ) -> Cow<'a, str> {
        match self {
            Self::CodeBlock => Cow::from(code.unwrap_or_default()),
            Self::List => Cow::from(items.join("\n")),
            Self::Heading | Self::Paragraph => Cow::from(text.unwrap_or_default()),
        }
    }
}

/// V1.7.1 (ADR-0040): a prose block retained by `GraphIndex` for retrieval.
///
/// Carries the artifact-authored `GraphBlockNode` payload plus two derived
/// fields: the variant discriminant (`kind`) and the nearest-ancestor-heading
/// breadcrumb (`heading_context`), both computed at artifact-load time and
/// never serialized back — `adoc.graph.v4` node shapes are untouched.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct GraphProseBlock {
    pub(crate) id: String,
    pub(crate) page_id: String,
    pub(crate) kind: ProseBlockKind,
    pub(crate) order: u32,
    pub(crate) text: Option<String>,
    pub(crate) code: Option<String>,
    pub(crate) items: Vec<String>,
    /// Ancestor headings joined with `" > "`, e.g.
    /// `"Billing basics > How credits are spent"`. `None` for blocks that
    /// precede any heading on their page.
    pub(crate) heading_context: Option<String>,
    pub(crate) source_span: GraphSourceSpan,
}

impl GraphProseBlock {
    /// The block's canonical searchable text: `text` for headings and
    /// paragraphs, `code` for code blocks, newline-joined `items` for lists
    /// (a list's `text` is its `ordered`/`unordered` marker, not content).
    /// Feeds both the lexical corpus and the `adoc.retrieval.v1` prose
    /// record's `text` field (ADR-0040). Borrows wherever the block already
    /// owns the text; only lists allocate (there is no pre-joined string).
    pub(crate) fn content_text_ref(&self) -> Cow<'_, str> {
        self.kind
            .content_text_from(self.text.as_deref(), self.code.as_deref(), &self.items)
    }

    /// Owned variant of [`Self::content_text_ref`] for record construction.
    pub(crate) fn content_text(&self) -> String {
        self.content_text_ref().into_owned()
    }
}

/// V5.8 inline evidence entry in the graph node's `evidence` array.
///
/// Serialized as `{ "kind": "<snake_case>", "value": "<inline text>" }`.
/// The optional `reference` field is reserved for TB2 (`ObjectRef` variant)
/// and is `None` for all inline entries written by TB1.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphEvidence {
    /// The canonical snake_case [`EvidenceKind`] string, e.g. `"source_code"`.
    pub(crate) kind: String,
    /// The inline evidence text value. `None` is reserved for future TB2
    /// `ObjectRef` entries where the evidence points to another object rather
    /// than carrying a literal string.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) value: Option<String>,
    /// Reserved for TB2: the object-reference target ID. Always `None` in TB1.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) reference: Option<String>,
}

impl GraphEvidence {
    /// Construct an inline `GraphEvidence` entry from a kind string and value.
    pub(crate) fn inline(kind: impl Into<String>, value: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: Some(value.into()),
            reference: None,
        }
    }

    /// Construct an object-reference `GraphEvidence` entry.
    ///
    /// `kind` is the evidence kind of the target `source` object (its
    /// `fields["kind"]` string). `id` is the target object's ID string.
    /// `value` is always `None` for object-ref entries.
    pub(crate) fn object_ref(kind: impl Into<String>, id: impl Into<String>) -> Self {
        Self {
            kind: kind.into(),
            value: None,
            reference: Some(id.into()),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GraphKnowledgeObjectNode {
    pub(crate) id: String,
    pub(crate) kind: String,
    #[serde(default)]
    pub(crate) content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) status: Option<String>,
    /// ADR-0039: the authored severity carrier. Populated for `warning`,
    /// `constraint`, and `contradiction` nodes; part of `content_hash`.
    /// Skipped when `None` so other kinds remain byte-stable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) severity: Option<String>,
    /// ADR-0039: the authored trust carrier. Populated for
    /// `agent_instruction` nodes only; part of `content_hash`. Skipped when
    /// `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) trust: Option<String>,
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
    /// V5.6 contradiction claim ID list. Populated for `contradiction` nodes
    /// only; skipped when empty so fixtures for other kinds remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) contradiction_claims: Vec<String>,
    /// V5.8 typed evidence array. Replaces flat `source`/`test`/`reviewed_by`
    /// fields. Skipped when empty so non-verified objects remain byte-stable.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) evidence: Vec<GraphEvidence>,
    /// V5.10 derived effective lifecycle status. `Some("stale")` when the
    /// authored status is `"verified"` and `expires_at < today`. Not authored,
    /// not hashed — purely additive projection. Skipped when `None` so existing
    /// fixtures without expiry remain byte-stable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effective_status: Option<String>,
    /// V5.10 reason string for `effective_status`. Format:
    /// `"expired:<YYYY-MM-DD>"`. Always `None` when `effective_status` is `None`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) effective_reason: Option<String>,
    /// V5.10 TB3: derived best evidence quality tier for this object. One of
    /// `"high"`, `"medium"`, or `"low"` when the object has at least one
    /// tier-able inline evidence entry. `None` when there is no evidence or
    /// all evidence entries are `ObjectRef` with an unrecognised kind.
    ///
    /// Not authored, not hashed — purely additive projection. Skipped when
    /// `None` so existing fixtures without evidence remain byte-stable.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub(crate) evidence_quality: Option<String>,
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
    /// V5.8 TB2: a derived edge from a claim to the `source` object named in
    /// `evidence_ref:`. Not a user relation field; not traversable via the
    /// `--relation` filter.
    Evidence,
    /// V6.5.3: a derived edge from an answered `question` to the
    /// claim/decision named in `resolved_by:`. Not a user relation field;
    /// not traversable via the `--relation` filter.
    ResolvedBy,
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
    /// V1.7.1: prose-block nodes (Heading, Paragraph, List, CodeBlock)
    /// retained for retrieval, keyed by block id (`<page-id>#block-NNNN`).
    /// `prose_block_count()` derives from this map, so the V4.3
    /// prose-only-project detection is unchanged. Not serialized.
    prose: BTreeMap<String, GraphProseBlock>,
    /// Per-page prose block ids in `order`-sorted document order.
    prose_by_page: BTreeMap<String, Vec<String>>,
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
        let mut prose_nodes: Vec<(ProseBlockKind, GraphBlockNode)> = Vec::new();
        let mut has_markdown_pages = false;

        for node in document.nodes {
            if let GraphNode::Page(page) = &node {
                page_ids.insert(page.id.clone());
                if page.source_path.ends_with(".md") {
                    has_markdown_pages = true;
                }
            }
            if let Some((kind, block)) = node.as_prose_block() {
                prose_nodes.push((kind, block.clone()));
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
            let (prose, prose_by_page) = index_prose_blocks(prose_nodes);
            Ok(Self {
                nodes,
                page_ids,
                edges,
                outgoing,
                incoming,
                prose,
                prose_by_page,
                has_markdown_pages,
            })
        } else {
            Err(diagnostics)
        }
    }

    pub(crate) fn prose_block_count(&self) -> usize {
        self.prose.len()
    }

    pub(crate) fn prose_block(&self, id: &str) -> Option<&GraphProseBlock> {
        self.prose.get(id)
    }

    /// Prose blocks in per-page document order (pages sorted by id).
    pub(crate) fn prose_blocks(&self) -> impl Iterator<Item = &GraphProseBlock> {
        self.prose_by_page
            .values()
            .flatten()
            .filter_map(|id| self.prose.get(id))
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

/// V1.7.1: build the retained prose maps and derive each block's
/// nearest-ancestor-heading context.
///
/// Blocks are grouped per page and walked in `order`. A heading stack of
/// `(level, text)` pairs tracks the open ancestors: a heading of level L pops
/// entries at level >= L, takes the remaining stack as its own context, then
/// pushes itself; every other block takes the full current stack. The
/// breadcrumb joins with `" > "`; blocks before the first heading get `None`.
fn index_prose_blocks(
    prose_nodes: Vec<(ProseBlockKind, GraphBlockNode)>,
) -> (
    BTreeMap<String, GraphProseBlock>,
    BTreeMap<String, Vec<String>>,
) {
    let mut by_page: BTreeMap<String, Vec<(ProseBlockKind, GraphBlockNode)>> = BTreeMap::new();
    for (kind, block) in prose_nodes {
        by_page
            .entry(block.page_id.clone())
            .or_default()
            .push((kind, block));
    }

    let mut prose = BTreeMap::new();
    let mut prose_by_page = BTreeMap::new();

    for (page_id, mut blocks) in by_page {
        blocks.sort_by_key(|(_, block)| block.order);
        let mut heading_stack: Vec<(u8, String)> = Vec::new();
        let mut page_block_ids = Vec::with_capacity(blocks.len());

        for (kind, block) in blocks {
            if kind == ProseBlockKind::Heading {
                let level = block.level.unwrap_or(1);
                while heading_stack
                    .last()
                    .is_some_and(|(open_level, _)| *open_level >= level)
                {
                    heading_stack.pop();
                }
            }
            let heading_context = (!heading_stack.is_empty()).then(|| {
                heading_stack
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<Vec<_>>()
                    .join(" > ")
            });
            if kind == ProseBlockKind::Heading {
                // Untitled headings (malformed source) contribute no
                // breadcrumb segment.
                let text = block.text.clone().unwrap_or_default();
                if !text.is_empty() {
                    heading_stack.push((block.level.unwrap_or(1), text));
                }
            }

            page_block_ids.push(block.id.clone());
            prose.insert(
                block.id.clone(),
                GraphProseBlock {
                    id: block.id,
                    page_id: block.page_id,
                    kind,
                    order: block.order,
                    text: block.text,
                    code: block.code,
                    items: block.items,
                    heading_context,
                    source_span: block.source_span,
                },
            );
        }

        prose_by_page.insert(page_id, page_block_ids);
    }

    (prose, prose_by_page)
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
            schema_version: "adoc.graph.v4".to_string(),
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
                    severity: None,
                    trust: None,
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
                    contradiction_claims: Vec::new(),
                    evidence: Vec::new(),
                    effective_status: None,
                    effective_reason: None,
                    evidence_quality: None,
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
            schema_version: "adoc.graph.v4".to_string(),
            nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    fn block(id: &str, page_id: &str, order: u32, level: Option<u8>, text: &str) -> GraphBlockNode {
        GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level,
            text: Some(text.to_string()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: GraphSourceSpan {
                path: "docs/guide.md".to_string(),
                line: order + 1,
                column: 1,
            },
        }
    }

    fn prose_document(nodes: Vec<GraphNode>) -> GraphArtifactDocument {
        let mut all_nodes = vec![GraphNode::Page(GraphPageNode {
            id: "guides.page".to_string(),
            order: 0,
            title: None,
            source_path: "docs/guide.md".to_string(),
        })];
        all_nodes.extend(nodes);
        GraphArtifactDocument {
            schema_version: "adoc.graph.v4".to_string(),
            nodes: all_nodes,
            edges: Vec::new(),
            diagnostics: Vec::new(),
        }
    }

    #[test]
    fn from_document_retains_prose_blocks_and_derives_count() {
        let graph = GraphIndex::from_document(prose_document(vec![
            GraphNode::Heading(block(
                "guides.page#block-0000",
                "guides.page",
                0,
                Some(1),
                "Intro",
            )),
            GraphNode::Paragraph(block(
                "guides.page#block-0001",
                "guides.page",
                1,
                None,
                "Body.",
            )),
        ]))
        .expect("indexes without errors");

        assert_eq!(graph.prose_block_count(), 2);
        let paragraph = graph
            .prose_block("guides.page#block-0001")
            .expect("paragraph retained");
        assert_eq!(paragraph.kind, ProseBlockKind::Paragraph);
        assert_eq!(paragraph.text.as_deref(), Some("Body."));
        assert_eq!(paragraph.source_span.line, 2);
    }

    #[test]
    fn heading_context_tracks_ancestor_stack_and_pops_on_level_decrease() {
        let graph = GraphIndex::from_document(prose_document(vec![
            GraphNode::Paragraph(block(
                "guides.page#block-0000",
                "guides.page",
                0,
                None,
                "Preamble.",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0001",
                "guides.page",
                1,
                Some(1),
                "Billing basics",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0002",
                "guides.page",
                2,
                Some(2),
                "How credits are spent",
            )),
            GraphNode::Paragraph(block(
                "guides.page#block-0003",
                "guides.page",
                3,
                None,
                "Credits burn on completion.",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0004",
                "guides.page",
                4,
                Some(2),
                "Refunds",
            )),
            GraphNode::Paragraph(block(
                "guides.page#block-0005",
                "guides.page",
                5,
                None,
                "Refunds are manual.",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0006",
                "guides.page",
                6,
                Some(1),
                "Appendix",
            )),
            GraphNode::Paragraph(block(
                "guides.page#block-0007",
                "guides.page",
                7,
                None,
                "Fin.",
            )),
        ]))
        .expect("indexes without errors");

        let context = |id: &str| {
            graph
                .prose_block(id)
                .expect("block retained")
                .heading_context
                .clone()
        };
        // Before any heading: no context.
        assert_eq!(context("guides.page#block-0000"), None);
        // A heading's own context is its ancestors, not itself.
        assert_eq!(context("guides.page#block-0001"), None);
        assert_eq!(
            context("guides.page#block-0002"),
            Some("Billing basics".to_string())
        );
        // A block under a nested heading gets the full breadcrumb.
        assert_eq!(
            context("guides.page#block-0003"),
            Some("Billing basics > How credits are spent".to_string())
        );
        // A sibling H2 pops the previous H2 before taking its context.
        assert_eq!(
            context("guides.page#block-0004"),
            Some("Billing basics".to_string())
        );
        assert_eq!(
            context("guides.page#block-0005"),
            Some("Billing basics > Refunds".to_string())
        );
        // A new H1 pops everything.
        assert_eq!(context("guides.page#block-0006"), None);
        assert_eq!(
            context("guides.page#block-0007"),
            Some("Appendix".to_string())
        );
    }

    /// The lexical corpus tokenizes via the borrowing accessor while the
    /// prose record materializes an owned copy — the two must never drift,
    /// and only lists (which have no pre-joined text) may allocate.
    #[test]
    fn content_text_ref_matches_content_text_and_borrows_where_possible() {
        let prose_block = |kind: ProseBlockKind| GraphProseBlock {
            id: "guides.page#block-0000".to_string(),
            page_id: "guides.page".to_string(),
            kind,
            order: 0,
            text: Some("Some text.".to_string()),
            code: Some("let x = 1;".to_string()),
            items: vec!["first".to_string(), "second".to_string()],
            heading_context: None,
            source_span: GraphSourceSpan {
                path: "docs/guide.md".to_string(),
                line: 1,
                column: 1,
            },
        };

        for kind in [
            ProseBlockKind::Heading,
            ProseBlockKind::Paragraph,
            ProseBlockKind::List,
            ProseBlockKind::CodeBlock,
        ] {
            let block = prose_block(kind);
            assert_eq!(block.content_text_ref(), block.content_text());
        }
        for kind in [
            ProseBlockKind::Heading,
            ProseBlockKind::Paragraph,
            ProseBlockKind::CodeBlock,
        ] {
            let block = prose_block(kind);
            assert!(
                matches!(block.content_text_ref(), std::borrow::Cow::Borrowed(_)),
                "{kind:?} owns its text and must not allocate for indexing"
            );
        }
    }

    #[test]
    fn heading_context_skips_headings_without_text() {
        // Defence-in-depth: a malformed source (e.g. a bare `#`) can yield a
        // heading with no text; the breadcrumb must not carry empty segments.
        let mut untitled_h1 = block("guides.page#block-0000", "guides.page", 0, Some(1), "");
        untitled_h1.text = None;
        let empty_h2 = block("guides.page#block-0003", "guides.page", 3, Some(2), "");
        let graph = GraphIndex::from_document(prose_document(vec![
            GraphNode::Heading(untitled_h1),
            GraphNode::Paragraph(block(
                "guides.page#block-0001",
                "guides.page",
                1,
                None,
                "Orphaned.",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0002",
                "guides.page",
                2,
                Some(1),
                "Named",
            )),
            GraphNode::Heading(empty_h2),
            GraphNode::Paragraph(block(
                "guides.page#block-0004",
                "guides.page",
                4,
                None,
                "Under the empty heading.",
            )),
        ]))
        .expect("indexes without errors");

        let context = |id: &str| {
            graph
                .prose_block(id)
                .expect("block retained")
                .heading_context
                .clone()
        };
        // Only ancestor is an untitled heading: no context, not Some("").
        assert_eq!(context("guides.page#block-0001"), None);
        // An empty heading between a named ancestor and the block contributes
        // no segment: "Named", not "Named > ".
        assert_eq!(context("guides.page#block-0004"), Some("Named".to_string()));
    }

    #[test]
    fn prose_blocks_iterate_in_page_order_regardless_of_artifact_node_order() {
        // Nodes deliberately out of document order in the artifact.
        let graph = GraphIndex::from_document(prose_document(vec![
            GraphNode::Paragraph(block(
                "guides.page#block-0002",
                "guides.page",
                2,
                None,
                "Second.",
            )),
            GraphNode::Heading(block(
                "guides.page#block-0000",
                "guides.page",
                0,
                Some(1),
                "Title",
            )),
            GraphNode::Paragraph(block(
                "guides.page#block-0001",
                "guides.page",
                1,
                None,
                "First.",
            )),
        ]))
        .expect("indexes without errors");

        let ids: Vec<&str> = graph.prose_blocks().map(|b| b.id.as_str()).collect();
        assert_eq!(
            ids,
            [
                "guides.page#block-0000",
                "guides.page#block-0001",
                "guides.page#block-0002"
            ]
        );
        // Order-derived context holds even with shuffled artifact nodes.
        assert_eq!(
            graph
                .prose_block("guides.page#block-0002")
                .expect("block retained")
                .heading_context
                .as_deref(),
            Some("Title")
        );
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

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::application::hashing::sha256_prefixed;
use crate::domain::ast::{BlockAst, ListKind, PageAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::graph::{
    GraphArtifactDocument, GraphBlockNode, GraphEdge, GraphEdgeKind, GraphEvidence,
    GraphKnowledgeObjectNode, GraphNode, GraphPageNode, GraphRelationKind, GraphRelations,
    GraphSourceSpan,
};
use crate::domain::inline::{InlineSegment, to_source};
use crate::domain::knowledge_object::{
    KnowledgeObject, RelationTarget, Relations, contradiction::Contradiction, policy::Policy,
    projection::MetadataField,
};
use crate::domain::ports::{artifact_reader::ArtifactReader, artifact_writer::ArtifactWriter};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphJsonArtifact;

pub(crate) const SUPPORTED_GRAPH_SCHEMA_VERSION: &str = "adoc.graph.v3";

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

impl ArtifactWriter<WorkspaceAst> for GraphJsonArtifact {
    type Output = GraphArtifactDocument;

    fn build(&self, workspace: &WorkspaceAst, diagnostics: &[Diagnostic]) -> GraphArtifactDocument {
        let mut nodes = Vec::new();
        let mut edges = Vec::new();

        for (page_index, page) in workspace.pages.iter().enumerate() {
            let page_id = page.id.as_str().to_string();
            nodes.push(GraphNode::Page(GraphPageNode {
                id: page_id.clone(),
                order: page_index as u32,
                title: page.title.clone(),
                source_path: page.source_path.display().to_string(),
            }));

            for (block_index, block) in page.blocks.iter().enumerate() {
                let order = block_index as u32;
                let node_id = block_node_id(page, block, order);
                nodes.push(block_to_graph_node(block, &node_id, &page_id, order));
                edges.push(GraphEdge {
                    kind: GraphEdgeKind::Contains,
                    source: page_id.clone(),
                    target: node_id.clone(),
                    relation: None,
                    order: Some(order),
                });
                push_reference_edges(&mut edges, block, &node_id);
                if let BlockAst::KnowledgeObject(knowledge_object) = block {
                    push_relation_edges(&mut edges, knowledge_object);
                }
            }
        }

        // Post-assembly pass: resolve evidence_ref entries on claims.
        // The target source node's kind lives in `fields["kind"]` of the
        // assembled KnowledgeObject graph node.
        let source_kind_map: std::collections::HashMap<String, String> = nodes
            .iter()
            .filter_map(|node| {
                let GraphNode::KnowledgeObject(ko) = node else {
                    return None;
                };
                if ko.kind != "source" {
                    return None;
                }
                let kind = ko.fields.get("kind")?.clone();
                Some((ko.id.clone(), kind))
            })
            .collect();

        // Append evidence-ref array entries and derived evidence edges for
        // every claim that carries evidence_refs.
        let claim_evidence_refs: Vec<(String, Vec<String>)> = workspace
            .pages
            .iter()
            .flat_map(|page| page.blocks.iter())
            .filter_map(|block| {
                let BlockAst::KnowledgeObject(ko) = block else {
                    return None;
                };
                let KnowledgeObject::Claim(claim) = ko.as_ref() else {
                    return None;
                };
                if claim.evidence_refs().is_empty() {
                    return None;
                }
                // Each entry is Evidence::ObjectRef; target_id() is always Some.
                let refs: Vec<String> = claim
                    .evidence_refs()
                    .iter()
                    .filter_map(|ev| ev.target_id())
                    .map(|id| id.as_str().to_string())
                    .collect();
                Some((claim.id().as_str().to_string(), refs))
            })
            .collect();

        for (claim_id, ref_ids) in &claim_evidence_refs {
            // Append evidence edges for this claim.
            for ref_id in ref_ids {
                edges.push(GraphEdge {
                    kind: GraphEdgeKind::Evidence,
                    source: claim_id.clone(),
                    target: ref_id.clone(),
                    relation: None,
                    order: None,
                });
            }
            // Append GraphEvidence entries to the claim's graph node.
            for node in nodes.iter_mut() {
                let GraphNode::KnowledgeObject(ko) = node else {
                    continue;
                };
                if ko.id != *claim_id {
                    continue;
                }
                for ref_id in ref_ids {
                    let kind = source_kind_map
                        .get(ref_id.as_str())
                        .map(String::as_str)
                        .unwrap_or("");
                    ko.evidence
                        .push(GraphEvidence::object_ref(kind, ref_id.clone()));
                }
                // Recompute the content hash now that evidence changed.
                ko.content_hash = graph_knowledge_object_content_hash(ko);
                break;
            }
        }

        nodes.sort();
        edges.sort();

        GraphArtifactDocument {
            schema_version: SUPPORTED_GRAPH_SCHEMA_VERSION.to_string(),
            nodes,
            edges,
            diagnostics: diagnostics.to_vec(),
        }
    }
}

impl ArtifactReader for GraphJsonArtifact {
    type Output = GraphArtifactDocument;

    fn read(&self, path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
        read_graph_artifact_document(path)
    }
}

fn block_node_id(page: &PageAst, block: &BlockAst, order: u32) -> String {
    match block {
        BlockAst::KnowledgeObject(knowledge_object) => knowledge_object.id().as_str().to_string(),
        _ => format!("{}#block-{order:04}", page.id.as_str()),
    }
}

fn block_to_graph_node(block: &BlockAst, id: &str, page_id: &str, order: u32) -> GraphNode {
    match block {
        BlockAst::Heading(heading) => GraphNode::Heading(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: Some(heading.level),
            text: Some(to_source(&heading.inlines)),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&heading.span),
        }),
        BlockAst::Paragraph(paragraph) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(to_source(&paragraph.inlines)),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&paragraph.span),
        }),
        BlockAst::List(list) => GraphNode::List(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(match list.kind {
                ListKind::Ordered => "ordered".to_string(),
                ListKind::Unordered => "unordered".to_string(),
            }),
            language: None,
            code: None,
            items: list
                .items
                .iter()
                .map(|item| {
                    // For tight items `item.content` is empty so this reduces
                    // to `to_source(&item.inlines)` — identical to the old
                    // behaviour.  For loose items the child-block text is
                    // appended so the item's full prose is searchable.
                    if item.content.is_empty() {
                        to_source(&item.inlines)
                    } else {
                        let mut parts = vec![to_source(&item.inlines)];
                        for child in &item.content {
                            collect_block_text(child, &mut parts);
                        }
                        parts.retain(|s| !s.is_empty());
                        parts.join(" ")
                    }
                })
                .collect(),
            source_span: source_span(&list.span),
        }),
        BlockAst::CodeBlock(code_block) => GraphNode::CodeBlock(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: None,
            language: code_block.language.clone(),
            code: Some(code_block.code.clone()),
            items: Vec::new(),
            source_span: source_span(&code_block.span),
        }),
        BlockAst::KnowledgeObject(knowledge_object) => {
            GraphNode::KnowledgeObject(knowledge_object_to_graph_node(knowledge_object, page_id))
        }
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before graph emission")
        }
        // V4 Compatibility Mode: block-level raw HTML quarantined from Markdown
        // sources is exposed as a prose block whose text is the original source
        // text per ADR-0023. No new graph kind is introduced.
        BlockAst::QuarantinedHtml(quarantined_html) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(quarantined_html.source_text.clone()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&quarantined_html.span),
        }),
        // V4.2: GFM Table, FootnoteDefinition, and UnknownExtension also
        // project to a single prose block whose text is the original source
        // text. The graph schema (`adoc.graph.v3`) is unchanged.
        BlockAst::Table(table) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(table.source_text.clone()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&table.span),
        }),
        BlockAst::FootnoteDefinition(footnote) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(footnote.source_text.clone()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&footnote.span),
        }),
        BlockAst::UnknownExtension(unknown) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(unknown.source_text.clone()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&unknown.span),
        }),
        // V4 Compatibility Mode: a thematic break carries its source text
        // (`---`, `***`, etc.) as a structural cue in the graph. No new
        // graph node kind is introduced; project as a prose block like the
        // other compat-only variants.
        BlockAst::ThematicBreak(thematic_break) => GraphNode::Paragraph(GraphBlockNode {
            id: id.to_string(),
            page_id: page_id.to_string(),
            order,
            level: None,
            text: Some(thematic_break.source_text.clone()),
            language: None,
            code: None,
            items: Vec::new(),
            source_span: source_span(&thematic_break.span),
        }),
    }
}

fn knowledge_object_to_graph_node(
    knowledge_object: &KnowledgeObject,
    page_id: &str,
) -> GraphKnowledgeObjectNode {
    let mut node = knowledge_object_to_graph_node_without_hash(knowledge_object, page_id);
    node.content_hash = graph_knowledge_object_content_hash(&node);
    node
}

fn knowledge_object_to_graph_node_without_hash(
    knowledge_object: &KnowledgeObject,
    page_id: &str,
) -> GraphKnowledgeObjectNode {
    let span = knowledge_object.span();
    let metadata = knowledge_object.metadata_projection();
    let status = metadata
        .discriminant()
        .map(|discriminant| discriminant.value_as_str().to_string());

    let approved_by = policy_approved_by(knowledge_object);
    let (allowed_actions, forbidden_actions) = agent_instruction_actions(knowledge_object);
    let contradiction_claims = contradiction_claims(knowledge_object);

    GraphKnowledgeObjectNode {
        id: knowledge_object.id().as_str().to_string(),
        kind: knowledge_object.kind().as_str().to_string(),
        content_hash: String::new(),
        status,
        body: knowledge_object.body().to_source(),
        page_id: page_id.to_string(),
        source_span: source_span(span),
        fields: metadata_fields_to_graph(metadata.fields()),
        relations: relations_to_graph(knowledge_object.relations()),
        impacts: impacts_to_graph(knowledge_object.impacts()),
        approved_by,
        allowed_actions,
        forbidden_actions,
        contradiction_claims,
        // V5.8: typed evidence array replaces flat source/test/reviewed_by fields.
        evidence: metadata.graph_evidence(),
    }
}

fn policy_approved_by(knowledge_object: &KnowledgeObject) -> Vec<String> {
    let KnowledgeObject::Policy(policy) = knowledge_object else {
        return Vec::new();
    };
    policy_to_approved_by_vec(policy)
}

/// Extract the `contradiction_claims` list from a `contradiction` node,
/// returning an empty vec for all other kinds. Mirrors `policy_approved_by`.
fn contradiction_claims(knowledge_object: &KnowledgeObject) -> Vec<String> {
    let KnowledgeObject::Contradiction(contradiction) = knowledge_object else {
        return Vec::new();
    };
    contradiction_to_claims_vec(contradiction)
}

fn contradiction_to_claims_vec(contradiction: &Contradiction) -> Vec<String> {
    contradiction
        .claims()
        .as_slice()
        .iter()
        .map(|id| id.as_str().to_string())
        .collect()
}

fn policy_to_approved_by_vec(policy: &Policy) -> Vec<String> {
    policy
        .approved_by()
        .as_slice()
        .iter()
        .map(|a| a.as_str().to_string())
        .collect()
}

/// Extract `(allowed_actions, forbidden_actions)` from an `agent_instruction`
/// node, returning two empty vecs for all other kinds. Mirrors the pattern of
/// `policy_approved_by`.
fn agent_instruction_actions(knowledge_object: &KnowledgeObject) -> (Vec<String>, Vec<String>) {
    let KnowledgeObject::AgentInstruction(ai) = knowledge_object else {
        return (Vec::new(), Vec::new());
    };
    let allowed = ai
        .action_set()
        .allowed()
        .iter()
        .map(|a| a.as_str().to_string())
        .collect();
    let forbidden = ai
        .action_set()
        .forbidden()
        .iter()
        .map(|a| a.as_str().to_string())
        .collect();
    (allowed, forbidden)
}

fn impacts_to_graph(impacts: &[crate::domain::value_objects::rel_path::RelPath]) -> Vec<String> {
    impacts
        .iter()
        .map(|path| path.as_str().to_string())
        .collect()
}

#[derive(Serialize)]
struct KnowledgeObjectHashPayload<'a> {
    id: &'a str,
    kind: &'a str,
    status: &'a Option<String>,
    body: &'a str,
    page_id: &'a str,
    source_span: &'a GraphSourceSpan,
    fields: &'a BTreeMap<String, String>,
    relations: &'a GraphRelations,
    /// V3.3: omitted from canonical JSON when empty so claims without
    /// `impacts:` keep their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    impacts: &'a Vec<String>,
    /// V5.4: omitted from canonical JSON when empty so non-policy nodes keep
    /// their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    approved_by: &'a Vec<String>,
    /// V5.5: omitted from canonical JSON when empty so non-agent_instruction
    /// nodes keep their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    allowed_actions: &'a Vec<String>,
    /// V5.5: omitted from canonical JSON when empty so non-agent_instruction
    /// nodes keep their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    forbidden_actions: &'a Vec<String>,
    /// V5.6: omitted from canonical JSON when empty so non-contradiction nodes
    /// keep their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    contradiction_claims: &'a Vec<String>,
    /// V5.8: omitted from canonical JSON when empty so non-verified nodes keep
    /// their existing `content_hash`.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    evidence: &'a Vec<GraphEvidence>,
}

pub(crate) fn graph_knowledge_object_content_hash(node: &GraphKnowledgeObjectNode) -> String {
    let payload = KnowledgeObjectHashPayload {
        id: &node.id,
        kind: &node.kind,
        status: &node.status,
        body: &node.body,
        page_id: &node.page_id,
        source_span: &node.source_span,
        fields: &node.fields,
        relations: &node.relations,
        impacts: &node.impacts,
        approved_by: &node.approved_by,
        allowed_actions: &node.allowed_actions,
        forbidden_actions: &node.forbidden_actions,
        contradiction_claims: &node.contradiction_claims,
        evidence: &node.evidence,
    };
    let canonical_json =
        serde_json::to_vec(&payload).expect("knowledge object hash payload serializes");
    sha256_prefixed(&canonical_json)
}

fn metadata_fields_to_graph(metadata_fields: &[MetadataField<'_>]) -> BTreeMap<String, String> {
    let mut fields = BTreeMap::new();
    for field in metadata_fields {
        let previous = fields.insert(field.key().to_string(), field.value_as_str().to_string());
        debug_assert!(
            previous.is_none(),
            "duplicate metadata field: {}",
            field.key()
        );
    }
    fields
}

fn relations_to_graph(relations: &Relations) -> GraphRelations {
    GraphRelations {
        depends_on: relation_ids(relations.targets(GraphRelationKind::DependsOn)),
        supersedes: relation_ids(relations.targets(GraphRelationKind::Supersedes)),
        related_to: relation_ids(relations.targets(GraphRelationKind::RelatedTo)),
    }
}

fn relation_ids(targets: &[RelationTarget]) -> Vec<String> {
    targets
        .iter()
        .map(|target| target.id().as_str().to_string())
        .collect()
}

fn push_relation_edges(edges: &mut Vec<GraphEdge>, knowledge_object: &KnowledgeObject) {
    for relation in GraphRelationKind::ALL {
        for target in knowledge_object.relations().targets(relation) {
            edges.push(GraphEdge {
                kind: GraphEdgeKind::Relation,
                source: knowledge_object.id().as_str().to_string(),
                target: target.id().as_str().to_string(),
                relation: Some(relation),
                order: None,
            });
        }
    }
}

/// Recursively collect plain-text representation of a block into `parts`.
/// Used by the list-item graph projection to surface child-block prose text
/// (loose-list continuation paragraphs, nested sub-list item text) without
/// adding new graph node types.
fn collect_block_text(block: &BlockAst, parts: &mut Vec<String>) {
    match block {
        BlockAst::Paragraph(p) => parts.push(to_source(&p.inlines)),
        BlockAst::Heading(h) => parts.push(to_source(&h.inlines)),
        BlockAst::List(list) => {
            for item in &list.items {
                parts.push(to_source(&item.inlines));
                for child in &item.content {
                    collect_block_text(child, parts);
                }
            }
        }
        BlockAst::CodeBlock(c) => parts.push(c.code.clone()),
        BlockAst::QuarantinedHtml(h) => parts.push(h.source_text.clone()),
        BlockAst::Table(t) => parts.push(t.source_text.clone()),
        BlockAst::FootnoteDefinition(f) => {
            for child in &f.content {
                collect_block_text(child, parts);
            }
        }
        BlockAst::UnknownExtension(u) => parts.push(u.source_text.clone()),
        BlockAst::ThematicBreak(t) => parts.push(t.source_text.clone()),
        BlockAst::KnowledgeObject(_) | BlockAst::KnowledgeObjectPending(_) => {}
    }
}

fn push_reference_edges(edges: &mut Vec<GraphEdge>, block: &BlockAst, source: &str) {
    match block {
        BlockAst::Heading(heading) => push_inline_reference_edges(edges, source, &heading.inlines),
        BlockAst::Paragraph(paragraph) => {
            push_inline_reference_edges(edges, source, &paragraph.inlines);
        }
        BlockAst::List(list) => {
            for item in &list.items {
                push_inline_reference_edges(edges, source, &item.inlines);
                for child in &item.content {
                    push_reference_edges(edges, child, source);
                }
            }
        }
        BlockAst::Table(table) => {
            for cell in &table.header {
                push_inline_reference_edges(edges, source, &cell.inlines);
            }
            for row in &table.rows {
                for cell in row {
                    push_inline_reference_edges(edges, source, &cell.inlines);
                }
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                push_reference_edges(edges, child, source);
            }
        }
        BlockAst::KnowledgeObject(knowledge_object) => {
            push_inline_reference_edges(edges, source, knowledge_object.body().inlines());
        }
        BlockAst::CodeBlock(_) => {}
        BlockAst::QuarantinedHtml(_)
        | BlockAst::UnknownExtension(_)
        | BlockAst::ThematicBreak(_) => {}
        BlockAst::KnowledgeObjectPending(_) => {
            unreachable!("resolver must replace pending knowledge objects before graph emission")
        }
    }
}

fn push_inline_reference_edges(
    edges: &mut Vec<GraphEdge>,
    source: &str,
    inlines: &[InlineSegment],
) {
    for segment in inlines {
        match segment {
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => {
                push_inline_reference_edges(edges, source, inner);
            }
            InlineSegment::Link { text, .. } => push_inline_reference_edges(edges, source, text),
            InlineSegment::ObjectReference { id, .. } => {
                edges.push(GraphEdge {
                    kind: GraphEdgeKind::Reference,
                    source: source.to_string(),
                    target: id.as_str().to_string(),
                    relation: None,
                    order: None,
                });
            }
            InlineSegment::Image { alt, .. } => {
                push_inline_reference_edges(edges, source, alt);
            }
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::QuarantinedHtml { .. }
            | InlineSegment::FootnoteReference { .. }
            | InlineSegment::UnknownExtension { .. } => {}
        }
    }
}

fn source_span(span: &SourceSpan) -> GraphSourceSpan {
    GraphSourceSpan {
        path: span.file.display().to_string(),
        line: span.start.line,
        column: span.start.column,
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

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{BlockAst, ListAst, ListItem, ListKind, PageAst, ParagraphAst};
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::identity::PageId;
    use crate::domain::inline::InlineSegment;
    use crate::domain::ports::artifact_writer::ArtifactWriter;

    fn dummy_span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("guide.md"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
        }
    }

    /// A loose list item whose `content` holds a continuation paragraph must:
    /// 1. Project its text (inline + child para text) into the list node's
    ///    `items` array — not as a separate top-level node.
    /// 2. NOT produce a separate top-level Paragraph graph node for the child
    ///    paragraph content.
    #[test]
    fn loose_list_child_paragraph_appears_under_list_not_at_page_level() {
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![ListItem {
                    inlines: vec![InlineSegment::Text("first line".to_string())],
                    span: dummy_span(),
                    task_state: None,
                    content: vec![BlockAst::Paragraph(ParagraphAst {
                        inlines: vec![InlineSegment::Text("continuation text".to_string())],
                        span: dummy_span(),
                    })],
                }],
                span: dummy_span(),
            })],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        // There must be exactly two nodes: the page node and the list node.
        // No extra Paragraph node for the continuation.
        let page_count = artifact
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::Page(_)))
            .count();
        let list_count = artifact
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::List(_)))
            .count();
        let para_count = artifact
            .nodes
            .iter()
            .filter(|n| matches!(n, GraphNode::Paragraph(_)))
            .count();
        assert_eq!(
            artifact.nodes.len(),
            2,
            "expected exactly two nodes (page + list); got {:?}",
            artifact.nodes.len()
        );
        assert_eq!(page_count, 1, "expected one page node");
        assert_eq!(list_count, 1, "expected one list node");
        assert_eq!(
            para_count, 0,
            "continuation paragraph must NOT produce a separate Paragraph node"
        );

        // The list node's items must include the continuation text.
        let list_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::List(block) => Some(block),
                _ => None,
            })
            .expect("list graph node must exist");
        assert_eq!(list_node.items.len(), 1);
        assert!(
            list_node.items[0].contains("continuation text"),
            "list item projection must include child paragraph text; got {:?}",
            list_node.items[0]
        );
    }

    /// Tight list projection must be unchanged from before (only inline text
    /// in items, no concatenation artefacts).
    #[test]
    fn tight_list_graph_projection_is_unchanged() {
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![
                    ListItem {
                        inlines: vec![InlineSegment::Text("one".to_string())],
                        span: dummy_span(),
                        task_state: None,
                        content: Vec::new(),
                    },
                    ListItem {
                        inlines: vec![InlineSegment::Text("two".to_string())],
                        span: dummy_span(),
                        task_state: None,
                        content: Vec::new(),
                    },
                ],
                span: dummy_span(),
            })],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let list_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::List(block) => Some(block),
                _ => None,
            })
            .expect("list graph node must exist");

        assert_eq!(list_node.items, vec!["one".to_string(), "two".to_string()]);
    }

    /// A `source` Knowledge Object graph node must carry `kind: "source"` and
    /// project the evidence kind + path through the `fields` map.
    #[test]
    fn source_knowledge_object_graph_node_has_kind_and_fields() {
        use crate::domain::diagnostic::{SourcePosition, SourceSpan};
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::source::Source;

        let span = SourceSpan {
            file: PathBuf::from("docs/source.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 40,
                offset: 39,
            },
        };

        let source = Source::try_new(
            "billing.consume-use-case",
            "source_code",
            Some("src/features/credits/consume.ts"),
            None,
            "Source implementation for credit consumption.",
            std::collections::BTreeMap::new(),
            span,
        )
        .expect("valid source");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.sources").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/sources.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(
                KnowledgeObject::Source(source),
            ))],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let ko_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) => Some(ko),
                _ => None,
            })
            .expect("source KnowledgeObject graph node must exist");

        assert_eq!(ko_node.kind, "source", "graph node kind must be 'source'");
        assert_eq!(
            ko_node.fields.get("kind").map(String::as_str),
            Some("source_code"),
            "evidence kind must appear in fields[\"kind\"]"
        );
        assert_eq!(
            ko_node.fields.get("path").map(String::as_str),
            Some("src/features/credits/consume.ts"),
            "repo-relative path must appear in fields[\"path\"]"
        );
        assert!(
            ko_node.status.is_none(),
            "source has no status discriminant"
        );
    }

    /// A `source` Knowledge Object graph node with a URL target must carry
    /// the URL in `fields["url"]`.
    #[test]
    fn source_knowledge_object_graph_node_with_url_target() {
        use crate::domain::diagnostic::{SourcePosition, SourceSpan};
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::source::Source;

        let span = SourceSpan {
            file: PathBuf::from("docs/source.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 40,
                offset: 39,
            },
        };

        let source = Source::try_new(
            "billing.pr-ref",
            "pull_request",
            None,
            Some("https://github.com/org/repo/pull/42"),
            "PR implementing credit consumption.",
            std::collections::BTreeMap::new(),
            span,
        )
        .expect("valid source with url");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.sources").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/sources.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(
                KnowledgeObject::Source(source),
            ))],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let ko_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) => Some(ko),
                _ => None,
            })
            .expect("source KnowledgeObject graph node must exist");

        assert_eq!(ko_node.kind, "source");
        assert_eq!(
            ko_node.fields.get("kind").map(String::as_str),
            Some("pull_request")
        );
        assert_eq!(
            ko_node.fields.get("url").map(String::as_str),
            Some("https://github.com/org/repo/pull/42")
        );
        assert!(
            !ko_node.fields.contains_key("path"),
            "url-target source must not have a path field"
        );
    }

    /// A loose list item whose `inlines` is empty (text lives in a child
    /// `Paragraph`) must project to the child text with NO leading space.
    ///
    /// Regression: before the fix `parts.join(" ")` produced `" child text"`
    /// because the empty `to_source(&item.inlines)` string was kept in `parts`.
    #[test]
    fn loose_list_item_projection_has_no_leading_space() {
        let page = PageAst {
            id: PageId::from_string("team.guide").expect("test id"),
            title: None,
            source_path: PathBuf::from("guide.md"),
            blocks: vec![BlockAst::List(ListAst {
                kind: ListKind::Unordered,
                items: vec![ListItem {
                    // Loose item: inlines is empty; text lives in child Paragraph.
                    inlines: vec![],
                    span: dummy_span(),
                    task_state: None,
                    content: vec![BlockAst::Paragraph(ParagraphAst {
                        inlines: vec![InlineSegment::Text("child text".to_string())],
                        span: dummy_span(),
                    })],
                }],
                span: dummy_span(),
            })],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let list_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::List(block) => Some(block),
                _ => None,
            })
            .expect("list graph node must exist");

        assert_eq!(list_node.items.len(), 1);
        let projected = &list_node.items[0];
        assert_eq!(
            projected, "child text",
            "loose item with empty inlines must project to child text without leading space; got {:?}",
            projected
        );
        assert!(
            !projected.starts_with(' '),
            "projected text must not start with a space; got {:?}",
            projected
        );
        assert!(
            !projected.ends_with(' '),
            "projected text must not end with a space; got {:?}",
            projected
        );
    }

    // ── V5.8 TB2: evidence_ref graph emission ────────────────────────────────

    /// Build helpers for evidence_ref graph tests.
    fn evidence_ref_span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("docs/claims.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 40,
                offset: 39,
            },
        }
    }

    /// A claim with an evidence_ref to a source object emits a `GraphEvidence`
    /// entry with `reference` set and the resolved source kind in `kind`, plus
    /// a derived `evidence` graph edge from the claim to the source.
    #[test]
    fn claim_with_evidence_ref_produces_graph_evidence_entry_and_edge() {
        use crate::domain::identity::ObjectId;
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;
        use crate::domain::knowledge_object::source::Source;

        let span = evidence_ref_span();
        let source = Source::try_new(
            "billing.consume-use-case",
            "source_code",
            Some("src/features/credits/consume.ts"),
            None,
            "Source implementation for credit consumption.",
            std::collections::BTreeMap::new(),
            span.clone(),
        )
        .expect("valid source");

        let claim = Claim::try_new_with_refs(
            "billing.credits",
            Some("plain"),
            "Credits apply after payment.",
            std::collections::BTreeMap::new(),
            vec![ObjectId::new("billing.consume-use-case").expect("valid id")],
            None,
            span,
        )
        .expect("valid claim with evidence_ref");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.claims").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/claims.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Source(source))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim))),
            ],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        // Find the claim node.
        let claim_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.credits" => Some(ko),
                _ => None,
            })
            .expect("claim graph node must exist");

        // It must have exactly one evidence entry with reference set.
        assert_eq!(
            claim_node.evidence.len(),
            1,
            "claim must have exactly one evidence entry"
        );
        let ev = &claim_node.evidence[0];
        assert_eq!(
            ev.reference.as_deref(),
            Some("billing.consume-use-case"),
            "evidence entry must have reference set to the source id"
        );
        assert_eq!(
            ev.kind, "source_code",
            "evidence entry kind must match the source object's kind field"
        );
        assert!(
            ev.value.is_none(),
            "object-ref evidence entry must have no value"
        );

        // Find the evidence edge.
        let evidence_edge = artifact.edges.iter().find(|edge| {
            edge.kind == GraphEdgeKind::Evidence
                && edge.source == "billing.credits"
                && edge.target == "billing.consume-use-case"
        });
        assert!(
            evidence_edge.is_some(),
            "a derived evidence edge claim→source must exist; edges: {:?}",
            artifact.edges
        );
    }

    /// When the target source is missing from the graph (already caught by
    /// workspace validation), the evidence entry is emitted with an empty kind
    /// rather than panicking.
    #[test]
    fn claim_with_unresolvable_evidence_ref_emits_entry_with_empty_kind() {
        use crate::domain::identity::ObjectId;
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;

        let span = evidence_ref_span();
        let claim = Claim::try_new_with_refs(
            "billing.credits",
            Some("plain"),
            "Credits apply after payment.",
            std::collections::BTreeMap::new(),
            vec![ObjectId::new("billing.missing-source").expect("valid id")],
            None,
            span,
        )
        .expect("valid claim with evidence_ref");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.claims").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/claims.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(
                claim,
            )))],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let claim_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.credits" => Some(ko),
                _ => None,
            })
            .expect("claim graph node must exist");

        // Evidence entry emitted with empty kind — not a panic.
        assert_eq!(claim_node.evidence.len(), 1);
        let ev = &claim_node.evidence[0];
        assert_eq!(
            ev.reference.as_deref(),
            Some("billing.missing-source"),
            "reference must be the unresolved id"
        );
        assert_eq!(ev.kind, "", "unresolved ref gets empty kind string");
    }
}

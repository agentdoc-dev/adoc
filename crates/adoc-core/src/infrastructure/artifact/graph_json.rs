use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use serde::Serialize;

use crate::application::hashing::sha256_prefixed;
use crate::domain::ast::{BlockAst, ListKind, PageAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::graph::{
    GraphArtifactDocument, GraphBlockNode, GraphEdge, GraphEdgeKind, GraphKnowledgeObjectNode,
    GraphNode, GraphPageNode, GraphRelationKind, GraphRelations, GraphSourceSpan,
};
use crate::domain::inline::{InlineSegment, to_source};
use crate::domain::knowledge_object::{
    KnowledgeObject, RelationTarget, Relations, projection::MetadataField,
};
use crate::domain::ports::{artifact_reader::ArtifactReader, artifact_writer::ArtifactWriter};

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphJsonArtifact;

pub(crate) const SUPPORTED_GRAPH_SCHEMA_VERSION: &str = "adoc.graph.v2";

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
                .map(|item| to_source(&item.inlines))
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
    }
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

fn push_reference_edges(edges: &mut Vec<GraphEdge>, block: &BlockAst, source: &str) {
    match block {
        BlockAst::Heading(heading) => push_inline_reference_edges(edges, source, &heading.inlines),
        BlockAst::Paragraph(paragraph) => {
            push_inline_reference_edges(edges, source, &paragraph.inlines);
        }
        BlockAst::List(list) => {
            for item in &list.items {
                push_inline_reference_edges(edges, source, &item.inlines);
            }
        }
        BlockAst::KnowledgeObject(knowledge_object) => {
            push_inline_reference_edges(edges, source, knowledge_object.body().inlines());
        }
        BlockAst::CodeBlock(_) => {}
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
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
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
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReferencePending { .. } => {}
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

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use chrono::NaiveDate;
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
use crate::domain::ports::{
    artifact_reader::{ArtifactReadError, ArtifactReader},
    artifact_writer::ArtifactWriter,
};
use crate::domain::value_objects::evidence_kind::EvidenceKind;
use crate::infrastructure::artifact::artifact_schema_version;

#[derive(Debug, Default, Clone, Copy)]
pub(crate) struct GraphJsonArtifact;

pub(crate) const SUPPORTED_GRAPH_SCHEMA_VERSION: &str = "adoc.graph.v4";

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
        // Default: no date-pinning; effective_status derivation is skipped.
        self.build_for_date(workspace, diagnostics, None)
    }
}

impl GraphJsonArtifact {
    /// Build a graph artifact with an explicit `today` date so that
    /// `effective_status` derivation can be pinned in tests.
    pub(crate) fn build_for_date(
        &self,
        workspace: &WorkspaceAst,
        diagnostics: &[Diagnostic],
        today: Option<NaiveDate>,
    ) -> GraphArtifactDocument {
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
                nodes.push(block_to_graph_node(block, &node_id, &page_id, order, today));
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
                    push_question_resolved_by_edge(&mut edges, knowledge_object);
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
        // every claim or decision that carries evidence_refs.
        let object_evidence_refs: Vec<(String, Vec<String>)> = workspace
            .pages
            .iter()
            .flat_map(|page| page.blocks.iter())
            .filter_map(|block| {
                let BlockAst::KnowledgeObject(ko) = block else {
                    return None;
                };
                let refs_slice = match ko.as_ref() {
                    KnowledgeObject::Claim(claim) => claim.evidence_refs(),
                    KnowledgeObject::Decision(decision) => decision.evidence_refs(),
                    KnowledgeObject::Api(api) => api.evidence_refs(),
                    KnowledgeObject::Observation(observation) => observation.evidence_refs(),
                    _ => return None,
                };
                if refs_slice.is_empty() {
                    return None;
                }
                // Each entry is Evidence::ObjectRef; target_id() is always Some.
                let refs: Vec<String> = refs_slice
                    .iter()
                    .filter_map(|ev| ev.target_id())
                    .map(|id| id.as_str().to_string())
                    .collect();
                Some((ko.id().as_str().to_string(), refs))
            })
            .collect();

        for (object_id, ref_ids) in &object_evidence_refs {
            // Append evidence edges for this object.
            for ref_id in ref_ids {
                edges.push(GraphEdge {
                    kind: GraphEdgeKind::Evidence,
                    source: object_id.clone(),
                    target: ref_id.clone(),
                    relation: None,
                    order: None,
                });
            }
            // Append GraphEvidence entries to the object's graph node.
            for node in nodes.iter_mut() {
                let GraphNode::KnowledgeObject(ko) = node else {
                    continue;
                };
                if ko.id != *object_id {
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
                // Recompute the derived evidence_quality projection.
                ko.evidence_quality = best_evidence_quality(&ko.evidence);
                break;
            }
        }

        // Post-assembly cross-object pass: propagate contradiction status to
        // referenced claim nodes.  This must run AFTER the evidence-ref pass
        // because that pass may set `effective_status` to `"stale"` for verified
        // claims with past `expires_at`, and stale WINS over contradicted.
        apply_contradiction_effective_status(&mut nodes);

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

    fn read(&self, path: &Path) -> Result<Self::Output, ArtifactReadError> {
        read_graph_artifact_document(path).map_err(|diagnostics| {
            ArtifactReadError::from_diagnostics(diagnostics)
                .with_schema_version(artifact_schema_version(path))
        })
    }
}

fn block_node_id(page: &PageAst, block: &BlockAst, order: u32) -> String {
    match block {
        BlockAst::KnowledgeObject(knowledge_object) => knowledge_object.id().as_str().to_string(),
        _ => format!("{}#block-{order:04}", page.id.as_str()),
    }
}

fn block_to_graph_node(
    block: &BlockAst,
    id: &str,
    page_id: &str,
    order: u32,
    today: Option<NaiveDate>,
) -> GraphNode {
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
        BlockAst::KnowledgeObject(knowledge_object) => GraphNode::KnowledgeObject(
            knowledge_object_to_graph_node(knowledge_object, page_id, today),
        ),
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
        // text. The graph schema is unchanged by these projections.
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
    today: Option<NaiveDate>,
) -> GraphKnowledgeObjectNode {
    let mut node = knowledge_object_to_graph_node_without_hash(knowledge_object, page_id, today);
    node.content_hash = graph_knowledge_object_content_hash(&node);
    node
}

fn knowledge_object_to_graph_node_without_hash(
    knowledge_object: &KnowledgeObject,
    page_id: &str,
    today: Option<NaiveDate>,
) -> GraphKnowledgeObjectNode {
    let span = knowledge_object.span();
    let metadata = knowledge_object.metadata_projection();
    let status = metadata
        .discriminant()
        .map(|discriminant| discriminant.value_as_str().to_string());

    let approved_by = policy_approved_by(knowledge_object);
    let (allowed_actions, forbidden_actions) = agent_instruction_actions(knowledge_object);
    let contradiction_claims = contradiction_claims(knowledge_object);

    // V5.10: derived effective_status — computed only when a pinned `today` is
    // provided. The `None` sentinel preserves the pre-V5.10 behaviour for the
    // `ArtifactWriter::build` default path (no date pinning needed at that
    // layer; the lifecycle rule already fires in the validation stage).
    let (effective_status, effective_reason) = today
        .and_then(|date| derive_effective_status(&status, knowledge_object, date))
        .map_or((None, None), |(s, r)| (Some(s), Some(r)));

    let evidence = metadata.graph_evidence();

    // V5.10 TB3: derive the best evidence quality tier across all evidence
    // entries whose `kind` string parses to a known EvidenceKind. ObjectRef
    // entries carry the kind of the referenced source object — we use whatever
    // kind string is present (same as inline entries). If no entry has a
    // parseable kind, the field is omitted.
    let evidence_quality = best_evidence_quality(&evidence);

    GraphKnowledgeObjectNode {
        id: knowledge_object.id().as_str().to_string(),
        kind: knowledge_object.kind().as_str().to_string(),
        content_hash: String::new(),
        status,
        // ADR-0039: sole authored carriers — part of content_hash.
        severity: metadata.severity().map(|s| s.as_str().to_string()),
        trust: metadata.trust().map(|t| t.as_str().to_string()),
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
        evidence,
        // V5.10: derived — NOT part of content_hash.
        effective_status,
        effective_reason,
        // V5.10 TB3: derived — NOT part of content_hash.
        evidence_quality,
    }
}

/// Derive effective lifecycle status for a graph node.
///
/// Returns `Some(("stale", "expired:<YYYY-MM-DD>"))` when:
/// - the authored status is exactly `"verified"`, AND
/// - the `expires_at` field parses as `%Y-%m-%d` and is strictly `< today`.
///
/// Returns `None` in all other cases:
/// - non-verified status (draft, plain, …),
/// - future or today expiry,
/// - missing or unparseable `expires_at`.
///
/// TB4 will add the `contradicted` case layered on top of this helper.
pub(crate) fn derive_effective_status(
    status: &Option<String>,
    knowledge_object: &KnowledgeObject,
    today: NaiveDate,
) -> Option<(String, String)> {
    let expires_at = knowledge_object
        .fields()
        .iter()
        .find_map(|(key, value)| (key == "expires_at").then_some(value.as_str()));

    derive_effective_status_from_fields(status.as_deref(), expires_at, today)
}

/// Field-string core of [`derive_effective_status`], shared with the V6.1
/// read-time re-derivation in `application/signals.rs`, which works over graph
/// artifact nodes (field maps) rather than domain Knowledge Objects.
pub(crate) fn derive_effective_status_from_fields(
    status: Option<&str>,
    expires_at: Option<&str>,
    today: NaiveDate,
) -> Option<(String, String)> {
    if status != Some("verified") {
        return None;
    }

    let expires_at_date = NaiveDate::parse_from_str(expires_at?, "%Y-%m-%d").ok()?;

    if expires_at_date < today {
        Some(("stale".to_string(), format!("expired:{expires_at_date}")))
    } else {
        None
    }
}

/// Reverse index over `contradiction_claims`: claim id → the sorted ids of
/// every **unresolved** contradiction node that references it.
///
/// Shared core of the build-time `effective_status: "contradicted"` post-pass
/// and the V6.2 `adoc contradictions` read-time evaluation in
/// `application/signals.rs`. Values are sorted ascending so element `[0]` is
/// the lexicographically smallest implicating contradiction id.
pub(crate) fn unresolved_contradiction_claim_index<'a>(
    objects: impl Iterator<Item = &'a crate::domain::graph::GraphKnowledgeObjectNode>,
) -> BTreeMap<String, Vec<String>> {
    let mut index: BTreeMap<String, Vec<String>> = BTreeMap::new();

    for ko in objects {
        if ko.kind != "contradiction" {
            continue;
        }
        if ko.status.as_deref() != Some("unresolved") {
            continue;
        }
        for claim_id in &ko.contradiction_claims {
            index
                .entry(claim_id.clone())
                .or_default()
                .push(ko.id.clone());
        }
    }

    for ids in index.values_mut() {
        ids.sort();
        ids.dedup();
    }

    index
}

/// Cross-object post-pass: propagate `effective_status: "contradicted"` to claim
/// nodes referenced by an **unresolved** contradiction node.
///
/// # Precedence
///
/// If a claim node already has `effective_status` set (e.g. `"stale"` from the
/// TB2 expiry pass), it is **not** overwritten. Stale is the stronger lifecycle
/// signal and always wins.
///
/// # Determinism with multiple contradictions
///
/// When more than one unresolved contradiction references the same claim, the
/// `effective_reason` is set to `"contradiction:<id>"` where `<id>` is the
/// **lexicographically smallest** contradiction id among those referencing the
/// claim. This ensures the output is byte-stable regardless of iteration order.
pub(crate) fn apply_contradiction_effective_status(nodes: &mut [crate::domain::graph::GraphNode]) {
    use crate::domain::graph::GraphNode;

    let contradicted = unresolved_contradiction_claim_index(nodes.iter().filter_map(|node| {
        let GraphNode::KnowledgeObject(ko) = node else {
            return None;
        };
        Some(ko)
    }));

    if contradicted.is_empty() {
        return;
    }

    // Apply to each claim node that has no existing effective_status.
    for node in nodes.iter_mut() {
        let GraphNode::KnowledgeObject(ko) = node else {
            continue;
        };
        if ko.kind != "claim" {
            continue;
        }
        if ko.effective_status.is_some() {
            // Stale (or any pre-set status) wins — do not overwrite.
            continue;
        }
        if let Some(ids) = contradicted.get(&ko.id) {
            ko.effective_status = Some("contradicted".to_string());
            ko.effective_reason = Some(format!("contradiction:{}", ids[0]));
        }
    }
}

/// Compute the best evidence quality tier across a list of `GraphEvidence`
/// entries.
///
/// Each entry's `kind` string is parsed with [`EvidenceKind::try_new`]. Entries
/// whose `kind` is empty or unknown are skipped. The highest-tier parseable
/// entry determines the result.
///
/// Returns `Some(tier_str)` when at least one tier-able entry exists; `None`
/// when the evidence list is empty or all kinds are unrecognised.
fn best_evidence_quality(evidence: &[GraphEvidence]) -> Option<String> {
    let best = evidence
        .iter()
        .filter_map(|ev| EvidenceKind::try_new(&ev.kind).ok())
        .map(EvidenceKind::quality_tier)
        .max()?;
    Some(best.as_str().to_string())
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
    /// ADR-0039: authored carriers, hashed. Omitted when absent so kinds that
    /// carry neither keep their v3 `content_hash` byte-for-byte.
    #[serde(skip_serializing_if = "Option::is_none")]
    severity: &'a Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    trust: &'a Option<String>,
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
        severity: &node.severity,
        trust: &node.trust,
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

/// V6.5.3: an answered question's `resolved_by:` reference becomes a derived
/// edge so traversal can walk question → answering claim/decision (the
/// evidence-edge precedent).
fn push_question_resolved_by_edge(edges: &mut Vec<GraphEdge>, knowledge_object: &KnowledgeObject) {
    let KnowledgeObject::Question(question) = knowledge_object else {
        return;
    };
    let Some(resolved_by) = question.resolved_by() else {
        return;
    };
    edges.push(GraphEdge {
        kind: GraphEdgeKind::ResolvedBy,
        source: question.id().as_str().to_string(),
        target: resolved_by.as_str().to_string(),
        relation: None,
        order: None,
    });
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

    // ── V5.8 TB3: decision graph evidence emission ────────────────────────────

    fn decision_ref_span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("docs/decisions.adoc"),
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

    /// An accepted decision with an `evidence_ref` emits a `GraphEvidence` entry
    /// with `reference` set and the resolved source kind, plus a derived
    /// `evidence` graph edge.
    #[test]
    fn accepted_decision_with_evidence_ref_produces_graph_evidence_entry_and_edge() {
        use crate::domain::identity::ObjectId;
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::decision::{AcceptedVerdict, DecidedBy, Decision};
        use crate::domain::knowledge_object::source::Source;

        let span = decision_ref_span();
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

        let verdict = AcceptedVerdict::new(
            DecidedBy::try_new("architecture").expect("decided_by"),
            Vec::new(),
        );
        let decision = Decision::try_new_with_refs(
            "billing.policy",
            Some("accepted"),
            "Use the ledger-first approach.",
            std::collections::BTreeMap::new(),
            vec![ObjectId::new("billing.consume-use-case").expect("valid id")],
            Some(verdict),
            span,
        )
        .expect("valid accepted decision with evidence_ref");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.decisions").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/decisions.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Source(source))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Decision(decision))),
            ],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        // Find the decision node.
        let decision_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.policy" => Some(ko),
                _ => None,
            })
            .expect("decision graph node must exist");

        // Must have exactly one evidence entry with reference set.
        assert_eq!(
            decision_node.evidence.len(),
            1,
            "decision must have exactly one evidence entry"
        );
        let ev = &decision_node.evidence[0];
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

        // Find the evidence edge decision → source.
        let evidence_edge = artifact.edges.iter().find(|edge| {
            edge.kind == GraphEdgeKind::Evidence
                && edge.source == "billing.policy"
                && edge.target == "billing.consume-use-case"
        });
        assert!(
            evidence_edge.is_some(),
            "a derived evidence edge decision→source must exist; edges: {:?}",
            artifact.edges
        );
    }

    /// An accepted decision with inline evidence emits the evidence in the
    /// typed `evidence` array on the graph node.
    #[test]
    fn accepted_decision_with_inline_evidence_produces_graph_evidence_entries() {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::decision::{AcceptedVerdict, DecidedBy, Decision};
        use crate::domain::value_objects::evidence::Evidence;
        use crate::domain::value_objects::evidence_kind::EvidenceKind;

        let span = decision_ref_span();
        let verdict = AcceptedVerdict::new(
            DecidedBy::try_new("architecture").expect("decided_by"),
            vec![
                Evidence::inline(EvidenceKind::SourceCode, "design note v2").expect("source ev"),
                Evidence::inline(EvidenceKind::Test, "cargo test billing").expect("test ev"),
            ],
        );
        let decision = Decision::try_new(
            "billing.policy",
            Some("accepted"),
            "Use the ledger-first approach.",
            std::collections::BTreeMap::new(),
            Some(verdict),
            span,
        )
        .expect("valid accepted decision with inline evidence");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.decisions").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/decisions.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(
                KnowledgeObject::Decision(decision),
            ))],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let decision_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.policy" => Some(ko),
                _ => None,
            })
            .expect("decision graph node must exist");

        // Two inline evidence entries; no ObjectRef entries.
        assert_eq!(
            decision_node.evidence.len(),
            2,
            "decision must have two inline evidence entries; got: {:?}",
            decision_node.evidence
        );
        let ev0 = &decision_node.evidence[0];
        assert_eq!(ev0.kind, "source_code");
        assert_eq!(ev0.value.as_deref(), Some("design note v2"));
        assert!(
            ev0.reference.is_none(),
            "inline entry must have no reference"
        );

        let ev1 = &decision_node.evidence[1];
        assert_eq!(ev1.kind, "test");
        assert_eq!(ev1.value.as_deref(), Some("cargo test billing"));
    }

    // ── V5.10 TB2: effective_status hash-stability ────────────────────────────

    fn make_ko_node(
        effective_status: Option<String>,
        effective_reason: Option<String>,
    ) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            content_hash: String::new(),
            status: Some("verified".to_string()),
            severity: None,
            trust: None,
            body: "Credits are verified.".to_string(),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: std::collections::BTreeMap::new(),
            relations: GraphRelations::default(),
            impacts: Vec::new(),
            approved_by: Vec::new(),
            allowed_actions: Vec::new(),
            forbidden_actions: Vec::new(),
            contradiction_claims: Vec::new(),
            evidence: Vec::new(),
            effective_status,
            effective_reason,
            evidence_quality: None,
        }
    }

    /// `content_hash` must be identical whether or not `effective_status` is set,
    /// proving that the two V5.10 derived fields are excluded from the hash payload.
    #[test]
    fn content_hash_is_stable_regardless_of_effective_status() {
        let node_without = make_ko_node(None, None);
        let node_with = make_ko_node(
            Some("stale".to_string()),
            Some("expired:2026-01-01".to_string()),
        );

        let hash_without = graph_knowledge_object_content_hash(&node_without);
        let hash_with = graph_knowledge_object_content_hash(&node_with);

        assert_eq!(
            hash_without, hash_with,
            "content_hash must be identical whether or not effective_status is set; \
             effective_status is not part of the hash payload"
        );
    }

    /// ADR-0039: `severity`/`trust` are authored carriers and MUST enter the
    /// hash payload — a severity or trust edit changes `content_hash`.
    #[test]
    fn content_hash_covers_severity_and_trust() {
        let node_without = make_ko_node(None, None);
        let mut node_with_severity = make_ko_node(None, None);
        node_with_severity.severity = Some("critical".to_string());
        let mut node_with_trust = make_ko_node(None, None);
        node_with_trust.trust = Some("team".to_string());

        let hash_without = graph_knowledge_object_content_hash(&node_without);

        assert_ne!(
            hash_without,
            graph_knowledge_object_content_hash(&node_with_severity),
            "content_hash must change when severity changes; severity is an \
             authored, hashed carrier per ADR-0039"
        );
        assert_ne!(
            hash_without,
            graph_knowledge_object_content_hash(&node_with_trust),
            "content_hash must change when trust changes; trust is an \
             authored, hashed carrier per ADR-0039"
        );
    }

    /// ADR-0039: kinds that carry neither severity nor trust keep their v3
    /// hash payload byte-for-byte — the absent fields are skipped, not null.
    #[test]
    fn hash_payload_omits_absent_severity_and_trust() {
        let node = make_ko_node(None, None);
        let payload = KnowledgeObjectHashPayload {
            id: &node.id,
            kind: &node.kind,
            status: &node.status,
            severity: &node.severity,
            trust: &node.trust,
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
        let canonical = serde_json::to_string(&payload).expect("payload serializes");
        assert!(
            !canonical.contains("severity") && !canonical.contains("trust"),
            "absent severity/trust must be omitted from the hash payload: {canonical}"
        );
    }

    // ── V5.10 TB3: evidence_quality projection ────────────────────────────────

    /// Utility: build a workspace containing a single verified claim with the
    /// given inline evidence kinds, and return its graph node.
    fn graph_node_for_claim_with_evidence_kinds(
        evidence_entries: &[(&str, &str)],
    ) -> GraphKnowledgeObjectNode {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::{Claim, Owner, Verification, VerifiedAt};
        use crate::domain::value_objects::evidence::Evidence;
        use crate::domain::value_objects::evidence_kind::EvidenceKind;

        let span = dummy_span();
        let owner = Owner::try_new("team-billing").expect("owner");
        let verified_at = VerifiedAt::try_new("2026-05-05").expect("verified_at");
        let evidence_vec: Vec<Evidence> = evidence_entries
            .iter()
            .map(|(kind_str, val)| {
                let kind = EvidenceKind::try_new(kind_str).expect("valid kind");
                Evidence::inline(kind, val).expect("valid evidence")
            })
            .collect();
        let verification = Verification::new(owner, verified_at, evidence_vec);
        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "Credits apply.",
            std::collections::BTreeMap::new(),
            Some(verification),
            span,
        )
        .expect("valid verified claim");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.billing").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/billing.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(
                claim,
            )))],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        artifact
            .nodes
            .into_iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.credits" => Some(ko),
                _ => None,
            })
            .expect("claim graph node must exist")
    }

    /// A verified claim with a `test:` evidence entry must produce
    /// `evidence_quality: "high"`.
    #[test]
    fn claim_with_test_evidence_has_high_evidence_quality() {
        let node = graph_node_for_claim_with_evidence_kinds(&[("test", "cargo test billing")]);
        assert_eq!(
            node.evidence_quality.as_deref(),
            Some("high"),
            "test evidence kind must produce high evidence_quality"
        );
    }

    /// A verified claim with only `external_url:` evidence must produce
    /// `evidence_quality: "low"`.
    #[test]
    fn claim_with_external_url_evidence_has_low_evidence_quality() {
        let node =
            graph_node_for_claim_with_evidence_kinds(&[("external_url", "https://example.com")]);
        assert_eq!(
            node.evidence_quality.as_deref(),
            Some("low"),
            "external_url evidence kind must produce low evidence_quality"
        );
    }

    /// A node with no evidence must have `evidence_quality: None` (field absent
    /// from JSON output).
    #[test]
    fn no_evidence_object_has_absent_evidence_quality() {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;

        let span = dummy_span();
        let claim = Claim::try_new(
            "billing.plain",
            Some("plain"),
            "No evidence here.",
            std::collections::BTreeMap::new(),
            None,
            span,
        )
        .expect("valid plain claim");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.billing").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/billing.adoc"),
            blocks: vec![BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(
                claim,
            )))],
        };
        let workspace = crate::domain::ast::WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build(&workspace, &[]);

        let node = artifact
            .nodes
            .into_iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "billing.plain" => Some(ko),
                _ => None,
            })
            .expect("claim graph node must exist");

        assert!(
            node.evidence_quality.is_none(),
            "no evidence must produce absent evidence_quality (None); got {:?}",
            node.evidence_quality
        );
    }

    /// Mixed evidence (Low + High) must resolve to `"high"` — best tier wins.
    #[test]
    fn mixed_evidence_resolves_to_best_tier() {
        let node = graph_node_for_claim_with_evidence_kinds(&[
            ("external_url", "https://example.com"),
            ("test", "cargo test billing"),
        ]);
        assert_eq!(
            node.evidence_quality.as_deref(),
            Some("high"),
            "when mixed tiers are present, the highest tier must win"
        );
    }

    /// `evidence_quality` must NOT affect `content_hash` (derived field is
    /// excluded from the hash payload, mirroring ADR-0033 / ADR-0034).
    #[test]
    fn content_hash_is_stable_regardless_of_evidence_quality() {
        let node_without = GraphKnowledgeObjectNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            content_hash: String::new(),
            status: Some("verified".to_string()),
            severity: None,
            trust: None,
            body: "Credits are verified.".to_string(),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: std::collections::BTreeMap::new(),
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
        };
        let mut node_with = node_without.clone();
        node_with.evidence_quality = Some("high".to_string());

        let hash_without = graph_knowledge_object_content_hash(&node_without);
        let hash_with = graph_knowledge_object_content_hash(&node_with);

        assert_eq!(
            hash_without, hash_with,
            "content_hash must be identical regardless of evidence_quality; \
             evidence_quality is not part of the hash payload"
        );
    }

    fn make_verified_claim_with_expires_at(
        id: &str,
        expires_at: &str,
    ) -> crate::domain::knowledge_object::KnowledgeObject {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::{
            Claim, Evidence, Owner, Verification, VerifiedAt,
        };

        let span = dummy_span();
        let owner = Owner::try_new("team-billing").expect("owner");
        let verified_at = VerifiedAt::try_new("2025-01-01").expect("verified_at");
        let source_ev = Evidence::from_field("source", "ledger").expect("evidence");
        let verification = Verification::new(owner, verified_at, vec![source_ev]);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("expires_at".to_string(), expires_at.to_string());
        let claim = Claim::try_new(
            id,
            Some("verified"),
            "Body.",
            fields,
            Some(verification),
            span,
        )
        .expect("valid verified claim");
        KnowledgeObject::Claim(claim)
    }

    fn make_plain_claim_with_expires_at(
        id: &str,
        expires_at: &str,
    ) -> crate::domain::knowledge_object::KnowledgeObject {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;

        let span = dummy_span();
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("expires_at".to_string(), expires_at.to_string());
        let claim = Claim::try_new(id, Some("plain"), "Body.", fields, None, span)
            .expect("valid plain claim");
        KnowledgeObject::Claim(claim)
    }

    /// `derive_effective_status` returns `Some(("stale", "expired:<date>"))` for a
    /// verified claim with a past `expires_at`.
    #[test]
    fn derive_effective_status_returns_stale_for_verified_past_expiry() {
        let ko = make_verified_claim_with_expires_at("billing.credits", "2026-01-01");
        let status = Some("verified".to_string());
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status(&status, &ko, today);

        assert_eq!(
            result,
            Some(("stale".to_string(), "expired:2026-01-01".to_string())),
            "verified claim with past expires_at must return stale"
        );
    }

    /// `derive_effective_status` returns `None` for a draft (non-verified) claim
    /// even when `expires_at` is in the past.
    #[test]
    fn derive_effective_status_returns_none_for_non_verified_past_expiry() {
        let ko = make_plain_claim_with_expires_at("billing.draft", "2026-01-01");
        let status = Some("plain".to_string());
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status(&status, &ko, today);

        assert!(
            result.is_none(),
            "non-verified status must not produce effective_status"
        );
    }

    /// `derive_effective_status` returns `None` for a verified claim with a
    /// future `expires_at`.
    #[test]
    fn derive_effective_status_returns_none_for_verified_future_expiry() {
        let ko = make_verified_claim_with_expires_at("billing.future", "2027-01-01");
        let status = Some("verified".to_string());
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status(&status, &ko, today);

        assert!(
            result.is_none(),
            "future expires_at must not produce effective_status"
        );
    }

    /// `derive_effective_status` returns `None` when `expires_at` equals
    /// `today`: expiry on the boundary day must not derive stale (staleness
    /// uses strict `<`).
    #[test]
    fn derive_effective_status_returns_none_for_verified_expiry_on_boundary_day() {
        let ko = make_verified_claim_with_expires_at("billing.today", "2026-05-08");
        let status = Some("verified".to_string());
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status(&status, &ko, today);

        assert!(
            result.is_none(),
            "expires_at equal to today must not produce effective_status"
        );
    }

    // ── V6.1: field-based derivation core (read-time reuse) ───────────────────

    /// `derive_effective_status_from_fields` mirrors the verified-past-expiry
    /// stale derivation without needing a domain `KnowledgeObject`.
    #[test]
    fn derive_effective_status_from_fields_returns_stale_for_verified_past_expiry() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result =
            derive_effective_status_from_fields(Some("verified"), Some("2026-01-01"), today);

        assert_eq!(
            result,
            Some(("stale".to_string(), "expired:2026-01-01".to_string())),
            "verified status with past expires_at must return stale"
        );
    }

    /// Non-verified statuses never derive stale, even with a past expiry.
    #[test]
    fn derive_effective_status_from_fields_returns_none_for_non_verified() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status_from_fields(Some("draft"), Some("2026-01-01"), today);

        assert!(
            result.is_none(),
            "non-verified status must not produce effective_status"
        );
    }

    /// `expires_at` equal to `today` is not stale (strict `<`).
    #[test]
    fn derive_effective_status_from_fields_returns_none_on_boundary_day() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result =
            derive_effective_status_from_fields(Some("verified"), Some("2026-05-08"), today);

        assert!(
            result.is_none(),
            "expires_at equal to today must not produce effective_status"
        );
    }

    /// Unparseable `expires_at` values are silently ignored.
    #[test]
    fn derive_effective_status_from_fields_returns_none_for_unparseable_date() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result =
            derive_effective_status_from_fields(Some("verified"), Some("not-a-date"), today);

        assert!(
            result.is_none(),
            "unparseable expires_at must not produce effective_status"
        );
    }

    /// Missing `expires_at` produces nothing.
    #[test]
    fn derive_effective_status_from_fields_returns_none_for_missing_expires_at() {
        let today = NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid date");

        let result = derive_effective_status_from_fields(Some("verified"), None, today);

        assert!(
            result.is_none(),
            "missing expires_at must not produce effective_status"
        );
    }

    // ── V5.10 TB4: contradiction effective_status cross-object pass ───────────

    /// Builds a workspace with two plain claims + one contradiction referencing
    /// both, compiles it, and returns the claim graph nodes.
    fn build_workspace_with_contradiction(
        contradiction_status: &str,
    ) -> (
        crate::domain::graph::GraphArtifactDocument,
        String, // contradiction id
    ) {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;
        use crate::domain::knowledge_object::contradiction::Contradiction;

        let span = dummy_span();

        let claim_a = Claim::try_new(
            "auth.a",
            Some("plain"),
            "Claim A body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("valid claim a");

        let claim_b = Claim::try_new(
            "auth.b",
            Some("plain"),
            "Claim B body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("valid claim b");

        let contradiction_id = "auth.conflict";
        let contradiction = Contradiction::try_new(
            contradiction_id,
            "high",
            contradiction_status,
            vec!["auth.a", "auth.b"],
            "A and B conflict.",
            std::collections::BTreeMap::new(),
            span.clone(),
        )
        .expect("valid contradiction");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim_a))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim_b))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(contradiction))),
            ],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        // Use a pinned today — irrelevant for this test since no expires_at.
        let today = Some(NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date"));
        let artifact = GraphJsonArtifact.build_for_date(&workspace, &[], today);
        (artifact, contradiction_id.to_string())
    }

    /// An unresolved contradiction must cause both referenced claims to have
    /// `effective_status: "contradicted"` with `effective_reason:
    /// "contradiction:auth.conflict"`, and the authored `status` must remain
    /// `"plain"` (never mutated).
    #[test]
    fn unresolved_contradiction_propagates_effective_status_to_referenced_claims() {
        let (artifact, cid) = build_workspace_with_contradiction("unresolved");

        for claim_id in &["auth.a", "auth.b"] {
            let node = artifact
                .nodes
                .iter()
                .find_map(|n| match n {
                    GraphNode::KnowledgeObject(ko) if ko.id == *claim_id => Some(ko),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("claim node {claim_id} must exist"));

            assert_eq!(
                node.effective_status.as_deref(),
                Some("contradicted"),
                "unresolved contradiction must set effective_status=contradicted on {claim_id}; got: {:?}",
                node.effective_status
            );
            assert_eq!(
                node.effective_reason.as_deref(),
                Some(format!("contradiction:{cid}").as_str()),
                "effective_reason must be contradiction:<id> for {claim_id}; got: {:?}",
                node.effective_reason
            );
            // Authored status must NOT be mutated.
            assert_eq!(
                node.status.as_deref(),
                Some("plain"),
                "authored status must remain plain (never mutated) for {claim_id}"
            );
        }
    }

    /// A resolved contradiction must NOT propagate `effective_status` to claims.
    #[test]
    fn resolved_contradiction_does_not_propagate_effective_status() {
        let (artifact, _cid) = build_workspace_with_contradiction("resolved");

        for claim_id in &["auth.a", "auth.b"] {
            let node = artifact
                .nodes
                .iter()
                .find_map(|n| match n {
                    GraphNode::KnowledgeObject(ko) if ko.id == *claim_id => Some(ko),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("claim node {claim_id} must exist"));

            assert!(
                node.effective_status.is_none(),
                "resolved contradiction must not set effective_status on {claim_id}; got: {:?}",
                node.effective_status
            );
        }
    }

    /// A dismissed contradiction must NOT propagate `effective_status`.
    #[test]
    fn dismissed_contradiction_does_not_propagate_effective_status() {
        let (artifact, _cid) = build_workspace_with_contradiction("dismissed");

        for claim_id in &["auth.a", "auth.b"] {
            let node = artifact
                .nodes
                .iter()
                .find_map(|n| match n {
                    GraphNode::KnowledgeObject(ko) if ko.id == *claim_id => Some(ko),
                    _ => None,
                })
                .unwrap_or_else(|| panic!("claim node {claim_id} must exist"));

            assert!(
                node.effective_status.is_none(),
                "dismissed contradiction must not set effective_status on {claim_id}; got: {:?}",
                node.effective_status
            );
        }
    }

    /// When a claim is both verified+expired (stale via TB2) AND referenced by
    /// an unresolved contradiction, the `effective_status` must remain `"stale"`
    /// (stale wins; contradicted must not overwrite).
    #[test]
    fn stale_claim_wins_over_contradicted_effective_status() {
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::{
            Claim, Evidence, Owner, Verification, VerifiedAt,
        };
        use crate::domain::knowledge_object::contradiction::Contradiction;

        let span = dummy_span();

        // Verified claim with expires_at in the past.
        let owner = Owner::try_new("team-billing").expect("owner");
        let verified_at = VerifiedAt::try_new("2025-01-01").expect("verified_at");
        let source_ev = Evidence::from_field("source", "ledger").expect("evidence");
        let verification = Verification::new(owner, verified_at, vec![source_ev]);
        let mut fields = std::collections::BTreeMap::new();
        fields.insert("expires_at".to_string(), "2025-06-01".to_string());
        let stale_claim = Claim::try_new(
            "auth.stale-and-contradicted",
            Some("verified"),
            "Claim body.",
            fields,
            Some(verification),
            span.clone(),
        )
        .expect("valid verified claim with past expiry");

        let plain_claim = Claim::try_new(
            "auth.b",
            Some("plain"),
            "Plain body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("valid plain claim");

        let contradiction = Contradiction::try_new(
            "auth.conflict",
            "high",
            "unresolved",
            vec!["auth.stale-and-contradicted", "auth.b"],
            "These two conflict.",
            std::collections::BTreeMap::new(),
            span.clone(),
        )
        .expect("valid contradiction");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(stale_claim))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(plain_claim))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(contradiction))),
            ],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        // today is after expires_at — so the stale claim is expired.
        let today = Some(NaiveDate::from_ymd_opt(2026, 6, 1).expect("valid date"));
        let artifact = GraphJsonArtifact.build_for_date(&workspace, &[], today);

        let stale_node = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "auth.stale-and-contradicted" => {
                    Some(ko)
                }
                _ => None,
            })
            .expect("stale node must exist");

        assert_eq!(
            stale_node.effective_status.as_deref(),
            Some("stale"),
            "stale must win over contradicted; got: {:?}",
            stale_node.effective_status
        );
        assert!(
            stale_node
                .effective_reason
                .as_deref()
                .map(|r| r.starts_with("expired:"))
                .unwrap_or(false),
            "effective_reason must start with expired: for stale claim; got: {:?}",
            stale_node.effective_reason
        );
    }

    /// `content_hash` must not change when `effective_status` is set to
    /// `"contradicted"` — derived fields are excluded from the hash payload.
    #[test]
    fn content_hash_stable_for_contradicted_effective_status() {
        let node_without = make_ko_node(None, None);
        let node_with = make_ko_node(
            Some("contradicted".to_string()),
            Some("contradiction:auth.conflict".to_string()),
        );
        let hash_without = graph_knowledge_object_content_hash(&node_without);
        let hash_with = graph_knowledge_object_content_hash(&node_with);
        assert_eq!(
            hash_without, hash_with,
            "content_hash must be identical with/without contradicted effective_status"
        );
    }

    /// When multiple unresolved contradictions reference the same claim, the
    /// effective_reason must use the lexicographically smallest contradiction id.
    #[test]
    fn multiple_contradictions_tie_breaks_to_lex_smallest_id() {
        use crate::domain::graph::GraphNode;
        use crate::domain::knowledge_object::KnowledgeObject;
        use crate::domain::knowledge_object::claim::Claim;
        use crate::domain::knowledge_object::contradiction::Contradiction;

        let span = dummy_span();

        let claim_a = Claim::try_new(
            "auth.a",
            Some("plain"),
            "Body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("claim a");
        let claim_b = Claim::try_new(
            "auth.b",
            Some("plain"),
            "Body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("claim b");
        let claim_c = Claim::try_new(
            "auth.c",
            Some("plain"),
            "Body.",
            std::collections::BTreeMap::new(),
            None,
            span.clone(),
        )
        .expect("claim c");

        // Two unresolved contradictions both referencing auth.a.
        // "auth.conflict.aaa" < "auth.conflict.zzz" lexicographically.
        let contradiction_zzz = Contradiction::try_new(
            "auth.conflict.zzz",
            "high",
            "unresolved",
            vec!["auth.a", "auth.b"],
            "zzz conflict.",
            std::collections::BTreeMap::new(),
            span.clone(),
        )
        .expect("contradiction zzz");
        let contradiction_aaa = Contradiction::try_new(
            "auth.conflict.aaa",
            "high",
            "unresolved",
            vec!["auth.a", "auth.c"],
            "aaa conflict.",
            std::collections::BTreeMap::new(),
            span.clone(),
        )
        .expect("contradiction aaa");

        let page = PageAst {
            id: crate::domain::identity::PageId::from_string("docs.auth").expect("page id"),
            title: None,
            source_path: PathBuf::from("docs/auth.adoc"),
            blocks: vec![
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim_a))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim_b))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim_c))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(
                    contradiction_zzz,
                ))),
                BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Contradiction(
                    contradiction_aaa,
                ))),
            ],
        };
        let workspace = WorkspaceAst { pages: vec![page] };
        let artifact = GraphJsonArtifact.build_for_date(&workspace, &[], None);

        let node_a = artifact
            .nodes
            .iter()
            .find_map(|n| match n {
                GraphNode::KnowledgeObject(ko) if ko.id == "auth.a" => Some(ko),
                _ => None,
            })
            .expect("auth.a node");

        // auth.a is referenced by both; lex-smallest is "auth.conflict.aaa".
        assert_eq!(
            node_a.effective_reason.as_deref(),
            Some("contradiction:auth.conflict.aaa"),
            "tie-break must use lex-smallest contradiction id; got: {:?}",
            node_a.effective_reason
        );
    }
}

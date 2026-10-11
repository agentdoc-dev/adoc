//! Issue 286: render one object's authored graph-node members as its `.adoc`
//! block, reusing the source-edit block writer. No placement, no prose.

use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::Value;

use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{
    GraphArtifactDocument, GraphEvidence, GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan,
};
use crate::domain::identity::ObjectId;
use crate::domain::source::SourceFile;
use crate::domain::source_edit::planner::{CreateInsertion, plan_create_object};
use crate::domain::source_edit::{LineEnding, SourceEditPlan, SpanEdit};
use crate::domain::value_objects::evidence::Evidence;
use crate::infrastructure::parser::layout::typed_block_layouts;
use crate::infrastructure::source::in_memory::InMemorySourceProvider;

use super::compile::compile_tolerating_for_date;
use super::local_today;

pub const RENDER_OBJECT_SCHEMA_VERSION: &str = "adoc.render_object.v0";
pub const MAX_RENDER_OBJECT_BYTES: usize = 1024 * 1024;

/// Every authored member of the graph node (`graph_json.rs`). Placement
/// (`page_id`, `source_span`, `source_binding`) and derived members
/// (`content_hash`, `effective_*`, `evidence_quality`) are not authored.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RenderObjectInput {
    #[allow(dead_code)]
    schema_version: String,
    id: String,
    kind: String,
    #[serde(default)]
    status: Option<String>,
    #[serde(default)]
    severity: Option<String>,
    #[serde(default)]
    trust: Option<String>,
    body: String,
    #[serde(default)]
    visibility: Option<String>,
    #[serde(default)]
    field_visibility: Option<BTreeMap<String, String>>,
    #[serde(default)]
    fields: BTreeMap<String, String>,
    #[serde(default)]
    relations: RelationsInput,
    #[serde(default)]
    impacts: Vec<String>,
    #[serde(default)]
    approved_by: Vec<String>,
    #[serde(default)]
    allowed_actions: Vec<String>,
    #[serde(default)]
    forbidden_actions: Vec<String>,
    #[serde(default)]
    contradiction_claims: Vec<String>,
    #[serde(default)]
    evidence: Vec<EvidenceInput>,
}

#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct RelationsInput {
    #[serde(default)]
    depends_on: Vec<String>,
    #[serde(default)]
    supersedes: Vec<String>,
    #[serde(default)]
    related_to: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EvidenceInput {
    kind: String,
    #[serde(default)]
    value: Option<String>,
    #[serde(default)]
    reference: Option<String>,
}

pub(super) fn insert(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    value: String,
) -> Result<(), DiagnosticCode> {
    if fields.get(key).is_some_and(|existing| existing != &value) {
        return Err(DiagnosticCode::PortableProjectionUnavailable);
    }
    fields.insert(key.into(), value);
    Ok(())
}

pub(super) fn list(
    fields: &mut BTreeMap<String, String>,
    key: &str,
    values: &[String],
) -> Result<(), DiagnosticCode> {
    if !values.is_empty() {
        if values
            .iter()
            .any(|value| value.contains([',', '[', ']', '\n', '\r']))
        {
            return Err(DiagnosticCode::PortableProjectionUnavailable);
        }
        insert(fields, key, format!("[{}]", values.join(", ")))?;
    }
    Ok(())
}

/// Map a node's authored carriers onto block field lines (the lifecycle `status`
/// goes to the block writer, which overrides a `status` field). `relations` is passed separately so
/// portable projection can hand in its filtered set; `evidence_ref_ok` gates
/// object-reference evidence.
pub(super) fn node_block_fields(
    node: &GraphKnowledgeObjectNode,
    relations: &GraphRelations,
    evidence_ref_ok: &dyn Fn(&str) -> bool,
) -> Result<BTreeMap<String, String>, DiagnosticCode> {
    let unavailable = DiagnosticCode::PortableProjectionUnavailable;
    // A kind without a lifecycle keeps `status` as an ordinary field.
    let mut fields = node.fields.clone();
    for (name, carrier) in [
        ("severity", &node.severity),
        ("trust", &node.trust),
        ("visibility", &node.visibility),
    ] {
        if let Some(value) = carrier {
            insert(&mut fields, name, value.clone())?;
        }
    }
    if let Some(visibility) = &node.field_visibility {
        insert(
            &mut fields,
            "field_visibility",
            visibility
                .iter()
                .map(|(key, value)| format!("{key}={value}"))
                .collect::<Vec<_>>()
                .join(", "),
        )?;
    }
    for (name, values) in [
        ("depends_on", &relations.depends_on),
        ("supersedes", &relations.supersedes),
        ("related_to", &relations.related_to),
    ] {
        fields.remove(name);
        list(&mut fields, name, values)?;
    }
    for (name, values) in [
        ("impacts", &node.impacts),
        ("approved_by", &node.approved_by),
        ("allowed_actions", &node.allowed_actions),
        ("forbidden_actions", &node.forbidden_actions),
        ("claims", &node.contradiction_claims),
    ] {
        list(&mut fields, name, values)?;
    }
    let mut evidence_refs = Vec::new();
    for evidence in &node.evidence {
        match (&evidence.value, &evidence.reference) {
            (None, Some(reference)) if evidence_ref_ok(reference) => {
                evidence_refs.push(reference.clone())
            }
            (Some(value), None) => {
                let field = ["source", "test", "reviewed_by", "external_url"]
                    .into_iter()
                    .find(|field| {
                        Evidence::from_field(field, value)
                            .and_then(|value| value.kind())
                            .is_some_and(|kind| kind.as_str() == evidence.kind)
                    });
                if let Some(field) = field {
                    insert(&mut fields, field, value.clone())?;
                } else {
                    return Err(unavailable);
                }
            }
            _ => return Err(unavailable),
        }
    }
    list(&mut fields, "evidence_ref", &evidence_refs)?;
    Ok(fields)
}

/// Cross-object resolution is the project check's job, not the renderer's.
fn is_cross_object(code: DiagnosticCode, has_evidence_refs: bool) -> bool {
    (has_evidence_refs && code == DiagnosticCode::ApiVerifiedMissingSchemaEvidence)
        || matches!(
            code,
            DiagnosticCode::RefBroken
                | DiagnosticCode::SchemaEvidenceTargetNotFound
                | DiagnosticCode::SchemaEvidenceTargetNotASource
                | DiagnosticCode::SchemaContradictionClaimNotFound
                | DiagnosticCode::SchemaQuestionResolvedByNotFound
        )
}

fn refusal(code: DiagnosticCode, message: impl Into<String>) -> Vec<Diagnostic> {
    let diagnostic = Diagnostic::error(code, message);
    // The default remedies of the input-shape codes point at graph artifacts.
    vec![if code == DiagnosticCode::RenderMemberNotRepresentable {
        diagnostic
    } else {
        diagnostic.with_help(INPUT_HELP)
    }]
}

const INPUT_HELP: &str =
    "Send one `adoc.render_object.v0` JSON document that uses only the documented members.";

fn not_representable(message: impl Into<String>) -> Vec<Diagnostic> {
    refusal(DiagnosticCode::RenderMemberNotRepresentable, message)
}

/// Scalars the block writer interpolates raw into block syntax: refuse
/// anything that could end the line or open a second block.
fn check_scalars(node: &GraphKnowledgeObjectNode) -> Result<(), Vec<Diagnostic>> {
    if ObjectId::new(node.id.as_str()).is_err() {
        return Err(not_representable(format!(
            "`id` {:?} is not a valid Object ID",
            node.id
        )));
    }
    let mut chars = node.kind.chars();
    let kind_ok = chars.next().is_some_and(|c| c.is_ascii_lowercase())
        && chars.all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '_');
    if !kind_ok {
        return Err(not_representable(format!(
            "`kind` {:?} must match [a-z][a-z0-9_]*",
            node.kind
        )));
    }
    if let Some(status) = &node.status
        && (status.is_empty() || status.chars().any(|c| c.is_whitespace() || c.is_control()))
    {
        return Err(not_representable(format!(
            "`status` {status:?} must be a single token"
        )));
    }
    Ok(())
}

/// The authored view of a node: placement and derived members removed.
fn authored(node: &GraphKnowledgeObjectNode) -> Value {
    let mut value = serde_json::to_value(node).unwrap_or(Value::Null);
    if let Value::Object(map) = &mut value {
        for member in [
            "content_hash",
            "page_id",
            "source_span",
            "source_binding",
            "effective_status",
            "effective_reason",
            "evidence_quality",
        ] {
            map.remove(member);
        }
        // An object-reference entry's kind is the target's, resolved from
        // the project; the reference id is the authored member.
        if let Some(Value::Array(entries)) = map.get_mut("evidence") {
            for entry in entries {
                if entry.get("reference").is_some_and(|r| !r.is_null()) {
                    entry["kind"] = Value::Null;
                }
            }
        }
    }
    value
}

/// Render the `.adoc` block for one `adoc.render_object.v0` document.
/// Deterministic: fields sort by key, one trailing newline. The compiled
/// block must rebuild to exactly the input's authored members.
pub fn run_render_object(bytes: &[u8]) -> Result<String, Vec<Diagnostic>> {
    render(bytes).map(|rendered| rendered.block)
}

/// A validated render: the node and block fields stay available for
/// `--into --append`, which re-plans the block against the target file.
struct Rendered {
    node: GraphKnowledgeObjectNode,
    fields: BTreeMap<String, String>,
    block: String,
}

fn render(bytes: &[u8]) -> Result<Rendered, Vec<Diagnostic>> {
    if bytes.len() > MAX_RENDER_OBJECT_BYTES {
        return Err(refusal(
            DiagnosticCode::IoArtifactMalformed,
            "render-object input exceeds its size bound",
        ));
    }
    let value: Value = serde_json::from_slice(bytes).map_err(|error| {
        refusal(
            DiagnosticCode::IoArtifactMalformed,
            format!("render-object input is not JSON: {error}"),
        )
    })?;
    if value.get("schema_version").and_then(Value::as_str) != Some(RENDER_OBJECT_SCHEMA_VERSION) {
        return Err(refusal(
            DiagnosticCode::SchemaUnsupportedVersion,
            format!("render-object input needs schema_version `{RENDER_OBJECT_SCHEMA_VERSION}`"),
        ));
    }
    let input: RenderObjectInput = serde_json::from_value(value).map_err(|error| {
        refusal(
            DiagnosticCode::IoArtifactMalformed,
            format!("render-object input is invalid: {error}"),
        )
    })?;
    let node = GraphKnowledgeObjectNode {
        id: input.id,
        kind: input.kind,
        content_hash: String::new(),
        status: input.status,
        severity: input.severity,
        trust: input.trust,
        body: input.body,
        page_id: String::new(),
        source_span: GraphSourceSpan {
            path: String::new(),
            line: 0,
            column: 0,
        },
        source_binding: None,
        visibility: input.visibility,
        field_visibility: input.field_visibility,
        fields: input.fields,
        relations: GraphRelations {
            depends_on: input.relations.depends_on,
            supersedes: input.relations.supersedes,
            related_to: input.relations.related_to,
        },
        impacts: input.impacts,
        approved_by: input.approved_by,
        allowed_actions: input.allowed_actions,
        forbidden_actions: input.forbidden_actions,
        contradiction_claims: input.contradiction_claims,
        evidence: input
            .evidence
            .into_iter()
            .map(|e| GraphEvidence {
                kind: e.kind,
                value: e.value,
                reference: e.reference,
            })
            .collect(),
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
    };
    check_scalars(&node)?;
    let fields = node_block_fields(&node, &node.relations, &|_| true)
        .map_err(|_| not_representable("an authored member cannot be written as a block field"))?;
    let block = plan_create_object(
        "",
        CreateInsertion::EndOfFile,
        &node.kind,
        &node.id,
        node.status.as_deref(),
        &fields,
        &node.body,
    )?
    .splice("")
    .map_err(|_| not_representable("the block could not be rendered"))?;

    // Kind, status and field validity come from the existing compiler; the
    // rebuilt node must equal the input on every authored member.
    let source = format!("# Render @doc(render.object)\n\n{block}");
    let provider = InMemorySourceProvider::new().with_source(SourceFile::new_with_identity_path(
        "render-object.adoc".into(),
        source,
        "render-object.adoc".into(),
    ));
    let has_evidence_refs = node.evidence.iter().any(|e| e.reference.is_some());
    let tolerated = |diagnostic: &Diagnostic| is_cross_object(diagnostic.code, has_evidence_refs);
    let compiled = compile_tolerating_for_date(&provider, local_today(), &|d| !tolerated(d));
    let errors: Vec<Diagnostic> = compiled
        .diagnostics
        .into_iter()
        .filter(|d| d.severity == Severity::Error && !tolerated(d))
        .map(|mut d| {
            // The spans name a synthetic file at shifted lines.
            d.span = None;
            d
        })
        .collect();
    if !errors.is_empty() {
        return Err(errors);
    }
    let rebuilt = compiled
        .artifacts
        .and_then(|artifacts| {
            serde_json::from_str::<GraphArtifactDocument>(&artifacts.graph_json).ok()
        })
        .map(|graph| {
            graph
                .nodes
                .iter()
                .filter_map(|n| n.as_knowledge_object())
                .cloned()
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();
    match rebuilt.as_slice() {
        [only] if only.id == node.id && authored(only) == authored(&node) => Ok(Rendered {
            node,
            fields,
            block,
        }),
        [only] if only.id == node.id => Err(not_representable(format!(
            "object `{}` does not rebuild to the input's authored members; a member is not representable for this kind or status",
            node.id
        ))),
        _ => Err(not_representable(format!(
            "the block did not compile to exactly one object `{}`",
            node.id
        ))),
    }
}

/// Issue 291: patch one object's block into `file`. The block whose ID equals
/// the input's `id` is replaced by its span; with `append` an absent ID is
/// inserted at the end of the file instead. Every other byte of `file` is
/// kept, and a CRLF file stays CRLF. Refusals write nothing (the caller owns
/// the write).
pub fn run_render_object_into(
    input: &[u8],
    file: &[u8],
    append: bool,
) -> Result<String, Vec<Diagnostic>> {
    if file.len() > MAX_RENDER_OBJECT_BYTES {
        return Err(vec![Diagnostic::error(
            DiagnosticCode::RenderIntoFileTooLarge,
            "render-object --into file exceeds its size bound",
        )]);
    }
    let rendered = render(input)?;
    let id = &rendered.node.id;
    let unparsable = |message: &str| {
        vec![Diagnostic::error(
            DiagnosticCode::RenderIntoFileUnparsable,
            message,
        )]
    };
    let text = std::str::from_utf8(file)
        .map_err(|_| unparsable("render-object --into file is not UTF-8"))?;
    let source_of = |text: &str| {
        SourceFile::new_with_identity_path(
            "render-object-into.adoc".into(),
            text.to_string(),
            "render-object-into.adoc".into(),
        )
    };
    let source = source_of(text);
    let (layouts, has_errors) = typed_block_layouts(&source, id);
    if has_errors {
        return Err(unparsable(
            "the --into file does not parse without errors; fix it before patching",
        ));
    }
    let into_error = |code, message: String| vec![Diagnostic::error(code, message)];
    let plan = match (layouts.as_slice(), append) {
        ([], true) => plan_create_object(
            text,
            CreateInsertion::EndOfFile,
            &rendered.node.kind,
            id,
            rendered.node.status.as_deref(),
            &rendered.fields,
            &rendered.node.body,
        )?,
        ([], false) => {
            return Err(into_error(
                DiagnosticCode::RenderIntoIdAbsent,
                format!("object `{id}` is not in the --into file; pass --append to add it"),
            ));
        }
        ([only], false) => {
            // The rendered block is LF; convert it to the file's convention.
            // Its body lines never carry `\r` (the writer strips them).
            let block = rendered.block.trim_end_matches('\n');
            let replacement = match LineEnding::detect(text) {
                LineEnding::Lf => block.to_string(),
                LineEnding::CrLf => block.replace('\n', "\r\n"),
            };
            SourceEditPlan::new(vec![SpanEdit {
                byte_range: only.open_fence.start..only.close_fence.end,
                replacement,
            }])
            .map_err(|_| unparsable("the block could not be spliced into the --into file"))?
        }
        ([_], true) => {
            return Err(into_error(
                DiagnosticCode::RenderIntoIdPresent,
                format!("object `{id}` is already in the --into file; drop --append to replace it"),
            ));
        }
        (many, _) => {
            return Err(into_error(
                DiagnosticCode::RenderIntoIdDuplicate,
                format!(
                    "object `{id}` appears {} times in the --into file; fix the duplicate first",
                    many.len()
                ),
            ));
        }
    };
    let patched = plan
        .splice(text)
        .map_err(|_| unparsable("the block could not be spliced into the --into file"))?;
    // Defence in depth: the result must parse cleanly with the ID exactly once.
    match typed_block_layouts(&source_of(&patched), id) {
        (found, false) if found.len() == 1 => Ok(patched),
        _ => Err(unparsable(
            "the patched file would not parse with the object exactly once; nothing written",
        )),
    }
}

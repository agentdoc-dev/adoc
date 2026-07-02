//! Patch application orchestration (V6.4, ADR-0036).
//!
//! Mirrors `application/patch.rs`'s reader-injected style but goes one step
//! further: after the unchanged V2 validation, apply recompiles the working
//! tree in memory (the source-drift gate and the source of fresh spans),
//! splices the target file through the pure `domain::source_edit` planners,
//! writes atomically through the `WorkspaceWriter` port, and re-checks. Every
//! refusal is a normal `adoc.patch.apply.v0` envelope with `applied: false`
//! and fix-oriented diagnostics — never a process error. No auto-revert,
//! ever: after the rename, the human and Git undo.

use std::path::{Path, PathBuf};

use serde::Serialize;

use crate::application::compile::compile_with_provider;
use crate::application::hashing::sha256_prefixed;
use crate::application::patch::{PatchCheckResult, check_patch_documents};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{
    GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode, GraphPageNode,
};
use crate::domain::obligation::ProofObligation;
use crate::domain::patch::{PatchDocument, PatchIntent, PlacementHint};
use crate::domain::ports::artifact_reader::ArtifactReader;
use crate::domain::ports::source_provider::SourceProvider;
use crate::domain::ports::workspace_writer::{WorkspaceWriteError, WorkspaceWriter};
use crate::domain::source::SourceFile;
use crate::domain::source_edit::SourceEditPlan;
use crate::domain::source_edit::planner::{
    CreateInsertion, plan_create_object, plan_replace_body, plan_update_fields,
};
use crate::infrastructure::parser::layout::typed_block_layout;

pub const PATCH_APPLY_SCHEMA_VERSION: &str = "adoc.patch.apply.v0";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct WrittenFile {
    pub path: String,
    pub before_file_hash: String,
    pub after_file_hash: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct ObjectHashes {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_content_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PostCheckReport {
    pub ran: bool,
    pub error_count: usize,
    pub warning_count: usize,
    pub diagnostics: Vec<Diagnostic>,
}

impl PostCheckReport {
    fn not_run() -> Self {
        Self {
            ran: false,
            error_count: 0,
            warning_count: 0,
            diagnostics: Vec::new(),
        }
    }
}

/// `trace.proposer` wire shape: the patch's proposer metadata, recorded not
/// enforced (the permission engine is explicitly deferred).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyProposer {
    pub kind: String,
    pub id: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ApplyTrace {
    pub interface: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposer: Option<ApplyProposer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PatchApplyResult {
    pub schema_version: &'static str,
    pub applied: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<String>,
    pub operation: String,
    /// The embedded `adoc.patch.check.v0` envelope. Absent only for refusals
    /// that never reached validation (e.g. the disabled MCP gate).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub check: Option<PatchCheckResult>,
    pub written_files: Vec<WrittenFile>,
    pub object: ObjectHashes,
    pub post_check: PostCheckReport,
    pub artifacts_stale: bool,
    pub proof_obligations: Vec<ProofObligation>,
    pub trace: ApplyTrace,
    pub diagnostics: Vec<Diagnostic>,
}

impl PatchApplyResult {
    /// Refusal that never reached patch validation: no embedded check, no
    /// target knowledge. Used for artifact-load failures and the TB4
    /// disabled-gate posture.
    pub fn refused(diagnostics: Vec<Diagnostic>, trace: ApplyTrace) -> Self {
        Self {
            schema_version: PATCH_APPLY_SCHEMA_VERSION,
            applied: false,
            target: None,
            operation: String::new(),
            check: None,
            written_files: Vec::new(),
            object: ObjectHashes::default(),
            post_check: PostCheckReport::not_run(),
            artifacts_stale: false,
            proof_obligations: Vec::new(),
            trace,
            diagnostics,
        }
    }

    /// Refusal carrying the validation result: either the check itself failed
    /// (`diagnostics` mirrors the check's), or the check passed and an
    /// apply-stage gate refused (`diagnostics` carries the apply-stage
    /// reasons).
    fn refused_with_check(
        check: PatchCheckResult,
        trace: ApplyTrace,
        diagnostics: Vec<Diagnostic>,
    ) -> Self {
        Self {
            schema_version: PATCH_APPLY_SCHEMA_VERSION,
            applied: false,
            target: check.target.clone(),
            operation: check.operation.clone(),
            proof_obligations: check.proof_obligations.clone(),
            check: Some(check),
            written_files: Vec::new(),
            object: ObjectHashes::default(),
            post_check: PostCheckReport::not_run(),
            artifacts_stale: false,
            trace,
            diagnostics,
        }
    }
}

/// V6.4 TB4 (ADR-0037): the disabled-gate refusal returned by the MCP
/// `adoc_patch_apply` tool when the project has not opted in. A normal
/// envelope — schema-identical to every other refusal — with exactly one
/// fix-oriented diagnostic naming the config key, never a protocol error.
pub fn mcp_patch_apply_disabled_refusal() -> PatchApplyResult {
    PatchApplyResult::refused(
        vec![Diagnostic::error(
            DiagnosticCode::McpPatchApplyDisabled,
            "MCP patch apply is disabled for this project; set `mcp: { patch_apply: enabled }` \
             in agentdoc.config.yaml to opt in. adoc_patch_check remains available.",
        )],
        ApplyTrace {
            interface: "mcp".to_string(),
            proposer: None,
        },
    )
}

pub(crate) fn apply_trace(interface: &str, patch: &PatchDocument) -> ApplyTrace {
    ApplyTrace {
        interface: interface.to_string(),
        proposer: patch.proposer.as_ref().map(|proposer| ApplyProposer {
            kind: proposer.proposer_type.clone(),
            id: proposer.id.clone(),
        }),
    }
}

/// Apply a patch document against the graph artifact at
/// `graph_artifact_path`, recompiling the working tree through
/// `source_provider` (which must resolve the docs root exactly as
/// `check`/`build` do — `content_hash` payloads embed source paths, so a
/// differently-spelled root reads as source drift) and writing through
/// `writer` (sandboxed to the project root).
pub(crate) fn apply_patch_with_ports<G, P, W>(
    graph_artifact_path: &Path,
    patch: PatchDocument,
    graph_reader: &G,
    source_provider: &P,
    writer: &W,
    interface: &str,
) -> PatchApplyResult
where
    G: ArtifactReader<Output = GraphArtifactDocument>,
    P: SourceProvider,
    W: WorkspaceWriter,
{
    let trace = apply_trace(interface, &patch);

    // 1. Load the artifact and run the unchanged V2 validation.
    let graph_document = match graph_reader.read(graph_artifact_path) {
        Ok(document) => document,
        Err(diagnostics) => return PatchApplyResult::refused(diagnostics, trace),
    };
    let target_node = find_object(&graph_document, &patch.target).cloned();
    // TB3: a create with an `after` anchor splices against that anchor's
    // block, so the anchor joins the drift gate. Captured before the check
    // consumes the document.
    let anchor_artifact_hash = match &patch.intent {
        PatchIntent::CreateObject {
            placement: Some(placement),
            ..
        } => placement
            .after
            .as_ref()
            .and_then(|after| find_object(&graph_document, after))
            .map(|node| node.content_hash.clone()),
        _ => None,
    };
    let check = check_patch_documents(graph_document, patch.clone());
    if !check.valid {
        let diagnostics = check.diagnostics.clone();
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }

    // 2. Recompile the working tree in memory. A dirty tree cannot prove
    //    graph-vs-source freshness, and the recompile supplies fresh spans.
    let recompiled = compile_with_provider(source_provider);
    if recompiled.has_errors() || recompiled.artifacts.is_none() {
        let mut diagnostics = vec![Diagnostic::error(
            DiagnosticCode::PatchSourceDrift,
            "working tree does not compile cleanly; run adoc build and re-propose",
        )];
        diagnostics.extend(recompiled.diagnostics);
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }
    let recompiled_artifacts = recompiled
        .artifacts
        .expect("checked is_none above; compile artifacts present");
    let recompiled_document: GraphArtifactDocument =
        serde_json::from_str(&recompiled_artifacts.graph_json)
            .expect("compile output graph_json is well-formed (compile pipeline invariant)");

    // TB3: create_object has its own drift-gate variant and placement
    // resolution; everything below this dispatch targets an existing object.
    if let PatchIntent::CreateObject {
        kind,
        status,
        body,
        fields,
        placement,
    } = &patch.intent
    {
        return apply_create_object(
            CreateApplyContext {
                check,
                trace,
                target: &patch.target,
                kind,
                status: status.as_deref(),
                body,
                fields,
                placement: placement.as_ref(),
                anchor_artifact_hash,
            },
            &recompiled_document,
            source_provider,
            writer,
        );
    }

    // 3. Source-drift gate: the recompiled target must reproduce the
    //    artifact's content_hash, else the artifact is stale over moved-on
    //    source and spans cannot be trusted.
    let Some(artifact_node) = target_node else {
        return refuse_unsupported_operation(check, trace);
    };
    let drifted = find_object(&recompiled_document, &patch.target)
        .map(|node| node.content_hash != artifact_node.content_hash)
        .unwrap_or(true);
    if drifted {
        let diagnostics = vec![source_drift(&patch.target)];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }
    let before_content_hash = artifact_node.content_hash.clone();

    // 4. Read the target file once; layout, splice, and the TOCTOU hash all
    //    derive from these exact bytes.
    let target_path = PathBuf::from(&artifact_node.source_span.path);
    let original_text = match writer.read_to_string(&target_path) {
        Ok(text) => text,
        Err(error) => {
            let diagnostics = vec![write_error_diagnostic(&error)];
            return PatchApplyResult::refused_with_check(check, trace, diagnostics);
        }
    };
    let before_file_hash = sha256_prefixed(original_text.as_bytes());

    // 5. Fresh spans from a re-parse of the bytes just read — never artifact
    //    spans (start-only, build-stale).
    let source_file = SourceFile::new_with_identity_path(
        target_path.clone(),
        original_text.clone(),
        target_path.clone(),
    );
    let Some(layout) = typed_block_layout(&source_file, &patch.target) else {
        let diagnostics = vec![source_drift(&patch.target)];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    };

    // 6. Plan the edit for the validated operation.
    let plan: Result<SourceEditPlan, Vec<Diagnostic>> = match &patch.intent {
        PatchIntent::ReplaceBody { body, .. } => plan_replace_body(&original_text, &layout, body),
        PatchIntent::UpdateFields { fields, .. } => {
            plan_update_fields(&original_text, &layout, fields)
        }
        // TB2: relation ops are field-line edits with the same splice
        // discipline. Supersede merges the (drift-gated) node's existing
        // targets with the validated new ones — existing first, patch order
        // after — into one bare comma-list value.
        PatchIntent::Supersede { supersedes, .. } => {
            let mut merged = artifact_node.relations.supersedes.clone();
            for target in supersedes {
                if !merged.iter().any(|existing| existing == target) {
                    merged.push(target.clone());
                }
            }
            plan_update_fields(
                &original_text,
                &layout,
                &std::collections::BTreeMap::from([("supersedes".to_string(), merged.join(", "))]),
            )
        }
        PatchIntent::Revoke { .. } => plan_update_fields(
            &original_text,
            &layout,
            &std::collections::BTreeMap::from([("status".to_string(), "revoked".to_string())]),
        ),
        PatchIntent::CreateObject { .. } => {
            return refuse_unsupported_operation(check, trace);
        }
    };
    let plan = match plan {
        Ok(plan) => plan,
        Err(diagnostics) => return PatchApplyResult::refused_with_check(check, trace, diagnostics),
    };
    let new_text = match plan.splice(&original_text) {
        Ok(text) => text,
        Err(error) => {
            let diagnostics = vec![Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                format!("internal splice failure: {error}"),
            )];
            return PatchApplyResult::refused_with_check(check, trace, diagnostics);
        }
    };
    let after_file_hash = sha256_prefixed(new_text.as_bytes());

    // 7. Atomic write with the pre-rename TOCTOU re-hash.
    if let Err(error) = writer.write_atomic(&target_path, &new_text, &before_file_hash) {
        let diagnostics = vec![write_error_diagnostic(&error)];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }

    // 8. Post-apply re-check from disk. Reported, never acted on.
    let (post_check, after_content_hash) = run_post_check(source_provider, &patch.target);

    PatchApplyResult {
        schema_version: PATCH_APPLY_SCHEMA_VERSION,
        applied: true,
        target: check.target.clone(),
        operation: check.operation.clone(),
        proof_obligations: check.proof_obligations.clone(),
        check: Some(check),
        written_files: vec![WrittenFile {
            path: artifact_node.source_span.path.clone(),
            before_file_hash,
            after_file_hash,
        }],
        object: ObjectHashes {
            before_content_hash: Some(before_content_hash),
            after_content_hash,
        },
        post_check,
        artifacts_stale: true,
        trace,
        diagnostics: Vec::new(),
    }
}

/// Borrowed inputs for the TB3 `create_object` apply path.
struct CreateApplyContext<'a> {
    check: PatchCheckResult,
    trace: ApplyTrace,
    target: &'a str,
    kind: &'a str,
    status: Option<&'a str>,
    body: &'a str,
    fields: &'a std::collections::BTreeMap<String, String>,
    placement: Option<&'a PlacementHint>,
    /// The artifact's `content_hash` for the `after` anchor, when one is
    /// named — the anchor's half of the drift gate.
    anchor_artifact_hash: Option<String>,
}

fn apply_create_object<P, W>(
    context: CreateApplyContext<'_>,
    recompiled_document: &GraphArtifactDocument,
    source_provider: &P,
    writer: &W,
) -> PatchApplyResult
where
    P: SourceProvider,
    W: WorkspaceWriter,
{
    let CreateApplyContext {
        check,
        trace,
        target,
        kind,
        status,
        body,
        fields,
        placement,
        anchor_artifact_hash,
    } = context;

    // Placement is a WARNING on --check and an ERROR here (ADR-0036).
    let Some(placement) = placement else {
        let diagnostics = vec![
            Diagnostic::error(
                DiagnosticCode::PatchCreateMissingPlacement,
                format!("create_object for `{target}` cannot apply without a placement"),
            )
            .with_object_id(target),
        ];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    };

    // Create-variant drift gate: the target must not already exist in the
    // current source, the placement page must still exist, and a named
    // anchor must reproduce its artifact hash (it anchors the splice).
    if find_object(recompiled_document, target).is_some() {
        let diagnostics = vec![
            Diagnostic::error(
                DiagnosticCode::PatchSourceDrift,
                format!(
                    "`{target}` already exists in current source but not in the graph artifact; \
                     run adoc build and re-propose"
                ),
            )
            .with_object_id(target),
        ];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }
    let Some(page) = find_page(recompiled_document, &placement.page_id) else {
        let diagnostics = vec![Diagnostic::error(
            DiagnosticCode::PatchSourceDrift,
            format!(
                "placement page `{}` is not in the current source; run adoc build and re-propose",
                placement.page_id
            ),
        )];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    };
    if !page.source_path.ends_with(".adoc") {
        let diagnostics = vec![Diagnostic::error(
            DiagnosticCode::PatchPlacementNotAdoc,
            format!(
                "placement page `{}` is backed by `{}`; .md pages cannot host typed blocks",
                placement.page_id, page.source_path
            ),
        )];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }

    let target_path = PathBuf::from(&page.source_path);
    let original_text = match writer.read_to_string(&target_path) {
        Ok(text) => text,
        Err(error) => {
            let diagnostics = vec![write_error_diagnostic(&error)];
            return PatchApplyResult::refused_with_check(check, trace, diagnostics);
        }
    };
    let before_file_hash = sha256_prefixed(original_text.as_bytes());

    let insertion = match &placement.after {
        Some(after) => {
            let anchor_fresh = matches!(
                (&anchor_artifact_hash, find_object(recompiled_document, after)),
                (Some(artifact_hash), Some(node)) if &node.content_hash == artifact_hash
            );
            if !anchor_fresh {
                let diagnostics = vec![source_drift(after)];
                return PatchApplyResult::refused_with_check(check, trace, diagnostics);
            }
            let source_file = SourceFile::new_with_identity_path(
                target_path.clone(),
                original_text.clone(),
                target_path.clone(),
            );
            let Some(layout) = typed_block_layout(&source_file, after) else {
                let diagnostics = vec![source_drift(after)];
                return PatchApplyResult::refused_with_check(check, trace, diagnostics);
            };
            CreateInsertion::AfterCloseFence {
                close_fence_end: layout.close_fence.end,
            }
        }
        None => CreateInsertion::EndOfFile,
    };

    let plan = match plan_create_object(
        &original_text,
        insertion,
        kind,
        target,
        status,
        fields,
        body,
    ) {
        Ok(plan) => plan,
        Err(diagnostics) => return PatchApplyResult::refused_with_check(check, trace, diagnostics),
    };
    let new_text = match plan.splice(&original_text) {
        Ok(text) => text,
        Err(error) => {
            let diagnostics = vec![Diagnostic::error(
                DiagnosticCode::PatchValidationFailed,
                format!("internal splice failure: {error}"),
            )];
            return PatchApplyResult::refused_with_check(check, trace, diagnostics);
        }
    };
    let after_file_hash = sha256_prefixed(new_text.as_bytes());

    if let Err(error) = writer.write_atomic(&target_path, &new_text, &before_file_hash) {
        let diagnostics = vec![write_error_diagnostic(&error)];
        return PatchApplyResult::refused_with_check(check, trace, diagnostics);
    }

    let (post_check, after_content_hash) = run_post_check(source_provider, target);

    PatchApplyResult {
        schema_version: PATCH_APPLY_SCHEMA_VERSION,
        applied: true,
        target: check.target.clone(),
        operation: check.operation.clone(),
        proof_obligations: check.proof_obligations.clone(),
        check: Some(check),
        written_files: vec![WrittenFile {
            path: page.source_path.clone(),
            before_file_hash,
            after_file_hash,
        }],
        object: ObjectHashes {
            before_content_hash: None,
            after_content_hash,
        },
        post_check,
        artifacts_stale: true,
        trace,
        diagnostics: Vec::new(),
    }
}

/// Recompile from disk after the rename and embed every diagnostic; returns
/// the report plus the target's post-apply `content_hash` when the recompile
/// produced artifacts. Reported, never acted on.
fn run_post_check<P: SourceProvider>(
    source_provider: &P,
    target: &str,
) -> (PostCheckReport, Option<String>) {
    let post_compile = compile_with_provider(source_provider);
    let error_count = count_severity(&post_compile.diagnostics, Severity::Error);
    let warning_count = count_severity(&post_compile.diagnostics, Severity::Warning);
    let after_content_hash = post_compile.artifacts.as_ref().and_then(|artifacts| {
        let document: GraphArtifactDocument = serde_json::from_str(&artifacts.graph_json)
            .expect("compile output graph_json is well-formed (compile pipeline invariant)");
        find_object(&document, target).map(|node| node.content_hash.clone())
    });
    (
        PostCheckReport {
            ran: true,
            error_count,
            warning_count,
            diagnostics: post_compile.diagnostics,
        },
        after_content_hash,
    )
}

fn refuse_unsupported_operation(check: PatchCheckResult, trace: ApplyTrace) -> PatchApplyResult {
    let operation = check.operation.clone();
    let diagnostics = vec![Diagnostic::error(
        DiagnosticCode::PatchValidationFailed,
        format!("`{operation}` is not yet supported by patch apply"),
    )];
    PatchApplyResult::refused_with_check(check, trace, diagnostics)
}

fn source_drift(target: &str) -> Diagnostic {
    Diagnostic::error(
        DiagnosticCode::PatchSourceDrift,
        format!("source changed since last build for `{target}`; run adoc build and re-propose"),
    )
    .with_object_id(target)
}

fn write_error_diagnostic(error: &WorkspaceWriteError) -> Diagnostic {
    match error {
        WorkspaceWriteError::ConcurrentModification { .. } => {
            Diagnostic::error(DiagnosticCode::PatchSourceDrift, error.to_string())
                .with_help("The file changed while apply was running; nothing was written. Re-run adoc build and re-propose.")
        }
        WorkspaceWriteError::OutsideSandbox { .. } | WorkspaceWriteError::Io { .. } => {
            Diagnostic::error(DiagnosticCode::PatchValidationFailed, error.to_string())
        }
    }
}

fn find_object<'a>(
    document: &'a GraphArtifactDocument,
    target: &str,
) -> Option<&'a GraphKnowledgeObjectNode> {
    document.nodes.iter().find_map(|node| match node {
        GraphNode::KnowledgeObject(object) if object.id == target => Some(object),
        _ => None,
    })
}

fn find_page<'a>(document: &'a GraphArtifactDocument, page_id: &str) -> Option<&'a GraphPageNode> {
    document.nodes.iter().find_map(|node| match node {
        GraphNode::Page(page) if page.id == page_id => Some(page),
        _ => None,
    })
}

fn count_severity(diagnostics: &[Diagnostic], severity: Severity) -> usize {
    diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == severity)
        .count()
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;
    use std::rc::Rc;

    use crate::domain::patch::PatchProposer;
    use crate::domain::ports::source_provider::SourceLoadError;

    use super::*;

    const PAGE_TEXT: &str = "\
# Billing

::claim billing.credits
owner: team-billing
status: draft
--
Original body line.
::
";

    const PAGE_PATH: &str = "docs/billing.adoc";

    /// Shared in-memory \"filesystem\" backing both the source provider (so
    /// the post-check recompile sees written content) and the workspace
    /// writer (so writes are observable).
    #[derive(Clone, Default)]
    struct SharedFs {
        files: Rc<RefCell<BTreeMap<PathBuf, String>>>,
    }

    impl SharedFs {
        fn with_file(path: &str, text: &str) -> Self {
            let fs = Self::default();
            fs.files
                .borrow_mut()
                .insert(PathBuf::from(path), text.to_string());
            fs
        }

        fn read(&self, path: &str) -> String {
            self.files.borrow()[&PathBuf::from(path)].clone()
        }
    }

    impl SourceProvider for SharedFs {
        fn load_sources(&self) -> Vec<Result<SourceFile, SourceLoadError>> {
            self.files
                .borrow()
                .iter()
                .map(|(path, text)| {
                    Ok(SourceFile::new_with_identity_path(
                        path.clone(),
                        text.clone(),
                        path.clone(),
                    ))
                })
                .collect()
        }
    }

    impl WorkspaceWriter for SharedFs {
        fn read_to_string(&self, path: &Path) -> Result<String, WorkspaceWriteError> {
            self.files
                .borrow()
                .get(path)
                .cloned()
                .ok_or_else(|| WorkspaceWriteError::Io {
                    path: path.to_path_buf(),
                    message: "not found".to_string(),
                })
        }

        fn write_atomic(
            &self,
            path: &Path,
            contents: &str,
            expected_current_hash: &str,
        ) -> Result<(), WorkspaceWriteError> {
            let current = self.read_to_string(path)?;
            if sha256_prefixed(current.as_bytes()) != expected_current_hash {
                return Err(WorkspaceWriteError::ConcurrentModification {
                    path: path.to_path_buf(),
                });
            }
            self.files
                .borrow_mut()
                .insert(path.to_path_buf(), contents.to_string());
            Ok(())
        }
    }

    struct StubGraphReader {
        document: GraphArtifactDocument,
    }

    impl ArtifactReader for StubGraphReader {
        type Output = GraphArtifactDocument;

        fn read(&self, _path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
            Ok(self.document.clone())
        }
    }

    /// Compile the shared fs and parse the resulting graph artifact — the
    /// in-memory analogue of `adoc build`, so artifact hashes always match a
    /// clean recompile.
    fn built_artifact(fs: &SharedFs) -> GraphArtifactDocument {
        let result = compile_with_provider(fs);
        assert!(
            !result.has_errors(),
            "fixture must compile cleanly: {:?}",
            result.diagnostics
        );
        serde_json::from_str(&result.artifacts.expect("artifacts").graph_json)
            .expect("graph json parses")
    }

    fn content_hash(document: &GraphArtifactDocument, id: &str) -> String {
        find_object(document, id)
            .expect("object present")
            .content_hash
            .clone()
    }

    fn replace_body_patch(base_hash: &str, body: &str) -> PatchDocument {
        PatchDocument {
            target: "billing.credits".to_string(),
            intent: PatchIntent::ReplaceBody {
                base_hash: base_hash.to_string(),
                body: body.to_string(),
            },
            reason: "test".to_string(),
            proposer: Some(PatchProposer {
                proposer_type: "agent".to_string(),
                id: "test-agent".to_string(),
            }),
        }
    }

    fn apply(
        fs: &SharedFs,
        document: GraphArtifactDocument,
        patch: PatchDocument,
    ) -> PatchApplyResult {
        apply_patch_with_ports(
            Path::new("dist/docs.graph.json"),
            patch,
            &StubGraphReader { document },
            fs,
            fs,
            "cli",
        )
    }

    #[test]
    fn replace_body_applies_and_rewrites_only_the_body() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");

        let result = apply(
            &fs,
            document,
            replace_body_patch(&base_hash, "New body line."),
        );

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.schema_version, PATCH_APPLY_SCHEMA_VERSION);
        assert_eq!(result.operation, "replace_body");
        assert_eq!(result.target.as_deref(), Some("billing.credits"));
        assert!(result.artifacts_stale);
        assert!(result.check.as_ref().expect("check embedded").valid);
        assert_eq!(result.written_files.len(), 1);
        assert_eq!(result.written_files[0].path, PAGE_PATH);
        assert_ne!(
            result.written_files[0].before_file_hash,
            result.written_files[0].after_file_hash
        );
        assert!(result.post_check.ran);
        assert_eq!(result.post_check.error_count, 0);
        assert_eq!(
            result.object.before_content_hash.as_deref(),
            Some(base_hash.as_str())
        );
        assert!(result.object.after_content_hash.is_some());
        assert_ne!(
            result.object.before_content_hash,
            result.object.after_content_hash
        );
        assert_eq!(result.trace.interface, "cli");
        assert_eq!(
            result.trace.proposer.as_ref().map(|p| p.kind.as_str()),
            Some("agent")
        );

        assert_eq!(
            fs.read(PAGE_PATH),
            PAGE_TEXT.replace("Original body line.", "New body line.")
        );
    }

    #[test]
    fn update_fields_applies_existing_and_new_keys() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");
        let patch = PatchDocument {
            target: "billing.credits".to_string(),
            intent: PatchIntent::UpdateFields {
                base_hash,
                fields: BTreeMap::from([
                    ("status".to_string(), "verified".to_string()),
                    ("verified_at".to_string(), "2026-06-12".to_string()),
                    ("reviewed_by".to_string(), "team-billing".to_string()),
                ]),
            },
            reason: "test".to_string(),
            proposer: None,
        };

        let result = apply(&fs, document, patch);

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        let written = fs.read(PAGE_PATH);
        assert_eq!(
            written,
            PAGE_TEXT.replace(
                "status: draft\n",
                "status: verified\nreviewed_by: team-billing\nverified_at: 2026-06-12\n"
            )
        );
        assert!(result.trace.proposer.is_none());
    }

    #[test]
    fn invalid_check_refuses_with_embedded_check_and_writes_nothing() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);

        let result = apply(&fs, document, replace_body_patch("sha256:stale", "x"));

        assert!(!result.applied);
        assert!(result.written_files.is_empty());
        assert!(!result.post_check.ran);
        assert!(!result.artifacts_stale);
        assert!(!result.check.as_ref().expect("check embedded").valid);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::PatchBaseHashMismatch)
        );
        assert_eq!(fs.read(PAGE_PATH), PAGE_TEXT, "refusal must write nothing");
    }

    #[test]
    fn source_drift_refuses_when_source_moved_on_after_build() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");

        // Source moves on after the artifact was built: same object, edited
        // body — the patch's base_hash still matches the (stale) artifact.
        fs.files.borrow_mut().insert(
            PathBuf::from(PAGE_PATH),
            PAGE_TEXT
                .replace("Original body line.", "Moved-on body line.")
                .to_string(),
        );

        let result = apply(&fs, document, replace_body_patch(&base_hash, "New body."));

        assert!(!result.applied);
        assert!(result.check.as_ref().expect("check embedded").valid);
        assert_eq!(result.diagnostics.len(), 1);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::PatchSourceDrift);
        assert!(
            fs.read(PAGE_PATH).contains("Moved-on body line."),
            "refusal must write nothing"
        );
    }

    #[test]
    fn dirty_working_tree_refuses_with_source_drift_and_compile_diagnostics() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");

        // Break the tree: an unclosed fence is a compile error.
        fs.files.borrow_mut().insert(
            PathBuf::from("docs/broken.adoc"),
            "# Broken\n\n::claim broken.block\nstatus: draft\n".to_string(),
        );

        let result = apply(&fs, document, replace_body_patch(&base_hash, "New body."));

        assert!(!result.applied);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::PatchSourceDrift);
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ParseUnclosedFence),
            "compile diagnostics must be embedded"
        );
        assert_eq!(fs.read(PAGE_PATH), PAGE_TEXT);
    }

    #[test]
    fn concurrent_modification_maps_to_source_drift_refusal() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");

        /// Writer wrapper that mutates the file between the planning read and
        /// the atomic write, simulating a concurrent editor.
        struct RacingWriter {
            fs: SharedFs,
        }

        impl WorkspaceWriter for RacingWriter {
            fn read_to_string(&self, path: &Path) -> Result<String, WorkspaceWriteError> {
                let text = self.fs.read_to_string(path)?;
                // Race: the file changes right after apply reads it.
                self.fs.files.borrow_mut().insert(
                    path.to_path_buf(),
                    text.replace("Original body line.", "Raced body line."),
                );
                Ok(text)
            }

            fn write_atomic(
                &self,
                path: &Path,
                contents: &str,
                expected_current_hash: &str,
            ) -> Result<(), WorkspaceWriteError> {
                self.fs.write_atomic(path, contents, expected_current_hash)
            }
        }

        let racing = RacingWriter { fs: fs.clone() };
        let result = apply_patch_with_ports(
            Path::new("dist/docs.graph.json"),
            replace_body_patch(&base_hash, "New body."),
            &StubGraphReader { document },
            &fs,
            &racing,
            "cli",
        );

        assert!(!result.applied);
        assert_eq!(result.diagnostics[0].code, DiagnosticCode::PatchSourceDrift);
        assert!(
            result.diagnostics[0]
                .message
                .contains("changed during apply"),
            "message: {}",
            result.diagnostics[0].message
        );
        assert!(fs.read(PAGE_PATH).contains("Raced body line."));
    }

    #[test]
    fn graph_read_failure_refuses_without_check() {
        struct FailingReader;
        impl ArtifactReader for FailingReader {
            type Output = GraphArtifactDocument;
            fn read(&self, path: &Path) -> Result<Self::Output, Vec<Diagnostic>> {
                Err(vec![Diagnostic::error(
                    DiagnosticCode::IoArtifactMissing,
                    format!("missing artifact at {}", path.display()),
                )])
            }
        }

        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let result = apply_patch_with_ports(
            Path::new("dist/docs.graph.json"),
            replace_body_patch("sha256:x", "y"),
            &FailingReader,
            &fs,
            &fs,
            "cli",
        );

        assert!(!result.applied);
        assert!(result.check.is_none());
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::IoArtifactMissing
        );
    }

    fn create_patch(
        target: &str,
        placement: Option<crate::domain::patch::PlacementHint>,
        fields: BTreeMap<String, String>,
    ) -> PatchDocument {
        PatchDocument {
            target: target.to_string(),
            intent: PatchIntent::CreateObject {
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                body: "New claim body.".to_string(),
                fields,
                placement,
            },
            reason: "test".to_string(),
            proposer: None,
        }
    }

    fn placement(
        page_id: &str,
        after: Option<&str>,
    ) -> Option<crate::domain::patch::PlacementHint> {
        Some(crate::domain::patch::PlacementHint {
            page_id: page_id.to_string(),
            after: after.map(str::to_string),
        })
    }

    #[test]
    fn create_appends_at_end_of_file_when_after_is_absent() {
        use crate::domain::review::object_diff::ObjectDiff;

        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_nodes: Vec<_> = document
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::KnowledgeObject(object) => Some(object.clone()),
                _ => None,
            })
            .collect();

        let result = apply(
            &fs,
            document,
            create_patch(
                "billing.new-claim",
                placement("docs.billing", None),
                BTreeMap::new(),
            ),
        );

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.operation, "create_object");
        assert!(result.object.before_content_hash.is_none());
        assert!(result.object.after_content_hash.is_some());
        assert_eq!(result.post_check.error_count, 0);
        assert_eq!(
            fs.read(PAGE_PATH),
            format!(
                "{PAGE_TEXT}\n::claim billing.new-claim\nstatus: draft\n--\nNew claim body.\n::\n"
            ),
            "block appended at EOF with one separating blank line"
        );

        // Post-apply recompile shows exactly one Added object.
        let head_nodes: Vec<_> = built_artifact(&fs)
            .nodes
            .iter()
            .filter_map(|node| match node {
                GraphNode::KnowledgeObject(object) => Some(object.clone()),
                _ => None,
            })
            .collect();
        let diff = ObjectDiff::compute(&base_nodes, &head_nodes);
        assert_eq!(diff.created.len(), 1);
        assert_eq!(diff.created[0].id, "billing.new-claim");
        assert!(diff.deleted.is_empty());
        assert!(diff.changed.is_empty());
    }

    #[test]
    fn create_inserts_immediately_after_the_anchor_close_fence() {
        let page_text = "\
# Billing

::claim billing.credits
status: draft
--
Original body line.
::

Trailing prose stays put.
";
        let fs = SharedFs::with_file(PAGE_PATH, page_text);
        let document = built_artifact(&fs);

        let result = apply(
            &fs,
            document,
            create_patch(
                "billing.new-claim",
                placement("docs.billing", Some("billing.credits")),
                BTreeMap::new(),
            ),
        );

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(
            fs.read(PAGE_PATH),
            "\
# Billing

::claim billing.credits
status: draft
--
Original body line.
::

::claim billing.new-claim
status: draft
--
New claim body.
::

Trailing prose stays put.
",
            "block inserted after the anchor's close fence; trailing prose byte-identical"
        );
    }

    #[test]
    fn create_renders_fields_in_sorted_order_with_status_merged() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);

        let result = apply(
            &fs,
            document,
            create_patch(
                "billing.new-claim",
                placement("docs.billing", None),
                BTreeMap::from([
                    ("owner".to_string(), "team-billing".to_string()),
                    ("expires_at".to_string(), "2120-01-01".to_string()),
                ]),
            ),
        );

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert!(fs.read(PAGE_PATH).ends_with(
            "\n::claim billing.new-claim\nexpires_at: 2120-01-01\nowner: team-billing\nstatus: draft\n--\nNew claim body.\n::\n"
        ));
    }

    #[test]
    fn create_without_placement_is_check_warning_but_apply_error() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);

        let result = apply(
            &fs,
            document,
            create_patch("billing.new-claim", None, BTreeMap::new()),
        );

        assert!(!result.applied);
        let check = result.check.as_ref().expect("check embedded");
        assert!(
            check.valid,
            "check accepts a placement-less create proposal"
        );
        assert!(check.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::PatchCreateMissingPlacement
                && diagnostic.severity == Severity::Warning
        }));
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchCreateMissingPlacement
        );
        assert_eq!(result.diagnostics[0].severity, Severity::Error);
        assert_eq!(fs.read(PAGE_PATH), PAGE_TEXT, "nothing written");
    }

    #[test]
    fn create_on_a_markdown_page_refuses_with_placement_not_adoc() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        fs.files.borrow_mut().insert(
            PathBuf::from("docs/notes.md"),
            "# Notes\n\nMarkdown prose only.\n".to_string(),
        );
        let document = built_artifact(&fs);

        let result = apply(
            &fs,
            document,
            create_patch(
                "billing.new-claim",
                placement("docs.notes", None),
                BTreeMap::new(),
            ),
        );

        assert!(!result.applied);
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::PatchPlacementNotAdoc
        );
        assert!(
            !fs.read("docs/notes.md").contains("billing.new-claim"),
            "nothing written"
        );
    }

    #[test]
    fn revoke_rewrites_only_the_status_field_line() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");
        let patch = PatchDocument {
            target: "billing.credits".to_string(),
            intent: PatchIntent::Revoke { base_hash },
            reason: "test".to_string(),
            proposer: None,
        };

        let result = apply(&fs, document, patch);

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.operation, "revoke");
        assert_eq!(
            fs.read(PAGE_PATH),
            PAGE_TEXT.replace("status: draft", "status: revoked"),
            "only the status value changes byte-wise"
        );
    }

    const SUPERSEDE_PAGE_TEXT: &str = "\
# Billing

::claim billing.one
status: draft
--
One.
::

::claim billing.two
status: draft
--
Two.
::

::claim billing.credits
status: draft
supersedes: billing.one
--
Original body line.
::
";

    #[test]
    fn supersede_merges_existing_targets_with_patch_targets_in_one_field_line() {
        let fs = SharedFs::with_file(PAGE_PATH, SUPERSEDE_PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");
        let patch = PatchDocument {
            target: "billing.credits".to_string(),
            intent: PatchIntent::Supersede {
                base_hash,
                supersedes: vec!["billing.two".to_string()],
            },
            reason: "test".to_string(),
            proposer: None,
        };

        let result = apply(&fs, document, patch);

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(result.operation, "supersede");
        assert_eq!(
            fs.read(PAGE_PATH),
            SUPERSEDE_PAGE_TEXT.replace(
                "supersedes: billing.one",
                "supersedes: billing.one, billing.two"
            ),
            "existing targets first, patch targets appended, one field line"
        );
        assert_eq!(result.post_check.error_count, 0);
    }

    #[test]
    fn supersede_inserts_the_field_line_when_the_block_has_none() {
        let fs = SharedFs::with_file(PAGE_PATH, SUPERSEDE_PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.two");
        let patch = PatchDocument {
            target: "billing.two".to_string(),
            intent: PatchIntent::Supersede {
                base_hash,
                supersedes: vec!["billing.one".to_string()],
            },
            reason: "test".to_string(),
            proposer: None,
        };

        let result = apply(&fs, document, patch);

        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);
        assert_eq!(
            fs.read(PAGE_PATH),
            SUPERSEDE_PAGE_TEXT.replace(
                "::claim billing.two\nstatus: draft\n",
                "::claim billing.two\nstatus: draft\nsupersedes: billing.one\n"
            ),
            "new supersedes line inserted after the last field line"
        );
    }

    #[test]
    fn recompiled_spliced_tree_yields_exactly_the_intended_object_change() {
        use crate::domain::review::field_change::FieldChange;
        use crate::domain::review::object_diff::ObjectDiff;

        fn knowledge_objects(document: &GraphArtifactDocument) -> Vec<GraphKnowledgeObjectNode> {
            document
                .nodes
                .iter()
                .filter_map(|node| match node {
                    GraphNode::KnowledgeObject(object) => Some(object.clone()),
                    _ => None,
                })
                .collect()
        }

        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        let document = built_artifact(&fs);
        let base_nodes = knowledge_objects(&document);
        let base_hash = content_hash(&document, "billing.credits");

        let result = apply(
            &fs,
            document,
            replace_body_patch(&base_hash, "New body line."),
        );
        assert!(result.applied, "diagnostics: {:?}", result.diagnostics);

        let head_nodes = knowledge_objects(&built_artifact(&fs));
        let diff = ObjectDiff::compute(&base_nodes, &head_nodes);
        assert!(diff.created.is_empty(), "no objects created");
        assert!(diff.deleted.is_empty(), "no objects deleted");
        assert_eq!(diff.changed.len(), 1, "exactly the intended change");
        assert_eq!(diff.changed[0].id, "billing.credits");
        assert!(
            matches!(diff.changed[0].field_changes(), [FieldChange::Body { .. }]),
            "exactly one body change: {:?}",
            diff.changed[0].field_changes()
        );
    }

    #[test]
    fn post_check_errors_are_reported_never_reverted() {
        let fs = SharedFs::with_file(PAGE_PATH, PAGE_TEXT);
        // A second page referencing nothing yet; the patch will introduce a
        // broken object reference into the body, which is a workspace-level
        // post-check error that only surfaces after the write.
        let document = built_artifact(&fs);
        let base_hash = content_hash(&document, "billing.credits");

        let result = apply(
            &fs,
            document,
            replace_body_patch(&base_hash, "See [[no.such.object]] for details."),
        );

        assert!(result.applied, "apply must not revert on post-check errors");
        assert!(result.post_check.ran);
        assert!(
            result.post_check.error_count > 0,
            "broken reference must surface as a post-check error: {:?}",
            result.post_check.diagnostics
        );
        assert!(
            fs.read(PAGE_PATH).contains("no.such.object"),
            "written content stays on disk"
        );
    }
}

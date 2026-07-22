use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

use chrono::NaiveDate;
use serde::Serialize;

use crate::application::compile::{CompileResult, compile_with_provider_anchored_for_date};
use crate::application::hashing::sha256_prefixed;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::graph::{GraphArtifactDocument, GraphKnowledgeObjectNode, GraphNode};
use crate::domain::ports::changed_files::{ChangedFilesError, ChangedFilesProvider};
use crate::domain::ports::snapshot_workspace::{
    SnapshotError, SnapshotSelector, SnapshotWorkspaceProvider,
};
use crate::domain::project_config::{
    EmbeddingsProvider, ParsedProjectConfig, ProjectConfigDocumentError, parse_project_config,
};
use crate::domain::review::object_diff::ObjectDiff;
use crate::domain::value_objects::rel_path::RelPath;
use crate::infrastructure::source::evidence_fs::FsEvidenceFileReader;
use crate::infrastructure::source::fs::FsSourceProvider;

pub const CHANGE_ASSESSMENT_SCHEMA_VERSION: &str = "adoc.change_assessment.v0";
const CONFIG_PATH: &str = "agentdoc.config.yaml";

#[derive(Debug, Clone)]
pub struct ChangeAssessmentInput {
    pub project_root: Option<PathBuf>,
    pub base_ref: String,
    pub head_ref: Option<String>,
    pub evaluation_date: NaiveDate,
}

#[derive(Debug, Clone)]
pub(crate) struct ResolvedAssessmentInput {
    pub requested_base_ref: String,
    pub requested_base_commit: String,
    pub comparison_base_commit: String,
    pub head_ref: Option<String>,
    pub head_commit: String,
    pub base: SnapshotSelector,
    pub head: SnapshotSelector,
    pub worktree_dirty: Option<bool>,
    pub evaluation_date: NaiveDate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentCompleteness {
    Complete,
    Partial,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AssessmentOutcome {
    Pass,
    ReviewRequired,
    Uncovered,
    Invalid,
    NotEvaluated,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentSnapshot {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub requested_ref: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolved_commit: Option<String>,
    pub immutable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub strategy: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub worktree_state: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentSnapshots {
    pub requested_base: AssessmentSnapshot,
    pub comparison_base: AssessmentSnapshot,
    pub head: AssessmentSnapshot,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KnowledgeSnapshot {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_schema_version: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub graph_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_set_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SnapshotConfig {
    pub status: String,
    pub source: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub docs_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentPolicy {
    pub status: String,
    pub effective_source_snapshot: String,
    pub exclude_paths: Vec<String>,
    pub generated_outputs: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_sha256: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposed_head_sha256: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentConfig {
    pub comparison_base: SnapshotConfig,
    pub head: SnapshotConfig,
    pub policy: AssessmentPolicy,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AssessmentSummary {
    pub changed_paths: usize,
    pub covered: usize,
    pub provisional: usize,
    pub uncovered: usize,
    pub excluded: usize,
    pub impacted_objects: usize,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct AssessmentValidation {
    pub errors_full: usize,
    pub errors_changed: usize,
    pub errors_unchanged: usize,
    pub errors_unattributed: usize,
    pub warnings: usize,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PathClassification {
    Covered,
    Provisional,
    Uncovered,
    Excluded,
}

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Serialize)]
pub struct AssessmentMatch {
    pub object_id: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_source_object: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessedPath {
    pub path: String,
    pub classification: PathClassification,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exclusion_reason: Option<String>,
    pub matches: Vec<AssessmentMatch>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Availability<T> {
    pub status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub value: Option<T>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentSource {
    pub path: String,
    pub line: u32,
    pub column: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentObjectReason {
    pub path: String,
    pub reason: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub via_source_object: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentObject {
    pub id: String,
    pub kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_status: Option<String>,
    pub content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub reviewers: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence_quality: Option<String>,
    pub source: AssessmentSource,
    pub authority: String,
    pub changed_in_pr: String,
    pub reasons: Vec<AssessmentObjectReason>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct KnowledgeChange {
    pub id: String,
    pub kind: String,
    pub authority: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub base_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub head_content_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub authored_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_status: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub owner: Option<String>,
    pub reviewers: Vec<String>,
    pub source: AssessmentSource,
    pub reason: String,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
pub struct KnowledgeChanges {
    pub created: Vec<KnowledgeChange>,
    pub changed: Vec<KnowledgeChange>,
    pub deleted: Vec<KnowledgeChange>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PolicyChanges {
    pub status: String,
    pub changed: bool,
    pub changed_fields: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentReviewer {
    pub owner: String,
    pub object_ids: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentObligation {
    pub object_id: String,
    pub kind: String,
    pub reason: String,
    pub required_evidence: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentSignal {
    pub object_id: String,
    pub kind: String,
    pub signal: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct AssessmentDiagnostic {
    pub code: String,
    pub severity: String,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub source: Option<AssessmentSource>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub object_id: Option<String>,
    pub changed_in_pr: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangeAssessmentEnvelope {
    pub schema_version: &'static str,
    pub completeness: AssessmentCompleteness,
    pub outcome: AssessmentOutcome,
    pub evaluation_date: String,
    pub snapshots: AssessmentSnapshots,
    pub knowledge_snapshot: KnowledgeSnapshot,
    pub assessment_config: AssessmentConfig,
    pub summary: AssessmentSummary,
    pub validation: AssessmentValidation,
    pub paths: Availability<Vec<AssessedPath>>,
    pub objects: Availability<Vec<AssessmentObject>>,
    pub knowledge_changes: Availability<KnowledgeChanges>,
    pub policy_changes: PolicyChanges,
    pub required_reviewers: Vec<AssessmentReviewer>,
    pub proof_obligations: Vec<AssessmentObligation>,
    pub signals: Vec<AssessmentSignal>,
    pub diagnostics: Vec<AssessmentDiagnostic>,
}

impl ChangeAssessmentEnvelope {
    pub fn is_complete(&self) -> bool {
        self.completeness == AssessmentCompleteness::Complete
    }
}

pub(crate) fn unresolved_envelope(
    input: &ChangeAssessmentInput,
    message: String,
) -> ChangeAssessmentEnvelope {
    let code = if message.contains("merge base") {
        DiagnosticCode::AssessmentComparisonBaseUnavailable
    } else {
        DiagnosticCode::AssessmentRefUnresolved
    };
    empty_envelope(
        input.evaluation_date,
        unresolved_snapshots(input),
        AssessmentCompleteness::Error,
        AssessmentOutcome::NotEvaluated,
        diagnostic_record(code, Severity::Error, message, None, &BTreeSet::new()),
    )
}

pub(crate) fn snapshot_failed_envelope(
    input: &ChangeAssessmentInput,
    message: String,
) -> ChangeAssessmentEnvelope {
    snapshot_failure(input.evaluation_date, unresolved_snapshots(input), message)
}

pub(crate) fn resolved_snapshot_failed_envelope(
    input: &ResolvedAssessmentInput,
    message: String,
) -> ChangeAssessmentEnvelope {
    snapshot_failure(input.evaluation_date, resolved_snapshots(input), message)
}

fn snapshot_failure(
    date: NaiveDate,
    snapshots: AssessmentSnapshots,
    message: String,
) -> ChangeAssessmentEnvelope {
    empty_envelope(
        date,
        snapshots,
        AssessmentCompleteness::Error,
        AssessmentOutcome::NotEvaluated,
        diagnostic_record(
            DiagnosticCode::AssessmentSnapshotFailed,
            Severity::Error,
            message,
            None,
            &BTreeSet::new(),
        ),
    )
}

pub(crate) fn assess_with_providers<S, C>(
    input: ResolvedAssessmentInput,
    snapshot_provider: &S,
    changed_files_provider: &C,
) -> ChangeAssessmentEnvelope
where
    S: SnapshotWorkspaceProvider,
    C: ChangedFilesProvider,
{
    let snapshots = resolved_snapshots(&input);
    let changed = match changed_files_provider.changed_files(&input.base, &input.head) {
        Ok(paths) => paths,
        Err(error) => {
            return empty_envelope(
                input.evaluation_date,
                snapshots,
                AssessmentCompleteness::Error,
                AssessmentOutcome::NotEvaluated,
                changed_set_diagnostic(error),
            );
        }
    };
    let changed_set = changed
        .iter()
        .map(|path| path.as_str().to_string())
        .collect::<BTreeSet<_>>();

    let head_workspace = match snapshot_provider.checkout(&input.head) {
        Ok(workspace) => workspace,
        Err(error) => {
            return empty_envelope(
                input.evaluation_date,
                snapshots,
                AssessmentCompleteness::Error,
                AssessmentOutcome::NotEvaluated,
                snapshot_diagnostic(error, &changed_set),
            );
        }
    };
    let head_config = match load_snapshot_config(head_workspace.path()) {
        Ok(config) => config,
        Err(message) => {
            return invalid_head_envelope(input.evaluation_date, snapshots, message, &changed_set);
        }
    };
    let head_sources = source_inventory(head_workspace.path(), &head_config.parsed.docs_path);
    let head_compile = compile_snapshot(
        head_workspace.path(),
        &head_config.parsed,
        input.evaluation_date,
    );
    if head_compile.has_errors() || head_compile.artifacts.is_none() {
        return head_compile_error_envelope(
            input.evaluation_date,
            snapshots,
            &head_config,
            head_workspace.path(),
            head_compile.diagnostics,
            &changed_set,
        );
    }

    let base_workspace = match snapshot_provider.checkout(&input.base) {
        Ok(workspace) => workspace,
        Err(error) => {
            return partial_envelope(
                input.evaluation_date,
                snapshots,
                &head_config,
                &head_compile,
                head_workspace.path(),
                changed,
                vec![snapshot_diagnostic(error, &changed_set)],
            );
        }
    };
    let base_config = match load_snapshot_config(base_workspace.path()) {
        Ok(config) => config,
        Err(message) => {
            return partial_envelope(
                input.evaluation_date,
                snapshots,
                &head_config,
                &head_compile,
                head_workspace.path(),
                changed,
                vec![diagnostic_record(
                    DiagnosticCode::AssessmentBasePartial,
                    Severity::Error,
                    message,
                    None,
                    &changed_set,
                )],
            );
        }
    };
    let base_sources = source_inventory(base_workspace.path(), &base_config.parsed.docs_path);
    let base_compile = compile_snapshot(
        base_workspace.path(),
        &base_config.parsed,
        input.evaluation_date,
    );
    if base_compile.has_errors() || base_compile.artifacts.is_none() {
        let diagnostics = base_compile
            .diagnostics
            .iter()
            .map(|diagnostic| project_diagnostic(diagnostic, base_workspace.path(), &changed_set))
            .chain(std::iter::once(diagnostic_record(
                DiagnosticCode::AssessmentBasePartial,
                Severity::Error,
                "comparison-base AgentDoc Source did not compile cleanly".to_string(),
                None,
                &changed_set,
            )))
            .collect();
        return partial_envelope(
            input.evaluation_date,
            snapshots,
            &head_config,
            &head_compile,
            head_workspace.path(),
            changed,
            diagnostics,
        );
    }

    complete_envelope(CompleteInput {
        evaluation_date: input.evaluation_date,
        snapshots,
        head_root: head_workspace.path(),
        base_config,
        head_config,
        base_sources,
        head_sources,
        base_compile,
        head_compile,
        changed,
    })
}

struct LoadedConfig {
    parsed: ParsedProjectConfig,
    normalized_json: Vec<u8>,
    sha256: String,
}

#[derive(Serialize)]
struct NormalizedConfig<'a> {
    docs_path: String,
    outputs: Vec<String>,
    embeddings_provider: &'a str,
    mcp_patch_apply_enabled: bool,
    assessment_exclude_paths: &'a [String],
}

fn load_snapshot_config(root: &Path) -> Result<LoadedConfig, String> {
    let path = root.join(CONFIG_PATH);
    let text = fs::read_to_string(&path)
        .map_err(|error| format!("could not read {CONFIG_PATH}: {error}"))?;
    let parsed = parse_project_config(&text).map_err(config_error_message)?;
    let normalized = NormalizedConfig {
        docs_path: path_string(&parsed.docs_path),
        outputs: configured_outputs(&parsed),
        embeddings_provider: match parsed.embeddings_provider {
            EmbeddingsProvider::Local => "local",
            EmbeddingsProvider::Deterministic => "deterministic",
            EmbeddingsProvider::None => "none",
        },
        mcp_patch_apply_enabled: parsed.mcp_patch_apply_enabled,
        assessment_exclude_paths: &parsed.assessment_exclude_paths,
    };
    let normalized_json = serde_json::to_vec(&normalized)
        .map_err(|error| format!("could not normalize {CONFIG_PATH}: {error}"))?;
    let sha256 = sha256_prefixed(&normalized_json);
    Ok(LoadedConfig {
        parsed,
        normalized_json,
        sha256,
    })
}

fn config_error_message(error: ProjectConfigDocumentError) -> String {
    format!("invalid {CONFIG_PATH}: {error}")
}

fn compile_snapshot(root: &Path, config: &ParsedProjectConfig, date: NaiveDate) -> CompileResult {
    let docs_root = root.join(&config.docs_path);
    let provider = FsSourceProvider::for_project(docs_root.clone(), root.to_path_buf(), docs_root);
    let anchor = FsEvidenceFileReader::new(root.to_path_buf());
    compile_with_provider_anchored_for_date(&provider, &anchor, date)
}

struct CompleteInput<'a> {
    evaluation_date: NaiveDate,
    snapshots: AssessmentSnapshots,
    head_root: &'a Path,
    base_config: LoadedConfig,
    head_config: LoadedConfig,
    base_sources: BTreeSet<String>,
    head_sources: BTreeSet<String>,
    base_compile: CompileResult,
    head_compile: CompileResult,
    changed: Vec<RelPath>,
}

fn complete_envelope(input: CompleteInput<'_>) -> ChangeAssessmentEnvelope {
    let base_objects = graph_objects(&input.base_compile);
    let head_objects = graph_objects(&input.head_compile);
    let diff = ObjectDiff::compute(&base_objects, &head_objects);
    let changed_ids = diff
        .created
        .iter()
        .map(|node| node.id.as_str())
        .chain(diff.changed.iter().map(|change| change.id.as_str()))
        .collect::<BTreeSet<_>>();
    let source_union = input
        .base_sources
        .union(&input.head_sources)
        .cloned()
        .collect::<BTreeSet<_>>();
    let base_outputs = configured_outputs(&input.base_config.parsed);
    let policy = policy_projection(&input.base_config.parsed, &input.head_config.parsed);
    let policy_changed = policy.0 != policy.1;

    let paths = classify_paths(
        &input.changed,
        &head_objects,
        &source_union,
        &base_outputs,
        &input.base_config.parsed.assessment_exclude_paths,
    );
    let object_reasons = reasons_by_object(&paths);
    let objects = head_objects
        .iter()
        .filter_map(|node| {
            object_reasons.get(node.id.as_str()).map(|reasons| {
                assessment_object(
                    node,
                    if changed_ids.contains(node.id.as_str()) {
                        "yes"
                    } else {
                        "no"
                    },
                    reasons.clone(),
                )
            })
        })
        .collect::<Vec<_>>();
    let knowledge_changes = knowledge_changes(&diff);
    let (reviewers, obligations) =
        review_requirements(&objects, &knowledge_changes, policy_changed);
    let signals = lifecycle_signals(&head_objects);
    let changed_set = input
        .changed
        .iter()
        .map(|path| path.as_str().to_string())
        .collect::<BTreeSet<_>>();
    let diagnostics = input
        .head_compile
        .diagnostics
        .iter()
        .map(|diagnostic| project_diagnostic(diagnostic, input.head_root, &changed_set))
        .collect::<Vec<_>>();
    let validation = validation_summary(&diagnostics);
    let summary = summary(&paths, objects.len());
    let has_uncovered = paths.iter().any(|path| {
        matches!(
            path.classification,
            PathClassification::Uncovered | PathClassification::Provisional
        )
    });
    let authoritative_impact = objects
        .iter()
        .any(|object| object.authority == "authoritative");
    let outcome = if has_uncovered {
        AssessmentOutcome::Uncovered
    } else if authoritative_impact
        || !diff.is_empty()
        || policy_changed
        || !reviewers.is_empty()
        || !obligations.is_empty()
        || !signals.is_empty()
    {
        AssessmentOutcome::ReviewRequired
    } else {
        AssessmentOutcome::Pass
    };
    let Some(head_artifacts) = input.head_compile.artifacts.as_ref() else {
        return empty_envelope(
            input.evaluation_date,
            input.snapshots,
            AssessmentCompleteness::Error,
            AssessmentOutcome::NotEvaluated,
            diagnostic_record(
                DiagnosticCode::AssessmentGraphFailed,
                Severity::Error,
                "head graph artifact is unavailable".to_string(),
                None,
                &BTreeSet::new(),
            ),
        );
    };
    let graph_json = &head_artifacts.graph_json;
    let graph_schema_version = serde_json::from_str::<serde_json::Value>(graph_json)
        .ok()
        .and_then(|value| value["schema_version"].as_str().map(str::to_string));
    let object_set = head_objects
        .iter()
        .map(|node| ObjectDigest {
            id: &node.id,
            content_hash: &node.content_hash,
        })
        .collect::<Vec<_>>();
    let object_set_json = compact_json(&object_set);
    let assessment_config = complete_config(&input.base_config, &input.head_config);

    ChangeAssessmentEnvelope {
        schema_version: CHANGE_ASSESSMENT_SCHEMA_VERSION,
        completeness: AssessmentCompleteness::Complete,
        outcome,
        evaluation_date: input.evaluation_date.to_string(),
        snapshots: input.snapshots,
        knowledge_snapshot: KnowledgeSnapshot {
            status: "available".to_string(),
            graph_schema_version,
            graph_sha256: Some(sha256_prefixed(graph_json.as_bytes())),
            object_set_sha256: Some(sha256_prefixed(&object_set_json)),
            docs_path: Some(path_string(&input.head_config.parsed.docs_path)),
        },
        assessment_config,
        summary,
        validation,
        paths: available(paths),
        objects: available(objects),
        knowledge_changes: available(knowledge_changes),
        policy_changes: PolicyChanges {
            status: "available".to_string(),
            changed: policy_changed,
            changed_fields: policy_fields(&policy.0, &policy.1),
        },
        required_reviewers: reviewers,
        proof_obligations: obligations,
        signals,
        diagnostics,
    }
}

#[derive(Serialize)]
struct ObjectDigest<'a> {
    id: &'a str,
    content_hash: &'a str,
}

fn graph_objects(result: &CompileResult) -> Vec<GraphKnowledgeObjectNode> {
    let Some(artifacts) = result.artifacts.as_ref() else {
        return Vec::new();
    };
    let Ok(document) = serde_json::from_str::<GraphArtifactDocument>(&artifacts.graph_json) else {
        return Vec::new();
    };
    document
        .nodes
        .into_iter()
        .filter_map(|node| match node {
            GraphNode::KnowledgeObject(object) => Some(object),
            _ => None,
        })
        .collect()
}

fn classify_paths(
    changed: &[RelPath],
    objects: &[GraphKnowledgeObjectNode],
    sources: &BTreeSet<String>,
    generated_outputs: &[String],
    configured_exclusions: &[String],
) -> Vec<AssessedPath> {
    changed
        .iter()
        .map(|path| {
            let value = path.as_str();
            let excluded = if sources.contains(value) {
                Some("knowledge_source".to_string())
            } else if value == CONFIG_PATH {
                Some("configuration".to_string())
            } else if let Some(output) = generated_outputs
                .iter()
                .find(|output| path_matches_exclusion(value, output))
            {
                Some(format!("generated_output:{output}"))
            } else {
                configured_exclusions
                    .iter()
                    .find(|entry| path_matches_exclusion(value, entry))
                    .map(|entry| format!("configured_exclusion:{entry}"))
            };
            if let Some(reason) = excluded {
                return AssessedPath {
                    path: value.to_string(),
                    classification: PathClassification::Excluded,
                    exclusion_reason: Some(reason),
                    matches: Vec::new(),
                };
            }
            let matches = matches_for_path(value, objects);
            let authoritative = matches.iter().any(|entry| {
                objects.iter().any(|node| {
                    node.id == entry.object_id
                        && is_authoritative_subject(&node.kind, node.status.as_deref())
                })
            });
            let classification = if authoritative {
                PathClassification::Covered
            } else if matches.is_empty() {
                PathClassification::Uncovered
            } else {
                PathClassification::Provisional
            };
            AssessedPath {
                path: value.to_string(),
                classification,
                exclusion_reason: None,
                matches,
            }
        })
        .collect()
}

fn matches_for_path(path: &str, objects: &[GraphKnowledgeObjectNode]) -> Vec<AssessmentMatch> {
    let source_paths = objects
        .iter()
        .filter(|node| node.kind == "source")
        .filter_map(|node| {
            node.fields
                .get("path")
                .map(|value| (node.id.as_str(), value.as_str()))
        })
        .collect::<BTreeMap<_, _>>();
    let mut matches = BTreeSet::new();
    for node in objects {
        if node.impacts.iter().any(|value| value == path) {
            matches.insert(AssessmentMatch {
                object_id: node.id.clone(),
                reason: "impacts_path".to_string(),
                via_source_object: None,
            });
        }
        for evidence in &node.evidence {
            if matches!(evidence.kind.as_str(), "source_code" | "test")
                && evidence.value.as_deref() == Some(path)
            {
                matches.insert(AssessmentMatch {
                    object_id: node.id.clone(),
                    reason: "evidence_path".to_string(),
                    via_source_object: None,
                });
            }
            if let Some(reference) = &evidence.reference
                && source_paths.get(reference.as_str()) == Some(&path)
            {
                matches.insert(AssessmentMatch {
                    object_id: node.id.clone(),
                    reason: "evidence_path".to_string(),
                    via_source_object: Some(reference.clone()),
                });
            }
        }
    }
    matches.into_iter().collect()
}

pub(crate) fn is_authoritative_subject(kind: &str, status: Option<&str>) -> bool {
    matches!(
        (kind, status),
        ("claim", Some("verified"))
            | ("decision", Some("accepted"))
            | ("api", Some("verified"))
            | ("policy", Some("active"))
            | ("procedure", Some("verified"))
    )
}

fn reasons_by_object(paths: &[AssessedPath]) -> BTreeMap<&str, Vec<AssessmentObjectReason>> {
    let mut result: BTreeMap<&str, Vec<AssessmentObjectReason>> = BTreeMap::new();
    for path in paths {
        for entry in &path.matches {
            result
                .entry(entry.object_id.as_str())
                .or_default()
                .push(AssessmentObjectReason {
                    path: path.path.clone(),
                    reason: entry.reason.clone(),
                    via_source_object: entry.via_source_object.clone(),
                });
        }
    }
    result
}

fn assessment_object(
    node: &GraphKnowledgeObjectNode,
    changed_in_pr: &str,
    reasons: Vec<AssessmentObjectReason>,
) -> AssessmentObject {
    AssessmentObject {
        id: node.id.clone(),
        kind: node.kind.clone(),
        authored_status: node.status.clone(),
        effective_status: node
            .effective_status
            .clone()
            .or_else(|| node.status.clone()),
        content_hash: node.content_hash.clone(),
        owner: node.fields.get("owner").cloned(),
        reviewers: reviewers_of(node),
        evidence_quality: node.evidence_quality.clone(),
        source: source(&node.source_span),
        authority: authority(node).to_string(),
        changed_in_pr: changed_in_pr.to_string(),
        reasons,
    }
}

fn knowledge_changes(diff: &ObjectDiff) -> KnowledgeChanges {
    KnowledgeChanges {
        created: diff
            .created
            .iter()
            .map(|node| knowledge_change(node, None, Some(&node.content_hash), "created"))
            .collect(),
        changed: diff
            .changed
            .iter()
            .map(|change| {
                knowledge_change(
                    &change.head,
                    Some(&change.base.content_hash),
                    Some(&change.head.content_hash),
                    "changed",
                )
            })
            .collect(),
        deleted: diff
            .deleted
            .iter()
            .map(|node| knowledge_change(node, Some(&node.content_hash), None, "deleted"))
            .collect(),
    }
}

fn knowledge_change(
    node: &GraphKnowledgeObjectNode,
    base_hash: Option<&String>,
    head_hash: Option<&String>,
    reason: &str,
) -> KnowledgeChange {
    KnowledgeChange {
        id: node.id.clone(),
        kind: node.kind.clone(),
        authority: authority(node).to_string(),
        base_content_hash: base_hash.cloned(),
        head_content_hash: head_hash.cloned(),
        authored_status: node.status.clone(),
        effective_status: node
            .effective_status
            .clone()
            .or_else(|| node.status.clone()),
        owner: node.fields.get("owner").cloned(),
        reviewers: reviewers_of(node),
        source: source(&node.source_span),
        reason: reason.to_string(),
    }
}

fn review_requirements(
    objects: &[AssessmentObject],
    changes: &KnowledgeChanges,
    policy_changed: bool,
) -> (Vec<AssessmentReviewer>, Vec<AssessmentObligation>) {
    let mut reviewers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    let mut obligations = BTreeMap::new();
    for object in objects
        .iter()
        .filter(|object| object.authority == "authoritative")
    {
        for owner in &object.reviewers {
            reviewers
                .entry(owner.clone())
                .or_default()
                .insert(object.id.clone());
        }
        obligations.insert(
            object.id.clone(),
            obligation(&object.id, &object.kind, "impacted"),
        );
    }
    for change in changes
        .created
        .iter()
        .chain(&changes.changed)
        .chain(&changes.deleted)
        .filter(|change| change.authority == "authoritative")
    {
        for owner in &change.reviewers {
            reviewers
                .entry(owner.clone())
                .or_default()
                .insert(change.id.clone());
        }
        obligations.insert(
            change.id.clone(),
            obligation(&change.id, &change.kind, &change.reason),
        );
    }
    if policy_changed {
        reviewers
            .entry("agentdoc-config-owner".to_string())
            .or_default()
            .insert(CONFIG_PATH.to_string());
        obligations.insert(
            CONFIG_PATH.to_string(),
            AssessmentObligation {
                object_id: CONFIG_PATH.to_string(),
                kind: "assessment_policy".to_string(),
                reason: "Review the proposed assessment policy change.".to_string(),
                required_evidence: vec!["human_review".to_string()],
            },
        );
    }
    (
        reviewers
            .into_iter()
            .map(|(owner, object_ids)| AssessmentReviewer {
                owner,
                object_ids: object_ids.into_iter().collect(),
            })
            .collect(),
        obligations.into_values().collect(),
    )
}

fn obligation(id: &str, kind: &str, disposition: &str) -> AssessmentObligation {
    AssessmentObligation {
        object_id: id.to_string(),
        kind: kind.to_string(),
        reason: format!("Review {disposition} authoritative {kind} `{id}`."),
        required_evidence: match kind {
            "policy" | "decision" => vec!["human_review".to_string()],
            _ => vec!["source_code".to_string()],
        },
    }
}

fn lifecycle_signals(objects: &[GraphKnowledgeObjectNode]) -> Vec<AssessmentSignal> {
    objects
        .iter()
        .filter_map(|node| {
            node.effective_status
                .as_ref()
                .map(|status| AssessmentSignal {
                    object_id: node.id.clone(),
                    kind: "lifecycle".to_string(),
                    signal: status.clone(),
                })
        })
        .collect()
}

fn complete_config(base: &LoadedConfig, head: &LoadedConfig) -> AssessmentConfig {
    let (effective, proposed) = policy_projection(&base.parsed, &head.parsed);
    let effective_json = compact_json(&effective);
    let proposed_json = compact_json(&proposed);
    #[derive(Serialize)]
    struct BoundConfig<'a> {
        comparison_base: &'a [u8],
        head: &'a [u8],
        effective_policy: &'a PolicyDigest,
        proposed_policy: &'a PolicyDigest,
    }
    let bound = compact_json(&BoundConfig {
        comparison_base: &base.normalized_json,
        head: &head.normalized_json,
        effective_policy: &effective,
        proposed_policy: &proposed,
    });
    AssessmentConfig {
        comparison_base: snapshot_config(base),
        head: snapshot_config(head),
        policy: AssessmentPolicy {
            status: "available".to_string(),
            effective_source_snapshot: "comparison_base".to_string(),
            exclude_paths: effective.exclude_paths,
            generated_outputs: effective.generated_outputs,
            effective_sha256: Some(sha256_prefixed(&effective_json)),
            proposed_head_sha256: Some(sha256_prefixed(&proposed_json)),
        },
        sha256: Some(sha256_prefixed(&bound)),
    }
}

fn snapshot_config(config: &LoadedConfig) -> SnapshotConfig {
    SnapshotConfig {
        status: "available".to_string(),
        source: "file".to_string(),
        docs_path: Some(path_string(&config.parsed.docs_path)),
        sha256: Some(config.sha256.clone()),
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
struct PolicyDigest {
    exclude_paths: Vec<String>,
    generated_outputs: Vec<String>,
}

fn policy_projection(
    base: &ParsedProjectConfig,
    head: &ParsedProjectConfig,
) -> (PolicyDigest, PolicyDigest) {
    (
        PolicyDigest {
            exclude_paths: base.assessment_exclude_paths.clone(),
            generated_outputs: configured_outputs(base),
        },
        PolicyDigest {
            exclude_paths: head.assessment_exclude_paths.clone(),
            generated_outputs: configured_outputs(head),
        },
    )
}

fn policy_fields(base: &PolicyDigest, head: &PolicyDigest) -> Vec<String> {
    let mut fields = Vec::new();
    if base.exclude_paths != head.exclude_paths {
        fields.push("exclude_paths".to_string());
    }
    if base.generated_outputs != head.generated_outputs {
        fields.push("generated_outputs".to_string());
    }
    fields
}

fn configured_outputs(config: &ParsedProjectConfig) -> Vec<String> {
    let mut paths = BTreeSet::new();
    if let Some(dir) = &config.outputs.dir
        && let Some(value) = portable_output(dir, true)
    {
        paths.insert(value);
    }
    for path in [
        config.outputs.html.as_ref(),
        config.outputs.graph.as_ref(),
        config.outputs.search.as_ref(),
    ]
    .into_iter()
    .flatten()
    {
        if let Some(value) = portable_output(path, false) {
            paths.insert(value);
        }
    }
    paths.into_iter().collect()
}

fn portable_output(path: &Path, directory: bool) -> Option<String> {
    if path.is_absolute() {
        return None;
    }
    let value = path.to_str()?.replace(std::path::MAIN_SEPARATOR, "/");
    if value.is_empty() || value == "." || value.starts_with("../") {
        return None;
    }
    Some(if directory {
        format!("{}/", value.trim_end_matches('/'))
    } else {
        value
    })
}

fn source_inventory(root: &Path, docs_path: &Path) -> BTreeSet<String> {
    let docs_root = root.join(docs_path);
    let mut result = BTreeSet::new();
    collect_sources(root, &docs_root, &mut result);
    result
}

fn collect_sources(project_root: &Path, current: &Path, result: &mut BTreeSet<String>) {
    let Ok(entries) = fs::read_dir(current) else {
        return;
    };
    let mut entries = entries.filter_map(Result::ok).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        if path.is_dir() {
            collect_sources(project_root, &path, result);
        } else if matches!(
            path.extension().and_then(|value| value.to_str()),
            Some("adoc" | "md")
        ) && let Ok(relative) = path.strip_prefix(project_root)
        {
            result.insert(path_string(relative));
        }
    }
}

fn path_matches_exclusion(path: &str, exclusion: &str) -> bool {
    if exclusion.ends_with('/') {
        path.starts_with(exclusion)
    } else {
        path == exclusion
    }
}

fn summary(paths: &[AssessedPath], impacted_objects: usize) -> AssessmentSummary {
    let mut summary = AssessmentSummary {
        changed_paths: paths.len(),
        impacted_objects,
        ..AssessmentSummary::default()
    };
    for path in paths {
        match path.classification {
            PathClassification::Covered => summary.covered += 1,
            PathClassification::Provisional => summary.provisional += 1,
            PathClassification::Uncovered => summary.uncovered += 1,
            PathClassification::Excluded => summary.excluded += 1,
        }
    }
    summary
}

fn validation_summary(diagnostics: &[AssessmentDiagnostic]) -> AssessmentValidation {
    AssessmentValidation {
        errors_full: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "error")
            .count(),
        errors_changed: diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == "error" && diagnostic.changed_in_pr == "yes"
            })
            .count(),
        errors_unchanged: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "error" && diagnostic.changed_in_pr == "no")
            .count(),
        errors_unattributed: diagnostics
            .iter()
            .filter(|diagnostic| {
                diagnostic.severity == "error" && diagnostic.changed_in_pr == "unknown"
            })
            .count(),
        warnings: diagnostics
            .iter()
            .filter(|diagnostic| diagnostic.severity == "warning")
            .count(),
    }
}

fn partial_envelope(
    date: NaiveDate,
    snapshots: AssessmentSnapshots,
    head_config: &LoadedConfig,
    head_compile: &CompileResult,
    head_root: &Path,
    changed: Vec<RelPath>,
    mut diagnostics: Vec<AssessmentDiagnostic>,
) -> ChangeAssessmentEnvelope {
    let head_objects = graph_objects(head_compile);
    let Some(head_artifacts) = head_compile.artifacts.as_ref() else {
        return empty_envelope(
            date,
            snapshots,
            AssessmentCompleteness::Error,
            AssessmentOutcome::NotEvaluated,
            diagnostic_record(
                DiagnosticCode::AssessmentGraphFailed,
                Severity::Error,
                "head graph artifact is unavailable".to_string(),
                None,
                &BTreeSet::new(),
            ),
        );
    };
    let graph_json = &head_artifacts.graph_json;
    let object_set = head_objects
        .iter()
        .map(|node| ObjectDigest {
            id: &node.id,
            content_hash: &node.content_hash,
        })
        .collect::<Vec<_>>();
    let object_set_json = compact_json(&object_set);
    let changed_set = changed
        .iter()
        .map(|path| path.as_str().to_string())
        .collect::<BTreeSet<_>>();
    diagnostics.extend(
        head_compile
            .diagnostics
            .iter()
            .map(|diagnostic| project_diagnostic(diagnostic, head_root, &changed_set)),
    );
    diagnostics.sort_by(|left, right| {
        (&left.code, &left.message, &left.object_id).cmp(&(
            &right.code,
            &right.message,
            &right.object_id,
        ))
    });
    let mut envelope = empty_envelope(
        date,
        snapshots,
        AssessmentCompleteness::Partial,
        AssessmentOutcome::NotEvaluated,
        diagnostic_record(
            DiagnosticCode::AssessmentBasePartial,
            Severity::Error,
            "comparison-base facts are unavailable; assessment is partial".to_string(),
            None,
            &BTreeSet::new(),
        ),
    );
    envelope.diagnostics = diagnostics;
    envelope.summary.changed_paths = changed.len();
    envelope.knowledge_snapshot = KnowledgeSnapshot {
        status: "available".to_string(),
        graph_schema_version: Some("adoc.graph.v5".to_string()),
        graph_sha256: Some(sha256_prefixed(graph_json.as_bytes())),
        object_set_sha256: Some(sha256_prefixed(&object_set_json)),
        docs_path: Some(path_string(&head_config.parsed.docs_path)),
    };
    envelope.assessment_config.head = snapshot_config(head_config);
    envelope.paths = Availability {
        status: "unavailable".to_string(),
        value: None,
    };
    envelope.objects = available(
        head_objects
            .iter()
            .map(|node| assessment_object(node, "unknown", Vec::new()))
            .collect(),
    );
    envelope
}

fn head_compile_error_envelope(
    date: NaiveDate,
    snapshots: AssessmentSnapshots,
    head_config: &LoadedConfig,
    head_root: &Path,
    diagnostics: Vec<Diagnostic>,
    changed: &BTreeSet<String>,
) -> ChangeAssessmentEnvelope {
    let mut envelope = empty_envelope(
        date,
        snapshots,
        AssessmentCompleteness::Error,
        AssessmentOutcome::Invalid,
        diagnostic_record(
            DiagnosticCode::AssessmentHeadInvalid,
            Severity::Error,
            "head AgentDoc Source did not compile cleanly".to_string(),
            None,
            changed,
        ),
    );
    envelope.assessment_config.head = snapshot_config(head_config);
    envelope.diagnostics = diagnostics
        .iter()
        .map(|diagnostic| project_diagnostic(diagnostic, head_root, changed))
        .chain(std::iter::once(diagnostic_record(
            DiagnosticCode::AssessmentGraphFailed,
            Severity::Error,
            "head graph artifact is unavailable".to_string(),
            None,
            changed,
        )))
        .collect();
    envelope.validation = validation_summary(&envelope.diagnostics);
    envelope
}

fn invalid_head_envelope(
    date: NaiveDate,
    snapshots: AssessmentSnapshots,
    message: String,
    changed: &BTreeSet<String>,
) -> ChangeAssessmentEnvelope {
    empty_envelope(
        date,
        snapshots,
        AssessmentCompleteness::Error,
        AssessmentOutcome::Invalid,
        diagnostic_record(
            DiagnosticCode::AssessmentHeadInvalid,
            Severity::Error,
            message,
            None,
            changed,
        ),
    )
}

fn empty_envelope(
    date: NaiveDate,
    snapshots: AssessmentSnapshots,
    completeness: AssessmentCompleteness,
    outcome: AssessmentOutcome,
    diagnostic: AssessmentDiagnostic,
) -> ChangeAssessmentEnvelope {
    ChangeAssessmentEnvelope {
        schema_version: CHANGE_ASSESSMENT_SCHEMA_VERSION,
        completeness,
        outcome,
        evaluation_date: date.to_string(),
        snapshots,
        knowledge_snapshot: KnowledgeSnapshot {
            status: "unavailable".to_string(),
            graph_schema_version: None,
            graph_sha256: None,
            object_set_sha256: None,
            docs_path: None,
        },
        assessment_config: AssessmentConfig {
            comparison_base: unavailable_config(),
            head: unavailable_config(),
            policy: AssessmentPolicy {
                status: "unavailable".to_string(),
                effective_source_snapshot: "comparison_base".to_string(),
                exclude_paths: Vec::new(),
                generated_outputs: Vec::new(),
                effective_sha256: None,
                proposed_head_sha256: None,
            },
            sha256: None,
        },
        summary: AssessmentSummary::default(),
        validation: AssessmentValidation::default(),
        paths: unavailable(),
        objects: unavailable(),
        knowledge_changes: unavailable(),
        policy_changes: PolicyChanges {
            status: "unavailable".to_string(),
            changed: false,
            changed_fields: Vec::new(),
        },
        required_reviewers: Vec::new(),
        proof_obligations: Vec::new(),
        signals: Vec::new(),
        diagnostics: vec![diagnostic],
    }
}

fn unavailable_config() -> SnapshotConfig {
    SnapshotConfig {
        status: "unavailable".to_string(),
        source: "file".to_string(),
        docs_path: None,
        sha256: None,
    }
}

fn available<T>(value: T) -> Availability<T> {
    Availability {
        status: "available".to_string(),
        value: Some(value),
    }
}

fn unavailable<T>() -> Availability<T> {
    Availability {
        status: "unavailable".to_string(),
        value: None,
    }
}

fn resolved_snapshots(input: &ResolvedAssessmentInput) -> AssessmentSnapshots {
    AssessmentSnapshots {
        requested_base: AssessmentSnapshot {
            requested_ref: Some(input.requested_base_ref.clone()),
            resolved_commit: Some(input.requested_base_commit.clone()),
            immutable: true,
            strategy: None,
            worktree_state: None,
        },
        comparison_base: AssessmentSnapshot {
            requested_ref: None,
            resolved_commit: Some(input.comparison_base_commit.clone()),
            immutable: true,
            strategy: Some("merge_base".to_string()),
            worktree_state: None,
        },
        head: AssessmentSnapshot {
            requested_ref: input.head_ref.clone(),
            resolved_commit: Some(input.head_commit.clone()),
            immutable: input.head_ref.is_some(),
            strategy: None,
            worktree_state: input
                .worktree_dirty
                .map(|dirty| if dirty { "dirty" } else { "clean" }.to_string()),
        },
    }
}

fn unresolved_snapshots(input: &ChangeAssessmentInput) -> AssessmentSnapshots {
    AssessmentSnapshots {
        requested_base: AssessmentSnapshot {
            requested_ref: Some(input.base_ref.clone()),
            resolved_commit: None,
            immutable: true,
            strategy: None,
            worktree_state: None,
        },
        comparison_base: AssessmentSnapshot {
            requested_ref: None,
            resolved_commit: None,
            immutable: true,
            strategy: Some("merge_base".to_string()),
            worktree_state: None,
        },
        head: AssessmentSnapshot {
            requested_ref: input.head_ref.clone(),
            resolved_commit: None,
            immutable: input.head_ref.is_some(),
            strategy: None,
            worktree_state: None,
        },
    }
}

fn changed_set_diagnostic(error: ChangedFilesError) -> AssessmentDiagnostic {
    let code = if matches!(error, ChangedFilesError::InvalidPath { .. }) {
        DiagnosticCode::AssessmentInvalidChangedPath
    } else {
        DiagnosticCode::AssessmentChangedSetFailed
    };
    diagnostic_record(
        code,
        Severity::Error,
        error.to_string(),
        None,
        &BTreeSet::new(),
    )
}

fn snapshot_diagnostic(error: SnapshotError, changed: &BTreeSet<String>) -> AssessmentDiagnostic {
    diagnostic_record(
        DiagnosticCode::AssessmentSnapshotFailed,
        Severity::Error,
        error.to_string(),
        None,
        changed,
    )
}

fn project_diagnostic(
    diagnostic: &Diagnostic,
    root: &Path,
    changed: &BTreeSet<String>,
) -> AssessmentDiagnostic {
    let source = diagnostic.span.as_ref().map(|span| {
        let path = span
            .file
            .strip_prefix(root)
            .unwrap_or(&span.file)
            .to_path_buf();
        AssessmentSource {
            path: path_string(&path),
            line: span.start.line,
            column: span.start.column,
        }
    });
    diagnostic_record(
        diagnostic.code,
        diagnostic.severity,
        diagnostic.message.clone(),
        source,
        changed,
    )
    .with_object_id(diagnostic.object_id.clone())
}

fn diagnostic_record(
    code: DiagnosticCode,
    severity: Severity,
    message: String,
    source: Option<AssessmentSource>,
    changed: &BTreeSet<String>,
) -> AssessmentDiagnostic {
    let changed_in_pr = source.as_ref().map_or("unknown", |source| {
        if changed.contains(source.path.as_str()) {
            "yes"
        } else {
            "no"
        }
    });
    AssessmentDiagnostic {
        code: code.as_str().to_string(),
        severity: severity.to_string(),
        message,
        source,
        object_id: None,
        changed_in_pr: changed_in_pr.to_string(),
    }
}

impl AssessmentDiagnostic {
    fn with_object_id(mut self, object_id: Option<String>) -> Self {
        self.object_id = object_id;
        self
    }
}

fn source(span: &crate::domain::graph::GraphSourceSpan) -> AssessmentSource {
    AssessmentSource {
        path: span.path.clone(),
        line: span.line,
        column: span.column,
    }
}

fn authority(node: &GraphKnowledgeObjectNode) -> &'static str {
    if is_authoritative_subject(&node.kind, node.status.as_deref()) {
        "authoritative"
    } else {
        "provisional"
    }
}

fn reviewers_of(node: &GraphKnowledgeObjectNode) -> Vec<String> {
    let mut reviewers = BTreeSet::new();
    if let Some(owner) = node.fields.get("owner") {
        reviewers.insert(owner.clone());
    }
    if let Some(decider) = node.fields.get("decided_by") {
        reviewers.insert(decider.clone());
    }
    reviewers.extend(node.approved_by.iter().cloned());
    reviewers.into_iter().collect()
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy()
        .replace(std::path::MAIN_SEPARATOR, "/")
}

fn compact_json<T: Serialize>(value: &T) -> Vec<u8> {
    match serde_json::to_vec(value) {
        Ok(bytes) => bytes,
        Err(_) => b"null".to_vec(),
    }
}

#[cfg(test)]
mod tests {
    use super::{is_authoritative_subject, path_matches_exclusion, reviewers_of};
    use crate::domain::review::object_diff::test_support::test_node;

    #[test]
    fn authority_table_is_closed_to_the_five_governing_pairs() {
        for pair in [
            ("claim", Some("verified")),
            ("decision", Some("accepted")),
            ("api", Some("verified")),
            ("policy", Some("active")),
            ("procedure", Some("verified")),
        ] {
            assert!(is_authoritative_subject(pair.0, pair.1), "{pair:?}");
        }
        for pair in [
            ("claim", Some("draft")),
            ("policy", Some("draft")),
            ("agent_instruction", Some("active")),
            ("constraint", None),
        ] {
            assert!(!is_authoritative_subject(pair.0, pair.1), "{pair:?}");
        }
    }

    #[test]
    fn directory_exclusions_are_component_boundary_aware() {
        assert!(path_matches_exclusion("vendor/lib.rs", "vendor/"));
        assert!(!path_matches_exclusion("vendorized/lib.rs", "vendor/"));
        assert!(path_matches_exclusion("generated.txt", "generated.txt"));
        assert!(!path_matches_exclusion(
            "generated.txt.bak",
            "generated.txt"
        ));
    }

    #[test]
    fn active_policy_uses_every_approver_as_a_required_reviewer() {
        let mut policy = test_node("billing.policy", "sha256:policy");
        policy.kind = "policy".to_string();
        policy.status = Some("active".to_string());
        policy.approved_by = vec!["security".to_string(), "architecture".to_string()];

        assert_eq!(reviewers_of(&policy), ["architecture", "security"]);
    }
}

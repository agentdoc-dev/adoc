use std::collections::BTreeMap;
use std::num::NonZeroUsize;
use std::path::PathBuf;
use std::time::Duration;

use adoc_core::{
    ArtifactInspection, ArtifactLoadStatus, ContradictionsEnvelope, Diagnostic, GraphDirection,
    GraphRelationKind, GraphTraversalEnvelope, ImpactedEnvelope, MigrateReportEnvelope,
    ObjectDiffEnvelope, PatchApplyResult, PatchCheckResult, ProseRecord, RetrievalEnvelope,
    RetrievalRecord, ReviewEnvelope, SearchRecordScope, StaleEnvelope,
};
use serde::Serialize;

use crate::{LocalContext, LocalError, PathPolicy};

mod artifact_commit;
mod changes;
mod project;
mod queries;
mod shared;

const DEFAULT_GRAPH_ARTIFACT_PATH: &str = "dist/docs.graph.json";
const DEFAULT_SEARCH_ARTIFACT_PATH: &str = "dist/docs.search.json";
const DEFAULT_HTML_ARTIFACT_PATH: &str = "dist/docs.html";
const PROJECT_STATUS_SCHEMA_VERSION: &str = "adoc.project.status.v0";
const INIT_CONFIG_PATH: &str = "agentdoc.config.yaml";
const INIT_INDEX_PATH: &str = "docs/index.adoc";
const INIT_CONFIG_TEMPLATE: &str = "\
version: 1
mode: strict
docs_path: docs
outputs:
  dir: dist
embeddings:
  provider: local
";

#[derive(Debug, Clone, Serialize)]
pub struct InitOutcome {
    pub created: Vec<PathBuf>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct CheckInput {
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct CheckOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct MigrateInput {
    pub path: Option<PathBuf>,
    pub write: bool,
    pub force: bool,
    /// V8.1.4: export strict prose-mode `.adoc` back to `.md` instead of
    /// importing (ADR-0043 §5).
    pub export: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct MigrateOutcome {
    pub report: MigrateReportEnvelope,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct BuildInput {
    pub path: Option<PathBuf>,
    pub out: Option<PathBuf>,
    pub no_embeddings: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildOutputs {
    pub html: PathBuf,
    pub graph: PathBuf,
    pub search: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct BuildOutcome {
    pub diagnostics: Vec<Diagnostic>,
    pub outputs: Option<BuildOutputs>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct WhyInput {
    pub object_id: String,
    pub artifact: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ResolvedRetrievalRecord {
    pub record: RetrievalRecord,
    pub related_statuses: BTreeMap<String, Option<String>>,
}

#[derive(Debug, Clone, Serialize)]
pub struct WhyOutcome {
    pub artifact: PathBuf,
    pub records: Vec<ResolvedRetrievalRecord>,
    pub diagnostics: Vec<Diagnostic>,
    #[serde(skip)]
    pub duration: Duration,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct GraphInput {
    pub object_id: String,
    pub artifact: Option<PathBuf>,
    pub relation: Option<GraphRelationKind>,
    pub direction: Option<GraphDirection>,
}

#[derive(Debug, Clone, Serialize)]
pub struct GraphOutcome {
    pub envelope: GraphTraversalEnvelope,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct StaleInput {
    pub artifact: Option<PathBuf>,
    /// `--within <N>d` horizon in days; `None` disables `expiring_soon`.
    pub within_days: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StaleOutcome {
    pub envelope: StaleEnvelope,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct ContradictionsInput {
    pub artifact: Option<PathBuf>,
    /// `--all`: include `resolved` and `dismissed` contradictions in the
    /// listing (never affects `contradicted_claims`).
    pub all: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ContradictionsOutcome {
    pub envelope: ContradictionsEnvelope,
    pub exit_code: i32,
}

/// V6.3 — the two mutually exclusive `adoc impacted-by` input shapes. The
/// XOR is enforced at the interface layer (clap / MCP argument validation);
/// this enum makes the exclusivity structural here.
#[derive(Debug, Clone)]
pub enum ImpactedChangedSet {
    /// Explicit repo-relative changed paths (`adoc impacted-by <path>...`).
    Paths(Vec<String>),
    /// Derive the changed set from git: `<git-ref>` vs the working tree
    /// (`adoc impacted-by --ref <git-ref>`).
    GitRef(String),
}

#[derive(Debug, Clone)]
pub struct ImpactedInput {
    pub artifact: Option<PathBuf>,
    pub changed: ImpactedChangedSet,
}

#[derive(Debug, Clone, Serialize)]
pub struct ImpactedOutcome {
    pub envelope: ImpactedEnvelope,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct SearchInput {
    pub query: String,
    pub artifact: Option<PathBuf>,
    pub search_artifact: Option<PathBuf>,
    pub semantic: bool,
    pub lexical: bool,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub source_path: Option<String>,
    pub related_to: Option<String>,
    pub relation: Option<GraphRelationKind>,
    pub direction: Option<GraphDirection>,
    pub top: NonZeroUsize,
    /// V1.7.1 (ADR-0040): which record types the search returns. Structural —
    /// the interface layers (clap conflicts, MCP argument validation) map
    /// their flag pairs onto this enum, so an invalid combination cannot
    /// reach the use case.
    pub scope: SearchRecordScope,
}

/// V1.7.1: one search result entry — a Knowledge Object record enriched with
/// its relation-target statuses, or a prose record (which has no relations to
/// enrich). Serializes with the same `record_type` tag as the wire envelope.
// Size asymmetry follows the `RetrievalEntry` precedent: the record carries
// all fields inline; boxing adds indirection for no wire benefit.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "record_type", rename_all = "snake_case")]
pub enum ResolvedSearchEntry {
    KnowledgeObject(ResolvedRetrievalRecord),
    Prose(ProseRecord),
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchOutcome {
    pub envelope: RetrievalEnvelope,
    pub records: Vec<ResolvedSearchEntry>,
    pub diagnostics: Vec<Diagnostic>,
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct PatchCheckInput {
    pub patch_path: PathBuf,
    pub artifact: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchCheckOutcome {
    #[serde(flatten)]
    pub result: PatchCheckResult,
    pub exit_code: i32,
}

/// V6.4 — patch source for `adoc patch --apply` and the MCP
/// `adoc_patch_apply` tool. Mirrors [`ReviewPatchSource`] (path vs inline
/// JSON) so the same driving adapters can populate either variant.
#[derive(Debug, Clone)]
pub enum PatchApplySource {
    Path(PathBuf),
    Inline(serde_json::Value),
}

#[derive(Debug, Clone)]
pub struct PatchApplyInput {
    pub patch: PatchApplySource,
    pub artifact: Option<PathBuf>,
    /// Recorded in the envelope's `trace.interface` (`"cli"` or `"mcp"`).
    pub interface: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct PatchApplyOutcome {
    #[serde(flatten)]
    pub result: PatchApplyResult,
    #[serde(skip)]
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct DiffInput {
    pub base_ref: String,
    pub head_ref: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiffOutcome {
    #[serde(flatten)]
    pub envelope: ObjectDiffEnvelope,
    #[serde(skip)]
    pub exit_code: i32,
}

#[derive(Debug, Clone)]
pub struct ReviewInput {
    pub base_ref: String,
    pub head_ref: Option<String>,
    /// V3.7 — optional patch source threaded through into
    /// [`adoc_core::review_with_patch`]. `None` produces the V3.3/V3.4/V3.6
    /// envelope unchanged.
    pub patch: Option<ReviewPatchSource>,
}

/// V3.7 — orchestration-layer patch source for `adoc review --patch` and the
/// equivalent MCP parameter. Mirrors the MCP `PatchInput` shape (path vs
/// inline JSON) so the same driving adapters can populate either variant.
#[derive(Debug, Clone)]
pub enum ReviewPatchSource {
    Path(PathBuf),
    Inline(serde_json::Value),
}

#[derive(Debug, Clone, Serialize)]
pub struct ReviewOutcome {
    #[serde(flatten)]
    pub envelope: ReviewEnvelope,
    #[serde(skip)]
    pub exit_code: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ProjectStatusRefresh {
    None,
    Check,
    Build,
}

#[derive(Debug, Clone)]
pub struct ProjectStatusInput {
    pub refresh: ProjectStatusRefresh,
    pub no_embeddings: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusConfig {
    pub discovered: bool,
    pub path: Option<PathBuf>,
    pub embeddings_provider: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusPaths {
    pub docs: PathBuf,
    pub html: Option<PathBuf>,
    pub graph: PathBuf,
    pub search: Option<PathBuf>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusRefreshReport {
    pub requested: ProjectStatusRefresh,
    pub exit_code: Option<i32>,
    pub diagnostics: Vec<Diagnostic>,
    pub outputs: Option<BuildOutputs>,
}

pub type ProjectArtifactLoadStatus = ArtifactLoadStatus;
pub type ProjectArtifactStatus = ArtifactInspection;

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusArtifacts {
    pub graph: ProjectArtifactStatus,
    pub search: ProjectArtifactStatus,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusReadiness {
    pub retrieval: bool,
    pub semantic_search: bool,
    pub patch_validation: bool,
    pub review: bool,
    /// V6.4 TB4 (ADR-0037): `true` only when the project opted into MCP
    /// patch apply via `mcp: { patch_apply: enabled }`. Agents check this
    /// before constructing a patch for apply.
    pub patch_apply_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ProjectStatusOutcome {
    pub schema_version: &'static str,
    pub project_root: PathBuf,
    pub config: ProjectStatusConfig,
    pub paths: ProjectStatusPaths,
    pub refresh: ProjectStatusRefreshReport,
    pub artifacts: ProjectStatusArtifacts,
    pub readiness: ProjectStatusReadiness,
    pub exit_code: i32,
}

/// The Local Workflow Layer's command surface: one method per local
/// operation, on the one type every adapter already holds. Bodies live in
/// the private `*_with_context` functions below; the `PathPolicy` generic
/// remains the test seam.
impl<P> LocalContext<P>
where
    P: PathPolicy,
{
    #[tracing::instrument(name = "adoc.init", level = "info", skip_all)]
    pub fn init(&self) -> Result<InitOutcome, LocalError> {
        project::init_with_context(self)
    }

    #[tracing::instrument(name = "adoc.check", level = "info", skip_all)]
    pub fn check(&self, input: CheckInput) -> Result<CheckOutcome, LocalError> {
        project::check_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.migrate", level = "info", skip_all)]
    pub fn migrate(&self, input: MigrateInput) -> Result<MigrateOutcome, LocalError> {
        project::migrate_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.build", level = "info", skip_all)]
    pub fn build(&self, input: BuildInput) -> Result<BuildOutcome, LocalError> {
        project::build_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.why", level = "info", skip_all)]
    pub fn why(&self, input: WhyInput) -> Result<WhyOutcome, LocalError> {
        queries::why_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.graph", level = "info", skip_all)]
    pub fn graph(&self, input: GraphInput) -> Result<GraphOutcome, LocalError> {
        queries::graph_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.stale", level = "info", skip_all)]
    pub fn stale(&self, input: StaleInput) -> Result<StaleOutcome, LocalError> {
        queries::stale_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.contradictions", level = "info", skip_all)]
    pub fn contradictions(
        &self,
        input: ContradictionsInput,
    ) -> Result<ContradictionsOutcome, LocalError> {
        queries::contradictions_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.impacted", level = "info", skip_all)]
    pub fn impacted(&self, input: ImpactedInput) -> Result<ImpactedOutcome, LocalError> {
        queries::impacted_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.search", level = "info", skip_all)]
    pub fn search(&self, input: SearchInput) -> Result<SearchOutcome, LocalError> {
        queries::search_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.patch_check", level = "info", skip_all)]
    pub fn patch_check(&self, input: PatchCheckInput) -> Result<PatchCheckOutcome, LocalError> {
        changes::patch_check_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.patch_apply", level = "info", skip_all)]
    pub fn patch_apply(&self, input: PatchApplyInput) -> Result<PatchApplyOutcome, LocalError> {
        changes::patch_apply_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.diff", level = "info", skip_all)]
    pub fn diff(&self, input: DiffInput) -> Result<DiffOutcome, LocalError> {
        changes::diff_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.review", level = "info", skip_all)]
    pub fn review(&self, input: ReviewInput) -> Result<ReviewOutcome, LocalError> {
        changes::review_with_context(self, input)
    }

    #[tracing::instrument(name = "adoc.project_status", level = "info", skip_all)]
    pub fn project_status(
        &self,
        input: ProjectStatusInput,
    ) -> Result<ProjectStatusOutcome, LocalError> {
        project::project_status_with_context(self, input)
    }
}

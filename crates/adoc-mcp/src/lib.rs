//! MCP adapter for local AgentDoc workflows.

use std::future::Future;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    GraphDirection, GraphRelationKind, PatchJsonInput, RetrievalEnvelope, check_patch_json,
};
use adoc_local::{
    BuildInput, BuildUseCase, CheckInput, CheckUseCase, ContradictionsInput, ContradictionsUseCase,
    DiffInput, DiffUseCase, GraphInput, GraphUseCase, ImpactedChangedSet, ImpactedInput,
    ImpactedUseCase, InitInput, InitUseCase, LocalContext, PatchCheckInput, PatchCheckUseCase,
    PathPolicy, ProjectConfig, ProjectRootPathPolicy, ProjectStatusInput, ProjectStatusRefresh,
    ProjectStatusUseCase, ReviewInput, ReviewPatchSource, ReviewUseCase, SearchInput,
    SearchUseCase, StaleInput, StaleUseCase, WhyInput, WhyUseCase,
};
use rmcp::{
    ServerHandler,
    handler::server::{router::tool::ToolRouter, wrapper::Parameters},
    model::{
        CallToolResult, ErrorData, GetPromptRequestParams, GetPromptResult, JsonObject,
        ListPromptsResult, ListResourcesResult, PaginatedRequestParams, Prompt,
        ReadResourceRequestParams, ReadResourceResult, Resource, ServerCapabilities, ServerInfo,
    },
    service::{MaybeSendFuture, RequestContext, RoleServer},
    tool, tool_handler, tool_router,
};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

mod envelope;
mod prompts;
mod resources;

use envelope::command_envelope;

const DEFAULT_GRAPH_ARTIFACT_PATH: &str = "dist/docs.graph.json";

#[derive(Debug, thiserror::Error)]
pub enum McpAdapterError {
    #[error(transparent)]
    Local(#[from] adoc_local::LocalError),

    #[error("error[mcp.invalid_arguments] {0}")]
    InvalidArguments(String),

    #[error("error[mcp.serialize] could not serialize tool result: {0}")]
    Serialize(#[from] serde_json::Error),
}

pub type McpAdapterResult<T> = Result<T, McpAdapterError>;

#[derive(Debug, Clone)]
pub struct AgentDocMcpServer {
    default_project_root: PathBuf,
    tool_router: ToolRouter<Self>,
}

impl AgentDocMcpServer {
    pub fn new(default_project_root: PathBuf) -> Self {
        Self {
            default_project_root,
            tool_router: Self::tool_router(),
        }
    }

    pub fn run_init(&self, params: InitParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = InitUseCase::new(context).run(InitInput)?;
        serde_json::to_value(command_envelope("adoc_init", outcome.exit_code, outcome))
            .map_err(Into::into)
    }

    pub fn run_check(&self, params: CheckParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = CheckUseCase::new(context).run(CheckInput { path: params.path })?;
        serde_json::to_value(command_envelope("adoc_check", outcome.exit_code, outcome))
            .map_err(Into::into)
    }

    pub fn run_build(&self, params: BuildParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = BuildUseCase::new(context).run(BuildInput {
            path: params.path,
            out: params.out,
            no_embeddings: params.no_embeddings,
        })?;
        serde_json::to_value(command_envelope("adoc_build", outcome.exit_code, outcome))
            .map_err(Into::into)
    }

    pub fn run_why(&self, params: WhyParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = WhyUseCase::new(context).run(WhyInput {
            object_id: params.object_id,
            artifact: params.artifact,
        })?;
        let envelope = RetrievalEnvelope::new(
            outcome
                .records
                .into_iter()
                .map(|record| record.record)
                .collect(),
            outcome.diagnostics,
        );
        serde_json::to_value(envelope).map_err(Into::into)
    }

    pub fn run_graph(&self, params: GraphParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = GraphUseCase::new(context).run(GraphInput {
            object_id: params.object_id,
            artifact: params.artifact,
            relation: parse_relation(params.relation.as_deref())?,
            direction: parse_direction(params.direction.as_deref())?,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_stale(&self, params: StaleParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = StaleUseCase::new(context).run(StaleInput {
            artifact: params.artifact,
            within_days: params.within_days,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_contradictions(
        &self,
        params: ContradictionsParams,
    ) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = ContradictionsUseCase::new(context).run(ContradictionsInput {
            artifact: params.artifact,
            all: params.all,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_impacted_by(&self, params: ImpactedByParams) -> McpAdapterResult<serde_json::Value> {
        // Empty `paths` is rejected like "neither", mirroring the CLI where
        // clap treats an empty Vec as "not present" — an empty changed set is
        // a question that was never asked, not an empty answer.
        let changed = match (params.paths, params.git_ref) {
            (Some(paths), None) if !paths.is_empty() => ImpactedChangedSet::Paths(paths),
            (None, Some(git_ref)) => ImpactedChangedSet::GitRef(git_ref),
            _ => {
                return Err(McpAdapterError::InvalidArguments(
                    "exactly one of `paths` (non-empty) or `ref` must be provided".to_string(),
                ));
            }
        };
        let context = self.context(params.project_root)?;
        let outcome = ImpactedUseCase::new(context).run(ImpactedInput {
            artifact: params.artifact,
            changed,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_search(&self, params: SearchParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let top = NonZeroUsize::new(params.top.unwrap_or(10)).ok_or_else(|| {
            McpAdapterError::InvalidArguments("top must be greater than zero".to_string())
        })?;
        let outcome = SearchUseCase::new(context).run(SearchInput {
            query: params.query,
            artifact: params.artifact,
            search_artifact: params.search_artifact,
            semantic: params.semantic,
            lexical: params.lexical,
            kind: params.kind,
            status: params.status,
            owner: params.owner,
            source_path: params.source_path,
            related_to: params.related_to,
            relation: parse_relation(params.relation.as_deref())?,
            direction: parse_direction(params.direction.as_deref())?,
            top,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_patch_check(
        &self,
        params: AdocPatchCheckParams,
    ) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        match params.input {
            PatchInput::Path { patch_path } => {
                let outcome = PatchCheckUseCase::new(context).run(PatchCheckInput {
                    patch_path,
                    artifact: params.artifact,
                })?;
                serde_json::to_value(outcome.result).map_err(Into::into)
            }
            PatchInput::Inline { patch } => {
                let graph_artifact_path =
                    resolve_graph_artifact_for_inline_patch(&context, params.artifact)?;
                let result = check_patch_json(PatchJsonInput {
                    graph_artifact_path,
                    patch,
                });
                serde_json::to_value(result).map_err(Into::into)
            }
        }
    }

    pub fn run_diff(&self, params: AdocDiffParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = DiffUseCase::new(context).run(DiffInput {
            base_ref: params.base_ref,
            head_ref: params.head_ref,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_review(&self, params: AdocReviewParams) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let patch = params.patch.map(|input| match input {
            PatchInput::Path { patch_path } => ReviewPatchSource::Path(patch_path),
            PatchInput::Inline { patch } => ReviewPatchSource::Inline(patch),
        });
        let outcome = ReviewUseCase::new(context).run(ReviewInput {
            base_ref: params.base_ref,
            head_ref: params.head_ref,
            patch,
        })?;
        serde_json::to_value(outcome.envelope).map_err(Into::into)
    }

    pub fn run_project_status(
        &self,
        params: ProjectStatusParams,
    ) -> McpAdapterResult<serde_json::Value> {
        let context = self.context(params.project_root)?;
        let outcome = ProjectStatusUseCase::new(context).run(ProjectStatusInput {
            refresh: parse_project_status_refresh(params.refresh.as_deref())?,
            no_embeddings: params.no_embeddings,
        })?;
        serde_json::to_value(outcome).map_err(Into::into)
    }

    pub fn list_agent_resources(&self) -> Vec<Resource> {
        resources::list()
    }

    pub fn read_agent_resource(&self, uri: &str) -> McpAdapterResult<ReadResourceResult> {
        resources::read(uri).ok_or_else(|| {
            McpAdapterError::InvalidArguments(format!("unknown AgentDoc resource URI {uri:?}"))
        })
    }

    pub fn list_agent_prompts(&self) -> Vec<Prompt> {
        prompts::list()
    }

    pub fn get_agent_prompt(
        &self,
        name: &str,
        arguments: Option<JsonObject>,
    ) -> McpAdapterResult<GetPromptResult> {
        prompts::get(name, arguments).ok_or_else(|| {
            McpAdapterError::InvalidArguments(format!("unknown AgentDoc prompt {name:?}"))
        })
    }

    fn context(
        &self,
        override_root: Option<PathBuf>,
    ) -> McpAdapterResult<LocalContext<ProjectRootPathPolicy>> {
        let root = override_root.unwrap_or_else(|| self.default_project_root.clone());
        let policy = ProjectRootPathPolicy::new(root)?;
        Ok(LocalContext::new(
            policy.project_root().to_path_buf(),
            policy,
        ))
    }
}

#[tool_router(router = tool_router)]
impl AgentDocMcpServer {
    #[tool(
        name = "adoc_init",
        description = "Create AgentDoc config and starter docs inside the project root."
    )]
    pub fn adoc_init(
        &self,
        Parameters(params): Parameters<InitParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_init(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_check",
        description = "Validate AgentDoc source and return diagnostics."
    )]
    pub fn adoc_check(
        &self,
        Parameters(params): Parameters<CheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_check(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_build",
        description = "Build AgentDoc HTML, graph, and optional search artifacts."
    )]
    pub fn adoc_build(
        &self,
        Parameters(params): Parameters<BuildParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_build(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_why",
        description = "Return the retrieval record for a Knowledge Object ID."
    )]
    pub fn adoc_why(
        &self,
        Parameters(params): Parameters<WhyParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_why(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_graph",
        description = "Traverse relation graph context for a Knowledge Object ID."
    )]
    pub fn adoc_graph(
        &self,
        Parameters(params): Parameters<GraphParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_graph(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_stale",
        description = "List stale, review-overdue, and (with within_days) expiring-soon Knowledge Objects, re-derived at read time from the graph artifact. Read-only query: records are data, not failures."
    )]
    pub fn adoc_stale(
        &self,
        Parameters(params): Parameters<StaleParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_stale(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_contradictions",
        description = "List unresolved contradictions and the claims they implicate (with all=true: resolved and dismissed too), joined from the graph artifact. Read-only query: findings are data, not failures."
    )]
    pub fn adoc_contradictions(
        &self,
        Parameters(params): Parameters<ContradictionsParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_contradictions(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_impacted_by",
        description = "List verified claims and accepted decisions implicated by changed source paths (explicit `paths` or a git `ref` against the working tree), with impact-review proof obligations, from the graph artifact. Read-only query: findings are data, not failures."
    )]
    pub fn adoc_impacted_by(
        &self,
        Parameters(params): Parameters<ImpactedByParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_impacted_by(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_search",
        description = "Search compiled AgentDoc graph and search artifacts."
    )]
    pub fn adoc_search(
        &self,
        Parameters(params): Parameters<SearchParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_search(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_patch_check",
        description = "Validate an adoc.patch.v0 file or inline patch JSON against the graph artifact."
    )]
    pub fn adoc_patch_check(
        &self,
        Parameters(params): Parameters<AdocPatchCheckParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_patch_check(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_diff",
        description = "Return the mechanical adoc.diff.v0 envelope for Knowledge Objects between a base git ref and the workdir (or an optional head ref). Read-only; requires readiness.review."
    )]
    pub fn adoc_diff(
        &self,
        Parameters(params): Parameters<AdocDiffParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_diff(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_review",
        description = "Return the enriched adoc.review.v0 report (diff plus source-path impact, required reviewers, and proof obligations) between a base git ref and the workdir (or an optional head ref). Optional `patch` parameter embeds an adoc.patch.check.v0 validation result against the head graph and unions patch-driven obligations into the top-level list (the patch is never applied). Read-only; requires readiness.review."
    )]
    pub fn adoc_review(
        &self,
        Parameters(params): Parameters<AdocReviewParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_review(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }

    #[tool(
        name = "adoc_project_status",
        description = "Inspect AgentDoc project readiness. refresh is one of none, check, or build; default is none. build may write configured artifacts."
    )]
    pub fn adoc_project_status(
        &self,
        Parameters(params): Parameters<ProjectStatusParams>,
    ) -> Result<CallToolResult, ErrorData> {
        self.run_project_status(params)
            .map(CallToolResult::structured)
            .map_err(adapter_error)
    }
}

#[tool_handler(router = self.tool_router)]
impl ServerHandler for AgentDocMcpServer {
    fn get_info(&self) -> ServerInfo {
        ServerInfo::new(
            ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
        )
            .with_instructions("AgentDoc local MCP gateway over compiled artifacts, source checks, builds, graph traversal, search, and patch validation.")
    }

    fn list_resources(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListResourcesResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(ListResourcesResult::with_all_items(
            self.list_agent_resources(),
        )))
    }

    fn read_resource(
        &self,
        request: ReadResourceRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ReadResourceResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(
            self.read_agent_resource(&request.uri)
                .map_err(adapter_error),
        )
    }

    fn list_prompts(
        &self,
        _request: Option<PaginatedRequestParams>,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<ListPromptsResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(Ok(prompts::list_result()))
    }

    fn get_prompt(
        &self,
        request: GetPromptRequestParams,
        _context: RequestContext<RoleServer>,
    ) -> impl Future<Output = Result<GetPromptResult, ErrorData>> + MaybeSendFuture + '_ {
        std::future::ready(
            self.get_agent_prompt(&request.name, request.arguments)
                .map_err(adapter_error),
        )
    }
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InitParams {
    pub project_root: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct CheckParams {
    pub project_root: Option<PathBuf>,
    pub path: Option<PathBuf>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct BuildParams {
    pub project_root: Option<PathBuf>,
    pub path: Option<PathBuf>,
    pub out: Option<PathBuf>,
    #[serde(default)]
    pub no_embeddings: bool,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WhyParams {
    pub project_root: Option<PathBuf>,
    pub object_id: String,
    pub artifact: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GraphParams {
    pub project_root: Option<PathBuf>,
    pub object_id: String,
    pub artifact: Option<PathBuf>,
    pub relation: Option<String>,
    pub direction: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct StaleParams {
    pub project_root: Option<PathBuf>,
    pub artifact: Option<PathBuf>,
    /// Additionally list verified objects whose `expires_at` falls within the
    /// next N days as `expiring_soon` records.
    pub within_days: Option<u32>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ContradictionsParams {
    pub project_root: Option<PathBuf>,
    pub artifact: Option<PathBuf>,
    /// Include `resolved` and `dismissed` contradictions in the listing
    /// (never affects `contradicted_claims`).
    #[serde(default)]
    pub all: bool,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ImpactedByParams {
    pub project_root: Option<PathBuf>,
    pub artifact: Option<PathBuf>,
    /// Explicit changed repo-relative paths (as emitted by
    /// `git diff --name-only`). Exactly one of `paths` / `ref`.
    pub paths: Option<Vec<String>>,
    /// Derive the changed set from git: the base ref against the working
    /// tree (the `adoc review <ref>` shape). Exactly one of `paths` / `ref`.
    #[serde(rename = "ref")]
    #[schemars(rename = "ref")]
    pub git_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    pub project_root: Option<PathBuf>,
    pub query: String,
    pub artifact: Option<PathBuf>,
    pub search_artifact: Option<PathBuf>,
    #[serde(default)]
    pub semantic: bool,
    #[serde(default)]
    pub lexical: bool,
    pub kind: Option<String>,
    pub status: Option<String>,
    pub owner: Option<String>,
    pub source_path: Option<String>,
    pub related_to: Option<String>,
    pub relation: Option<String>,
    pub direction: Option<String>,
    pub top: Option<usize>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
pub struct AdocPatchCheckParams {
    pub project_root: Option<PathBuf>,
    pub artifact: Option<PathBuf>,
    #[serde(flatten)]
    pub input: PatchInput,
}

#[derive(Debug, Clone, Deserialize, Serialize, JsonSchema)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum PatchInput {
    Path { patch_path: PathBuf },
    Inline { patch: serde_json::Value },
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdocDiffParams {
    pub project_root: Option<PathBuf>,
    pub base_ref: String,
    #[serde(default)]
    pub head_ref: Option<String>,
}

#[derive(Debug, Clone, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AdocReviewParams {
    pub project_root: Option<PathBuf>,
    pub base_ref: String,
    #[serde(default)]
    pub head_ref: Option<String>,
    /// V3.7 — optional adoc.patch.v0 to validate against the head graph.
    /// When supplied, the returned envelope embeds an adoc.patch.check.v0
    /// report and unions patch-driven proof obligations into the top-level
    /// obligation list. Reuses V2.1's [`PatchInput`] shape (path or inline).
    #[serde(default)]
    pub patch: Option<PatchInput>,
}

#[derive(Debug, Clone, Default, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProjectStatusParams {
    pub project_root: Option<PathBuf>,
    pub refresh: Option<String>,
    #[serde(default)]
    pub no_embeddings: bool,
}

fn resolve_graph_artifact_for_inline_patch<P>(
    context: &LocalContext<P>,
    artifact: Option<PathBuf>,
) -> McpAdapterResult<PathBuf>
where
    P: PathPolicy,
{
    if let Some(artifact) = artifact {
        return context
            .path_policy()
            .resolve_read_path(&artifact)
            .map_err(Into::into);
    }
    let config = ProjectConfig::discover_from(context.config_start())?;
    let artifact = config
        .as_ref()
        .and_then(|config| config.outputs.graph.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_GRAPH_ARTIFACT_PATH));
    context
        .path_policy()
        .resolve_read_path(&artifact)
        .map_err(Into::into)
}

fn parse_relation(value: Option<&str>) -> McpAdapterResult<Option<GraphRelationKind>> {
    value
        .map(|value| match value {
            "depends_on" => Ok(GraphRelationKind::DependsOn),
            "supersedes" => Ok(GraphRelationKind::Supersedes),
            "related_to" => Ok(GraphRelationKind::RelatedTo),
            other => Err(McpAdapterError::InvalidArguments(format!(
                "unsupported relation {other:?}; expected depends_on, supersedes, or related_to"
            ))),
        })
        .transpose()
}

fn parse_direction(value: Option<&str>) -> McpAdapterResult<Option<GraphDirection>> {
    value
        .map(|value| match value {
            "outgoing" => Ok(GraphDirection::Outgoing),
            "incoming" => Ok(GraphDirection::Incoming),
            "both" => Ok(GraphDirection::Both),
            other => Err(McpAdapterError::InvalidArguments(format!(
                "unsupported direction {other:?}; expected outgoing, incoming, or both"
            ))),
        })
        .transpose()
}

fn parse_project_status_refresh(value: Option<&str>) -> McpAdapterResult<ProjectStatusRefresh> {
    match value.unwrap_or("none") {
        "none" => Ok(ProjectStatusRefresh::None),
        "check" => Ok(ProjectStatusRefresh::Check),
        "build" => Ok(ProjectStatusRefresh::Build),
        other => Err(McpAdapterError::InvalidArguments(format!(
            "unsupported refresh {other:?}; expected none, check, or build"
        ))),
    }
}

fn adapter_error(error: McpAdapterError) -> ErrorData {
    match error {
        McpAdapterError::InvalidArguments(message) => ErrorData::invalid_params(message, None),
        McpAdapterError::Local(error) => ErrorData::invalid_params(error.to_string(), None),
        McpAdapterError::Serialize(error) => ErrorData::internal_error(error.to_string(), None),
    }
}

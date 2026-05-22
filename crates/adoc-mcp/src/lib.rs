//! MCP adapter for local AgentDoc workflows.

use std::future::Future;
use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    GraphDirection, GraphRelationKind, PatchJsonInput, RetrievalEnvelope, check_patch_json,
};
use adoc_local::{
    BuildInput, BuildUseCase, CheckInput, CheckUseCase, GraphInput, GraphUseCase, InitInput,
    InitUseCase, LocalContext, PatchCheckInput, PatchCheckUseCase, PathPolicy, ProjectConfig,
    ProjectRootPathPolicy, ProjectStatusInput, ProjectStatusRefresh, ProjectStatusUseCase,
    SearchInput, SearchUseCase, WhyInput, WhyUseCase,
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

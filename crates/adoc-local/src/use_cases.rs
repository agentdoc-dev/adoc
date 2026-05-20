use std::collections::BTreeMap;
use std::fs;
use std::io::{self, Write};
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant};

use adoc_core::{
    BuildArtifacts, BuildEmbeddingMode, BuildInput as CoreBuildInput, CompileInput, CompileResult,
    Diagnostic, DiagnosticCode, GraphDirection, GraphInput as CoreGraphInput, GraphRelationKind,
    GraphTraversalEnvelope, GraphTraversalQuery, GraphTraversalResult, PatchCheckResult,
    PatchInput, RetrievalEnvelope, RetrievalInput, RetrievalLoadResult, RetrievalRecord,
    SearchFilters, SearchMode, SearchQuery, Severity, build_workspace,
    check_patch as core_check_patch, compile_workspace, embed_query, load_graph_session,
    load_retrieval_session, search as core_search, traverse_graph, why_object,
};
use serde::Serialize;

use crate::{EmbeddingsProvider, LocalContext, LocalError, PathPolicy, ProjectConfig};

const DEFAULT_GRAPH_ARTIFACT_PATH: &str = "dist/docs.graph.json";
const DEFAULT_SEARCH_ARTIFACT_PATH: &str = "dist/docs.search.json";
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

#[derive(Debug, Clone)]
pub struct InitInput;

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
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchOutcome {
    pub envelope: RetrievalEnvelope,
    pub records: Vec<ResolvedRetrievalRecord>,
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

#[derive(Debug, Clone)]
pub struct InitUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> InitUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, _input: InitInput) -> Result<InitOutcome, LocalError> {
        init_with_context(&self.context)
    }
}

#[derive(Debug, Clone)]
pub struct CheckUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> CheckUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: CheckInput) -> Result<CheckOutcome, LocalError> {
        check_with_context(&self.context, input)
    }
}

#[derive(Debug, Clone)]
pub struct BuildUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> BuildUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: BuildInput) -> Result<BuildOutcome, LocalError> {
        build_with_context(&self.context, input)
    }
}

#[derive(Debug, Clone)]
pub struct WhyUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> WhyUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: WhyInput) -> Result<WhyOutcome, LocalError> {
        why_with_context(&self.context, input)
    }
}

#[derive(Debug, Clone)]
pub struct GraphUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> GraphUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: GraphInput) -> Result<GraphOutcome, LocalError> {
        graph_with_context(&self.context, input)
    }
}

#[derive(Debug, Clone)]
pub struct SearchUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> SearchUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: SearchInput) -> Result<SearchOutcome, LocalError> {
        search_with_context(&self.context, input)
    }
}

#[derive(Debug, Clone)]
pub struct PatchCheckUseCase<P>
where
    P: PathPolicy,
{
    context: LocalContext<P>,
}

impl<P> PatchCheckUseCase<P>
where
    P: PathPolicy,
{
    pub fn new(context: LocalContext<P>) -> Self {
        Self { context }
    }

    pub fn run(&self, input: PatchCheckInput) -> Result<PatchCheckOutcome, LocalError> {
        patch_check_with_context(&self.context, input)
    }
}

fn init_with_context<P>(context: &LocalContext<P>) -> Result<InitOutcome, LocalError>
where
    P: PathPolicy,
{
    let project_root = context
        .path_policy()
        .resolve_write_path(context.config_start())?;
    write_init_files(&project_root)?;
    Ok(InitOutcome {
        created: vec![
            project_root.join(INIT_CONFIG_PATH),
            project_root.join(INIT_INDEX_PATH),
        ],
        exit_code: 0,
    })
}

fn check_with_context<P>(
    context: &LocalContext<P>,
    input: CheckInput,
) -> Result<CheckOutcome, LocalError>
where
    P: PathPolicy,
{
    let path = input
        .path
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(path.is_none(), context.config_start())?;
    let path = resolve_docs_path_with_config(path, config.as_ref())?;
    let path = context.path_policy().resolve_read_path(&path)?;
    let result = compile_workspace(CompileInput { root: path });
    let exit_code = if result.has_errors() { 1 } else { 0 };

    Ok(CheckOutcome {
        diagnostics: result.diagnostics,
        exit_code,
    })
}

fn build_with_context<P>(
    context: &LocalContext<P>,
    input: BuildInput,
) -> Result<BuildOutcome, LocalError>
where
    P: PathPolicy,
{
    let path = input
        .path
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let out = input
        .out
        .as_deref()
        .map(|path| context.path_policy().resolve_write_path(path))
        .transpose()?;
    let needs_config = path.is_none() || out.is_none() || !input.no_embeddings;
    let config = discover_project_config_if(needs_config, context.config_start())?;
    let path = resolve_docs_path_with_config(path, config.as_ref())?;
    let path = context.path_policy().resolve_read_path(&path)?;
    let embedding_mode = resolve_embedding_mode(config.as_ref(), input.no_embeddings);

    match out {
        Some(out) => build_to_dir(path, out, embedding_mode),
        None => {
            let output_paths = resolve_build_output_paths(config.as_ref(), embedding_mode)?;
            let output_paths = resolve_build_output_paths_with_policy(&output_paths, context)?;
            build_to_paths(path, output_paths, embedding_mode)
        }
    }
}

fn why_with_context<P>(context: &LocalContext<P>, input: WhyInput) -> Result<WhyOutcome, LocalError>
where
    P: PathPolicy,
{
    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(artifact.is_none(), context.config_start())?;
    let artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let artifact = context.path_policy().resolve_read_path(&artifact)?;
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.clone(),
        search_artifact_path: None,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let load_exit_code = why_exit_code_for_diagnostics(&load_diagnostics);
    let Some(session) = session.filter(|_| !diagnostics_have_errors(&load_diagnostics)) else {
        return Ok(WhyOutcome {
            artifact,
            records: Vec::new(),
            diagnostics: load_diagnostics,
            duration: Duration::ZERO,
            exit_code: load_exit_code,
        });
    };

    let started = Instant::now();
    let why_result = why_object(&session, &input.object_id);
    let duration = started.elapsed();
    let diagnostics = merge_diagnostics(load_diagnostics, why_result.diagnostics);
    let exit_code = why_exit_code_for_diagnostics(&diagnostics);
    let records = why_result
        .records
        .into_iter()
        .map(|record| resolved_record(&session, record))
        .collect();

    Ok(WhyOutcome {
        artifact,
        records,
        diagnostics,
        duration,
        exit_code,
    })
}

fn graph_with_context<P>(
    context: &LocalContext<P>,
    input: GraphInput,
) -> Result<GraphOutcome, LocalError>
where
    P: PathPolicy,
{
    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(artifact.is_none(), context.config_start())?;
    let graph_artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let graph_artifact = context.path_policy().resolve_read_path(&graph_artifact)?;
    let load_result = load_graph_session(CoreGraphInput {
        graph_artifact_path: graph_artifact,
    });
    let mut diagnostics = load_result.diagnostics;
    let Some(session) = load_result.session else {
        let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
        return Ok(GraphOutcome {
            envelope: GraphTraversalEnvelope::new(
                input.object_id,
                Vec::new(),
                Vec::new(),
                diagnostics,
            ),
            exit_code,
        });
    };

    if diagnostics_have_errors(&diagnostics) {
        let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
        return Ok(GraphOutcome {
            envelope: GraphTraversalEnvelope::new(
                input.object_id,
                Vec::new(),
                Vec::new(),
                diagnostics,
            ),
            exit_code,
        });
    }

    let traversal = traverse_graph(
        &session,
        GraphTraversalQuery {
            root_id: input.object_id.clone(),
            direction: input.direction.unwrap_or_default(),
            relations: input.relation.into_iter().collect(),
        },
    );
    diagnostics = merge_diagnostics(diagnostics, traversal.diagnostics);
    let exit_code = graph_exit_code_for_diagnostics(&diagnostics);
    let result = GraphTraversalResult {
        root: traversal.root,
        nodes: traversal.nodes,
        edges: traversal.edges,
        diagnostics,
    };

    Ok(GraphOutcome {
        envelope: GraphTraversalEnvelope::from(result),
        exit_code,
    })
}

fn search_with_context<P>(
    context: &LocalContext<P>,
    input: SearchInput,
) -> Result<SearchOutcome, LocalError>
where
    P: PathPolicy,
{
    let requested_mode = if input.semantic {
        SearchMode::Semantic
    } else if input.lexical {
        SearchMode::Lexical
    } else {
        SearchMode::Hybrid
    };

    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let search_artifact = input
        .search_artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let needs_search_config = matches!(requested_mode, SearchMode::Hybrid | SearchMode::Semantic)
        && search_artifact.is_none();
    let config = discover_project_config_if(
        artifact.is_none() || needs_search_config,
        context.config_start(),
    )?;
    let artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let artifact = context.path_policy().resolve_read_path(&artifact)?;
    let search_artifact_path = match requested_mode {
        SearchMode::Lexical => None,
        SearchMode::Hybrid | SearchMode::Semantic => {
            let path = resolve_search_artifact_path_with_config(search_artifact, config.as_ref());
            Some(context.path_policy().resolve_read_path(&path)?)
        }
    };
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact,
        search_artifact_path,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let Some(session) = session.filter(|_| !diagnostics_have_errors(&load_diagnostics)) else {
        return Ok(search_outcome(Vec::new(), load_diagnostics, 2));
    };

    if requested_mode == SearchMode::Semantic && !session.has_semantic_index() {
        let mut diagnostics = load_diagnostics;
        diagnostics.push(Diagnostic {
            code: DiagnosticCode::SearchArtifactMissing,
            severity: Severity::Error,
            message: "Semantic search requested but no search artifact is loaded.".to_string(),
            span: None,
            object_id: None,
            help: Some(
                DiagnosticCode::SearchArtifactMissing
                    .default_help()
                    .to_string(),
            ),
        });
        return Ok(search_outcome(Vec::new(), diagnostics, 2));
    }

    let mode = match requested_mode {
        SearchMode::Hybrid if session.has_semantic_index() => SearchMode::Hybrid,
        SearchMode::Hybrid => SearchMode::Lexical,
        mode => mode,
    };
    let needs_query_vector = matches!(mode, SearchMode::Hybrid | SearchMode::Semantic);
    let query_vector = if needs_query_vector {
        match embed_query(&input.query) {
            Ok(vector) => Some(vector),
            Err(embed_error) => {
                let mode_label = match mode {
                    SearchMode::Hybrid => "hybrid search",
                    SearchMode::Semantic => "semantic search",
                    SearchMode::Lexical => "lexical search",
                };
                let (code, message) = match &embed_error {
                    adoc_core::EmbedQueryError::ModelLoad(msg) => (
                        DiagnosticCode::EmbedModelLoadFailed,
                        format!("{mode_label} requested but embedding model failed to load: {msg}"),
                    ),
                    adoc_core::EmbedQueryError::Compute(msg) => (
                        DiagnosticCode::EmbedComputeFailed,
                        format!("{mode_label} requested but query embedding failed: {msg}"),
                    ),
                };
                let diagnostic = Diagnostic {
                    code,
                    severity: Severity::Error,
                    message,
                    span: None,
                    object_id: None,
                    help: Some(code.default_help().to_string()),
                };
                return Ok(search_outcome(Vec::new(), vec![diagnostic], 2));
            }
        }
    } else {
        None
    };

    let search_result = core_search(
        &session,
        SearchQuery {
            text: input.query,
            mode,
            filters: SearchFilters {
                kind: input.kind,
                status: input.status,
                owner: input.owner,
                source_path: input.source_path,
                related_to: input.related_to,
                relation: input.relation,
                direction: input.direction,
            },
            top: input.top,
            query_vector,
        },
    );
    let diagnostics = merge_diagnostics(load_diagnostics, search_result.diagnostics);
    let exit_code = search_exit_code(&diagnostics);
    let records = search_result
        .records
        .into_iter()
        .map(|record| resolved_record(&session, record))
        .collect::<Vec<_>>();
    let envelope = RetrievalEnvelope::new(
        records
            .iter()
            .map(|resolved| resolved.record.clone())
            .collect(),
        diagnostics.clone(),
    );

    Ok(SearchOutcome {
        envelope,
        records,
        diagnostics,
        exit_code,
    })
}

fn patch_check_with_context<P>(
    context: &LocalContext<P>,
    input: PatchCheckInput,
) -> Result<PatchCheckOutcome, LocalError>
where
    P: PathPolicy,
{
    let patch_path = context.path_policy().resolve_read_path(&input.patch_path)?;
    let artifact = input
        .artifact
        .as_deref()
        .map(|path| context.path_policy().resolve_read_path(path))
        .transpose()?;
    let config = discover_project_config_if(artifact.is_none(), context.config_start())?;
    let graph_artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let graph_artifact = context.path_policy().resolve_read_path(&graph_artifact)?;
    let result = core_check_patch(PatchInput {
        graph_artifact_path: graph_artifact,
        patch_path,
    });
    let exit_code = patch_exit_code(&result);

    Ok(PatchCheckOutcome { result, exit_code })
}

fn write_init_files(project_root: &Path) -> Result<(), LocalError> {
    let config_path = project_root.join(INIT_CONFIG_PATH);
    let index_path = project_root.join(INIT_INDEX_PATH);

    for target in [&config_path, &index_path] {
        if target.exists() {
            return Err(LocalError::InitTargetExists {
                path: target.to_path_buf(),
            });
        }
    }

    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|source| LocalError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    write_new_file(&config_path, INIT_CONFIG_TEMPLATE.as_bytes())?;
    if let Err(error) = write_new_file(&index_path, init_index_template().as_bytes()) {
        cleanup_init_paths([&config_path]);
        return Err(error);
    }

    Ok(())
}

fn init_index_template() -> &'static str {
    "\
# AgentDoc Project @doc(project.index)

This project was initialized with AgentDoc.

::claim project.initialized
status: draft
--
The project has an initialized AgentDoc source tree.
::
"
}

fn write_new_file(path: &Path, contents: &[u8]) -> Result<(), LocalError> {
    let mut file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|source| {
            if source.kind() == io::ErrorKind::AlreadyExists {
                LocalError::InitTargetExists {
                    path: path.to_path_buf(),
                }
            } else {
                LocalError::WriteFailed {
                    path: path.to_path_buf(),
                    source,
                }
            }
        })?;

    if let Err(source) = file.write_all(contents) {
        cleanup_init_paths([path]);
        return Err(LocalError::WriteFailed {
            path: path.to_path_buf(),
            source,
        });
    }

    Ok(())
}

fn cleanup_init_paths<P: AsRef<Path>>(paths: impl IntoIterator<Item = P>) {
    for path in paths {
        let _ = fs::remove_file(path.as_ref());
    }
}

fn build_to_dir(
    path: PathBuf,
    out: PathBuf,
    embedding_mode: BuildEmbeddingMode,
) -> Result<BuildOutcome, LocalError> {
    let result = build_workspace(CoreBuildInput {
        root: path,
        embeddings: embedding_mode,
        prior_search_artifact_path: Some(out.join("docs.search.json")),
    });
    finish_build_result(result, &out)
}

fn build_to_paths(
    path: PathBuf,
    output_paths: BuildOutputs,
    embedding_mode: BuildEmbeddingMode,
) -> Result<BuildOutcome, LocalError> {
    let result = build_workspace(CoreBuildInput {
        root: path,
        embeddings: embedding_mode,
        prior_search_artifact_path: output_paths.search.clone(),
    });
    finish_build_result_at_paths(result, &output_paths)
}

fn finish_build_result(result: CompileResult, out: &Path) -> Result<BuildOutcome, LocalError> {
    let has_errors = result.has_errors();
    let diagnostics = result.diagnostics;

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => {
            return Ok(BuildOutcome {
                diagnostics,
                outputs: None,
                exit_code: 1,
            });
        }
        None => return Err(LocalError::BuildMissingArtifacts),
    };

    let paths = output_paths_for_dir(out, artifacts.search_json.is_some())?;
    write_artifacts_to_paths(&paths, &artifacts)?;
    Ok(BuildOutcome {
        diagnostics,
        outputs: Some(paths),
        exit_code: i32::from(has_errors),
    })
}

fn finish_build_result_at_paths(
    result: CompileResult,
    paths: &BuildOutputs,
) -> Result<BuildOutcome, LocalError> {
    let has_errors = result.has_errors();
    let diagnostics = result.diagnostics;

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => {
            return Ok(BuildOutcome {
                diagnostics,
                outputs: None,
                exit_code: 1,
            });
        }
        None => return Err(LocalError::BuildMissingArtifacts),
    };

    write_artifacts_to_paths(paths, &artifacts)?;
    Ok(BuildOutcome {
        diagnostics,
        outputs: Some(paths.clone()),
        exit_code: i32::from(has_errors),
    })
}

fn output_paths_for_dir(out: &Path, include_search: bool) -> Result<BuildOutputs, LocalError> {
    if out.exists() && !out.is_dir() {
        return Err(LocalError::OutputPathIsFile {
            path: out.to_path_buf(),
        });
    }

    fs::create_dir_all(out).map_err(|source| LocalError::CreateOutputDirectory {
        path: out.to_path_buf(),
        source,
    })?;

    Ok(BuildOutputs {
        html: out.join("docs.html"),
        graph: out.join("docs.graph.json"),
        search: include_search.then(|| out.join("docs.search.json")),
    })
}

struct ArtifactWriteEntry {
    path: PathBuf,
    contents: Vec<u8>,
}

fn write_artifacts_to_paths(
    paths: &BuildOutputs,
    artifacts: &BuildArtifacts,
) -> Result<(), LocalError> {
    for entry in serialize_artifacts(paths, artifacts) {
        write_file_with_parents(&entry.path, &entry.contents)?;
    }

    Ok(())
}

fn serialize_artifacts(
    paths: &BuildOutputs,
    artifacts: &BuildArtifacts,
) -> Vec<ArtifactWriteEntry> {
    let mut entries = vec![ArtifactWriteEntry {
        path: paths.html.clone(),
        contents: artifacts.html.as_bytes().to_vec(),
    }];

    entries.push(ArtifactWriteEntry {
        path: paths.graph.clone(),
        contents: artifacts.graph_json.as_bytes().to_vec(),
    });

    if let (Some(search_json), Some(search_path)) =
        (artifacts.search_json.as_ref(), paths.search.as_ref())
    {
        entries.push(ArtifactWriteEntry {
            path: search_path.clone(),
            contents: search_json.as_bytes().to_vec(),
        });
    }

    entries
}

fn write_file_with_parents(path: &Path, contents: &[u8]) -> Result<(), LocalError> {
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent).map_err(|source| LocalError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(path, contents).map_err(|source| LocalError::WriteFailed {
        path: path.to_path_buf(),
        source,
    })
}

fn resolve_embedding_mode(
    config: Option<&ProjectConfig>,
    no_embeddings: bool,
) -> BuildEmbeddingMode {
    if no_embeddings
        || config
            .map(|config| config.embeddings_provider == EmbeddingsProvider::None)
            .unwrap_or(false)
    {
        BuildEmbeddingMode::Skipped
    } else {
        BuildEmbeddingMode::Enabled
    }
}

fn resolve_build_output_paths(
    config: Option<&ProjectConfig>,
    embedding_mode: BuildEmbeddingMode,
) -> Result<BuildOutputs, LocalError> {
    let Some(config) = config else {
        return Err(LocalError::ConfigMissing {
            message: "adoc build requires --out or agentdoc.config.yaml outputs".to_string(),
            config_path: None,
        });
    };

    let search_required = embedding_mode == BuildEmbeddingMode::Enabled;
    let html = config.outputs.html.clone();
    let graph = config.outputs.graph.clone();
    let search = config.outputs.search.clone();

    match (html, graph, search_required, search) {
        (Some(html), Some(graph), true, Some(search)) => Ok(BuildOutputs {
            html,
            graph,
            search: Some(search),
        }),
        (Some(html), Some(graph), false, search) => Ok(BuildOutputs {
            html,
            graph,
            search,
        }),
        _ => Err(LocalError::ConfigMissing {
            message: if search_required {
                "adoc build requires outputs.dir or exact html, graph, and search outputs"
            } else {
                "adoc build requires outputs.dir or exact html and graph outputs"
            }
            .to_string(),
            config_path: Some(config.path.clone()),
        }),
    }
}

fn resolve_build_output_paths_with_policy<P>(
    paths: &BuildOutputs,
    context: &LocalContext<P>,
) -> Result<BuildOutputs, LocalError>
where
    P: PathPolicy,
{
    Ok(BuildOutputs {
        html: context.path_policy().resolve_write_path(&paths.html)?,
        graph: context.path_policy().resolve_write_path(&paths.graph)?,
        search: paths
            .search
            .as_deref()
            .map(|path| context.path_policy().resolve_write_path(path))
            .transpose()?,
    })
}

fn discover_project_config_if(
    needed: bool,
    start: &Path,
) -> Result<Option<ProjectConfig>, LocalError> {
    if needed {
        ProjectConfig::discover_from(start)
    } else {
        Ok(None)
    }
}

fn resolve_docs_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> Result<PathBuf, LocalError> {
    path.or_else(|| config.map(|config| config.docs_path.clone()))
        .ok_or_else(|| LocalError::ConfigMissing {
            message: "adoc check/build requires a path or agentdoc.config.yaml with docs_path"
                .to_string(),
            config_path: config.map(|config| config.path.clone()),
        })
}

fn resolve_graph_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.graph.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_GRAPH_ARTIFACT_PATH))
}

fn resolve_search_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.search.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SEARCH_ARTIFACT_PATH))
}

fn resolved_record(
    session: &adoc_core::RetrievalSession,
    record: RetrievalRecord,
) -> ResolvedRetrievalRecord {
    let related_statuses = session.related_statuses(&record);
    ResolvedRetrievalRecord {
        record,
        related_statuses,
    }
}

fn search_outcome(
    records: Vec<ResolvedRetrievalRecord>,
    diagnostics: Vec<Diagnostic>,
    exit_code: i32,
) -> SearchOutcome {
    let envelope = RetrievalEnvelope::new(
        records
            .iter()
            .map(|resolved| resolved.record.clone())
            .collect(),
        diagnostics.clone(),
    );
    SearchOutcome {
        envelope,
        records,
        diagnostics,
        exit_code,
    }
}

fn diagnostics_have_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

fn merge_diagnostics(
    mut load_diagnostics: Vec<Diagnostic>,
    mut command_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    load_diagnostics.append(&mut command_diagnostics);
    load_diagnostics
}

fn exit_code_for_diagnostics(
    diagnostics: &[Diagnostic],
    mapper: impl Fn(&Diagnostic) -> Option<i32>,
) -> i32 {
    diagnostics.iter().filter_map(mapper).min().unwrap_or(0)
}

fn why_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, why_diagnostic_exit_code)
}

fn why_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::RetrievalObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

fn graph_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, graph_diagnostic_exit_code)
}

fn graph_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::GraphObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

fn search_exit_code(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, search_diagnostic_exit_code)
}

fn search_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::SearchInvalidFilter, _) => Some(1),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

fn patch_exit_code(result: &PatchCheckResult) -> i32 {
    if result.valid {
        0
    } else {
        exit_code_for_diagnostics(&result.diagnostics, patch_diagnostic_exit_code).max(1)
    }
}

fn patch_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::PatchBaseHashMismatch, _) => Some(4),
        (DiagnosticCode::GraphObjectNotFound, _) => Some(3),
        (
            DiagnosticCode::IoArtifactMissing
            | DiagnosticCode::IoArtifactUnreadable
            | DiagnosticCode::IoArtifactMalformed
            | DiagnosticCode::SchemaUnsupportedVersion
            | DiagnosticCode::IdDuplicateInArtifact
            | DiagnosticCode::IdInvalid,
            _,
        ) => Some(2),
        (
            DiagnosticCode::PatchInvalidDocument
            | DiagnosticCode::PatchValidationFailed
            | DiagnosticCode::PatchTargetAlreadyExists
            | DiagnosticCode::PatchPlacementInvalid,
            _,
        ) => Some(1),
        (_, Severity::Error) => Some(1),
        _ => None,
    }
}

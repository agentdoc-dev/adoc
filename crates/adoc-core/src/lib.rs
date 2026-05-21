mod application;
mod domain;
mod infrastructure;

pub use application::artifact_inspection::{
    ArtifactInspection, ArtifactLoadStatus, GraphArtifactInspectionInput,
    SearchArtifactInspectionInput,
};
pub use application::compile::{
    BuildArtifacts, BuildEmbeddingMode, BuildInput, CompileInput, CompileResult,
};
pub use application::graph::{
    GRAPH_TRAVERSAL_SCHEMA_VERSION, GraphInput, GraphLoadResult, GraphSession,
    GraphTraversalEnvelope, traverse_graph,
};
pub use application::patch::{
    PATCH_CHECK_SCHEMA_VERSION, PatchCheckResult, PatchInput, PatchJsonInput,
};
pub use application::retrieval::{
    RETRIEVAL_SCHEMA_VERSION, RetrievalEnvelope, RetrievalInput, RetrievalLoadResult,
    RetrievalSession, SearchFilters, SearchQuery, SearchResult, WhyResult, search, why_object,
};
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use domain::graph::{
    GraphDirection, GraphRelationKind, GraphTraversalEdge, GraphTraversalNode, GraphTraversalQuery,
    GraphTraversalResult,
};
pub use domain::patch::{AffectedRelation, PatchDiff, PatchOperation, ProofObligation};
pub use domain::retrieval::{
    RetrievalMatch, RetrievalRecord, RetrievalRelations, RetrievalSource, SearchMode,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EmbeddingProviderSelection {
    Local,
    Deterministic,
}

impl EmbeddingProviderSelection {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Local => "local",
            Self::Deterministic => "deterministic",
        }
    }
}

/// Error returned by [`embed_query`].
#[derive(Debug)]
pub enum EmbedQueryError {
    /// The embedding model could not be loaded.
    ModelLoad(String),
    /// The query vector could not be computed.
    Compute(String),
}

impl std::fmt::Display for EmbedQueryError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            EmbedQueryError::ModelLoad(msg) => write!(f, "embedding provider unavailable: {msg}"),
            EmbedQueryError::Compute(msg) => write!(f, "query embedding failed: {msg}"),
        }
    }
}

impl std::error::Error for EmbedQueryError {}

/// Embed a query string using the default embedding provider.
///
/// Returns the query vector as a `Vec<f32>`.
pub fn embed_query(query: &str) -> Result<Vec<f32>, EmbedQueryError> {
    embed_query_with_embedding_provider(query, EmbeddingProviderSelection::Local)
}

pub fn embed_query_with_embedding_provider(
    query: &str,
    provider: EmbeddingProviderSelection,
) -> Result<Vec<f32>, EmbedQueryError> {
    let provider = embedding_provider(provider).map_err(map_provider_error)?;
    provider.embed_query(query).map_err(map_provider_error)
}

pub(crate) fn map_provider_error(
    err: domain::ports::embedding_provider::EmbeddingError,
) -> EmbedQueryError {
    use domain::ports::embedding_provider::EmbeddingError;
    match err {
        EmbeddingError::ModelLoad(msg) => EmbedQueryError::ModelLoad(msg),
        EmbeddingError::Compute(msg) => EmbedQueryError::Compute(msg),
        EmbeddingError::DimensionMismatch { expected, actual } => EmbedQueryError::Compute(
            format!("query vector dim {actual} does not match provider dim {expected}"),
        ),
    }
}

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    application::compile::compile_with_provider(&provider)
}

pub fn build_workspace(input: BuildInput) -> CompileResult {
    build_workspace_with_embedding_provider(input, EmbeddingProviderSelection::Local)
}

pub fn build_workspace_with_embedding_provider(
    input: BuildInput,
    provider: EmbeddingProviderSelection,
) -> CompileResult {
    build_workspace_with_embedding_provider_factory(input, || embedding_provider(provider))
}

pub fn load_graph_session(input: GraphInput) -> GraphLoadResult {
    application::graph::load_graph_session_with_readers(
        input,
        &infrastructure::artifact::GraphJsonArtifact,
    )
}

pub fn check_patch(input: PatchInput) -> PatchCheckResult {
    application::patch::check_patch_with_readers(
        input,
        &infrastructure::artifact::GraphJsonArtifact,
        &infrastructure::artifact::PatchJsonArtifact,
    )
}

pub fn check_patch_json(input: PatchJsonInput) -> PatchCheckResult {
    use domain::ports::artifact_reader::ArtifactReader;

    let graph_document =
        match infrastructure::artifact::GraphJsonArtifact.read(&input.graph_artifact_path) {
            Ok(document) => document,
            Err(diagnostics) => return application::patch::PatchCheckResult::failure(diagnostics),
        };
    let patch_document = match infrastructure::artifact::read_patch_document_value(
        input.patch,
        "Inline patch document",
    ) {
        Ok(document) => document,
        Err(diagnostics) => return application::patch::PatchCheckResult::failure(diagnostics),
    };

    application::patch::check_patch_documents(graph_document, patch_document)
}

fn embedding_provider(
    provider: EmbeddingProviderSelection,
) -> Result<
    Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
    domain::ports::embedding_provider::EmbeddingError,
> {
    match provider {
        EmbeddingProviderSelection::Deterministic => Ok(Box::new(
            infrastructure::embedding::deterministic::DeterministicProvider::default(),
        )),
        EmbeddingProviderSelection::Local => local_embedding_provider(),
    }
}

fn local_embedding_provider() -> Result<
    Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
    domain::ports::embedding_provider::EmbeddingError,
> {
    #[cfg(feature = "test-embedding-provider")]
    if use_deterministic_test_embedding_provider() {
        return embedding_provider(EmbeddingProviderSelection::Deterministic);
    }

    #[cfg(feature = "test-embedding-provider")]
    if use_force_load_fail_test_embedding_provider() {
        return Err(
            domain::ports::embedding_provider::EmbeddingError::ModelLoad(
                "forced load failure for tests".into(),
            ),
        );
    }

    local_fast_embedding_provider()
}

#[cfg(feature = "embeddings")]
fn local_fast_embedding_provider() -> Result<
    Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
    domain::ports::embedding_provider::EmbeddingError,
> {
    infrastructure::embedding::fastembed::FastEmbedProvider::try_new().map(|provider| {
        Box::new(provider) as Box<dyn domain::ports::embedding_provider::EmbeddingProvider>
    })
}

#[cfg(not(feature = "embeddings"))]
fn local_fast_embedding_provider() -> Result<
    Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
    domain::ports::embedding_provider::EmbeddingError,
> {
    Err(domain::ports::embedding_provider::EmbeddingError::ModelLoad(
        "embedding support is disabled; rebuild with the `embeddings` feature or run with --no-embeddings".to_string(),
    ))
}

#[cfg(feature = "test-embedding-provider")]
fn use_deterministic_test_embedding_provider() -> bool {
    matches!(
        std::env::var("ADOC_TEST_EMBEDDING_PROVIDER").as_deref(),
        Ok("deterministic" | "in-memory")
    )
}

#[cfg(feature = "test-embedding-provider")]
fn use_force_load_fail_test_embedding_provider() -> bool {
    std::env::var("ADOC_TEST_EMBEDDING_PROVIDER").as_deref() == Ok("force-load-fail")
}

fn build_workspace_with_embedding_provider_factory<F>(
    input: BuildInput,
    mut provider_factory: F,
) -> CompileResult
where
    F: FnMut() -> Result<
        Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
        domain::ports::embedding_provider::EmbeddingError,
    >,
{
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    match input.embeddings {
        BuildEmbeddingMode::Enabled => application::compile::build_with_provider(
            &provider,
            application::compile::BuildOptions {
                embeddings: application::compile::BuildEmbeddingBehavior::EnabledFactory {
                    provider_factory: &mut provider_factory,
                },
                prior_search_artifact_path: input.prior_search_artifact_path,
            },
        ),
        BuildEmbeddingMode::Skipped => application::compile::build_with_provider(
            &provider,
            application::compile::BuildOptions {
                embeddings: application::compile::BuildEmbeddingBehavior::Skipped,
                prior_search_artifact_path: input.prior_search_artifact_path,
            },
        ),
    }
}

pub fn load_retrieval_session(input: RetrievalInput) -> RetrievalLoadResult {
    load_retrieval_session_with_embedding_provider(input, EmbeddingProviderSelection::Local)
}

pub fn load_retrieval_session_with_embedding_provider(
    input: RetrievalInput,
    provider: EmbeddingProviderSelection,
) -> RetrievalLoadResult {
    // Only resolve the active provider's model header when a search-artifact
    // path is provided; lexical-only callers must not pay the embedding-model
    // metadata lookup. Resolution itself is metadata-only — no model download.
    let active_model = if input.search_artifact_path.is_some() {
        active_search_model_header_for(provider)
    } else {
        None
    };
    application::retrieval::load_retrieval_session_with_readers(
        input,
        &infrastructure::artifact::SearchJsonArtifact,
        &infrastructure::artifact::GraphJsonArtifact,
        active_model,
    )
}

pub fn inspect_graph_artifact(input: GraphArtifactInspectionInput) -> ArtifactInspection {
    application::artifact_inspection::inspect_graph_artifact(input)
}

pub fn inspect_search_artifact(input: SearchArtifactInspectionInput) -> ArtifactInspection {
    application::artifact_inspection::inspect_search_artifact(input)
}

/// Resolves the active embedding provider's `SearchModelHeader` without
/// loading the underlying model. Returns `None` when the binary was built
/// without the `embeddings` feature.
fn active_search_model_header_for(
    provider: EmbeddingProviderSelection,
) -> Option<domain::artifact::SearchModelHeader> {
    match provider {
        EmbeddingProviderSelection::Deterministic => {
            Some(infrastructure::embedding::deterministic::DeterministicProvider::metadata_header())
        }
        EmbeddingProviderSelection::Local => local_search_model_header(),
    }
}

fn local_search_model_header() -> Option<domain::artifact::SearchModelHeader> {
    #[cfg(feature = "test-embedding-provider")]
    if use_deterministic_test_embedding_provider() {
        return active_search_model_header_for(EmbeddingProviderSelection::Deterministic);
    }

    // When the test provider is set to force a load failure, return None so
    // that the model-mismatch gate is bypassed.  The failure is then surfaced
    // by `embed_query` itself, which is the code path the caller intends to
    // exercise.
    #[cfg(feature = "test-embedding-provider")]
    if use_force_load_fail_test_embedding_provider() {
        return None;
    }

    #[cfg(feature = "embeddings")]
    {
        Some(infrastructure::embedding::fastembed::FastEmbedProvider::metadata_header())
    }

    #[cfg(not(feature = "embeddings"))]
    {
        None
    }
}

#[cfg(test)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider};

    #[test]
    fn build_workspace_enabled_maps_default_embedding_provider_load_failure() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        let result = build_workspace_with_embedding_provider_factory(
            BuildInput {
                root: workspace.path().to_path_buf(),
                embeddings: BuildEmbeddingMode::Enabled,
                prior_search_artifact_path: None,
            },
            || Err(EmbeddingError::ModelLoad("model unavailable".to_string())),
        );

        assert!(result.has_errors());
        let artifacts = result
            .artifacts
            .expect("clean source still returns V0 artifacts on embedding failure");
        assert!(
            artifacts.search_json.is_none(),
            "failed embedding build should not return a search artifact"
        );
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::EmbedModelLoadFailed)
        );
    }

    #[test]
    fn build_workspace_skipped_does_not_construct_default_embedding_provider() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        let constructed = Cell::new(false);
        let result = build_workspace_with_embedding_provider_factory(
            BuildInput {
                root: workspace.path().to_path_buf(),
                embeddings: BuildEmbeddingMode::Skipped,
                prior_search_artifact_path: None,
            },
            || -> Result<Box<dyn EmbeddingProvider>, EmbeddingError> {
                constructed.set(true);
                panic!("skipped build must not construct embedding provider")
            },
        );

        assert!(!constructed.get());
        assert!(!result.has_errors(), "skipped build should pass");
    }

    #[test]
    fn build_workspace_enabled_does_not_construct_embedding_provider_when_source_has_errors() {
        let workspace = tempfile::tempdir().expect("temp workspace");
        std::fs::write(
            workspace.path().join("guide.adoc"),
            "# Guide @doc(team.guide)\n\n<div>raw</div>\n",
        )
        .expect("source can be written");
        let constructed = Cell::new(false);

        let result = build_workspace_with_embedding_provider_factory(
            BuildInput {
                root: workspace.path().to_path_buf(),
                embeddings: BuildEmbeddingMode::Enabled,
                prior_search_artifact_path: None,
            },
            || -> Result<Box<dyn EmbeddingProvider>, EmbeddingError> {
                constructed.set(true);
                panic!("source errors must not construct embedding provider")
            },
        );

        assert!(!constructed.get());
        assert!(result.has_errors(), "raw HTML should produce an error");
        assert!(result.artifacts.is_none());
        assert!(
            result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::ParseRawHtml)
        );
    }

    #[cfg(not(feature = "embeddings"))]
    #[test]
    fn build_workspace_enabled_without_embeddings_feature_reports_model_load_failure() {
        let workspace = tempfile::tempdir().expect("temp workspace");

        let result = build_workspace(BuildInput {
            root: workspace.path().to_path_buf(),
            embeddings: BuildEmbeddingMode::Enabled,
            prior_search_artifact_path: None,
        });

        assert!(result.has_errors());
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::EmbedModelLoadFailed)
            .expect("model load diagnostic");
        assert!(
            diagnostic.message.contains("`embeddings` feature"),
            "diagnostic should explain the missing feature: {diagnostic:?}"
        );
        assert!(
            diagnostic
                .help
                .as_deref()
                .expect("help")
                .contains("--no-embeddings")
        );
    }

    #[test]
    fn embed_query_compute_error_does_not_leak_debug_format() {
        let err = map_provider_error(EmbeddingError::Compute("encoder failed".to_string()));
        let msg = err.to_string();
        assert!(
            !msg.contains("Compute("),
            "Debug variant name must not appear in user message: {msg}"
        );
        assert!(
            !msg.contains('"'),
            "Debug-style quotes must not appear in user message: {msg}"
        );
        assert!(
            !msg.contains('{'),
            "Debug-style braces must not appear in user message: {msg}"
        );
        assert!(
            msg.contains("encoder failed"),
            "Inner message must be preserved verbatim: {msg}"
        );
    }

    #[test]
    fn embed_query_model_load_error_does_not_leak_debug_format() {
        let err = map_provider_error(EmbeddingError::ModelLoad("model unavailable".to_string()));
        let msg = err.to_string();
        assert!(
            !msg.contains("ModelLoad("),
            "Debug variant name must not appear in user message: {msg}"
        );
        assert!(
            !msg.contains('"'),
            "Debug-style quotes must not appear in user message: {msg}"
        );
        assert!(
            !msg.contains('{'),
            "Debug-style braces must not appear in user message: {msg}"
        );
        assert!(
            msg.contains("model unavailable"),
            "Inner message must be preserved verbatim: {msg}"
        );
    }

    #[test]
    fn embed_query_dimension_mismatch_maps_to_compute_with_dims() {
        let err = map_provider_error(EmbeddingError::DimensionMismatch {
            expected: 384,
            actual: 512,
        });
        let msg = err.to_string();
        assert!(
            matches!(err, EmbedQueryError::Compute(_)),
            "DimensionMismatch must map to Compute variant"
        );
        assert!(
            msg.contains("384"),
            "Expected dim must appear in message: {msg}"
        );
        assert!(
            msg.contains("512"),
            "Actual dim must appear in message: {msg}"
        );
        assert!(
            !msg.contains("DimensionMismatch"),
            "Debug variant name must not appear in user message: {msg}"
        );
    }

    #[cfg(feature = "test-embedding-provider")]
    #[test]
    fn test_embedding_provider_env_uses_deterministic_only_when_explicitly_requested() {
        temp_env_remove("ADOC_TEST_EMBEDDING_PROVIDER", || {
            assert!(!use_deterministic_test_embedding_provider());
        });
        temp_env_set("ADOC_TEST_EMBEDDING_PROVIDER", "fastembed", || {
            assert!(!use_deterministic_test_embedding_provider());
        });
        temp_env_set("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic", || {
            assert!(use_deterministic_test_embedding_provider());
        });
        temp_env_set("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory", || {
            assert!(use_deterministic_test_embedding_provider());
        });
    }

    #[cfg(feature = "test-embedding-provider")]
    fn temp_env_remove(name: &str, test: impl FnOnce()) {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::remove_var(name);
        }
        test();
        restore_env(name, previous);
    }

    #[cfg(feature = "test-embedding-provider")]
    fn temp_env_set(name: &str, value: &str, test: impl FnOnce()) {
        let previous = std::env::var_os(name);
        unsafe {
            std::env::set_var(name, value);
        }
        test();
        restore_env(name, previous);
    }

    #[cfg(feature = "test-embedding-provider")]
    fn restore_env(name: &str, value: Option<std::ffi::OsString>) {
        unsafe {
            match value {
                Some(value) => std::env::set_var(name, value),
                None => std::env::remove_var(name),
            }
        }
    }
}

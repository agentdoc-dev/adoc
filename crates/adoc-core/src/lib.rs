mod application;
mod domain;
mod infrastructure;

pub use application::compile::{
    BuildArtifacts, BuildEmbeddingMode, BuildInput, CompileInput, CompileResult,
};
pub use application::ports::{Clock, RecordResolver, ResolverError};
pub use application::retrieval::{
    ExplainResult, RETRIEVAL_SCHEMA_VERSION, RetrievalEnvelope, RetrievalInput,
    RetrievalLoadResult, RetrievalSession, SearchFilters, SearchQuery, SearchResult,
    explain_object, search,
};
pub use application::services::{ExplainError, ExplainService};
pub use application::views::{ExpiresInfo, ExplainView, RenderMeta};
pub use domain::artifact::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan,
    SearchArtifactDocument,
};
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use domain::retrieval::{RetrievalMatch, RetrievalRecord, RetrievalSource, SearchMode};

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
    let provider = default_embedding_provider().map_err(map_provider_error)?;
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
    build_workspace_with_embedding_provider_factory(input, default_embedding_provider)
}

fn default_embedding_provider() -> Result<
    Box<dyn domain::ports::embedding_provider::EmbeddingProvider>,
    domain::ports::embedding_provider::EmbeddingError,
> {
    #[cfg(feature = "test-embedding-provider")]
    if use_in_memory_test_embedding_provider() {
        return Ok(Box::new(
            infrastructure::embedding::in_memory::InMemoryProvider::new(384),
        ));
    }

    #[cfg(feature = "embeddings")]
    {
        infrastructure::embedding::fastembed::FastEmbedProvider::try_new().map(|provider| {
            Box::new(provider) as Box<dyn domain::ports::embedding_provider::EmbeddingProvider>
        })
    }

    #[cfg(not(feature = "embeddings"))]
    {
        Err(domain::ports::embedding_provider::EmbeddingError::ModelLoad(
            "embedding support is disabled; rebuild with the `embeddings` feature or run with --no-embeddings".to_string(),
        ))
    }
}

#[cfg(feature = "test-embedding-provider")]
fn use_in_memory_test_embedding_provider() -> bool {
    std::env::var("ADOC_TEST_EMBEDDING_PROVIDER").as_deref() == Ok("in-memory")
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
    // Only resolve the active provider's model header when a search-artifact
    // path is provided; lexical-only callers must not pay the embedding-model
    // metadata lookup. Resolution itself is metadata-only — no model download.
    let active_model = if input.search_artifact_path.is_some() {
        active_search_model_header()
    } else {
        None
    };
    application::retrieval::load_retrieval_session_with_reader(
        input,
        &infrastructure::artifact::AgentJsonArtifact,
        active_model,
    )
}

/// Resolves the active embedding provider's `SearchModelHeader` without
/// loading the underlying model. Returns `None` when the binary was built
/// without the `embeddings` feature.
fn active_search_model_header() -> Option<domain::artifact::SearchModelHeader> {
    #[cfg(feature = "test-embedding-provider")]
    if use_in_memory_test_embedding_provider() {
        return Some(infrastructure::embedding::in_memory::InMemoryProvider::metadata_header(384));
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
    fn test_embedding_provider_env_uses_in_memory_only_when_explicitly_requested() {
        temp_env_remove("ADOC_TEST_EMBEDDING_PROVIDER", || {
            assert!(!use_in_memory_test_embedding_provider());
        });
        temp_env_set("ADOC_TEST_EMBEDDING_PROVIDER", "fastembed", || {
            assert!(!use_in_memory_test_embedding_provider());
        });
        temp_env_set("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory", || {
            assert!(use_in_memory_test_embedding_provider());
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

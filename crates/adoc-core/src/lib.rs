mod application;
mod domain;
mod infrastructure;

pub use application::compile::{
    BuildArtifacts, BuildEmbeddingMode, BuildInput, CompileInput, CompileResult,
};
pub use application::retrieval::{
    ExplainResult, RETRIEVAL_SCHEMA_VERSION, RetrievalEnvelope, RetrievalInput,
    RetrievalLoadResult, RetrievalSession, SearchFilters, SearchQuery, SearchResult,
    explain_object, search,
};
pub use application::retrieval_format::{
    JsonRetrievalFormatter, RetrievalFormatError, RetrievalFormatter, TextRetrievalFormatter,
};
pub use domain::artifact::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan,
    SearchArtifactDocument,
};
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use domain::retrieval::{RetrievalMatch, RetrievalRecord, RetrievalSource, SearchMode};

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    application::compile::compile_with_provider(&provider)
}

pub fn build_workspace(input: BuildInput) -> CompileResult {
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    match input.embeddings {
        BuildEmbeddingMode::Enabled => {
            let embedding_provider =
                infrastructure::embedding::in_memory::InMemoryProvider::new(384);
            application::compile::build_with_provider(
                &provider,
                application::compile::BuildOptions {
                    embeddings: application::compile::BuildEmbeddingBehavior::Enabled {
                        provider: &embedding_provider,
                    },
                    prior_search_artifact_path: input.prior_search_artifact_path,
                },
            )
        }
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
    application::retrieval::load_retrieval_session_with_reader(
        input,
        &infrastructure::artifact::AgentJsonArtifact,
    )
}

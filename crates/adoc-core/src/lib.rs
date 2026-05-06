mod application;
mod domain;
mod infrastructure;

pub use application::compile::{BuildArtifacts, CompileInput, CompileResult};
pub use application::retrieval::{
    ExplainResult, RETRIEVAL_SCHEMA_VERSION, RetrievalEnvelope, RetrievalInput,
    RetrievalLoadResult, RetrievalSession, explain_object, load_retrieval_session,
};
pub use domain::artifact::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan,
};
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
pub use domain::retrieval::{RetrievalRecord, RetrievalSource};

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    application::compile::compile_with_provider(&provider)
}

mod application;
mod domain;
mod infrastructure;

pub use application::compile::{BuildArtifacts, CompileInput, CompileResult};
pub use domain::artifact::AgentJsonDocument;
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let provider = infrastructure::source::fs::FsSourceProvider::new(input.root);
    application::compile::compile_with_provider(&provider)
}

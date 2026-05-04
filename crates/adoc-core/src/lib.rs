mod application;
mod compile;
mod domain;
mod infrastructure;
mod parser;
mod validate;

pub use domain::artifact::AgentJsonDocument;
pub use compile::{BuildArtifacts, CompileInput, CompileResult, compile_workspace};
pub use domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};

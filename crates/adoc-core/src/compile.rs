use std::path::PathBuf;

use crate::artifact::{AgentJsonArtifact, AgentJsonDocument, ArtifactWriter};
use crate::ast::WorkspaceAst;
use crate::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::parser::parse_page;
use crate::render::{HtmlRenderer, Renderer};
use crate::source_provider::{FsSourceProvider, SourceProvider};

#[derive(Debug, Clone)]
pub struct CompileInput {
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct CompileResult {
    pub diagnostics: Vec<Diagnostic>,
    pub artifacts: Option<BuildArtifacts>,
}

impl CompileResult {
    pub fn has_errors(&self) -> bool {
        self.diagnostics
            .iter()
            .any(|diagnostic| diagnostic.severity == Severity::Error)
    }
}

#[derive(Debug, Clone)]
pub struct BuildArtifacts {
    pub html: String,
    pub agent_json: AgentJsonDocument,
}

pub fn compile_workspace(input: CompileInput) -> CompileResult {
    let provider = FsSourceProvider::new(input.root);
    compile_with_provider(&provider)
}

pub(crate) fn compile_with_provider<P: SourceProvider>(provider: &P) -> CompileResult {
    let mut diagnostics = Vec::new();
    let mut pages = Vec::new();

    for result in provider.load_sources() {
        match result {
            Ok(source) => {
                let (page, page_diagnostics) = parse_page(&source);
                diagnostics.extend(page_diagnostics);
                pages.push(page);
            }
            Err(load_error) => diagnostics.push(Diagnostic::error(
                DiagnosticCode::IoUnreadableFile,
                format!(
                    "could not read AgentDoc Source {}: {}",
                    load_error.path.display(),
                    load_error.message,
                ),
            )),
        }
    }

    let _workspace = WorkspaceAst {
        pages: pages.clone(),
    };

    let artifacts = diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        .then(|| BuildArtifacts {
            html: HtmlRenderer.render(&pages),
            agent_json: AgentJsonArtifact.write(&pages, &diagnostics),
        });

    CompileResult {
        diagnostics,
        artifacts,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SourceFile;
    use crate::source_provider::InMemorySourceProvider;

    fn source_file(identity: &str, text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from(format!("/work/{identity}")),
            text.to_string(),
            PathBuf::from(identity),
        )
    }

    #[test]
    fn compile_with_provider_emits_artifacts_for_clean_source() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\nHello.\n",
        ));

        let result = compile_with_provider(&provider);

        assert!(!result.has_errors());
        assert!(result.artifacts.is_some(), "expected artifacts to be built");
    }

    #[test]
    fn compile_with_provider_blocks_artifacts_when_source_has_errors() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\n<div>x</div>\n",
        ));

        let result = compile_with_provider(&provider);

        assert!(result.has_errors(), "raw HTML should produce an error");
        assert!(
            result.artifacts.is_none(),
            "artifacts must not be produced when errors are present"
        );
    }

    #[test]
    fn compile_with_provider_translates_load_error_into_io_diagnostic() {
        let provider = InMemorySourceProvider::new()
            .with_error(PathBuf::from("/work/missing.adoc"), "permission denied");

        let result = compile_with_provider(&provider);

        assert_eq!(result.diagnostics.len(), 1);
        let diagnostic = &result.diagnostics[0];
        assert_eq!(diagnostic.code, "io.unreadable_file");
        assert_eq!(diagnostic.severity, Severity::Error);
        assert!(
            diagnostic
                .message
                .contains("could not read AgentDoc Source /work/missing.adoc")
        );
        assert!(diagnostic.message.contains("permission denied"));
        assert!(result.artifacts.is_none());
    }

    #[test]
    fn compile_with_provider_emits_empty_artifacts_for_empty_workspace() {
        let provider = InMemorySourceProvider::new();

        let result = compile_with_provider(&provider);

        assert!(!result.has_errors());
        let artifacts = result.artifacts.expect("empty workspace still builds");
        assert!(artifacts.agent_json.pages.is_empty());
    }
}

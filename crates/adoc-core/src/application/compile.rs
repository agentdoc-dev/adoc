use std::cmp::Ordering;
use std::path::PathBuf;

use crate::application::resolve_knowledge_objects::{
    resolve_knowledge_objects, suppress_unknown_kind_shape_diagnostics,
};
use crate::application::resolve_object_references::resolve_object_references;
use crate::domain::artifact::{AgentJsonDocument, SearchArtifactDocument};
use crate::domain::ast::{PageAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::ports::artifact_writer::ArtifactWriter;
use crate::domain::ports::renderer::Renderer;
use crate::domain::ports::source_provider::{SourceLoadError, SourceLoadErrorKind, SourceProvider};
use crate::domain::source::SourceFile;
use crate::infrastructure::artifact::AgentJsonArtifact;
use crate::infrastructure::parser::parse_page;
use crate::infrastructure::render::HtmlRenderer;
use crate::infrastructure::validate::{
    validate_resolved_page, validate_source_page, validate_workspace,
};

#[derive(Debug, Clone)]
pub struct CompileInput {
    /// Input path for compilation: either one `.adoc` file or a directory that
    /// will be scanned recursively for `.adoc` files.
    pub root: PathBuf,
}

#[derive(Debug, Clone)]
pub struct BuildInput {
    /// Input path for compilation: either one `.adoc` file or a directory that
    /// will be scanned recursively for `.adoc` files.
    pub root: PathBuf,
    /// Build-time embedding behavior. `adoc check` never uses this path.
    pub embeddings: BuildEmbeddingMode,
    /// Existing `docs.search.json` from the output directory, used by later
    /// embedding slices for vector reuse.
    pub prior_search_artifact_path: Option<PathBuf>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BuildEmbeddingMode {
    Enabled,
    Skipped,
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
    pub search_json: Option<SearchArtifactDocument>,
}

pub(crate) fn compile_with_provider<P: SourceProvider>(provider: &P) -> CompileResult {
    run_compile_pipeline(provider, None)
}

pub(crate) fn build_with_provider<P: SourceProvider>(
    provider: &P,
    options: BuildOptions,
) -> CompileResult {
    run_compile_pipeline(provider, Some(options))
}

#[derive(Debug, Clone)]
pub(crate) struct BuildOptions {
    pub(crate) embeddings: BuildEmbeddingMode,
    pub(crate) prior_search_artifact_path: Option<PathBuf>,
}

fn run_compile_pipeline<P: SourceProvider>(
    provider: &P,
    build_options: Option<BuildOptions>,
) -> CompileResult {
    // Pipeline stages: load → validate-source-pages → resolve-KOs →
    // resolve-object-references → validate-resolved-pages → assemble →
    // validate-workspace → build. Each stage is a separate function below so
    // it can be unit-tested in isolation and the orchestrator reads as a
    // sequence of named domain operations rather than one walls-of-text loop.
    // Pages move through the pipeline without cloning. See ADR-0006 addendum.
    let (mut parsed, mut diagnostics) = load_pages(provider);
    diagnostics.extend(validate_source_pages(&parsed));
    suppress_unknown_kind_shape_diagnostics(&parsed, &mut diagnostics);
    let resolved_knowledge_objects = resolve_knowledge_objects(&mut parsed);
    diagnostics.extend(resolved_knowledge_objects.diagnostics);
    diagnostics.extend(resolve_object_references(
        &mut parsed,
        &resolved_knowledge_objects.declared_ids,
    ));
    diagnostics.extend(validate_resolved_pages(&parsed));
    let workspace = assemble_workspace(parsed);
    diagnostics.extend(validate_workspace(&workspace));
    if let Some(options) = build_options {
        diagnostics.extend(build_embedding_diagnostics(&options));
    }
    sort_diagnostics_by_source(&mut diagnostics);
    let artifacts = build_artifacts(&workspace, &diagnostics);
    CompileResult {
        diagnostics,
        artifacts,
    }
}

fn build_embedding_diagnostics(options: &BuildOptions) -> Vec<Diagnostic> {
    let _prior_search_artifact_path = &options.prior_search_artifact_path;
    match options.embeddings {
        BuildEmbeddingMode::Enabled => Vec::new(),
        BuildEmbeddingMode::Skipped => vec![Diagnostic::info(
            DiagnosticCode::BuildEmbeddingsSkipped,
            "Embedding generation skipped; docs.search.json was not written.",
        )],
    }
}

/// Load every source the provider yields, parse each successfully-loaded one
/// into a `PageAst`, and translate load failures into I/O diagnostics. Returns
/// the (source, page) pairs for downstream validation plus the parse-time and
/// load-time diagnostics in source order.
fn load_pages<P: SourceProvider>(provider: &P) -> (Vec<(SourceFile, PageAst)>, Vec<Diagnostic>) {
    let mut parsed = Vec::new();
    let mut diagnostics = Vec::new();
    for result in provider.load_sources() {
        match result {
            Ok(source) => {
                let (page, parse_diagnostics) = parse_page(&source);
                diagnostics.extend(parse_diagnostics);
                parsed.push((source, page));
            }
            Err(load_error) => diagnostics.push(load_error_diagnostic(load_error)),
        }
    }
    (parsed, diagnostics)
}

fn load_error_diagnostic(load_error: SourceLoadError) -> Diagnostic {
    match load_error.kind {
        SourceLoadErrorKind::Unreadable => Diagnostic::error(
            DiagnosticCode::IoUnreadableFile,
            format!(
                "could not read AgentDoc Source {}: {}",
                load_error.path.display(),
                load_error.message,
            ),
        ),
        SourceLoadErrorKind::UnreadableDirectory => Diagnostic::error(
            DiagnosticCode::IoUnreadableDirectory,
            format!(
                "could not read AgentDoc Source directory {}: {}",
                load_error.path.display(),
                load_error.message,
            ),
        ),
        SourceLoadErrorKind::UnsupportedSourceExtension => Diagnostic::error(
            DiagnosticCode::IoUnsupportedSourceExtension,
            format!(
                "unsupported AgentDoc Source extension for {}; expected a .adoc file",
                load_error.path.display(),
            ),
        ),
    }
}

/// Run every source-page rule against the (source, page) pairs in order.
/// These rules see parser output, including pending Knowledge Objects.
fn validate_source_pages(parsed: &[(SourceFile, PageAst)]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (source, page) in parsed {
        diagnostics.extend(validate_source_page(page, source));
    }
    diagnostics
}

/// Run every resolved-page rule after Knowledge Object resolution. Rules in
/// this phase can rely on typed aggregate data instead of pending parser shells.
fn validate_resolved_pages(parsed: &[(SourceFile, PageAst)]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (source, page) in parsed {
        diagnostics.extend(validate_resolved_page(page, source));
    }
    diagnostics
}

/// Consume the (source, page) pairs into the final aggregate. Sources are
/// dropped here — they're no longer needed once per-page rules have run.
fn assemble_workspace(parsed: Vec<(SourceFile, PageAst)>) -> WorkspaceAst {
    WorkspaceAst {
        pages: parsed.into_iter().map(|(_, page)| page).collect(),
    }
}

/// Gate artifacts on diagnostic severity: produce an HTML + agent JSON pair
/// only when no diagnostic has `Severity::Error`. Renderer and ArtifactWriter
/// ports are statically dispatched per ADR-0006.
fn build_artifacts(workspace: &WorkspaceAst, diagnostics: &[Diagnostic]) -> Option<BuildArtifacts> {
    diagnostics
        .iter()
        .all(|diagnostic| diagnostic.severity != Severity::Error)
        .then(|| BuildArtifacts {
            html: HtmlRenderer.render(&workspace.pages),
            agent_json: AgentJsonArtifact.build(&workspace.pages, diagnostics),
            search_json: None,
        })
}

fn sort_diagnostics_by_source(diagnostics: &mut [Diagnostic]) {
    diagnostics.sort_by(|left, right| match (&left.span, &right.span) {
        (Some(left_span), Some(right_span)) => left_span
            .file
            .cmp(&right_span.file)
            .then_with(|| left_span.start.line.cmp(&right_span.start.line))
            .then_with(|| left_span.start.column.cmp(&right_span.start.column)),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::ast::BlockAst;
    use crate::domain::source::SourceFile;
    use crate::infrastructure::source::in_memory::InMemorySourceProvider;

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
        assert_eq!(diagnostic.code, DiagnosticCode::IoUnreadableFile);
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

    // --- stage-level pipeline tests (TB-7) ---

    #[test]
    fn load_pages_returns_parsed_pairs_in_provider_order() {
        let provider = InMemorySourceProvider::new()
            .with_source(source_file("team/alpha.adoc", "# Alpha\n"))
            .with_source(source_file("team/beta.adoc", "# Beta\n"));

        let (parsed, diagnostics) = load_pages(&provider);

        assert_eq!(parsed.len(), 2);
        assert_eq!(parsed[0].1.title.as_deref(), Some("Alpha"));
        assert_eq!(parsed[1].1.title.as_deref(), Some("Beta"));
        assert!(diagnostics.is_empty());
    }

    #[test]
    fn load_pages_translates_load_error_into_io_diagnostic() {
        let provider = InMemorySourceProvider::new()
            .with_error(PathBuf::from("/work/missing.adoc"), "permission denied");

        let (parsed, diagnostics) = load_pages(&provider);

        assert!(parsed.is_empty());
        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::IoUnreadableFile);
    }

    #[test]
    fn load_error_diagnostic_maps_unreadable_directory_to_io_diagnostic() {
        let diagnostic = load_error_diagnostic(SourceLoadError::unreadable_directory(
            PathBuf::from("/work/blocked"),
            "permission denied",
        ));

        assert_eq!(diagnostic.code, DiagnosticCode::IoUnreadableDirectory);
        assert_eq!(diagnostic.severity, Severity::Error);
        assert!(
            diagnostic
                .message
                .contains("could not read AgentDoc Source directory /work/blocked")
        );
        assert!(diagnostic.message.contains("permission denied"));
    }

    #[test]
    fn validate_source_pages_emits_per_page_diagnostics_in_source_order() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\n<div>x</div>\n",
        ));

        let (parsed, _) = load_pages(&provider);
        let diagnostics = validate_source_pages(&parsed);

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
    }

    #[test]
    fn validate_resolved_pages_returns_empty_for_page_without_knowledge_objects() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\nHello.\n",
        ));

        let (parsed, _) = load_pages(&provider);
        let diagnostics = validate_resolved_pages(&parsed);

        assert!(diagnostics.is_empty());
    }

    #[test]
    fn assemble_workspace_drops_sources_and_keeps_pages_in_order() {
        let provider = InMemorySourceProvider::new()
            .with_source(source_file("a.adoc", "# A\n"))
            .with_source(source_file("b.adoc", "# B\n"));

        let (parsed, _) = load_pages(&provider);
        let workspace = assemble_workspace(parsed);

        assert_eq!(workspace.pages.len(), 2);
        assert_eq!(workspace.pages[0].title.as_deref(), Some("A"));
        assert_eq!(workspace.pages[1].title.as_deref(), Some("B"));
    }

    #[test]
    fn build_artifacts_returns_some_when_no_errors() {
        let provider =
            InMemorySourceProvider::new().with_source(source_file("guide.adoc", "# Guide\n"));
        let (parsed, _) = load_pages(&provider);
        let workspace = assemble_workspace(parsed);

        let artifacts = build_artifacts(&workspace, &[]);

        let artifacts = artifacts.expect("clean workspace yields artifacts");
        assert!(artifacts.html.contains("<h1>Guide</h1>"));
    }

    #[test]
    fn build_artifacts_returns_none_when_any_diagnostic_is_error() {
        let workspace = WorkspaceAst { pages: Vec::new() };
        let error_diagnostic = Diagnostic::error(DiagnosticCode::ParseRawHtml, "x");

        let artifacts = build_artifacts(&workspace, &[error_diagnostic]);

        assert!(artifacts.is_none());
    }

    #[test]
    fn compile_with_provider_resolves_a_top_level_claim_into_kos() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "Some prose.\n\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "The system credits users automatically.\n",
                "::\n",
            ),
        ));

        let result = compile_with_provider(&provider);

        assert!(
            !result.has_errors(),
            "expected no errors, got: {:?}",
            result.diagnostics
        );
        assert!(result.artifacts.is_some(), "artifacts must be produced");

        // Walk the parsed page to verify the KnowledgeObject block is present.
        // We re-parse to inspect blocks (compile only exposes artifacts).
        let source = source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "Some prose.\n\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "The system credits users automatically.\n",
                "::\n",
            ),
        );
        let provider2 = InMemorySourceProvider::new().with_source(source);
        let (mut parsed, _) = load_pages(&provider2);
        resolve_knowledge_objects(&mut parsed);

        let page = &parsed[0].1;
        let ko_count = page
            .blocks
            .iter()
            .filter(|b| matches!(b, BlockAst::KnowledgeObject(_)))
            .count();
        assert_eq!(ko_count, 1, "exactly one KnowledgeObject block expected");
    }

    #[test]
    fn compile_with_provider_drops_invalid_claim_and_blocks_artifacts() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::claim billing.credits\n",
                // no status field
                "--\n",
                "The system credits users automatically.\n",
                "::\n",
            ),
        ));

        let result = compile_with_provider(&provider);

        assert!(result.has_errors(), "missing status must produce an error");
        assert!(
            result.artifacts.is_none(),
            "artifacts must be blocked on error"
        );
        assert_eq!(
            result.diagnostics.len(),
            1,
            "exactly one diagnostic expected, got: {:?}",
            result.diagnostics
        );
        assert_eq!(
            result.diagnostics[0].code,
            DiagnosticCode::SchemaMissingField,
            "expected SchemaMissingField"
        );
    }
}

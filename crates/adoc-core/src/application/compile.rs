use std::cmp::Ordering;
use std::path::PathBuf;

use chrono::NaiveDate;

use crate::application::resolve_knowledge_objects::{
    resolve_knowledge_objects, suppress_unknown_kind_shape_diagnostics,
};
use crate::application::resolve_object_references::resolve_object_references;
use crate::domain::ast::{PageAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, Severity};
use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider};
use crate::domain::ports::source_provider::{SourceLoadError, SourceLoadErrorKind, SourceProvider};
use crate::domain::source::SourceFile;
use crate::infrastructure::artifact::GraphJsonArtifact;
use crate::infrastructure::render::HtmlRenderer;
use crate::infrastructure::validate::mode_pipeline::pipeline_for;
use crate::infrastructure::validate::validate_workspace;

use super::local_today;
use super::search_artifact::{
    build_search_artifact, cache_count_diagnostic, embedding_error_diagnostic,
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
    pub graph_json: String,
    pub search_json: Option<String>,
}

pub(crate) fn compile_with_provider<P: SourceProvider>(provider: &P) -> CompileResult {
    compile_with_provider_for_date(provider, local_today())
}

pub(crate) fn compile_with_provider_for_date<P: SourceProvider>(
    provider: &P,
    today: NaiveDate,
) -> CompileResult {
    run_compile_pipeline(provider, None, today)
}

pub(crate) fn build_with_provider<P: SourceProvider>(
    provider: &P,
    options: BuildOptions<'_>,
) -> CompileResult {
    build_with_provider_for_date(provider, options, local_today())
}

pub(crate) fn build_with_provider_for_date<P: SourceProvider>(
    provider: &P,
    options: BuildOptions<'_>,
    today: NaiveDate,
) -> CompileResult {
    run_compile_pipeline(provider, Some(options), today)
}

pub(crate) struct BuildOptions<'a> {
    pub(crate) embeddings: BuildEmbeddingBehavior<'a>,
    pub(crate) prior_search_artifact_path: Option<PathBuf>,
}

pub(crate) enum BuildEmbeddingBehavior<'a> {
    #[cfg(test)]
    Enabled {
        provider: &'a dyn EmbeddingProvider,
    },
    EnabledFactory {
        provider_factory: &'a mut dyn FnMut() -> Result<Box<dyn EmbeddingProvider>, EmbeddingError>,
    },
    Skipped,
}

fn run_compile_pipeline<P: SourceProvider>(
    provider: &P,
    mut build_options: Option<BuildOptions<'_>>,
    today: NaiveDate,
) -> CompileResult {
    // Pipeline stages: load → validate-source-pages → resolve-KOs →
    // resolve-object-references → validate-resolved-pages → assemble →
    // validate-workspace → build. Each stage is a separate function below so
    // it can be unit-tested in isolation and the orchestrator reads as a
    // sequence of named domain operations rather than one walls-of-text loop.
    // Pages move through the pipeline without cloning. See ADR-0006 addendum.
    //
    // V4 stores `SourceMode` on each `SourceFile` at the SourceProvider
    // boundary (ADR-0022) so the validators dispatch between strict and
    // compatibility rule sets without re-deriving from the path. Knowledge
    // Object resolution is mode-agnostic — Markdown sources produce no
    // `KnowledgeObjectPending` blocks, so the resolver is a no-op for them.
    let (mut parsed, mut diagnostics) = load_pages(provider);
    diagnostics.extend(validate_source_pages(&parsed));
    suppress_unknown_kind_shape_diagnostics(&parsed, &mut diagnostics);
    let resolved_knowledge_objects = resolve_knowledge_objects(&mut parsed);
    diagnostics.extend(resolved_knowledge_objects.diagnostics);
    diagnostics.extend(resolve_object_references(
        &mut parsed,
        &resolved_knowledge_objects.declared_ids,
    ));
    diagnostics.extend(validate_resolved_pages(&parsed, today));
    let workspace = assemble_workspace(parsed);
    diagnostics.extend(validate_workspace(&workspace));
    if let Some(options) = &build_options {
        diagnostics.extend(build_embedding_diagnostics(options));
    }
    let artifact_result = build_artifacts_for_build(
        &workspace,
        &diagnostics,
        build_options.as_mut(),
        Some(today),
    );
    let artifacts = artifact_result.artifacts;
    diagnostics.extend(artifact_result.diagnostics);
    sort_diagnostics_by_source(&mut diagnostics);
    CompileResult {
        diagnostics,
        artifacts,
    }
}

fn build_embedding_diagnostics(options: &BuildOptions<'_>) -> Vec<Diagnostic> {
    let _prior_search_artifact_path = &options.prior_search_artifact_path;
    match options.embeddings {
        #[cfg(test)]
        BuildEmbeddingBehavior::Enabled { .. } => Vec::new(),
        BuildEmbeddingBehavior::EnabledFactory { .. } => Vec::new(),
        BuildEmbeddingBehavior::Skipped => vec![Diagnostic::info(
            DiagnosticCode::BuildEmbeddingsSkipped,
            "Embedding generation skipped; docs.search.json was not written.",
        )],
    }
}

/// Load every source the provider yields, parse each successfully-loaded one
/// into a `PageAst`, and translate load failures into I/O diagnostics. Returns
/// `(source, page)` pairs for downstream validation plus the parse-time and
/// load-time diagnostics in source order. The parser is dispatched by
/// `source.mode()` — Strict for `.adoc`, Compat for `.md` per ADR-0022 — and
/// the mode is carried along on the source itself so downstream stages never
/// re-derive it.
fn load_pages<P: SourceProvider>(provider: &P) -> (Vec<(SourceFile, PageAst)>, Vec<Diagnostic>) {
    let mut parsed = Vec::new();
    let mut diagnostics = Vec::new();
    for result in provider.load_sources() {
        match result {
            Ok(source) => {
                let pipeline = pipeline_for(source.mode());
                let (page, parse_diagnostics) = (pipeline.parse)(&source);
                diagnostics.extend(parse_diagnostics);
                parsed.push((source, page));
            }
            Err(load_error) => diagnostics.push(load_error_diagnostic(load_error)),
        }
    }
    (parsed, diagnostics)
}

pub(crate) fn load_error_diagnostic(load_error: SourceLoadError) -> Diagnostic {
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
                "unsupported AgentDoc Source extension for {}; expected a .adoc or .md file",
                load_error.path.display(),
            ),
        ),
    }
}

/// Run every source-page rule against the `(source, page)` pairs in order,
/// dispatching by the per-mode pipeline selected from `source.mode()`. The
/// orchestrator does not know which rules run — that selection is data in
/// [`pipeline_for`].
fn validate_source_pages(parsed: &[(SourceFile, PageAst)]) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (source, page) in parsed {
        let pipeline = pipeline_for(source.mode());
        diagnostics.extend((pipeline.validate_source_page)(page, source));
    }
    diagnostics
}

/// Run every resolved-page rule after Knowledge Object resolution. The
/// per-mode pipeline owns the policy — Compat returns an empty Vec because
/// its `ResolvedPagePolicy` is `Empty`, not because the orchestrator branches.
fn validate_resolved_pages(parsed: &[(SourceFile, PageAst)], today: NaiveDate) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for (source, page) in parsed {
        let pipeline = pipeline_for(source.mode());
        diagnostics.extend(pipeline.validate_resolved_page.run(page, source, today));
    }
    diagnostics
}

/// Consume the `(source, page)` pairs into the final aggregate. Sources are
/// dropped here — they're no longer needed once per-page rules have run.
fn assemble_workspace(parsed: Vec<(SourceFile, PageAst)>) -> WorkspaceAst {
    WorkspaceAst {
        pages: parsed.into_iter().map(|(_, page)| page).collect(),
    }
}

/// Gate artifacts on diagnostic severity: produce derived read artifacts only
/// when no diagnostic has `Severity::Error`. ArtifactWriter ports are
/// statically dispatched per ADR-0006.
#[cfg(test)]
fn build_artifacts(workspace: &WorkspaceAst, diagnostics: &[Diagnostic]) -> Option<BuildArtifacts> {
    build_artifacts_for_build(workspace, diagnostics, None, None).artifacts
}

struct ArtifactBuildResult {
    artifacts: Option<BuildArtifacts>,
    diagnostics: Vec<Diagnostic>,
}

fn build_artifacts_for_build(
    workspace: &WorkspaceAst,
    diagnostics: &[Diagnostic],
    mut build_options: Option<&mut BuildOptions<'_>>,
    today: Option<NaiveDate>,
) -> ArtifactBuildResult {
    if diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return ArtifactBuildResult {
            artifacts: None,
            diagnostics: Vec::new(),
        };
    }

    let graph_diagnostics = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.code != DiagnosticCode::BuildEmbeddingsSkipped)
        .cloned()
        .collect::<Vec<_>>();
    let graph_document = GraphJsonArtifact.build_for_date(workspace, &graph_diagnostics, today);
    let graph_json = graph_document
        .to_pretty_json()
        .expect("graph artifact serialization should not fail");
    let html = HtmlRenderer.render_workspace_for_date(workspace, today);
    let prior_search_artifact_path = build_options
        .as_ref()
        .and_then(|options| options.prior_search_artifact_path.clone());
    let mut artifact_diagnostics = Vec::new();
    let search_json = match build_options
        .as_mut()
        .map(|options| &mut options.embeddings)
    {
        #[cfg(test)]
        Some(BuildEmbeddingBehavior::Enabled { provider }) => {
            match build_search_artifact(
                &graph_document,
                &graph_json,
                *provider,
                prior_search_artifact_path.as_ref(),
            ) {
                Ok(search_build) => {
                    artifact_diagnostics.extend(search_build.diagnostics);
                    if search_build.cached_count != 0 || search_build.computed_count != 0 {
                        artifact_diagnostics.push(cache_count_diagnostic(
                            search_build.cached_count,
                            search_build.computed_count,
                        ));
                    }
                    Some(search_build.json)
                }
                Err(diagnostics) => {
                    return ArtifactBuildResult {
                        artifacts: Some(BuildArtifacts {
                            html,
                            graph_json,
                            search_json: None,
                        }),
                        diagnostics,
                    };
                }
            }
        }
        Some(BuildEmbeddingBehavior::EnabledFactory { provider_factory }) => {
            let provider = match provider_factory() {
                Ok(provider) => provider,
                Err(error) => {
                    return ArtifactBuildResult {
                        artifacts: Some(BuildArtifacts {
                            html,
                            graph_json,
                            search_json: None,
                        }),
                        diagnostics: vec![embedding_error_diagnostic(error)],
                    };
                }
            };
            match build_search_artifact(
                &graph_document,
                &graph_json,
                provider.as_ref(),
                prior_search_artifact_path.as_ref(),
            ) {
                Ok(search_build) => {
                    artifact_diagnostics.extend(search_build.diagnostics);
                    if search_build.cached_count != 0 || search_build.computed_count != 0 {
                        artifact_diagnostics.push(cache_count_diagnostic(
                            search_build.cached_count,
                            search_build.computed_count,
                        ));
                    }
                    Some(search_build.json)
                }
                Err(diagnostics) => {
                    return ArtifactBuildResult {
                        artifacts: Some(BuildArtifacts {
                            html,
                            graph_json,
                            search_json: None,
                        }),
                        diagnostics,
                    };
                }
            }
        }
        Some(BuildEmbeddingBehavior::Skipped) | None => None,
    };

    ArtifactBuildResult {
        artifacts: Some(BuildArtifacts {
            html,
            graph_json,
            search_json,
        }),
        diagnostics: artifact_diagnostics,
    }
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
    use crate::domain::artifact::SearchArtifactDocument;
    use crate::domain::ast::BlockAst;
    use crate::domain::ports::embedding_provider::{EmbeddingError, EmbeddingProvider, ModelId};
    use crate::domain::source::SourceFile;
    use crate::infrastructure::artifact::search_json::SUPPORTED_SEARCH_SCHEMA_VERSION;
    use crate::infrastructure::embedding::deterministic::DeterministicProvider;
    use crate::infrastructure::source::in_memory::InMemorySourceProvider;
    use chrono::NaiveDate;
    use std::cell::RefCell;
    use std::fs;

    fn source_file(identity: &str, text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from(format!("/work/{identity}")),
            text.to_string(),
            PathBuf::from(identity),
        )
    }

    fn fixed_today() -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 5, 8).expect("valid test date")
    }

    fn parse_search_json(search_json: &str) -> SearchArtifactDocument {
        serde_json::from_str(search_json).expect("search artifact JSON is valid")
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
        let graph_json: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph artifact JSON is valid");
        assert!(
            graph_json["nodes"]
                .as_array()
                .expect("nodes is an array")
                .is_empty()
        );
    }

    #[test]
    fn lifecycle_expired_warning_is_emitted_for_parseable_past_expires_at() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\n::claim billing.credits\nstatus: draft\nexpires_at: 2026-05-07\n--\nCredits expire before today.\n::\n",
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "expiry warning must not block compile"
        );
        assert!(
            result.artifacts.is_some(),
            "warning diagnostics must not block artifact generation"
        );
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::LifecycleExpired)
            .expect("expected lifecycle.expired warning");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.object_id.as_deref(), Some("billing.credits"));
        assert_eq!(
            diagnostic.span.as_ref().map(|span| (
                span.file.as_path(),
                span.start.line,
                span.start.column
            )),
            Some((std::path::Path::new("/work/guide.adoc"), 3, 1))
        );
        assert!(
            diagnostic.message.contains("billing.credits")
                && diagnostic.message.contains("2026-05-07")
                && diagnostic.message.contains("expired"),
            "message should name the object, date, and expiry: {}",
            diagnostic.message
        );
    }

    #[test]
    fn lifecycle_expired_warning_ignores_today_and_future_expires_at() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\n::claim billing.today\nstatus: draft\nexpires_at: 2026-05-08\n--\nToday is still valid.\n::\n\n::claim billing.future\nstatus: draft\nexpires_at: 2026-05-09\n--\nFuture is still valid.\n::\n",
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result
                .diagnostics
                .iter()
                .any(|diagnostic| diagnostic.code == DiagnosticCode::LifecycleExpired),
            "today and future expires_at values must not warn"
        );
        assert!(
            result.artifacts.is_some(),
            "valid lifecycle fields must not block artifact generation"
        );
    }

    #[test]
    fn lifecycle_invalid_expires_at_warning_is_emitted_for_unparseable_value() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\n::claim billing.invalid\nstatus: draft\nexpires_at: not-a-date\n--\nInvalid dates should be reported.\n::\n",
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "invalid expires_at warning must not block compile"
        );
        assert!(
            result.artifacts.is_some(),
            "warning diagnostics must not block artifact generation"
        );
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::LifecycleInvalidExpiresAt)
            .expect("expected lifecycle.invalid_expires_at warning");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert_eq!(diagnostic.object_id.as_deref(), Some("billing.invalid"));
        assert_eq!(
            diagnostic.span.as_ref().map(|span| (
                span.file.as_path(),
                span.start.line,
                span.start.column
            )),
            Some((std::path::Path::new("/work/guide.adoc"), 3, 1))
        );
        assert!(
            diagnostic.message.contains("billing.invalid")
                && diagnostic.message.contains("not-a-date")
                && diagnostic.message.contains("YYYY-MM-DD"),
            "message should name the object, value, and expected format: {}",
            diagnostic.message
        );
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
        let diagnostics = validate_resolved_pages(&parsed, fixed_today());

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

    #[test]
    fn build_with_provider_embeds_knowledge_objects_into_search_artifact() {
        let source_provider = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            concat!(
                "# Billing @doc(team.billing)\n",
                "\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "owner: team-billing\n",
                "--\n",
                "Credits apply after successful payment.\n",
                "::\n",
            ),
        ));
        let embedding_provider = DeterministicProvider::new(4);

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert!(
            !result.has_errors(),
            "embedding build should pass: {:?}",
            result.diagnostics
        );
        let artifacts = result.artifacts.expect("artifacts are built");
        let search_json = artifacts.search_json.expect("search artifact is built");
        let search = parse_search_json(&search_json);
        let expected_vector = embedding_provider
            .embed_passages(&[concat!(
                "claim: Credits apply after successful payment.\n",
                "[id: billing.credits] [status: draft] [owner: team-billing]"
            )
            .to_string()])
            .expect("test embedding succeeds")
            .remove(0);

        assert_eq!(search.schema_version, SUPPORTED_SEARCH_SCHEMA_VERSION);
        assert_eq!(search.model.id, "hash-v1");
        assert_eq!(search.model.provider, "deterministic");
        assert_eq!(search.model.dim, 4);
        assert!(search.graph_artifact_hash.starts_with("sha256:"));
        assert_eq!(search.embeddings.len(), 1);
        assert_eq!(search.embeddings[0].id, "billing.credits");
        assert!(search.embeddings[0].content_hash.starts_with("sha256:"));
        assert_eq!(search.embeddings[0].vector, expected_vector);
    }

    #[test]
    fn build_with_provider_matches_v1_3_deterministic_search_fixture() {
        let source_text = fs::read_to_string(repo_fixture_path("v1_3_embed/input.adoc"))
            .expect("fixture source is readable");
        let source_provider = InMemorySourceProvider::new()
            .with_source(source_file("v1_3_embed/input.adoc", &source_text));
        let embedding_provider = DeterministicProvider::new(4);

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert!(
            !result.has_errors(),
            "fixture build should pass: {:?}",
            result.diagnostics
        );
        let actual = result
            .artifacts
            .expect("artifacts are built")
            .search_json
            .expect("search artifact is built");
        let actual_json = actual.clone();
        let actual = parse_search_json(&actual);
        let expected_text = fs::read_to_string(repo_fixture_path(
            "v1_3_embed/in_memory_baseline.search.json",
        ))
        .expect("baseline fixture is readable");
        let expected: SearchArtifactDocument =
            serde_json::from_str(&expected_text).unwrap_or_else(|error| {
                panic!("baseline fixture parse failed: {error}\nactual:\n{actual_json}")
            });

        assert_eq!(
            actual, expected,
            "deterministic search artifact fixture drifted"
        );
    }

    #[test]
    fn build_with_provider_omits_cache_count_diagnostic_when_no_knowledge_objects() {
        let source_provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            "# Guide @doc(team.guide)\n\nProse without Knowledge Objects.\n",
        ));
        let embedding_provider = RecordingEmbeddingProvider::new(4);

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert!(
            !result.has_errors(),
            "build should pass: {:?}",
            result.diagnostics
        );
        assert!(
            embedding_provider.recorded_inputs().is_empty(),
            "no Knowledge Objects should skip provider computation"
        );
        let search = result
            .artifacts
            .as_ref()
            .expect("artifacts are built")
            .search_json
            .as_ref()
            .expect("search artifact is built");
        let search = parse_search_json(search);
        assert!(search.embeddings.is_empty());
        assert_no_cache_count_diagnostic(&result);
    }

    #[test]
    fn build_with_provider_reuses_cached_vectors_by_model_id_and_content_hash() {
        let first_source = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let first_provider = RecordingEmbeddingProvider::new(4);
        let first_result = build_with_provider(
            &first_source,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &first_provider,
                },
                prior_search_artifact_path: None,
            },
        );
        let first_search = first_result
            .artifacts
            .expect("first artifacts")
            .search_json
            .expect("first search artifact");
        let first_search_document = parse_search_json(&first_search);
        let prior = tempfile::Builder::new()
            .prefix("adoc-cache-")
            .suffix(".search.json")
            .tempfile()
            .expect("cache file");
        fs::write(prior.path(), &first_search).expect("cache write");

        let second_source = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require manual audit review.",
            ),
        ));
        let second_provider = RecordingEmbeddingProvider::new(4);

        let second_result = build_with_provider(
            &second_source,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &second_provider,
                },
                prior_search_artifact_path: Some(prior.path().to_path_buf()),
            },
        );

        assert!(
            !second_result.has_errors(),
            "second build should pass: {:?}",
            second_result.diagnostics
        );
        let second_search = second_result
            .artifacts
            .as_ref()
            .expect("second artifacts")
            .search_json
            .as_ref()
            .expect("second search artifact");
        let second_search = parse_search_json(second_search);
        let recorded_inputs = second_provider.recorded_inputs();
        assert_eq!(
            recorded_inputs,
            vec![
                concat!(
                    "claim: Refunds require manual audit review.\n",
                    "[id: billing.refunds] [status: draft] [owner: unknown]"
                )
                .to_string()
            ],
            "only the changed object should be embedded"
        );
        assert_eq!(
            second_search.embeddings[0].vector, first_search_document.embeddings[0].vector,
            "unchanged object vector should be reused"
        );
        assert_ne!(
            second_search.embeddings[1].vector, first_search_document.embeddings[1].vector,
            "changed object vector should be recomputed"
        );
        assert_cache_count_diagnostic(&second_result, 1, 1);
    }

    #[test]
    fn build_with_provider_reports_all_cached_vectors_when_sources_are_unchanged() {
        let first_source = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let first_provider = RecordingEmbeddingProvider::new(4);
        let first_result = build_with_provider(
            &first_source,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &first_provider,
                },
                prior_search_artifact_path: None,
            },
        );
        let first_search = first_result
            .artifacts
            .expect("first artifacts")
            .search_json
            .expect("first search artifact");
        let prior = tempfile::Builder::new()
            .prefix("adoc-cache-")
            .suffix(".search.json")
            .tempfile()
            .expect("cache file");
        fs::write(prior.path(), &first_search).expect("cache write");

        let second_source = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let second_provider = RecordingEmbeddingProvider::new(4);

        let second_result = build_with_provider(
            &second_source,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &second_provider,
                },
                prior_search_artifact_path: Some(prior.path().to_path_buf()),
            },
        );

        assert!(
            !second_result.has_errors(),
            "second build should pass: {:?}",
            second_result.diagnostics
        );
        assert!(
            second_provider.recorded_inputs().is_empty(),
            "unchanged objects should be served entirely from cache"
        );
        assert_cache_count_diagnostic(&second_result, 2, 0);
    }

    #[test]
    fn build_with_provider_warns_and_recomputes_for_malformed_prior_search_cache() {
        let source_provider = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let prior = tempfile::Builder::new()
            .prefix("adoc-cache-")
            .suffix(".search.json")
            .tempfile()
            .expect("cache file");
        fs::write(prior.path(), "{not valid json").expect("malformed cache write");
        let embedding_provider = RecordingEmbeddingProvider::new(4);

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: Some(prior.path().to_path_buf()),
            },
        );

        assert!(
            !result.has_errors(),
            "malformed prior cache should not fail build: {:?}",
            result.diagnostics
        );
        assert_eq!(
            embedding_provider.recorded_inputs().len(),
            2,
            "malformed cache should be ignored and all objects recomputed"
        );
        assert_cache_ignored_diagnostic(
            &result,
            DiagnosticCode::IoArtifactMalformed,
            "Ignoring prior search artifact cache",
        );
        assert_cache_count_diagnostic(&result, 0, 2);
    }

    #[test]
    fn build_with_provider_warns_and_recomputes_for_unsupported_prior_search_cache_schema() {
        let source_provider = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let prior = tempfile::Builder::new()
            .prefix("adoc-cache-")
            .suffix(".search.json")
            .tempfile()
            .expect("cache file");
        fs::write(
            prior.path(),
            serde_json::json!({
                "schema_version": "adoc.search.v99",
                "model": { "id": "recording", "provider": "test", "dim": 4 },
                "graph_artifact_hash": "sha256:graph",
                "embeddings": []
            })
            .to_string(),
        )
        .expect("unsupported schema cache write");
        let embedding_provider = RecordingEmbeddingProvider::new(4);

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: Some(prior.path().to_path_buf()),
            },
        );

        assert!(
            !result.has_errors(),
            "unsupported prior cache schema should not fail build: {:?}",
            result.diagnostics
        );
        assert_eq!(
            embedding_provider.recorded_inputs().len(),
            2,
            "unsupported cache should be ignored and all objects recomputed"
        );
        assert_cache_ignored_diagnostic(
            &result,
            DiagnosticCode::SchemaUnsupportedVersion,
            "Ignoring prior search artifact cache",
        );
        assert_cache_count_diagnostic(&result, 0, 2);
    }

    #[test]
    fn build_with_provider_warns_and_recomputes_for_prior_search_cache_model_mismatch() {
        let source_provider = InMemorySourceProvider::new().with_source(source_file(
            "billing.adoc",
            &two_claim_source(
                "Credits apply after successful payment.",
                "Refunds require audit review.",
            ),
        ));
        let first_provider = RecordingEmbeddingProvider::new(4);
        let first_result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &first_provider,
                },
                prior_search_artifact_path: None,
            },
        );
        let mut prior_search = first_result
            .artifacts
            .expect("first artifacts")
            .search_json
            .map(|json| parse_search_json(&json))
            .expect("first search artifact");
        prior_search.model.id = "other-model".to_string();
        let prior = tempfile::Builder::new()
            .prefix("adoc-cache-")
            .suffix(".search.json")
            .tempfile()
            .expect("cache file");
        fs::write(
            prior.path(),
            prior_search
                .to_pretty_json()
                .expect("search artifact serializes"),
        )
        .expect("mismatched cache write");
        let second_provider = RecordingEmbeddingProvider::new(4);

        let second_result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &second_provider,
                },
                prior_search_artifact_path: Some(prior.path().to_path_buf()),
            },
        );

        assert!(
            !second_result.has_errors(),
            "model-mismatched prior cache should not fail build: {:?}",
            second_result.diagnostics
        );
        assert_eq!(
            second_provider.recorded_inputs().len(),
            2,
            "model-mismatched cache should be ignored and all objects recomputed"
        );
        assert_cache_ignored_diagnostic(
            &second_result,
            DiagnosticCode::BuildEmbeddingsCacheIgnored,
            "Ignoring prior search artifact cache",
        );
        assert_cache_count_diagnostic(&second_result, 0, 2);
    }

    #[test]
    fn build_with_provider_maps_embedding_compute_error_and_blocks_artifacts() {
        let source_provider = InMemorySourceProvider::new()
            .with_source(source_file("billing.adoc", &one_claim_source()));
        let embedding_provider = ControlledEmbeddingProvider::new(
            4,
            Err(EmbeddingError::Compute("encoder failed".to_string())),
        );

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert_embedding_error_result(&result, DiagnosticCode::EmbedComputeFailed);
    }

    #[test]
    fn build_with_provider_maps_embedding_model_load_error_and_blocks_artifacts() {
        let source_provider = InMemorySourceProvider::new()
            .with_source(source_file("billing.adoc", &one_claim_source()));
        let embedding_provider = ControlledEmbeddingProvider::new(
            4,
            Err(EmbeddingError::ModelLoad(
                "model cache unavailable".to_string(),
            )),
        );

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert_embedding_error_result(&result, DiagnosticCode::EmbedModelLoadFailed);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::EmbedModelLoadFailed)
            .expect("model load diagnostic");
        assert!(
            diagnostic
                .help
                .as_deref()
                .expect("help")
                .contains("adoc build"),
            "model load help should tell user how to retry/fix"
        );
    }

    #[test]
    fn build_with_provider_rejects_wrong_embedding_vector_count_and_blocks_artifacts() {
        let source_provider = InMemorySourceProvider::new()
            .with_source(source_file("billing.adoc", &one_claim_source()));
        let embedding_provider = ControlledEmbeddingProvider::new(4, Ok(Vec::new()));

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert_embedding_error_result(&result, DiagnosticCode::EmbedUnexpectedDimension);
    }

    #[test]
    fn build_with_provider_rejects_bad_embedding_vector_dim_and_blocks_artifacts() {
        let source_provider = InMemorySourceProvider::new()
            .with_source(source_file("billing.adoc", &one_claim_source()));
        let embedding_provider = ControlledEmbeddingProvider::new(4, Ok(vec![vec![1.0, 2.0, 3.0]]));

        let result = build_with_provider(
            &source_provider,
            BuildOptions {
                embeddings: BuildEmbeddingBehavior::Enabled {
                    provider: &embedding_provider,
                },
                prior_search_artifact_path: None,
            },
        );

        assert_embedding_error_result(&result, DiagnosticCode::EmbedUnexpectedDimension);
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::EmbedUnexpectedDimension)
            .expect("dimension diagnostic");
        assert_eq!(
            diagnostic.message,
            "embedding provider returned dimension 3; expected 4"
        );
    }

    fn assert_embedding_error_result(result: &CompileResult, expected_code: DiagnosticCode) {
        assert!(
            result.has_errors(),
            "build should fail: {:?}",
            result.diagnostics
        );
        let artifacts = result
            .artifacts
            .as_ref()
            .expect("embedding failures preserve V0 artifacts");
        assert!(
            artifacts.search_json.is_none(),
            "embedding failures must suppress only search artifacts"
        );
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == expected_code)
            .expect("expected embedding diagnostic");
        assert_eq!(diagnostic.severity, Severity::Error);
    }

    fn assert_cache_count_diagnostic(
        result: &CompileResult,
        expected_cached: usize,
        expected_computed: usize,
    ) {
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == DiagnosticCode::BuildEmbeddingsCached)
            .expect("cache count diagnostic");
        assert_eq!(diagnostic.severity, Severity::Info);
        assert_eq!(
            diagnostic.message,
            format!("embeddings: cached {expected_cached}, computed {expected_computed}")
        );
    }

    fn assert_no_cache_count_diagnostic(result: &CompileResult) {
        assert!(
            result
                .diagnostics
                .iter()
                .all(|diagnostic| diagnostic.code != DiagnosticCode::BuildEmbeddingsCached),
            "cache count diagnostic should be omitted: {:?}",
            result.diagnostics
        );
    }

    fn assert_cache_ignored_diagnostic(
        result: &CompileResult,
        expected_code: DiagnosticCode,
        expected_message: &str,
    ) {
        let diagnostic = result
            .diagnostics
            .iter()
            .find(|diagnostic| diagnostic.code == expected_code)
            .expect("cache ignored diagnostic");
        assert_eq!(diagnostic.severity, Severity::Warning);
        assert!(
            diagnostic.message.contains(expected_message),
            "message should explain ignored cache: {}",
            diagnostic.message
        );
        assert!(
            diagnostic
                .help
                .as_deref()
                .expect("help")
                .contains("recomputed"),
            "help should explain recompute behavior"
        );
    }

    fn one_claim_source() -> String {
        concat!(
            "# Billing @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits apply after successful payment.\n",
            "::\n",
        )
        .to_string()
    }

    fn two_claim_source(credits_body: &str, refunds_body: &str) -> String {
        format!(
            concat!(
                "# Billing @doc(team.billing)\n",
                "\n",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "{credits_body}\n",
                "::\n",
                "\n",
                "::claim billing.refunds\n",
                "status: draft\n",
                "--\n",
                "{refunds_body}\n",
                "::\n",
            ),
            credits_body = credits_body,
            refunds_body = refunds_body
        )
    }

    fn repo_fixture_path(relative: &str) -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .join("tests/fixtures")
            .join(relative)
    }

    #[derive(Debug)]
    struct ControlledEmbeddingProvider {
        model_id: ModelId,
        dim: usize,
        result: Result<Vec<Vec<f32>>, EmbeddingError>,
    }

    impl ControlledEmbeddingProvider {
        fn new(dim: usize, result: Result<Vec<Vec<f32>>, EmbeddingError>) -> Self {
            Self {
                model_id: ModelId::new("controlled", "test"),
                dim,
                result,
            }
        }
    }

    impl EmbeddingProvider for ControlledEmbeddingProvider {
        fn model_id(&self) -> &ModelId {
            &self.model_id
        }

        fn dim(&self) -> usize {
            self.dim
        }

        fn embed_passages(&self, _inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            self.result.clone()
        }

        fn embed_query(&self, _query: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok(vec![0.0; self.dim])
        }
    }

    #[derive(Debug)]
    struct RecordingEmbeddingProvider {
        model_id: ModelId,
        dim: usize,
        recorded_inputs: RefCell<Vec<String>>,
    }

    impl RecordingEmbeddingProvider {
        fn new(dim: usize) -> Self {
            Self {
                model_id: ModelId::new("recording", "test"),
                dim,
                recorded_inputs: RefCell::new(Vec::new()),
            }
        }

        fn recorded_inputs(&self) -> Vec<String> {
            self.recorded_inputs.borrow().clone()
        }
    }

    impl EmbeddingProvider for RecordingEmbeddingProvider {
        fn model_id(&self) -> &ModelId {
            &self.model_id
        }

        fn dim(&self) -> usize {
            self.dim
        }

        fn embed_passages(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>, EmbeddingError> {
            self.recorded_inputs
                .borrow_mut()
                .extend(inputs.iter().cloned());
            Ok(inputs
                .iter()
                .map(|input| {
                    (0..self.dim)
                        .map(|index| (input.len() + index) as f32)
                        .collect()
                })
                .collect())
        }

        fn embed_query(&self, query: &str) -> Result<Vec<f32>, EmbeddingError> {
            Ok((0..self.dim)
                .map(|index| (query.len() + index) as f32)
                .collect())
        }
    }

    // ── V5.10 TB2: derived effective_status ──────────────────────────────────

    /// A `verified` claim with a past `expires_at` must get
    /// `effective_status:"stale"` and `effective_reason:"expired:<date>"` in
    /// the graph JSON, while the authored `status` stays `"verified"`.
    #[test]
    fn verified_claim_with_past_expires_at_gets_effective_status_stale() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::claim billing.credits\n",
                "status: verified\n",
                "owner: team-billing\n",
                "verified_at: 2025-01-01\n",
                "source: ledger\n",
                "expires_at: 2026-01-01\n",
                "--\n",
                "Credits expire.\n",
                "::\n",
            ),
        ));

        // today = 2026-05-08, expires_at = 2026-01-01 → past → stale
        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        let artifacts = result.artifacts.expect("artifacts must be produced");
        let graph: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph JSON valid");
        let node = graph["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|n| n["id"] == "billing.credits")
            .expect("billing.credits node");

        assert_eq!(
            node["status"].as_str(),
            Some("verified"),
            "authored status must remain 'verified'"
        );
        assert_eq!(
            node["effective_status"].as_str(),
            Some("stale"),
            "effective_status must be 'stale'"
        );
        assert_eq!(
            node["effective_reason"].as_str(),
            Some("expired:2026-01-01"),
            "effective_reason must record the expiry date"
        );
    }

    /// A `verified` claim with a future `expires_at` must NOT get
    /// `effective_status`.
    #[test]
    fn verified_claim_with_future_expires_at_has_no_effective_status() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::claim billing.future\n",
                "status: verified\n",
                "owner: team-billing\n",
                "verified_at: 2025-01-01\n",
                "source: ledger\n",
                "expires_at: 2027-01-01\n",
                "--\n",
                "Credits valid.\n",
                "::\n",
            ),
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        let artifacts = result.artifacts.expect("artifacts must be produced");
        let graph: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph JSON valid");
        let node = graph["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|n| n["id"] == "billing.future")
            .expect("billing.future node");

        assert!(
            node["effective_status"].is_null(),
            "future expiry must not set effective_status"
        );
        assert!(
            node["effective_reason"].is_null(),
            "future expiry must not set effective_reason"
        );
    }

    /// A non-`verified` (draft) claim with a past `expires_at` must NOT get
    /// `effective_status`. The existing `lifecycle.expired` warning still fires.
    #[test]
    fn draft_claim_with_past_expires_at_has_no_effective_status() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::claim billing.draft\n",
                "status: draft\n",
                "expires_at: 2026-01-01\n",
                "--\n",
                "Draft expires.\n",
                "::\n",
            ),
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        let artifacts = result.artifacts.expect("artifacts must be produced");

        // The existing lifecycle.expired warning must still fire.
        assert!(
            result
                .diagnostics
                .iter()
                .any(|d| d.code == DiagnosticCode::LifecycleExpired),
            "lifecycle.expired warning must still fire for non-verified expired claims"
        );

        let graph: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph JSON valid");
        let node = graph["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|n| n["id"] == "billing.draft")
            .expect("billing.draft node");

        assert!(
            node["effective_status"].is_null(),
            "draft status must not set effective_status"
        );
    }

    /// A `verified` claim with no `expires_at` field must NOT get
    /// `effective_status`.
    #[test]
    fn verified_claim_without_expires_at_has_no_effective_status() {
        let provider = InMemorySourceProvider::new().with_source(source_file(
            "guide.adoc",
            concat!(
                "# Guide @doc(team.guide)\n\n",
                "::claim billing.no-expiry\n",
                "status: verified\n",
                "owner: team-billing\n",
                "verified_at: 2025-01-01\n",
                "source: ledger\n",
                "--\n",
                "No expiry set.\n",
                "::\n",
            ),
        ));

        let result = compile_with_provider_for_date(&provider, fixed_today());

        assert!(
            !result.has_errors(),
            "unexpected errors: {:?}",
            result.diagnostics
        );
        let artifacts = result.artifacts.expect("artifacts must be produced");
        let graph: serde_json::Value =
            serde_json::from_str(&artifacts.graph_json).expect("graph JSON valid");
        let node = graph["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .find(|n| n["id"] == "billing.no-expiry")
            .expect("billing.no-expiry node");

        assert!(
            node["effective_status"].is_null(),
            "absent expires_at must not set effective_status"
        );
    }
}

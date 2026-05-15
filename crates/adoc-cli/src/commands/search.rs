use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    Diagnostic, DiagnosticCode, EmbedQueryError, GraphDirection, GraphRelationKind, RetrievalInput,
    RetrievalLoadResult, SearchFilters, SearchMode, SearchQuery, SearchResult, Severity,
    embed_query, load_retrieval_session, search,
};

use crate::error::CliError;
use crate::presentation::{ResolvedFormat, RetrievalView, make_presenter};

use super::{
    discover_project_config_if, emit_retrieval_error, eprint_diagnostics,
    exit_code_for_diagnostics, gate_retrieval_load, merge_diagnostics,
    presentation_record_from_session, report, resolve_graph_artifact_path_with_config,
    resolve_search_artifact_path_with_config,
};

pub(crate) struct SearchCommandInput {
    pub(crate) query: String,
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) search_artifact: Option<PathBuf>,
    pub(crate) semantic: bool,
    pub(crate) lexical: bool,
    pub(crate) kind: Option<String>,
    pub(crate) status: Option<String>,
    pub(crate) owner: Option<String>,
    pub(crate) source_path: Option<String>,
    pub(crate) related_to: Option<String>,
    pub(crate) relation: Option<GraphRelationKind>,
    pub(crate) direction: Option<GraphDirection>,
    pub(crate) top: NonZeroUsize,
}

pub(crate) fn search_command(input: SearchCommandInput, resolved: ResolvedFormat) -> i32 {
    let requested_mode = if input.semantic {
        SearchMode::Semantic
    } else if input.lexical {
        SearchMode::Lexical
    } else {
        SearchMode::Hybrid
    };

    let needs_search_config = matches!(requested_mode, SearchMode::Hybrid | SearchMode::Semantic)
        && input.search_artifact.is_none();
    let config = match discover_project_config_if(input.artifact.is_none() || needs_search_config) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let artifact = resolve_graph_artifact_path_with_config(input.artifact, config.as_ref());
    let search_artifact_path = match requested_mode {
        SearchMode::Lexical => None,
        SearchMode::Hybrid | SearchMode::Semantic => Some(
            resolve_search_artifact_path_with_config(input.search_artifact, config.as_ref()),
        ),
    };
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact,
        search_artifact_path,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let (session, load_diagnostics) =
        match gate_retrieval_load(session, load_diagnostics, resolved, 2) {
            Ok(loaded) => loaded,
            Err(exit_code) => return exit_code,
        };

    // Explicit semantic mode cannot run without a vector index. Hybrid mode
    // degrades to lexical below so missing embeddings do not pay model-load
    // cost just to fall back.
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
        return emit_retrieval_error(diagnostics, resolved, 2);
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
                    EmbedQueryError::ModelLoad(msg) => (
                        DiagnosticCode::EmbedModelLoadFailed,
                        format!("{mode_label} requested but embedding model failed to load: {msg}"),
                    ),
                    EmbedQueryError::Compute(msg) => (
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
                return emit_retrieval_error(vec![diagnostic], resolved, 2);
            }
        }
    } else {
        None
    };

    let search_result = search(
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
    let search_result = SearchResult {
        records: search_result.records,
        diagnostics: merge_diagnostics(load_diagnostics, search_result.diagnostics),
    };
    let exit_code = search_exit_code(&search_result);
    let view = RetrievalView {
        records: search_result
            .records
            .into_iter()
            .map(|record| presentation_record_from_session(&session, record, false))
            .collect(),
        diagnostics: search_result.diagnostics,
        footer: None,
    };

    if resolved == ResolvedFormat::Json {
        let presenter = make_presenter(ResolvedFormat::Json, Vec::new());
        return presenter
            .present(&view, &mut std::io::stdout())
            .map_or_else(
                |source| report(CliError::RetrievalIo { source }),
                |()| exit_code,
            );
    }

    if exit_code != 0 {
        eprint_diagnostics(&view.diagnostics);
        return exit_code;
    }

    if !view.diagnostics.is_empty() {
        eprint_diagnostics(&view.diagnostics);
    }
    if view.records.is_empty() {
        println!("(no matches)");
        return 0;
    }

    let presenter = make_presenter(resolved, Vec::new());
    presenter
        .present(&view, &mut std::io::stdout())
        .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 0)
}

fn search_exit_code(result: &SearchResult) -> i32 {
    exit_code_for_diagnostics(&result.diagnostics, search_diagnostic_exit_code)
}

fn search_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::SearchInvalidFilter, _) => Some(1),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn search_exit_code_prioritizes_invalid_filters_over_generic_errors() {
        let result = SearchResult {
            records: Vec::new(),
            diagnostics: vec![
                Diagnostic {
                    code: DiagnosticCode::IoArtifactMalformed,
                    severity: Severity::Error,
                    message: "artifact error".to_string(),
                    span: None,
                    object_id: None,
                    help: None,
                },
                Diagnostic {
                    code: DiagnosticCode::SearchInvalidFilter,
                    severity: Severity::Error,
                    message: "invalid filter".to_string(),
                    span: None,
                    object_id: None,
                    help: None,
                },
            ],
        };

        assert_eq!(search_exit_code(&result), 1);
    }

    #[test]
    fn search_exit_code_returns_one_for_invalid_filter_without_artifact_error() {
        let result = SearchResult {
            records: Vec::new(),
            diagnostics: vec![Diagnostic {
                code: DiagnosticCode::SearchInvalidFilter,
                severity: Severity::Error,
                message: "invalid filter".to_string(),
                span: None,
                object_id: None,
                help: None,
            }],
        };

        assert_eq!(search_exit_code(&result), 1);
    }
}

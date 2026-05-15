use std::path::PathBuf;
use std::time::Instant;

use adoc_core::{
    Diagnostic, DiagnosticCode, RetrievalInput, RetrievalLoadResult, Severity,
    load_retrieval_session, why_object,
};

use crate::error::CliError;
use crate::presentation::{RenderMeta, ResolvedFormat, RetrievalView, make_presenter};

use super::{
    discover_project_config_if, emit_retrieval_error, eprint_diagnostics,
    exit_code_for_diagnostics, gate_retrieval_load, merge_diagnostics,
    presentation_record_from_session, report, resolve_graph_artifact_path_with_config,
};

pub(crate) fn why(object_id: String, artifact: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let config = match discover_project_config_if(artifact.is_none()) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let artifact = resolve_graph_artifact_path_with_config(artifact, config.as_ref());
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.clone(),
        search_artifact_path: None,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let load_exit_code = why_exit_code_for_diagnostics(&load_diagnostics);
    let (session, load_diagnostics) =
        match gate_retrieval_load(session, load_diagnostics, resolved, load_exit_code) {
            Ok(loaded) => loaded,
            Err(exit_code) => return exit_code,
        };

    let started = Instant::now();
    let why_result = why_object(&session, &object_id);
    let duration = started.elapsed();
    let diagnostics = merge_diagnostics(load_diagnostics, why_result.diagnostics);
    let exit_code = why_exit_code_for_diagnostics(&diagnostics);

    if exit_code != 0 {
        return emit_retrieval_error(diagnostics, resolved, exit_code);
    }

    if resolved != ResolvedFormat::Json && !diagnostics.is_empty() {
        eprint_diagnostics(&diagnostics);
    }

    let records: Vec<_> = why_result
        .records
        .into_iter()
        .map(|record| presentation_record_from_session(&session, record, true))
        .collect();
    let footer = records.first().map(|presentation_record| RenderMeta {
        artifact,
        trust: presentation_record.record.fields.get("trust").cloned(),
        duration,
    });
    let view = RetrievalView {
        records,
        diagnostics,
        footer,
    };
    let presenter = make_presenter(resolved, Vec::new());
    presenter
        .present(&view, &mut std::io::stdout())
        .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 0)
}

fn why_exit_code_for_diagnostics(diagnostics: &[Diagnostic]) -> i32 {
    exit_code_for_diagnostics(diagnostics, why_diagnostic_exit_code)
}

fn why_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::RetrievalObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

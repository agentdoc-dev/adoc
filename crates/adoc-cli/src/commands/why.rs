use std::path::PathBuf;
use std::time::Instant;

use adoc_core::{
    Diagnostic, DiagnosticCode, RetrievalEnvelope, RetrievalInput, RetrievalLoadResult, Severity,
    load_retrieval_session, why_object,
};

use crate::error::CliError;
use crate::presentation::{
    RenderMeta, ResolvedFormat, RetrievalView, json as json_presentation, make_presenter,
};

use super::{
    diagnostics_have_errors, discover_project_config_if, eprint_diagnostics, merge_diagnostics,
    presentation_record_from_session, report, resolve_agent_artifact_path_with_config,
};

pub(crate) fn why(object_id: String, artifact: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let config = match discover_project_config_if(artifact.is_none()) {
        Ok(config) => config,
        Err(error) => return report(error),
    };
    let artifact = resolve_agent_artifact_path_with_config(artifact, config.as_ref());
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.clone(),
        search_artifact_path: None,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let session = match session {
        Some(session) => session,
        None => {
            let exit_code = why_exit_code_for_diagnostics(&load_diagnostics);
            if resolved == ResolvedFormat::Json {
                return json_presentation::write_envelope_json(
                    &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                    &mut std::io::stdout(),
                )
                .map_or_else(
                    |source| report(CliError::RetrievalIo { source }),
                    |()| exit_code,
                );
            }
            eprint_diagnostics(&load_diagnostics);
            return exit_code;
        }
    };

    if diagnostics_have_errors(&load_diagnostics) {
        let exit_code = why_exit_code_for_diagnostics(&load_diagnostics);
        if resolved == ResolvedFormat::Json {
            return json_presentation::write_envelope_json(
                &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                &mut std::io::stdout(),
            )
            .map_or_else(
                |source| report(CliError::RetrievalIo { source }),
                |()| exit_code,
            );
        }
        eprint_diagnostics(&load_diagnostics);
        return exit_code;
    }

    let started = Instant::now();
    let why_result = why_object(&session, &object_id);
    let duration = started.elapsed();
    let diagnostics = merge_diagnostics(load_diagnostics, why_result.diagnostics);
    let exit_code = why_exit_code_for_diagnostics(&diagnostics);

    if exit_code != 0 {
        if resolved == ResolvedFormat::Json {
            return json_presentation::write_envelope_json(
                &RetrievalEnvelope::new(Vec::new(), diagnostics),
                &mut std::io::stdout(),
            )
            .map_or_else(
                |source| report(CliError::RetrievalIo { source }),
                |()| exit_code,
            );
        }
        eprint_diagnostics(&diagnostics);
        return exit_code;
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
    diagnostics
        .iter()
        .filter_map(why_diagnostic_exit_code)
        .min()
        .unwrap_or(0)
}

fn why_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::IdInvalid, _) => Some(1),
        (DiagnosticCode::RetrievalObjectNotFound, _) => Some(3),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
}

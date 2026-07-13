use std::path::PathBuf;

use adoc_local::{LocalContext, UnrestrictedPathPolicy, WhyInput};

use crate::error::CliError;
use crate::presentation::{
    PresentationEntry, RenderMeta, ResolvedFormat, RetrievalView, make_presenter,
};

use super::{
    current_dir, emit_retrieval_error, eprint_diagnostics, presentation_record_from_resolved,
    report,
};

pub(crate) fn why(object_id: String, artifact: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.why(WhyInput {
        object_id,
        artifact,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let diagnostics = outcome.diagnostics;
    let exit_code = outcome.exit_code;

    if exit_code != 0 {
        return emit_retrieval_error(diagnostics, resolved, exit_code);
    }

    if resolved != ResolvedFormat::Json && !diagnostics.is_empty() {
        eprint_diagnostics(&diagnostics);
    }

    let records: Vec<_> = outcome
        .records
        .into_iter()
        .map(|record| {
            PresentationEntry::KnowledgeObject(presentation_record_from_resolved(record, true))
        })
        .collect();
    let footer = records.first().map(|entry| RenderMeta {
        artifact: outcome.artifact,
        trust: match entry {
            PresentationEntry::KnowledgeObject(presentation_record) => {
                presentation_record.record.fields.get("trust").cloned()
            }
            // `adoc why` is Knowledge-Object-only (ADR-0040).
            PresentationEntry::Prose(_) => None,
        },
        duration: outcome.duration,
    });
    let view = RetrievalView {
        records,
        diagnostics,
        footer,
    };
    let presenter = make_presenter(resolved, Vec::new());
    presenter
        .present(&view, &mut std::io::stdout())
        .map_or_else(|source| report(CliError::StdoutIo { source }), |()| 0)
}

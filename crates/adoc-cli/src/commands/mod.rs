mod build;
mod check;
mod graph;
mod init;
mod patch;
mod search;
mod why;

use std::path::PathBuf;

use adoc_core::{Diagnostic, RetrievalEnvelope, RetrievalRecord, Severity};
use adoc_local::ResolvedRetrievalRecord;

use crate::error::CliError;
use crate::presentation::{
    ExpiresInfo, PresentationRecord, ResolvedFormat, json as json_presentation,
};

pub(crate) use build::build;
pub(crate) use check::check;
pub(crate) use graph::{GraphCommandInput, graph};
pub(crate) use init::init;
pub(crate) use patch::{PatchCommandInput, patch};
pub(crate) use search::{SearchCommandInput, search_command};
pub(crate) use why::why;

fn emit_retrieval_error(
    diagnostics: Vec<Diagnostic>,
    resolved: ResolvedFormat,
    exit_code: i32,
) -> i32 {
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
    exit_code
}

fn presentation_record_from_resolved(
    resolved: ResolvedRetrievalRecord,
    include_expires: bool,
) -> PresentationRecord {
    let record = resolved.record;
    let expires = include_expires.then(|| expires_info(&record)).flatten();
    PresentationRecord {
        record,
        related_statuses: resolved.related_statuses,
        expires,
    }
}

fn expires_info(record: &RetrievalRecord) -> Option<ExpiresInfo> {
    record
        .fields
        .get("expires_at")
        .and_then(|value| chrono::NaiveDate::parse_from_str(value, "%Y-%m-%d").ok())
        .map(|date| {
            let today = chrono::Local::now().date_naive();
            ExpiresInfo {
                date,
                days_until: (date - today).num_days(),
            }
        })
}

fn report(error: CliError) -> i32 {
    eprintln!("{error}");
    error.exit_code()
}

fn current_dir() -> Result<PathBuf, CliError> {
    std::env::current_dir().map_err(|source| adoc_local::LocalError::CurrentDir { source }.into())
}

fn print_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        if let Some(span) = &diagnostic.span {
            println!(
                "{}:{}:{}: {}[{}] {}",
                span.file.display(),
                span.start.line,
                span.start.column,
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message
            );
        } else {
            println!(
                "{}[{}] {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            );
        }
        if let Some(object_id) = &diagnostic.object_id {
            println!("  object_id: {object_id}");
        }
        if let Some(help) = &diagnostic.help {
            println!("  help: {help}");
        }
    }
}

fn eprint_diagnostics(diagnostics: &[Diagnostic]) {
    for diagnostic in diagnostics {
        if let Some(span) = &diagnostic.span {
            eprintln!(
                "{}:{}:{}: {}[{}] {}",
                span.file.display(),
                span.start.line,
                span.start.column,
                diagnostic.severity,
                diagnostic.code,
                diagnostic.message
            );
        } else {
            eprintln!(
                "{}[{}] {}",
                diagnostic.severity, diagnostic.code, diagnostic.message
            );
        }
        if let Some(object_id) = &diagnostic.object_id {
            eprintln!("  object_id: {object_id}");
        }
        if let Some(help) = &diagnostic.help {
            eprintln!("  help: {help}");
        }
    }
}

fn print_summary(diagnostics: &[Diagnostic]) {
    let errors = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Error)
        .count();
    let warnings = diagnostics
        .iter()
        .filter(|diagnostic| diagnostic.severity == Severity::Warning)
        .count();

    println!("{errors} errors, {warnings} warnings");
}

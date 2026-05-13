mod build;
mod check;
mod init;
mod search;
mod why;

use std::path::PathBuf;

use adoc_core::{Diagnostic, RetrievalRecord, RetrievalSession, Severity};

use crate::config::ProjectConfig;
use crate::error::CliError;
use crate::presentation::{ExpiresInfo, PresentationRecord};

pub(crate) use build::build;
pub(crate) use check::check;
pub(crate) use init::init;
pub(crate) use search::{SearchCommandInput, search_command};
pub(crate) use why::why;

const DEFAULT_AGENT_ARTIFACT_PATH: &str = "dist/docs.agent.json";
const DEFAULT_SEARCH_ARTIFACT_PATH: &str = "dist/docs.search.json";

fn discover_project_config_if(needed: bool) -> Result<Option<ProjectConfig>, CliError> {
    if needed {
        ProjectConfig::discover()
    } else {
        Ok(None)
    }
}

fn resolve_docs_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> Result<PathBuf, CliError> {
    path.or_else(|| config.map(|config| config.docs_path.clone()))
        .ok_or_else(|| CliError::ConfigMissing {
            message: "adoc check/build requires a path or agentdoc.config.yaml with docs_path"
                .to_string(),
            config_path: config.map(|config| config.path.clone()),
        })
}

fn resolve_agent_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.agent_json.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_AGENT_ARTIFACT_PATH))
}

fn resolve_search_artifact_path_with_config(
    path: Option<PathBuf>,
    config: Option<&ProjectConfig>,
) -> PathBuf {
    if let Some(path) = path {
        return path;
    }

    config
        .as_ref()
        .and_then(|config| config.outputs.search.clone())
        .unwrap_or_else(|| PathBuf::from(DEFAULT_SEARCH_ARTIFACT_PATH))
}

fn diagnostics_have_errors(diagnostics: &[Diagnostic]) -> bool {
    diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
}

fn merge_diagnostics(
    mut load_diagnostics: Vec<Diagnostic>,
    mut command_diagnostics: Vec<Diagnostic>,
) -> Vec<Diagnostic> {
    load_diagnostics.append(&mut command_diagnostics);
    load_diagnostics
}

fn presentation_record_from_session(
    session: &RetrievalSession,
    record: RetrievalRecord,
    include_expires: bool,
) -> PresentationRecord {
    let expires = include_expires.then(|| expires_info(&record)).flatten();
    let related_statuses = session.related_statuses(&record);
    PresentationRecord {
        record,
        related_statuses,
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

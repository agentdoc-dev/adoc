mod error;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adoc_core::{
    AgentJsonDocument, BuildEmbeddingMode, BuildInput, CompileInput, CompileResult, Diagnostic,
    DiagnosticCode, ExplainResult, JsonRetrievalFormatter, RetrievalEnvelope, RetrievalFormatter,
    RetrievalInput, RetrievalLoadResult, SearchArtifactDocument, SearchFilters, SearchMode,
    SearchQuery, SearchResult, Severity, TextRetrievalFormatter, build_workspace,
    compile_workspace, explain_object, load_retrieval_session, search,
};
use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};

use crate::error::CliError;

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args()) as u8)
}

fn run(arguments: impl IntoIterator<Item = String>) -> i32 {
    match Cli::try_parse_from(arguments) {
        Ok(cli) => match cli.command {
            Commands::Check { path } => check(path),
            Commands::Build {
                path,
                out,
                no_embeddings,
            } => build(path, out, no_embeddings),
            Commands::Explain {
                object_id,
                artifact,
                format,
            } => explain(object_id, artifact, format),
            Commands::Search {
                query,
                artifact,
                kind,
                status,
                owner,
                source_path,
                top,
                format,
            } => search_command(
                query,
                artifact,
                SearchFilters {
                    kind,
                    status,
                    owner,
                    source_path,
                },
                top,
                format,
            ),
        },
        Err(error) => report_parse_error(error),
    }
}

fn report_parse_error(error: clap::Error) -> i32 {
    let exit_code = match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
        _ => 1,
    };

    if let Err(source) = error.print() {
        eprintln!("error[cli.output] could not print command line output: {source}");
        return 1;
    }

    exit_code
}

#[derive(Parser)]
#[command(name = "adoc", version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Check {
        path: PathBuf,
    },
    Build {
        path: PathBuf,
        #[arg(long)]
        out: PathBuf,
        #[arg(long)]
        no_embeddings: bool,
    },
    Explain {
        object_id: String,
        #[arg(long, default_value = "dist/docs.agent.json")]
        artifact: PathBuf,
        #[arg(long, value_enum, default_value = "text")]
        format: RetrievalOutputFormat,
    },
    Search {
        query: String,
        #[arg(long, default_value = "dist/docs.agent.json")]
        artifact: PathBuf,
        #[arg(long)]
        kind: Option<String>,
        #[arg(long)]
        status: Option<String>,
        #[arg(long)]
        owner: Option<String>,
        #[arg(long)]
        source_path: Option<String>,
        #[arg(long, default_value = "10")]
        top: NonZeroUsize,
        #[arg(long, value_enum, default_value = "text")]
        format: RetrievalOutputFormat,
    },
}

fn check(path: PathBuf) -> i32 {
    let result = compile_workspace(CompileInput { root: path });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    if result.has_errors() { 1 } else { 0 }
}

fn build(path: PathBuf, out: PathBuf, no_embeddings: bool) -> i32 {
    let embedding_mode = if no_embeddings {
        BuildEmbeddingMode::Skipped
    } else {
        BuildEmbeddingMode::Enabled
    };
    let result = build_workspace(BuildInput {
        root: path,
        embeddings: embedding_mode,
        prior_search_artifact_path: Some(out.join("docs.search.json")),
    });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    finish_build_result(result, &out)
}

fn finish_build_result(result: CompileResult, out: &Path) -> i32 {
    let has_errors = result.has_errors();

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None if has_errors => return 1,
        None => return report(CliError::BuildMissingArtifacts),
    };

    match write_artifacts(
        out,
        &artifacts.html,
        &artifacts.agent_json,
        artifacts.search_json.as_ref(),
    ) {
        Ok(()) if has_errors => 1,
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn explain(object_id: String, artifact: PathBuf, format: RetrievalOutputFormat) -> i32 {
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let session = match session {
        Some(session) => session,
        None => {
            if format.is_json() {
                return print_retrieval_json(&RetrievalEnvelope::new(Vec::new(), load_diagnostics))
                    .map_or_else(report, |()| 2);
            }
            eprint_diagnostics(&load_diagnostics);
            return 2;
        }
    };

    if diagnostics_have_errors(&load_diagnostics) {
        if format.is_json() {
            return print_retrieval_json(&RetrievalEnvelope::new(Vec::new(), load_diagnostics))
                .map_or_else(report, |()| 2);
        }
        eprint_diagnostics(&load_diagnostics);
        return 2;
    }

    let explain_result = explain_object(&session, &object_id);
    let explain_result = ExplainResult {
        records: explain_result.records,
        diagnostics: merge_diagnostics(load_diagnostics, explain_result.diagnostics),
    };
    let exit_code = explain_exit_code(&explain_result);

    if format.is_json() {
        return print_retrieval_json(&RetrievalEnvelope::from(explain_result))
            .map_or_else(report, |()| exit_code);
    }

    if exit_code != 0 {
        eprint_diagnostics(&explain_result.diagnostics);
        return exit_code;
    }

    if !explain_result.diagnostics.is_empty() {
        eprint_diagnostics(&explain_result.diagnostics);
    }

    print_retrieval_text(&RetrievalEnvelope::from(explain_result)).map_or_else(report, |()| 0)
}

fn search_command(
    query: String,
    artifact: PathBuf,
    filters: SearchFilters,
    top: NonZeroUsize,
    format: RetrievalOutputFormat,
) -> i32 {
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let session = match session {
        Some(session) => session,
        None => {
            if format.is_json() {
                return print_retrieval_json(&RetrievalEnvelope::new(Vec::new(), load_diagnostics))
                    .map_or_else(report, |()| 2);
            }
            eprint_diagnostics(&load_diagnostics);
            return 2;
        }
    };

    if diagnostics_have_errors(&load_diagnostics) {
        if format.is_json() {
            return print_retrieval_json(&RetrievalEnvelope::new(Vec::new(), load_diagnostics))
                .map_or_else(report, |()| 2);
        }
        eprint_diagnostics(&load_diagnostics);
        return 2;
    }

    let search_result = search(
        &session,
        SearchQuery {
            text: query,
            mode: SearchMode::Lexical,
            filters,
            top,
        },
    );
    let search_result = SearchResult {
        records: search_result.records,
        diagnostics: merge_diagnostics(load_diagnostics, search_result.diagnostics),
    };
    let exit_code = search_exit_code(&search_result);

    if format.is_json() {
        return print_retrieval_json(&RetrievalEnvelope::from(search_result))
            .map_or_else(report, |()| exit_code);
    }

    if exit_code != 0 {
        eprint_diagnostics(&search_result.diagnostics);
        return exit_code;
    }

    let envelope = RetrievalEnvelope::from(search_result);
    if !envelope.diagnostics.is_empty() {
        eprint_diagnostics(&envelope.diagnostics);
    }
    if envelope.records.is_empty() {
        println!("(no matches)");
        return 0;
    }

    print_retrieval_text(&envelope).map_or_else(report, |()| 0)
}

fn print_retrieval_json(envelope: &RetrievalEnvelope) -> Result<(), CliError> {
    let text = JsonRetrievalFormatter
        .render(envelope)
        .map_err(|source| CliError::RetrievalFormat { source })?;
    println!("{text}");
    Ok(())
}

fn explain_exit_code(result: &ExplainResult) -> i32 {
    if result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.code == DiagnosticCode::RetrievalObjectNotFound)
    {
        return 3;
    }
    if result
        .diagnostics
        .iter()
        .any(|diagnostic| diagnostic.severity == Severity::Error)
    {
        return 2;
    }
    0
}

fn search_exit_code(result: &SearchResult) -> i32 {
    result
        .diagnostics
        .iter()
        .filter_map(search_diagnostic_exit_code)
        .min()
        .unwrap_or(0)
}

fn search_diagnostic_exit_code(diagnostic: &Diagnostic) -> Option<i32> {
    match (diagnostic.code, diagnostic.severity) {
        (DiagnosticCode::SearchInvalidFilter, _) => Some(1),
        (_, Severity::Error) => Some(2),
        _ => None,
    }
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

fn print_retrieval_text(envelope: &RetrievalEnvelope) -> Result<(), CliError> {
    let text = TextRetrievalFormatter
        .render(envelope)
        .map_err(|source| CliError::RetrievalFormat { source })?;
    print!("{text}");
    Ok(())
}

fn report(error: CliError) -> i32 {
    eprintln!("{error}");
    error.exit_code()
}

fn write_artifacts(
    out: &Path,
    html: &str,
    agent_json: &AgentJsonDocument,
    search_json: Option<&SearchArtifactDocument>,
) -> Result<(), CliError> {
    if out.exists() && !out.is_dir() {
        return Err(CliError::OutputPathIsFile {
            path: out.to_path_buf(),
        });
    }

    fs::create_dir_all(out).map_err(|source| CliError::CreateOutputDirectory {
        path: out.to_path_buf(),
        source,
    })?;

    let html_path = out.join("docs.html");
    fs::write(&html_path, html).map_err(|source| CliError::WriteFailed {
        path: html_path,
        source,
    })?;

    let agent_json_text = agent_json
        .to_pretty_json()
        .map_err(|source| CliError::AgentJsonSerialize { source })?;
    let agent_json_path = out.join("docs.agent.json");
    fs::write(&agent_json_path, agent_json_text).map_err(|source| CliError::WriteFailed {
        path: agent_json_path,
        source,
    })?;

    if let Some(search_json) = search_json {
        let search_json_text = search_json
            .to_pretty_json()
            .map_err(|source| CliError::SearchJsonSerialize { source })?;
        let search_json_path = out.join("docs.search.json");
        fs::write(&search_json_path, search_json_text).map_err(|source| CliError::WriteFailed {
            path: search_json_path,
            source,
        })?;
    }

    Ok(())
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

#[derive(Clone, Copy, ValueEnum)]
enum RetrievalOutputFormat {
    Text,
    Json,
}

impl RetrievalOutputFormat {
    fn is_json(&self) -> bool {
        matches!(self, Self::Json)
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::time::{SystemTime, UNIX_EPOCH};

    use adoc_core::{
        AgentJsonDocument, AgentJsonRelations, BuildArtifacts, CompileResult, RetrievalRecord,
        RetrievalSource,
    };

    use super::*;

    #[test]
    fn finish_build_result_writes_v0_artifacts_and_preserves_prior_search_on_embedding_error() {
        let output_directory = unique_temp_dir("embedding-error-output");
        fs::create_dir_all(&output_directory).expect("output directory can be created");
        fs::write(
            output_directory.join("docs.search.json"),
            "prior search artifact",
        )
        .expect("prior search artifact can be written");
        let result = CompileResult {
            diagnostics: vec![Diagnostic {
                code: DiagnosticCode::EmbedComputeFailed,
                severity: Severity::Error,
                message: "embedding computation failed: encoder failed".to_string(),
                span: None,
                object_id: None,
                help: None,
            }],
            artifacts: Some(BuildArtifacts {
                html: "<h1>Guide</h1>".to_string(),
                agent_json: AgentJsonDocument {
                    schema_version: "adoc.agent.v0".to_string(),
                    pages: Vec::new(),
                    objects: Vec::new(),
                    diagnostics: Vec::new(),
                },
                search_json: None,
            }),
        };

        let exit_code = finish_build_result(result, &output_directory);

        assert_eq!(exit_code, 1);
        assert_eq!(
            fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written"),
            "<h1>Guide</h1>"
        );
        assert!(
            fs::read_to_string(output_directory.join("docs.agent.json"))
                .expect("agent JSON is written")
                .contains("\"schema_version\": \"adoc.agent.v0\"")
        );
        assert_eq!(
            fs::read_to_string(output_directory.join("docs.search.json"))
                .expect("prior search artifact remains readable"),
            "prior search artifact"
        );
    }

    fn unique_temp_dir(name: &str) -> PathBuf {
        let nanos = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time after Unix epoch")
            .as_nanos();
        std::env::temp_dir().join(format!("adoc-cli-{name}-{}-{nanos}", std::process::id()))
    }

    #[test]
    fn explain_exit_code_allows_warning_diagnostics_when_record_is_present() {
        let result = ExplainResult {
            records: vec![RetrievalRecord {
                id: "billing.credits".to_string(),
                kind: "claim".to_string(),
                status: Some("verified".to_string()),
                owner: None,
                verified_at: None,
                body: "Credits decrement after payment succeeds.".to_string(),
                source: RetrievalSource {
                    path: "docs/billing.adoc".to_string(),
                    line: 1,
                    column: 1,
                },
                evidence: BTreeMap::new(),
                fields: BTreeMap::new(),
                relations: AgentJsonRelations::default(),
                search_match: None,
            }],
            diagnostics: vec![Diagnostic {
                code: DiagnosticCode::ClaimStatusCasing,
                severity: Severity::Warning,
                message: "status casing".to_string(),
                span: None,
                object_id: None,
                help: None,
            }],
        };

        assert_eq!(explain_exit_code(&result), 0);
    }

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

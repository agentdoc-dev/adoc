mod adapters;
mod error;
mod presentation;

use std::fs;
use std::num::NonZeroUsize;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adoc_core::{
    AgentJsonDocument, BuildEmbeddingMode, BuildInput, CompileInput, CompileResult, Diagnostic,
    DiagnosticCode, EmbedQueryError, ExplainError, RetrievalEnvelope, RetrievalInput,
    RetrievalLoadResult, SearchArtifactDocument, SearchFilters, SearchMode, SearchQuery,
    SearchResult, Severity, build_workspace, compile_workspace, embed_query,
    load_retrieval_session, search,
};
use clap::{Parser, Subcommand, ValueEnum, error::ErrorKind};

use crate::adapters::{ArtifactRecordResolver, SystemClock};
use crate::error::CliError;
use crate::presentation::{
    ColorChoice, FormatChoice, ResolvedFormat, json as json_presentation, make_presenter,
    plain as plain_presentation, terminal,
};

const INIT_CONFIG_PATH: &str = "agentdoc.config.yaml";
const INIT_INDEX_PATH: &str = "docs/index.adoc";
const INIT_CONFIG_TEMPLATE: &str = "\
version: 1
mode: strict
docs_path: docs
outputs:
  dir: dist
  html: dist/docs.html
  agent_json: dist/docs.agent.json
  search: dist/docs.search.json
embeddings:
  provider: local
";
const INIT_INDEX_TEMPLATE: &str = "\
# AgentDoc Project @doc(project.index)

This project was initialized with AgentDoc.

::claim project.initialized
status: verified
owner: team-docs
verified_at: 2026-05-08
source: adoc init template
expires_at: 2027-05-08
--
The project has an initialized AgentDoc source tree.
::
";

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args()) as u8)
}

fn run(arguments: impl IntoIterator<Item = String>) -> i32 {
    match Cli::try_parse_from(arguments) {
        Ok(cli) => {
            let resolved = terminal::detect(cli.format.into(), cli.color.into());
            match cli.command {
                Commands::Init => init(),
                Commands::Check { path } => check(path),
                Commands::Build {
                    path,
                    out,
                    no_embeddings,
                } => build(path, out, no_embeddings),
                Commands::Explain {
                    object_id,
                    artifact,
                } => explain(object_id, artifact, resolved),
                Commands::Search {
                    query,
                    artifact,
                    search_artifact,
                    semantic,
                    lexical,
                    kind,
                    status,
                    owner,
                    source_path,
                    top,
                } => search_command(
                    SearchCommandInput {
                        query,
                        artifact,
                        search_artifact,
                        semantic,
                        lexical,
                        filters: SearchFilters {
                            kind,
                            status,
                            owner,
                            source_path,
                        },
                        top,
                    },
                    resolved,
                ),
            }
        }
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

/// The output format requested on the command line (`--format`).
#[derive(Clone, Copy, Default, ValueEnum)]
enum CliFormat {
    /// Auto-detect: styled when stdout is a TTY, plain otherwise.
    #[default]
    Auto,
    /// Plain uncoloured text.
    Plain,
    /// Styled text with ANSI colour codes.
    Styled,
    /// Machine-readable JSON.
    Json,
}

impl From<CliFormat> for FormatChoice {
    fn from(f: CliFormat) -> Self {
        match f {
            CliFormat::Auto => Self::Auto,
            CliFormat::Plain => Self::Plain,
            CliFormat::Styled => Self::Styled,
            CliFormat::Json => Self::Json,
        }
    }
}

/// The colour mode requested on the command line (`--color`).
#[derive(Clone, Copy, Default, ValueEnum)]
enum CliColor {
    /// Enable colour only when stdout is a TTY and `NO_COLOR` is unset.
    #[default]
    Auto,
    /// Always emit ANSI colour codes.
    Always,
    /// Never emit ANSI colour codes.
    Never,
}

impl From<CliColor> for ColorChoice {
    fn from(c: CliColor) -> Self {
        match c {
            CliColor::Auto => Self::Auto,
            CliColor::Always => Self::Always,
            CliColor::Never => Self::Never,
        }
    }
}

#[derive(Parser)]
#[command(name = "adoc", version)]
struct Cli {
    /// Output format.  `auto` selects `styled` when stdout is a TTY and
    /// `NO_COLOR` is unset, otherwise `plain`.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    format: CliFormat,

    /// Colour output.  `auto` enables colour only on a TTY without `NO_COLOR`.
    /// `always` overrides the TTY check.  `never` disables colour.
    #[arg(long, global = true, value_enum, default_value = "auto")]
    color: CliColor,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    Init,
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
    },
    Search {
        query: String,
        #[arg(long, default_value = "dist/docs.agent.json")]
        artifact: PathBuf,
        #[arg(long, default_value = "dist/docs.search.json")]
        search_artifact: PathBuf,
        #[arg(long, conflicts_with = "lexical")]
        semantic: bool,
        /// Reserved for the V1.5/V1.6 hybrid slice; today this is the default
        /// when neither --semantic nor --lexical is set, so the flag is a no-op.
        #[arg(long, conflicts_with = "semantic")]
        lexical: bool,
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
    },
}

fn init() -> i32 {
    match write_init_files() {
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn write_init_files() -> Result<(), CliError> {
    let config_path = PathBuf::from(INIT_CONFIG_PATH);
    let index_path = PathBuf::from(INIT_INDEX_PATH);

    for target in [&config_path, &index_path] {
        if target.exists() {
            return Err(CliError::InitTargetExists {
                path: target.to_path_buf(),
            });
        }
    }

    if let Some(parent) = index_path.parent() {
        fs::create_dir_all(parent).map_err(|source| CliError::CreateOutputDirectory {
            path: parent.to_path_buf(),
            source,
        })?;
    }

    fs::write(&config_path, INIT_CONFIG_TEMPLATE).map_err(|source| CliError::WriteFailed {
        path: config_path,
        source,
    })?;
    fs::write(&index_path, INIT_INDEX_TEMPLATE).map_err(|source| CliError::WriteFailed {
        path: index_path,
        source,
    })?;

    Ok(())
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

fn explain(object_id: String, artifact: PathBuf, resolved: ResolvedFormat) -> i32 {
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
            if resolved == ResolvedFormat::Json {
                return json_presentation::write_envelope_json(
                    &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                    &mut std::io::stdout(),
                )
                .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
            }
            eprint_diagnostics(&load_diagnostics);
            return 2;
        }
    };

    if diagnostics_have_errors(&load_diagnostics) {
        if resolved == ResolvedFormat::Json {
            return json_presentation::write_envelope_json(
                &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                &mut std::io::stdout(),
            )
            .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
        }
        eprint_diagnostics(&load_diagnostics);
        return 2;
    }

    // Build the application service with the full record set from the session
    // so that relation targets can be resolved for `related_statuses`.
    let resolver = ArtifactRecordResolver::new(session.records());
    let service = adoc_core::ExplainService::new(resolver, SystemClock, artifact);

    match service.execute(&object_id) {
        Ok(view) => match resolved {
            ResolvedFormat::Json => {
                let presenter = make_presenter(ResolvedFormat::Json, load_diagnostics);
                presenter
                    .present(&view, &mut std::io::stdout())
                    .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 0)
            }
            _ => {
                if !load_diagnostics.is_empty() {
                    eprint_diagnostics(&load_diagnostics);
                }
                let presenter = make_presenter(resolved, Vec::new());
                presenter
                    .present(&view, &mut std::io::stdout())
                    .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 0)
            }
        },
        Err(ExplainError::NotFound(id)) => {
            let diagnostic = Diagnostic::not_found(id);
            if resolved == ResolvedFormat::Json {
                return json_presentation::write_envelope_json(
                    &RetrievalEnvelope::new(Vec::new(), vec![diagnostic]),
                    &mut std::io::stdout(),
                )
                .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 3);
            }
            eprint_diagnostics(&[diagnostic]);
            3
        }
        Err(ExplainError::Resolver(source)) => {
            let diagnostic = Diagnostic::resolver(&source);
            if resolved == ResolvedFormat::Json {
                return json_presentation::write_envelope_json(
                    &RetrievalEnvelope::new(Vec::new(), vec![diagnostic]),
                    &mut std::io::stdout(),
                )
                .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
            }
            eprint_diagnostics(&[diagnostic]);
            2
        }
    }
}

struct SearchCommandInput {
    query: String,
    artifact: PathBuf,
    search_artifact: PathBuf,
    semantic: bool,
    lexical: bool,
    filters: SearchFilters,
    top: NonZeroUsize,
}

fn search_command(input: SearchCommandInput, resolved: ResolvedFormat) -> i32 {
    let requested_mode = if input.semantic {
        SearchMode::Semantic
    } else if input.lexical {
        SearchMode::Lexical
    } else {
        SearchMode::Hybrid
    };

    let search_artifact_path = match requested_mode {
        SearchMode::Lexical => None,
        SearchMode::Hybrid | SearchMode::Semantic => Some(input.search_artifact),
    };

    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: input.artifact,
        search_artifact_path,
    });
    let RetrievalLoadResult {
        session,
        diagnostics: load_diagnostics,
    } = load_result;
    let session = match session {
        Some(session) => session,
        None => {
            if resolved == ResolvedFormat::Json {
                return json_presentation::write_envelope_json(
                    &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                    &mut std::io::stdout(),
                )
                .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
            }
            eprint_diagnostics(&load_diagnostics);
            return 2;
        }
    };

    if diagnostics_have_errors(&load_diagnostics) {
        if resolved == ResolvedFormat::Json {
            return json_presentation::write_envelope_json(
                &RetrievalEnvelope::new(Vec::new(), load_diagnostics),
                &mut std::io::stdout(),
            )
            .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
        }
        eprint_diagnostics(&load_diagnostics);
        return 2;
    }

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
        let envelope = RetrievalEnvelope::new(Vec::new(), diagnostics);
        if resolved == ResolvedFormat::Json {
            return json_presentation::write_envelope_json(&envelope, &mut std::io::stdout())
                .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
        }
        eprint_diagnostics(&envelope.diagnostics);
        return 2;
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
                if resolved == ResolvedFormat::Json {
                    return json_presentation::write_envelope_json(
                        &RetrievalEnvelope::new(Vec::new(), vec![diagnostic]),
                        &mut std::io::stdout(),
                    )
                    .map_or_else(|source| report(CliError::RetrievalIo { source }), |()| 2);
                }
                eprint_diagnostics(&[diagnostic]);
                return 2;
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
            filters: input.filters,
            top: input.top,
            query_vector,
        },
    );
    let search_result = SearchResult {
        records: search_result.records,
        diagnostics: merge_diagnostics(load_diagnostics, search_result.diagnostics),
    };
    let exit_code = search_exit_code(&search_result);

    if resolved == ResolvedFormat::Json {
        return json_presentation::write_envelope_json(
            &RetrievalEnvelope::from(search_result),
            &mut std::io::stdout(),
        )
        .map_or_else(
            |source| report(CliError::RetrievalIo { source }),
            |()| exit_code,
        );
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

    // Search renders multiple records; delegate to the shared plain-text helper
    // in the presentation layer so there is a single rendering path.
    let mut buf = String::new();
    for (index, record) in envelope.records.iter().enumerate() {
        if index > 0 {
            buf.push('\n');
        }
        plain_presentation::render_record(&mut buf, record, None);
    }
    if let Err(source) = std::io::Write::write_all(&mut std::io::stdout(), buf.as_bytes()) {
        return report(CliError::RetrievalIo { source });
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

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use adoc_core::{AgentJsonDocument, BuildArtifacts, CompileResult};

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

mod error;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adoc_core::{
    AgentJsonDocument, CompileInput, Diagnostic, DiagnosticCode, ExplainResult,
    JsonRetrievalFormatter, RetrievalEnvelope, RetrievalFormatter, RetrievalInput, Severity,
    TextRetrievalFormatter, compile_workspace, explain_object, load_retrieval_session,
};

use crate::error::CliError;

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args().skip(1).collect()) as u8)
}

fn run(arguments: Vec<String>) -> i32 {
    match parse_command(arguments) {
        Ok(Command::Check { path }) => check(path),
        Ok(Command::Build { path, out }) => build(path, out),
        Ok(Command::Explain {
            object_id,
            artifact,
            format,
        }) => explain(object_id, artifact, format),
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: adoc check <path>");
            eprintln!("       adoc build <path> --out <directory>");
            eprintln!("       adoc explain <object-id> [--artifact <path>] [--format text|json]");
            error.exit_code()
        }
    }
}

fn check(path: PathBuf) -> i32 {
    let result = compile_workspace(CompileInput { root: path });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    if result.has_errors() { 1 } else { 0 }
}

fn build(path: PathBuf, out: PathBuf) -> i32 {
    let result = compile_workspace(CompileInput { root: path });
    print_diagnostics(&result.diagnostics);
    print_summary(&result.diagnostics);

    if result.has_errors() {
        return 1;
    }

    let artifacts = match result.artifacts {
        Some(artifacts) => artifacts,
        None => return report(CliError::BuildMissingArtifacts),
    };

    match write_artifacts(&out, &artifacts.html, &artifacts.agent_json) {
        Ok(()) => 0,
        Err(error) => report(error),
    }
}

fn explain(object_id: String, artifact: PathBuf, format: ExplainFormat) -> i32 {
    let load_result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact,
    });
    let session = match load_result.session {
        Some(session) => session,
        None => {
            if format.is_json() {
                return print_explain_json(&RetrievalEnvelope::new(
                    Vec::new(),
                    load_result.diagnostics,
                ))
                .map_or_else(report, |()| 2);
            }
            eprint_diagnostics(&load_result.diagnostics);
            return 2;
        }
    };

    let explain_result = explain_object(&session, &object_id);
    let exit_code = explain_exit_code(&explain_result);

    if format.is_json() {
        return print_explain_json(&RetrievalEnvelope::from(explain_result))
            .map_or_else(report, |()| exit_code);
    }

    if exit_code != 0 {
        eprint_diagnostics(&explain_result.diagnostics);
        return exit_code;
    }

    print_explain_text(&RetrievalEnvelope::from(explain_result)).map_or_else(report, |()| 0)
}

fn print_explain_json(envelope: &RetrievalEnvelope) -> Result<(), CliError> {
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
    if !result.diagnostics.is_empty() {
        return 2;
    }
    0
}

fn print_explain_text(envelope: &RetrievalEnvelope) -> Result<(), CliError> {
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

fn write_artifacts(out: &Path, html: &str, agent_json: &AgentJsonDocument) -> Result<(), CliError> {
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

enum Command {
    Check {
        path: PathBuf,
    },
    Build {
        path: PathBuf,
        out: PathBuf,
    },
    Explain {
        object_id: String,
        artifact: PathBuf,
        format: ExplainFormat,
    },
}

enum ExplainFormat {
    Text,
    Json,
}

impl ExplainFormat {
    fn is_json(&self) -> bool {
        matches!(self, Self::Json)
    }
}

fn parse_command(arguments: Vec<String>) -> Result<Command, CliError> {
    match arguments.as_slice() {
        [command, path] if command == "check" => Ok(Command::Check {
            path: PathBuf::from(path),
        }),
        [command, path, out_flag, out] if command == "build" && out_flag == "--out" => {
            Ok(Command::Build {
                path: PathBuf::from(path),
                out: PathBuf::from(out),
            })
        }
        [command, arguments @ ..] if command == "explain" => parse_explain_command(arguments),
        [] => Err(CliError::MissingCommand),
        [command, ..] if command == "build" => Err(CliError::InvalidBuildUsage),
        [command, ..] => Err(CliError::UnknownCommand {
            command: command.clone(),
        }),
    }
}

fn parse_explain_command(arguments: &[String]) -> Result<Command, CliError> {
    let Some((object_id, rest)) = arguments.split_first() else {
        return Err(CliError::InvalidExplainUsage);
    };
    if object_id.starts_with("--") {
        return Err(CliError::InvalidExplainUsage);
    }

    let mut artifact = PathBuf::from("dist/docs.agent.json");
    let mut format = ExplainFormat::Text;
    let mut index = 0;
    while index < rest.len() {
        match rest[index].as_str() {
            "--artifact" => {
                let Some(path) = rest.get(index + 1) else {
                    return Err(CliError::InvalidExplainUsage);
                };
                artifact = PathBuf::from(path);
                index += 2;
            }
            "--format" => {
                let Some(value) = rest.get(index + 1) else {
                    return Err(CliError::InvalidExplainUsage);
                };
                format = match value.as_str() {
                    "text" => ExplainFormat::Text,
                    "json" => ExplainFormat::Json,
                    unsupported => {
                        return Err(CliError::UnsupportedExplainFormat {
                            format: unsupported.to_string(),
                        });
                    }
                };
                index += 2;
            }
            _ => return Err(CliError::InvalidExplainUsage),
        }
    }

    Ok(Command::Explain {
        object_id: object_id.clone(),
        artifact,
        format,
    })
}

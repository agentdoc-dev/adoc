mod error;

use std::fs;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use adoc_core::{AgentJsonDocument, CompileInput, Diagnostic, Severity, compile_workspace};

use crate::error::CliError;

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args().skip(1).collect()) as u8)
}

fn run(arguments: Vec<String>) -> i32 {
    match parse_command(arguments) {
        Ok(Command::Check { path }) => check(path),
        Ok(Command::Build { path, out }) => build(path, out),
        Err(error) => {
            eprintln!("{error}");
            eprintln!("usage: adoc check <path>");
            eprintln!("       adoc build <path> --out <directory>");
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
    Check { path: PathBuf },
    Build { path: PathBuf, out: PathBuf },
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
        [] => Err(CliError::MissingCommand),
        [command, ..] if command == "build" => Err(CliError::InvalidBuildUsage),
        [command, ..] => Err(CliError::UnknownCommand {
            command: command.clone(),
        }),
    }
}

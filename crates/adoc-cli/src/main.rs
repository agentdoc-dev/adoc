use std::fs;
use std::path::{Path, PathBuf};

use adoc_core::{AgentJsonDocument, CompileInput, Diagnostic, Severity, compile_workspace};

fn main() {
    let exit_code = run(std::env::args().skip(1).collect());
    std::process::exit(exit_code);
}

fn run(arguments: Vec<String>) -> i32 {
    match parse_command(arguments) {
        Ok(Command::Check { path }) => check(path),
        Ok(Command::Build { path, out }) => build(path, out),
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: adoc check <path>");
            eprintln!("       adoc build <path> --out <directory>");
            2
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

    let Some(artifacts) = result.artifacts else {
        eprintln!("build did not produce artifacts");
        return 1;
    };

    if let Err(error) = write_artifacts(&out, &artifacts.html, &artifacts.agent_json) {
        eprintln!("{error}");
        return 1;
    }

    0
}

fn write_artifacts(out: &Path, html: &str, agent_json: &AgentJsonDocument) -> Result<(), String> {
    fs::create_dir_all(out).map_err(|error| {
        format!(
            "error[io.output_not_directory] could not create output directory {}: {error}",
            out.display()
        )
    })?;

    fs::write(out.join("docs.html"), html).map_err(|error| {
        format!(
            "error[io.write_failed] could not write {}: {error}",
            out.join("docs.html").display()
        )
    })?;

    let agent_json = agent_json.to_pretty_json().map_err(|error| {
        format!("error[artifact.agent_json] could not serialize agent JSON: {error}")
    })?;
    fs::write(out.join("docs.agent.json"), agent_json).map_err(|error| {
        format!(
            "error[io.write_failed] could not write {}: {error}",
            out.join("docs.agent.json").display()
        )
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

fn parse_command(arguments: Vec<String>) -> Result<Command, String> {
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
        [] => Err("missing command".to_string()),
        [command, ..] => Err(format!("unknown or invalid command: {command}")),
    }
}

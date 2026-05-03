use std::path::PathBuf;

use adoc_core::{CompileInput, Diagnostic, Severity, compile_workspace};

fn main() {
    let exit_code = run(std::env::args().skip(1).collect());
    std::process::exit(exit_code);
}

fn run(arguments: Vec<String>) -> i32 {
    match parse_command(arguments) {
        Ok(Command::Check { path }) => check(path),
        Err(message) => {
            eprintln!("{message}");
            eprintln!("usage: adoc check <path>");
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
}

fn parse_command(arguments: Vec<String>) -> Result<Command, String> {
    match arguments.as_slice() {
        [command, path] if command == "check" => Ok(Command::Check {
            path: PathBuf::from(path),
        }),
        [] => Err("missing command".to_string()),
        [command, ..] => Err(format!("unknown or invalid command: {command}")),
    }
}

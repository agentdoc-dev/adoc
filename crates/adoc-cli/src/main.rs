mod cli;
mod commands;
mod error;
mod presentation;

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::cli::{Cli, Commands};
use crate::commands::{
    ContradictionsCommandInput, DiffCommandInput, GraphCommandInput, ImpactedByCommandInput,
    MigrateCommandInput, PatchCommandInput, ReviewCommandInput, SearchCommandInput,
    StaleCommandInput, build, check, contradictions, diff, graph, impacted_by, init, migrate,
    patch, review, search_command, stale, why,
};
use crate::presentation::{ResolvedFormat, terminal};

fn main() -> ExitCode {
    init_tracing();
    ExitCode::from(run(std::env::args()) as u8)
}

/// Logs go to stderr only — stdout carries command output and JSON
/// envelopes. Filtered by `ADOC_LOG` (falling back to `RUST_LOG`); silent
/// when neither is set, so default CLI behavior is unchanged.
fn init_tracing() {
    use tracing_subscriber::EnvFilter;

    let filter = EnvFilter::try_from_env("ADOC_LOG")
        .or_else(|_| EnvFilter::try_from_default_env())
        .unwrap_or_else(|_| EnvFilter::new("off"));
    let _ = tracing_subscriber::fmt()
        .with_env_filter(filter)
        .with_span_events(tracing_subscriber::fmt::format::FmtSpan::CLOSE)
        .with_writer(std::io::stderr)
        .try_init();
}

fn run(arguments: impl IntoIterator<Item = String>) -> i32 {
    match Cli::try_parse_from(arguments) {
        Ok(cli) => {
            let resolved = terminal::detect(cli.format.into(), cli.color.into());
            // Markdown is PR-comment output; reject it before any command
            // that does not implement a markdown presenter. Check, diff,
            // review, and impacted-by pass through; every other command
            // exits non-zero with a fix-oriented stderr line.
            if resolved == ResolvedFormat::Markdown
                && !matches!(
                    cli.command,
                    Commands::Check { .. }
                        | Commands::Diff { .. }
                        | Commands::Review { .. }
                        | Commands::ImpactedBy { .. }
                )
            {
                eprintln!(
                    "error[cli.format] --format markdown is only supported by `adoc check`, `adoc diff`, `adoc review`, and `adoc impacted-by`"
                );
                return 2;
            }
            match cli.command {
                Commands::Init => init(),
                Commands::Check { path } => check(path, resolved),
                Commands::Migrate {
                    path,
                    write,
                    force,
                    export,
                } => migrate(
                    MigrateCommandInput {
                        path,
                        write,
                        force,
                        export,
                    },
                    resolved,
                ),
                Commands::Build {
                    path,
                    out,
                    no_embeddings,
                } => build(path, out, no_embeddings),
                Commands::Why {
                    object_id,
                    artifact,
                } => why(object_id, artifact, resolved),
                Commands::Graph {
                    object_id,
                    artifact,
                    relation,
                    direction,
                } => graph(
                    GraphCommandInput {
                        object_id,
                        artifact,
                        relation: relation.map(Into::into),
                        direction: direction.map(Into::into),
                    },
                    resolved,
                ),
                Commands::Stale { artifact, within } => stale(
                    StaleCommandInput {
                        artifact,
                        within_days: within,
                    },
                    resolved,
                ),
                Commands::Contradictions { artifact, all } => {
                    contradictions(ContradictionsCommandInput { artifact, all }, resolved)
                }
                Commands::ImpactedBy {
                    paths,
                    git_ref,
                    artifact,
                } => impacted_by(
                    ImpactedByCommandInput {
                        paths,
                        git_ref,
                        artifact,
                    },
                    resolved,
                ),
                Commands::Patch {
                    check,
                    apply,
                    artifact,
                } => patch(
                    PatchCommandInput {
                        check,
                        apply,
                        artifact,
                    },
                    resolved,
                ),
                Commands::Diff { base_ref } => diff(DiffCommandInput { base_ref }, resolved),
                Commands::Review { base_ref, patch } => {
                    review(ReviewCommandInput { base_ref, patch }, resolved)
                }
                Commands::Search {
                    query,
                    artifact,
                    search_artifact,
                    semantic,
                    lexical,
                    objects_only,
                    prose_only,
                    kind,
                    status,
                    owner,
                    source_path,
                    related_to,
                    relation,
                    direction,
                    top,
                } => search_command(
                    SearchCommandInput {
                        query,
                        artifact,
                        search_artifact,
                        semantic,
                        lexical,
                        objects_only,
                        prose_only,
                        kind,
                        status,
                        owner,
                        source_path,
                        related_to,
                        relation: relation.map(Into::into),
                        direction: direction.map(Into::into),
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

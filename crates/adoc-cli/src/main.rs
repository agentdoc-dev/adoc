mod cli;
mod commands;
mod error;
mod presentation;

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::cli::{Cli, Commands};
use crate::commands::{
    DiffCommandInput, GraphCommandInput, PatchCommandInput, ReviewCommandInput, SearchCommandInput,
    StaleCommandInput, build, check, diff, graph, init, patch, review, search_command, stale, why,
};
use crate::presentation::{ResolvedFormat, terminal};

fn main() -> ExitCode {
    ExitCode::from(run(std::env::args()) as u8)
}

fn run(arguments: impl IntoIterator<Item = String>) -> i32 {
    match Cli::try_parse_from(arguments) {
        Ok(cli) => {
            let resolved = terminal::detect(cli.format.into(), cli.color.into());
            // Markdown is review-only output; reject it before any command
            // that does not implement a markdown presenter. Diff and review
            // pass through; every other command exits non-zero with a
            // fix-oriented stderr line.
            if resolved == ResolvedFormat::Markdown
                && !matches!(cli.command, Commands::Diff { .. } | Commands::Review { .. })
            {
                eprintln!(
                    "error[cli.format] --format markdown is only supported by `adoc diff` and `adoc review`"
                );
                return 2;
            }
            match cli.command {
                Commands::Init => init(),
                Commands::Check { path } => check(path),
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
                Commands::Patch { check, artifact } => patch(
                    PatchCommandInput {
                        patch_path: check,
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

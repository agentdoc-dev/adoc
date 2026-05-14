mod cli;
mod commands;
mod config;
mod error;
mod presentation;

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::cli::{Cli, Commands};
use crate::commands::{
    GraphCommandInput, SearchCommandInput, build, check, graph, init, search_command, why,
};
use crate::presentation::terminal;

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
                Commands::Why {
                    object_id,
                    artifact,
                } => why(object_id, artifact, resolved),
                Commands::Graph {
                    object_id,
                    artifact,
                    agent_artifact,
                    relation,
                    direction,
                } => graph(
                    GraphCommandInput {
                        object_id,
                        artifact,
                        agent_artifact,
                        relation: relation.map(Into::into),
                        direction: direction.map(Into::into),
                    },
                    resolved,
                ),
                Commands::Search {
                    query,
                    artifact,
                    search_artifact,
                    graph_artifact,
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
                        graph_artifact,
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

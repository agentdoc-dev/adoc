mod cli;
mod commands;
mod error;
mod presentation;

use std::process::ExitCode;

use clap::{Parser, error::ErrorKind};

use crate::cli::{Cli, Commands};
use crate::commands::{
    AssessChangesCommandInput, BaselineCommandInput, ContradictionsCommandInput, DiffCommandInput,
    GraphCommandInput, ImpactedByCommandInput, MigrateCommandInput, PatchCommandInput,
    ReviewCommandInput, SearchCommandInput, StaleCommandInput, assess_changes, baseline, build,
    check, check_receipt, contradictions, diff, graph, impacted_by, init, migrate, patch, review,
    search_command, stale, why,
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
    let arguments = arguments.into_iter().collect::<Vec<_>>();
    let assessment_requested = arguments
        .iter()
        .any(|argument| matches!(argument.as_str(), "assess-changes" | "baseline"));
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
                        | Commands::AssessChanges { .. }
                        | Commands::Baseline { .. }
                )
            {
                eprintln!(
                    "error[cli.format] --format markdown is only supported by `adoc check`, `adoc diff`, `adoc review`, and `adoc impacted-by`"
                );
                return 2;
            }
            match cli.command {
                Commands::Init => init(),
                Commands::Check {
                    path,
                    style,
                    as_of,
                    receipt,
                    runtime_binary_digest,
                    context_artifact,
                    semantic_context,
                    semantic_subject_revision,
                    semantic_source_revision,
                    semantic_base_revision,
                    semantic_head_revision,
                    semantic_assessment_digest,
                    semantic_required_class,
                } => match (receipt, runtime_binary_digest, as_of) {
                    // clap `requires` guarantees the digest and --as-of
                    // accompany --receipt; the triple is re-matched here so
                    // the plain path never sees receipt state.
                    (Some(receipt), Some(runtime_binary_digest), Some(as_of)) => {
                        // Receipt mode writes the canonical envelope and
                        // prints plain diagnostics; it has no markdown
                        // presenter — refuse rather than silently fall
                        // back (mirrors the command gate above).
                        if resolved == ResolvedFormat::Markdown {
                            eprintln!(
                                "error[cli.format] --format markdown is not supported with --receipt; receipt mode prints plain diagnostics and writes the canonical envelope to the receipt path"
                            );
                            return 2;
                        }
                        check_receipt(
                            path,
                            as_of,
                            receipt,
                            runtime_binary_digest,
                            context_artifact,
                            semantic_context,
                            match (
                                semantic_subject_revision,
                                semantic_source_revision,
                                semantic_base_revision,
                                semantic_head_revision,
                                semantic_assessment_digest,
                                semantic_required_class,
                            ) {
                                (
                                    Some(subject_revision),
                                    Some(source_revision),
                                    Some(base_revision),
                                    Some(head_revision),
                                    Some(assessment_digest),
                                    required_context_classes,
                                ) if !required_context_classes.is_empty() => {
                                    Some(adoc_core::SemanticContextExpectedBindings {
                                        subject_revision,
                                        source_revision,
                                        base_revision,
                                        head_revision,
                                        assessment_digest,
                                        required_context_classes,
                                    })
                                }
                                (None, None, None, None, None, required_context_classes)
                                    if required_context_classes.is_empty() =>
                                {
                                    None
                                }
                                // Unreachable while the clap `requires_all` wiring holds
                                // (pinned by semantic_context_requires_complete_validation_basis).
                                // Refuse loudly if it is loosened: passing `None` would produce a
                                // fail receipt that blames the context instead of the invocation.
                                _ => {
                                    eprintln!(
                                        "error[cli.semantic_context] --semantic-context requires all four exact revision flags, --semantic-assessment-digest, and at least one --semantic-required-class; refusing to run with a partial validation basis"
                                    );
                                    return 2;
                                }
                            },
                        )
                    }
                    // Unreachable while the clap `requires` wiring holds
                    // (pinned by check_receipt_requires_as_of_and_runtime_binary_digest);
                    // if that wiring is ever loosened, refuse loudly —
                    // falling through to a plain check would exit 0 with
                    // no receipt written, and a harness reading exit 0 as
                    // "receipt produced" would consume a stale file.
                    (Some(_), _, _) => {
                        eprintln!(
                            "error[cli.receipt] --receipt requires --as-of and --runtime-binary-digest; refusing to run a plain check in their place"
                        );
                        2
                    }
                    (None, _, as_of) => check(path, style.into(), as_of, resolved),
                },
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
                    as_of,
                } => build(path, out, no_embeddings, as_of),
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
                Commands::AssessChanges { base, head, as_of } => assess_changes(
                    AssessChangesCommandInput {
                        base_ref: base,
                        head_ref: head,
                        as_of,
                    },
                    resolved,
                ),
                Commands::Baseline { git_ref, as_of } => {
                    baseline(BaselineCommandInput { git_ref, as_of }, resolved)
                }
                Commands::Patch {
                    check,
                    apply,
                    artifact,
                    as_of,
                } => patch(
                    PatchCommandInput {
                        check,
                        apply,
                        artifact,
                        as_of,
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
        Err(error) => report_parse_error(error, assessment_requested),
    }
}

fn report_parse_error(error: clap::Error, assessment_requested: bool) -> i32 {
    let exit_code = match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => 0,
        _ if assessment_requested => 2,
        _ => 1,
    };

    if let Err(source) = error.print() {
        eprintln!("error[cli.output] could not print command line output: {source}");
        return 1;
    }

    exit_code
}

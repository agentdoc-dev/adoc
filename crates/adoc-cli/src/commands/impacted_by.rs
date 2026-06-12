//! V6.3 `adoc impacted-by` — which knowledge is implicated by changed source
//! paths? A pure graph-artifact read in the V6.1/V6.2 signal-command shape,
//! plus the V3.5 markdown presenter for PR comments.

use std::fmt::Write as FmtWrite;
use std::io;
use std::path::PathBuf;

use adoc_core::{ImpactReason, ImpactReasonKind, ImpactedEnvelope, ImpactedRecord};
use adoc_local::{
    ImpactedChangedSet, ImpactedInput as LocalImpactedInput, ImpactedUseCase, LocalContext,
    UnrestrictedPathPolicy,
};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{MarkdownReviewPresenter, ResolvedFormat, json as json_presentation};

use super::{current_dir, eprint_diagnostics, report};

pub(crate) struct ImpactedByCommandInput {
    pub(crate) paths: Vec<String>,
    pub(crate) git_ref: Option<String>,
    pub(crate) artifact: Option<PathBuf>,
}

pub(crate) fn impacted_by(input: ImpactedByCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    // clap enforces the XOR (`required_unless_present` + `conflicts_with`),
    // so `git_ref` present means the paths list is empty.
    let changed = match input.git_ref {
        Some(git_ref) => ImpactedChangedSet::GitRef(git_ref),
        None => ImpactedChangedSet::Paths(input.paths),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match ImpactedUseCase::new(context).run(LocalImpactedInput {
        artifact: input.artifact,
        changed,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let exit_code = outcome.exit_code;
    if exit_code != 0 {
        return emit_impacted_error(outcome.envelope, resolved, exit_code);
    }
    if resolved != ResolvedFormat::Json && !outcome.envelope.diagnostics.is_empty() {
        eprint_diagnostics(&outcome.envelope.diagnostics);
    }
    match resolved {
        ResolvedFormat::Json => write_impacted_json(&outcome.envelope, exit_code),
        ResolvedFormat::Plain => write_impacted_text(&outcome.envelope, false),
        ResolvedFormat::Styled => write_impacted_text(&outcome.envelope, true),
        ResolvedFormat::Markdown => write_impacted_markdown(&outcome.envelope, exit_code),
    }
}

fn emit_impacted_error(
    envelope: ImpactedEnvelope,
    resolved: ResolvedFormat,
    exit_code: i32,
) -> i32 {
    match resolved {
        // JSON consumers always get the envelope (ADR-0038).
        ResolvedFormat::Json => write_impacted_json(&envelope, exit_code),
        // Markdown is a PR-comment surface: a bot pasting stdout must show a
        // visible refusal, not an empty comment. Stderr keeps the
        // fix-oriented diagnostics for terminal users either way.
        ResolvedFormat::Markdown => {
            eprint_diagnostics(&envelope.diagnostics);
            MarkdownReviewPresenter::write_impacted_error(&envelope.diagnostics, &mut io::stdout())
                .map_or_else(
                    |source| report(CliError::RetrievalIo { source }),
                    |()| exit_code,
                )
        }
        ResolvedFormat::Plain | ResolvedFormat::Styled => {
            eprint_diagnostics(&envelope.diagnostics);
            exit_code
        }
    }
}

fn write_impacted_json(envelope: &ImpactedEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_impacted_markdown(envelope: &ImpactedEnvelope, exit_code: i32) -> i32 {
    MarkdownReviewPresenter::write_impacted(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_impacted_text(envelope: &ImpactedEnvelope, styled: bool) -> i32 {
    let mut output = String::new();
    render_impacted_text(&mut output, envelope, styled);
    print!("{output}");
    0
}

fn render_impacted_text(output: &mut String, envelope: &ImpactedEnvelope, styled: bool) {
    let changed_header = format!("{} path(s)", envelope.changed_paths.len());
    if styled {
        writeln!(output, "{} {changed_header}", faint_label("Changed paths:"))
            .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Changed paths: {changed_header}").expect("writing to String cannot fail");
    }
    for path in &envelope.changed_paths {
        writeln!(output, "- {path}").expect("writing to String cannot fail");
    }

    let impacted_header = format!("{} object(s)", envelope.impacted.len());
    if styled {
        writeln!(output, "{} {impacted_header}", faint_label("Impacted:"))
            .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Impacted: {impacted_header}").expect("writing to String cannot fail");
    }
    if envelope.impacted.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
    }
    for record in &envelope.impacted {
        render_impacted_record(output, record, styled);
    }

    if !envelope.proof_obligations.is_empty() {
        let obligations_header = format!("{} obligation(s)", envelope.proof_obligations.len());
        if styled {
            writeln!(
                output,
                "{} {obligations_header}",
                faint_label("Proof obligations:")
            )
            .expect("writing to String cannot fail");
        } else {
            writeln!(output, "Proof obligations: {obligations_header}")
                .expect("writing to String cannot fail");
        }
        for obligation in &envelope.proof_obligations {
            if obligation.required_evidence.is_empty() {
                writeln!(output, "- {}: {}", obligation.object_id, obligation.reason)
                    .expect("writing to String cannot fail");
            } else {
                writeln!(
                    output,
                    "- {}: {} (evidence: {})",
                    obligation.object_id,
                    obligation.reason,
                    obligation.required_evidence.join(", ")
                )
                .expect("writing to String cannot fail");
            }
        }
    }
}

fn render_impacted_record(output: &mut String, record: &ImpactedRecord, styled: bool) {
    let owner = record
        .owner
        .as_ref()
        .map(|owner| format!(", owner: {owner}"))
        .unwrap_or_default();
    if styled {
        writeln!(
            output,
            "- {} ({}, {} {}{owner})",
            record.id,
            record.kind,
            cyan_key("status"),
            record.status,
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(
            output,
            "- {} ({}, {}{owner})",
            record.id, record.kind, record.status,
        )
        .expect("writing to String cannot fail");
    }
    for reason in &record.reasons {
        writeln!(output, "  via {}", reason_label(reason)).expect("writing to String cannot fail");
    }
}

fn reason_label(reason: &ImpactReason) -> String {
    let kind = match reason.kind {
        ImpactReasonKind::ImpactsPath => "impacts_path",
        ImpactReasonKind::EvidencePath => "evidence_path",
    };
    match &reason.via_source_object {
        Some(source) => format!("{kind}: {} (source: {source})", reason.matched_path),
        None => format!("{kind}: {}", reason.matched_path),
    }
}

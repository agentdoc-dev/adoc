use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::{ImpactedObject, ProofObligation, RequiredReviewer, ReviewEnvelope};
use adoc_local::{LocalContext, ReviewInput, ReviewUseCase, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{MarkdownReviewPresenter, ResolvedFormat, json as json_presentation};

use super::diff::render_diff_text;
use super::{current_dir, eprint_diagnostics, report};

pub(crate) struct ReviewCommandInput {
    pub(crate) base_ref: String,
}

pub(crate) fn review(input: ReviewCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match ReviewUseCase::new(context).run(ReviewInput {
        base_ref: input.base_ref,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let envelope = outcome.envelope;
    let exit_code = outcome.exit_code;

    match resolved {
        ResolvedFormat::Json => write_review_json(&envelope, exit_code),
        ResolvedFormat::Plain => write_review_text(&envelope, false, exit_code),
        ResolvedFormat::Styled => write_review_text(&envelope, true, exit_code),
        ResolvedFormat::Markdown => write_review_markdown(&envelope, exit_code),
    }
}

fn write_review_json(envelope: &ReviewEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_review_markdown(envelope: &ReviewEnvelope, exit_code: i32) -> i32 {
    if !envelope.diagnostics.is_empty() {
        eprint_diagnostics(&envelope.diagnostics);
    }
    MarkdownReviewPresenter::write_review(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_review_text(envelope: &ReviewEnvelope, styled: bool, exit_code: i32) -> i32 {
    if !envelope.diagnostics.is_empty() {
        eprint_diagnostics(&envelope.diagnostics);
    }
    let mut output = String::new();
    render_diff_text(&mut output, &envelope.diff, styled);
    render_impact_section(&mut output, &envelope.impact, styled);
    render_required_reviewers_section(&mut output, &envelope.required_reviewers, styled);
    render_proof_obligations_section(&mut output, &envelope.proof_obligations, styled);
    print!("{output}");
    exit_code
}

fn render_impact_section(output: &mut String, entries: &[ImpactedObject], styled: bool) {
    let label = "Impact:";
    if styled {
        writeln!(output, "{}", faint_label(label)).expect("write to String");
    } else {
        writeln!(output, "{label}").expect("write to String");
    }
    if entries.is_empty() {
        writeln!(output, "  (none)").expect("write to String");
        return;
    }
    for entry in entries {
        let id = if styled {
            cyan_key(&entry.id)
        } else {
            entry.id.clone()
        };
        let paths = entry.paths.join(", ");
        writeln!(output, "  - {id} ({paths})").expect("write to String");
    }
}

fn render_required_reviewers_section(
    output: &mut String,
    entries: &[RequiredReviewer],
    styled: bool,
) {
    let label = "Required reviewers:";
    if styled {
        writeln!(output, "{}", faint_label(label)).expect("write to String");
    } else {
        writeln!(output, "{label}").expect("write to String");
    }
    if entries.is_empty() {
        writeln!(output, "  (none)").expect("write to String");
        return;
    }
    for entry in entries {
        let owner = if styled {
            cyan_key(&entry.owner)
        } else {
            entry.owner.clone()
        };
        let ids = entry.object_ids.join(", ");
        writeln!(output, "  - {owner}: {ids}").expect("write to String");
    }
}

fn render_proof_obligations_section(
    output: &mut String,
    entries: &[ProofObligation],
    styled: bool,
) {
    let label = "Proof obligations:";
    if styled {
        writeln!(output, "{}", faint_label(label)).expect("write to String");
    } else {
        writeln!(output, "{label}").expect("write to String");
    }
    if entries.is_empty() {
        writeln!(output, "  (none)").expect("write to String");
        return;
    }
    for entry in entries {
        let id = if styled {
            cyan_key(&entry.object_id)
        } else {
            entry.object_id.clone()
        };
        let evidence = entry.required_evidence.join(", ");
        if evidence.is_empty() {
            writeln!(output, "  - {id}: {}", entry.reason).expect("write to String");
        } else {
            writeln!(output, "  - {id}: {} [evidence: {evidence}]", entry.reason)
                .expect("write to String");
        }
    }
}

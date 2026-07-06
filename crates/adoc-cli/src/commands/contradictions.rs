use std::fmt::Write as FmtWrite;
use std::io;
use std::path::PathBuf;

use adoc_core::{ContradictedClaimRecord, ContradictionRecord, ContradictionsEnvelope};
use adoc_local::{
    ContradictionsInput as LocalContradictionsInput, LocalContext, UnrestrictedPathPolicy,
};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{current_dir, eprint_diagnostics, report};

pub(crate) struct ContradictionsCommandInput {
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) all: bool,
}

pub(crate) fn contradictions(input: ContradictionsCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.contradictions(LocalContradictionsInput {
        artifact: input.artifact,
        all: input.all,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let exit_code = outcome.exit_code;
    if exit_code != 0 {
        return emit_contradictions_error(outcome.envelope, resolved, exit_code);
    }
    if resolved != ResolvedFormat::Json && !outcome.envelope.diagnostics.is_empty() {
        eprint_diagnostics(&outcome.envelope.diagnostics);
    }
    match resolved {
        ResolvedFormat::Json => write_contradictions_json(&outcome.envelope, exit_code),
        ResolvedFormat::Plain => write_contradictions_text(&outcome.envelope, false),
        ResolvedFormat::Styled => write_contradictions_text(&outcome.envelope, true),
        ResolvedFormat::Markdown => {
            unreachable!(
                "main.rs rejects markdown format for `adoc contradictions` before dispatch"
            )
        }
    }
}

fn emit_contradictions_error(
    envelope: ContradictionsEnvelope,
    resolved: ResolvedFormat,
    exit_code: i32,
) -> i32 {
    if resolved == ResolvedFormat::Json {
        return write_contradictions_json(&envelope, exit_code);
    }
    eprint_diagnostics(&envelope.diagnostics);
    exit_code
}

fn write_contradictions_json(envelope: &ContradictionsEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_contradictions_text(envelope: &ContradictionsEnvelope, styled: bool) -> i32 {
    let mut output = String::new();
    render_contradictions_text(&mut output, envelope, styled);
    print!("{output}");
    0
}

fn render_contradictions_text(
    output: &mut String,
    envelope: &ContradictionsEnvelope,
    styled: bool,
) {
    let header = format!("{} contradiction(s)", envelope.contradictions.len());
    if styled {
        writeln!(output, "{} {header}", faint_label("Contradictions:"))
            .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Contradictions: {header}").expect("writing to String cannot fail");
    }
    if envelope.contradictions.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
    }
    for record in &envelope.contradictions {
        render_contradiction(output, record, styled);
    }

    let claims_header = format!(
        "{} contradicted claim(s)",
        envelope.contradicted_claims.len()
    );
    if styled {
        writeln!(
            output,
            "{} {claims_header}",
            faint_label("Contradicted claims:")
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Contradicted claims: {claims_header}")
            .expect("writing to String cannot fail");
    }
    if envelope.contradicted_claims.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
    }
    for record in &envelope.contradicted_claims {
        render_contradicted_claim(output, record, styled);
    }
}

fn render_contradiction(output: &mut String, record: &ContradictionRecord, styled: bool) {
    let owner = record
        .owner
        .as_ref()
        .map(|owner| format!(", owner: {owner}"))
        .unwrap_or_default();
    if styled {
        writeln!(
            output,
            "- {} ({} {}, {} {}{owner}, {})",
            record.id,
            cyan_key("severity"),
            record.severity,
            cyan_key("status"),
            record.status,
            record.source_path,
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(
            output,
            "- {} ({}, {}{owner}, {})",
            record.id, record.severity, record.status, record.source_path,
        )
        .expect("writing to String cannot fail");
    }
    writeln!(output, "  claims: {}", record.claims.join(", "))
        .expect("writing to String cannot fail");
    if !record.summary.is_empty() {
        writeln!(output, "  {}", record.summary).expect("writing to String cannot fail");
    }
}

fn render_contradicted_claim(output: &mut String, record: &ContradictedClaimRecord, styled: bool) {
    let status = match (&record.authored_status, &record.effective_status) {
        (Some(authored), Some(effective)) if authored != effective => {
            format!("{authored} -> {effective}")
        }
        (Some(authored), _) => authored.clone(),
        (None, Some(effective)) => effective.clone(),
        (None, None) => String::new(),
    };
    let via = if record.contradiction_ids.is_empty() {
        "authored only".to_string()
    } else {
        format!("via {}", record.contradiction_ids.join(", "))
    };
    if styled {
        writeln!(
            output,
            "- {} ({} {status}, {via})",
            record.id,
            cyan_key("status"),
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(output, "- {} ({status}, {via})", record.id)
            .expect("writing to String cannot fail");
    }
}

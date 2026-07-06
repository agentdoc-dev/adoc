use std::fmt::Write as FmtWrite;
use std::io;
use std::path::PathBuf;

use adoc_core::{StaleCategory, StaleEnvelope, StaleRecord};
use adoc_local::{LocalContext, StaleInput as LocalStaleInput, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{current_dir, eprint_diagnostics, report};

pub(crate) struct StaleCommandInput {
    pub(crate) artifact: Option<PathBuf>,
    pub(crate) within_days: Option<u32>,
}

pub(crate) fn stale(input: StaleCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.stale(LocalStaleInput {
        artifact: input.artifact,
        within_days: input.within_days,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let exit_code = outcome.exit_code;
    if exit_code != 0 {
        return emit_stale_error(outcome.envelope, resolved, exit_code);
    }
    if resolved != ResolvedFormat::Json && !outcome.envelope.diagnostics.is_empty() {
        eprint_diagnostics(&outcome.envelope.diagnostics);
    }
    match resolved {
        ResolvedFormat::Json => write_stale_json(&outcome.envelope, exit_code),
        ResolvedFormat::Plain => write_stale_text(&outcome.envelope, false),
        ResolvedFormat::Styled => write_stale_text(&outcome.envelope, true),
        ResolvedFormat::Markdown => {
            unreachable!("main.rs rejects markdown format for `adoc stale` before dispatch")
        }
    }
}

fn emit_stale_error(envelope: StaleEnvelope, resolved: ResolvedFormat, exit_code: i32) -> i32 {
    if resolved == ResolvedFormat::Json {
        return write_stale_json(&envelope, exit_code);
    }
    eprint_diagnostics(&envelope.diagnostics);
    exit_code
}

fn write_stale_json(envelope: &StaleEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_stale_text(envelope: &StaleEnvelope, styled: bool) -> i32 {
    let mut output = String::new();
    render_stale_text(&mut output, envelope, styled);
    print!("{output}");
    0
}

fn render_stale_text(output: &mut String, envelope: &StaleEnvelope, styled: bool) {
    let header = format!(
        "{} record(s) as of {}",
        envelope.records.len(),
        envelope.evaluated_at
    );
    if styled {
        writeln!(output, "{} {header}", faint_label("Stale:"))
            .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Stale: {header}").expect("writing to String cannot fail");
    }

    if envelope.records.is_empty() {
        writeln!(output, "(none)").expect("writing to String cannot fail");
        return;
    }
    for record in &envelope.records {
        render_record(output, record, styled);
    }
}

fn render_record(output: &mut String, record: &StaleRecord, styled: bool) {
    let category = category_str(record.category);
    let status = match (&record.authored_status, &record.effective_status) {
        (Some(authored), Some(effective)) if authored != effective => {
            format!(", {authored} -> {effective}")
        }
        (Some(authored), _) => format!(", {authored}"),
        (None, Some(effective)) => format!(", {effective}"),
        (None, None) => String::new(),
    };
    let days = match record.category {
        StaleCategory::Stale | StaleCategory::ReviewOverdue => record
            .days_overdue
            .map(|days| format!(", {days} days overdue"))
            .unwrap_or_default(),
        StaleCategory::ExpiringSoon => record
            .days_remaining
            .map(|days| format!(", {days} days remaining"))
            .unwrap_or_default(),
    };
    let owner = record
        .owner
        .as_ref()
        .map(|owner| format!(", owner: {owner}"))
        .unwrap_or_default();
    if styled {
        writeln!(
            output,
            "- {} ({} {category}, {} {}{status}, {}{days}{owner}, {})",
            record.id,
            cyan_key("category"),
            cyan_key("kind"),
            record.kind,
            record.reason,
            record.source_path,
        )
        .expect("writing to String cannot fail");
    } else {
        writeln!(
            output,
            "- {} ({category}, {}{status}, {}{days}{owner}, {})",
            record.id, record.kind, record.reason, record.source_path,
        )
        .expect("writing to String cannot fail");
    }
}

fn category_str(category: StaleCategory) -> &'static str {
    match category {
        StaleCategory::Stale => "stale",
        StaleCategory::ReviewOverdue => "review_overdue",
        StaleCategory::ExpiringSoon => "expiring_soon",
    }
}

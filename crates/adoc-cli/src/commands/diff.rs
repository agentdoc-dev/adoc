use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::ObjectDiffEnvelope;
use adoc_local::{DiffInput, DiffUseCase, LocalContext, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::style::key::cyan_key;
use crate::presentation::style::kv::faint_label;
use crate::presentation::{ResolvedFormat, json as json_presentation};

use super::{current_dir, eprint_diagnostics, report};

pub(crate) struct DiffCommandInput {
    pub(crate) base_ref: String,
}

pub(crate) fn diff(input: DiffCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };
    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match DiffUseCase::new(context).run(DiffInput {
        base_ref: input.base_ref,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    let envelope = outcome.envelope;
    let exit_code = outcome.exit_code;

    match resolved {
        ResolvedFormat::Json => write_diff_json(&envelope, exit_code),
        ResolvedFormat::Plain => write_diff_text(&envelope, false, exit_code),
        ResolvedFormat::Styled => write_diff_text(&envelope, true, exit_code),
    }
}

fn write_diff_json(envelope: &ObjectDiffEnvelope, exit_code: i32) -> i32 {
    json_presentation::write_json(envelope, &mut io::stdout()).map_or_else(
        |source| report(CliError::RetrievalIo { source }),
        |()| exit_code,
    )
}

fn write_diff_text(envelope: &ObjectDiffEnvelope, styled: bool, exit_code: i32) -> i32 {
    if !envelope.diagnostics.is_empty() {
        eprint_diagnostics(&envelope.diagnostics);
    }
    let mut output = String::new();
    render_diff_text(&mut output, envelope, styled);
    print!("{output}");
    exit_code
}

fn render_diff_text(output: &mut String, envelope: &ObjectDiffEnvelope, styled: bool) {
    let summary = format!(
        "Diff: {} created, {} deleted, {} changed",
        envelope.created_count(),
        envelope.deleted_count(),
        envelope.changed_count()
    );
    if styled {
        writeln!(output, "{}", faint_label(&summary)).expect("write to String");
    } else {
        writeln!(output, "{summary}").expect("write to String");
    }

    render_id_list(output, "Created:", envelope.created_ids(), styled);
    render_id_list(output, "Deleted:", envelope.deleted_ids(), styled);
    render_id_list(output, "Changed:", envelope.changed_ids(), styled);
}

fn render_id_list<'a, I: Iterator<Item = &'a str>>(
    output: &mut String,
    label: &str,
    ids: I,
    styled: bool,
) {
    if styled {
        writeln!(output, "{}", faint_label(label)).expect("write to String");
    } else {
        writeln!(output, "{label}").expect("write to String");
    }
    let mut empty = true;
    for id in ids {
        empty = false;
        if styled {
            writeln!(output, "  - {}", cyan_key(id)).expect("write to String");
        } else {
            writeln!(output, "  - {id}").expect("write to String");
        }
    }
    if empty {
        writeln!(output, "  (none)").expect("write to String");
    }
}

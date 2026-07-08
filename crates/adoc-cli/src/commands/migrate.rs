use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use adoc_core::{MigrateDirection, MigrateReportEnvelope};
use adoc_local::{LocalContext, MigrateInput, UnrestrictedPathPolicy};

use super::{current_dir, print_diagnostics, print_summary, report, write_json_or_report};
use crate::presentation::ResolvedFormat;
use crate::presentation::style::kv::faint_label;

pub(crate) struct MigrateCommandInput {
    pub(crate) path: Option<PathBuf>,
    pub(crate) write: bool,
    pub(crate) force: bool,
    pub(crate) export: bool,
}

pub(crate) fn migrate(input: MigrateCommandInput, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.migrate(MigrateInput {
        path: input.path,
        write: input.write,
        force: input.force,
        export: input.export,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };

    match resolved {
        // One arm for success and refusal alike: an exit-1 refusal still
        // ships the full envelope — its diagnostics carry the refusal codes.
        ResolvedFormat::Json => write_json_or_report(&outcome.report, outcome.exit_code),
        ResolvedFormat::Plain => write_migrate_text(&outcome.report, outcome.exit_code, false),
        ResolvedFormat::Styled => write_migrate_text(&outcome.report, outcome.exit_code, true),
        ResolvedFormat::Markdown => {
            unreachable!("main.rs rejects markdown format for `adoc migrate` before dispatch")
        }
    }
}

fn write_migrate_text(report: &MigrateReportEnvelope, exit_code: i32, styled: bool) -> i32 {
    let (done, would) = match report.direction {
        MigrateDirection::Import => ("migrated", "would migrate"),
        MigrateDirection::Export => ("exported", "would export"),
    };
    for file in &report.files {
        let verb = if file.written { done } else { would };
        println!(
            "{verb} {} -> {}",
            file.source.display(),
            file.target.display()
        );
    }
    print_diagnostics(&report.diagnostics);
    print!("{}", render_report_block(report, styled));
    print_summary(&report.diagnostics);
    exit_code
}

/// The §28.3 human summary: the seven counts, the numbered
/// suggested-next-steps list, and the §28.4 typed-block suggestions,
/// mirroring the JSON envelope field for field. The JSON wire names stay
/// fixed across directions (ADR-0043 §4); these human labels follow the
/// direction for the same reason `suggested_next_steps` does — on export
/// the quarantine-named counts tally fence unwraps, and the labels would
/// lie otherwise.
fn render_report_block(report: &MigrateReportEnvelope, styled: bool) -> String {
    let (header, files_label, raw_html_label, extensions_label) = match report.direction {
        MigrateDirection::Import => (
            "Migration report",
            "Files imported",
            "Raw HTML blocks quarantined",
            "Unrecognized extensions",
        ),
        MigrateDirection::Export => (
            "Export report",
            "Files exported",
            "Raw HTML fences unwrapped",
            "Markdown fences unwrapped",
        ),
    };
    let mut output = String::new();
    if styled {
        writeln!(output, "{}", faint_label(header)).expect("writing to String cannot fail");
    } else {
        writeln!(output, "{header}").expect("writing to String cannot fail");
    }
    let counts = &report.counts;
    for (label, value) in [
        (files_label, counts.files_imported),
        ("Pages created", counts.pages_created),
        ("Prose blocks", counts.prose_blocks),
        (raw_html_label, counts.raw_html_quarantined),
        ("Broken links", counts.broken_links),
        (extensions_label, counts.unrecognized_extensions),
        ("Suggested typed blocks", counts.suggested_typed_blocks),
    ] {
        writeln!(output, "  {label}: {value}").expect("writing to String cannot fail");
    }
    if !report.suggested_next_steps.is_empty() {
        if styled {
            writeln!(output, "{}", faint_label("Suggested next steps:"))
                .expect("writing to String cannot fail");
        } else {
            writeln!(output, "Suggested next steps:").expect("writing to String cannot fail");
        }
        for (index, step) in report.suggested_next_steps.iter().enumerate() {
            writeln!(output, "  {}. {step}", index + 1).expect("writing to String cannot fail");
        }
    }
    if !report.suggestions.is_empty() {
        if styled {
            writeln!(output, "{}", faint_label("Typed-block suggestions:"))
                .expect("writing to String cannot fail");
        } else {
            writeln!(output, "Typed-block suggestions:").expect("writing to String cannot fail");
        }
        for suggestion in &report.suggestions {
            writeln!(
                output,
                "  {}:{}:{} {} ({}): {}",
                suggestion.span.file.display(),
                suggestion.span.start.line,
                suggestion.span.start.column,
                suggestion.suggested_kind,
                suggestion.matched_rule,
                suggestion.excerpt
            )
            .expect("writing to String cannot fail");
        }
    }
    output
}

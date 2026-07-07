use std::fmt::Write as FmtWrite;
use std::path::PathBuf;

use adoc_core::MigrateReportEnvelope;
use adoc_local::{LocalContext, MigrateInput, UnrestrictedPathPolicy};

use super::{current_dir, print_diagnostics, print_summary, report, write_json_or_report};
use crate::presentation::ResolvedFormat;
use crate::presentation::style::kv::faint_label;

pub(crate) struct MigrateCommandInput {
    pub(crate) path: Option<PathBuf>,
    pub(crate) write: bool,
    pub(crate) force: bool,
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
    for file in &report.files {
        let verb = if file.written {
            "migrated"
        } else {
            "would migrate"
        };
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

/// The §28.3 human summary: the seven counts plus the numbered
/// suggested-next-steps list, mirroring the JSON envelope field for field.
fn render_report_block(report: &MigrateReportEnvelope, styled: bool) -> String {
    let mut output = String::new();
    if styled {
        writeln!(output, "{}", faint_label("Migration report"))
            .expect("writing to String cannot fail");
    } else {
        writeln!(output, "Migration report").expect("writing to String cannot fail");
    }
    let counts = &report.counts;
    for (label, value) in [
        ("Files imported", counts.files_imported),
        ("Pages created", counts.pages_created),
        ("Prose blocks", counts.prose_blocks),
        ("Raw HTML blocks quarantined", counts.raw_html_quarantined),
        ("Broken links", counts.broken_links),
        ("Unrecognized extensions", counts.unrecognized_extensions),
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
    output
}

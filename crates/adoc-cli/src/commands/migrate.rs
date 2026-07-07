use std::path::PathBuf;

use adoc_local::{LocalContext, MigrateInput, UnrestrictedPathPolicy};

use super::{current_dir, print_diagnostics, print_summary, report};
use crate::presentation::ResolvedFormat;

pub(crate) struct MigrateCommandInput {
    pub(crate) path: Option<PathBuf>,
    pub(crate) write: bool,
    pub(crate) force: bool,
}

pub(crate) fn migrate(input: MigrateCommandInput, resolved: ResolvedFormat) -> i32 {
    // A silent plain-text fallback under `--format json` would be exactly the
    // silent-fallback failure the diagnostics rules forbid; the versioned
    // report envelope ships in V8.1.2 and this guard is deleted with it.
    if resolved == ResolvedFormat::Json {
        eprintln!(
            "error[cli.format] --format json for `adoc migrate` ships with the \
             adoc.migrate.report.v0 envelope (V8.1.2); use the plain output for now"
        );
        return 2;
    }

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

    for file in &outcome.report.files {
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
    print_diagnostics(&outcome.report.diagnostics);
    print_summary(&outcome.report.diagnostics);

    outcome.exit_code
}

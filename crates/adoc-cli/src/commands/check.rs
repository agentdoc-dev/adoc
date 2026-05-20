use std::path::PathBuf;

use adoc_local::{CheckInput, CheckUseCase, LocalContext, UnrestrictedPathPolicy};

use super::{current_dir, print_diagnostics, print_summary, report};

pub(crate) fn check(path: Option<PathBuf>) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match CheckUseCase::new(context).run(CheckInput { path }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    print_diagnostics(&outcome.diagnostics);
    print_summary(&outcome.diagnostics);

    outcome.exit_code
}

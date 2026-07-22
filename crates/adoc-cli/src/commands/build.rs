use std::path::PathBuf;

use adoc_local::{BuildInput, LocalContext, UnrestrictedPathPolicy};

use super::{current_dir, print_diagnostics, print_summary, report};

pub(crate) fn build(
    path: Option<PathBuf>,
    out: Option<PathBuf>,
    no_embeddings: bool,
    as_of: Option<chrono::NaiveDate>,
) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.build(BuildInput {
        path,
        out,
        no_embeddings,
        as_of,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };

    print_diagnostics(&outcome.diagnostics);
    print_summary(&outcome.diagnostics);
    outcome.exit_code
}

use std::io;
use std::path::PathBuf;

use adoc_local::{CheckInput, LocalContext, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::{MarkdownReviewPresenter, ResolvedFormat};

use super::{current_dir, eprint_diagnostics, print_diagnostics, print_summary, report};

pub(crate) fn check(path: Option<PathBuf>, resolved: ResolvedFormat) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.check(CheckInput { path }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    // V8.3.1: markdown is the PR-comment surface — stdout carries the body a
    // bot pastes verbatim; the fix-oriented diagnostics stay on stderr for
    // terminal users. Exit code is identical across formats (§24.3 keeps
    // advisory-vs-strict a workflow decision, not a format one).
    if resolved == ResolvedFormat::Markdown {
        eprint_diagnostics(&outcome.diagnostics);
        return MarkdownReviewPresenter::write_check(&outcome.diagnostics, &mut io::stdout())
            .map_or_else(
                |source| report(CliError::StdoutIo { source }),
                |()| outcome.exit_code,
            );
    }
    print_diagnostics(&outcome.diagnostics);
    print_summary(&outcome.diagnostics);

    outcome.exit_code
}

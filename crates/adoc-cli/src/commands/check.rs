use std::io;
use std::path::PathBuf;

use adoc_local::{CheckInput, CheckReceiptInput, LocalContext, UnrestrictedPathPolicy};

use crate::error::CliError;
use crate::presentation::{CheckStyle, MarkdownReviewPresenter, ResolvedFormat};

use super::{current_dir, eprint_diagnostics, print_diagnostics, print_summary, report};

pub(crate) fn check(
    path: Option<PathBuf>,
    style: CheckStyle,
    as_of: Option<chrono::NaiveDate>,
    resolved: ResolvedFormat,
) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start.clone(), UnrestrictedPathPolicy);
    let outcome = match context.check(CheckInput { path, as_of }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    // V8.3.1: markdown is the PR-comment surface — stdout carries the body a
    // bot pastes verbatim; the fix-oriented diagnostics stay on stderr for
    // terminal users. Exit code is identical across formats (§24.3 keeps
    // advisory-vs-strict a workflow decision, not a format one).
    if resolved == ResolvedFormat::Markdown {
        eprint_diagnostics(&outcome.diagnostics);
        return MarkdownReviewPresenter::write_check(
            &outcome.diagnostics,
            &config_start,
            style,
            &mut io::stdout(),
        )
        .map_or_else(
            |source| report(CliError::StdoutIo { source }),
            |()| outcome.exit_code,
        );
    }
    print_diagnostics(&outcome.diagnostics);
    print_summary(&outcome.diagnostics);

    outcome.exit_code
}

/// E1.7: receipt-mode check — the same validation, plus a digest-bound
/// `adoc.validation_receipt.v0` written byte-for-byte in its canonical
/// serialization. The runtime binary digest is the harness's attested
/// input (`scripts/validation-runtime/run.sh` verifies its pin first);
/// this binary contributes its own release version.
pub(crate) fn check_receipt(
    path: Option<PathBuf>,
    as_of: chrono::NaiveDate,
    receipt_path: PathBuf,
    runtime_binary_digest: String,
) -> i32 {
    let config_start = match current_dir() {
        Ok(path) => path,
        Err(error) => return report(error),
    };

    let context = LocalContext::new(config_start, UnrestrictedPathPolicy);
    let outcome = match context.check_receipt(CheckReceiptInput {
        path,
        as_of,
        runtime_version: env!("CARGO_PKG_VERSION").to_string(),
        runtime_binary_digest,
    }) {
        Ok(outcome) => outcome,
        Err(error) => return report(error.into()),
    };
    if let Err(source) = std::fs::write(&receipt_path, outcome.receipt.to_canonical_json()) {
        return report(
            adoc_local::LocalError::WriteFailed {
                path: receipt_path,
                source,
            }
            .into(),
        );
    }
    print_diagnostics(&outcome.diagnostics);
    print_summary(&outcome.diagnostics);

    outcome.exit_code
}

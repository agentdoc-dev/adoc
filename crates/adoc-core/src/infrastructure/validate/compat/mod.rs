//! Compatibility-mode validation pass for Markdown sources.
//!
//! Runs in parallel to the strict-mode pipeline in
//! [`crate::infrastructure::validate`]. Each rule implements
//! [`CompatRule`] whose sink type is [`CompatDiagnostic`] — a newtype around
//! `Diagnostic` whose only constructor is `warning(...)`. ADR-0023's
//! warning-only invariant is therefore enforced at compile time: a future
//! commit that tries to make a compat rule emit `Severity::Error` is a type
//! error, not a code-review catch.
//!
//! [`validate_compat_source_page`] runs every rule, unwraps each
//! `CompatDiagnostic` back to a plain `Diagnostic` at this seam, and returns
//! the unified shape the orchestrator's pipeline expects.

mod raw_html_quarantine;
mod unknown_extension;
mod unsafe_image_src_dropped;
mod unsafe_link_dropped;

use raw_html_quarantine::RawHtmlQuarantine;
use unknown_extension::UnknownExtension;
use unsafe_image_src_dropped::UnsafeImageSrcDropped;
use unsafe_link_dropped::UnsafeLinkDropped;

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::{CompatDiagnostic, Diagnostic};
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;

const COMPAT_SOURCE_PAGE_RULES: &[&dyn CompatRule] = &[
    &RawHtmlQuarantine,
    &UnsafeLinkDropped,
    &UnsafeImageSrcDropped,
    &UnknownExtension,
];

/// Run every compat source-page rule against `page`. The runner mirrors
/// [`crate::infrastructure::validate::validate_source_page`] for the strict
/// pipeline so the orchestrator can pick one or the other by `source.mode()`.
/// Diagnostics are unwrapped from [`CompatDiagnostic`] here so callers see
/// the same `Vec<Diagnostic>` shape they get from the strict pipeline.
pub(crate) fn validate_compat_source_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    let mut compat_diagnostics: Vec<CompatDiagnostic> = Vec::new();
    for rule in COMPAT_SOURCE_PAGE_RULES {
        rule.check(page, source, &mut compat_diagnostics);
    }
    compat_diagnostics
        .into_iter()
        .map(CompatDiagnostic::into_diagnostic)
        .collect()
}

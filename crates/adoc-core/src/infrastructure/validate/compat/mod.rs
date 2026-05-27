//! Compatibility-mode validation pass for Markdown sources.
//!
//! Runs in parallel to the strict-mode pipeline in
//! [`crate::infrastructure::validate`]. Each rule emits
//! [`crate::domain::diagnostic::Severity::Warning`] only — Markdown ingestion
//! must never block `adoc check` or `adoc build`. Sibling strict-mode `.adoc`
//! files in the same project still fail the build on their own errors per
//! V0 behavior.

mod raw_html_quarantine;
mod unknown_extension;
mod unsafe_image_src_dropped;
mod unsafe_link_dropped;

use raw_html_quarantine::RawHtmlQuarantine;
use unknown_extension::UnknownExtension;
use unsafe_image_src_dropped::UnsafeImageSrcDropped;
use unsafe_link_dropped::UnsafeLinkDropped;

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

const COMPAT_SOURCE_PAGE_RULES: &[&dyn ValidationRule] = &[
    &RawHtmlQuarantine,
    &UnsafeLinkDropped,
    &UnsafeImageSrcDropped,
    &UnknownExtension,
];

/// Run every compat source-page rule against `page`. The runner mirrors
/// [`crate::infrastructure::validate::validate_source_page`] for the strict
/// pipeline so the composition root can pick one or the other by source mode.
pub(crate) fn validate_compat_source_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in COMPAT_SOURCE_PAGE_RULES {
        rule.check(page, source, &mut diagnostics);
    }
    diagnostics
}

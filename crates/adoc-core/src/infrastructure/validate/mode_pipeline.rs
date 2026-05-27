//! Per-mode validation pipeline as data.
//!
//! ADR-0022 makes the file extension the only mode signal; ADR-0007 says
//! "rule registries are data, not code". This module joins the two: each
//! [`SourceMode`] maps to a [`ModePipeline`] row that names the parser,
//! source-page validator, and resolved-page policy for that mode. The
//! orchestrator in `application/compile.rs` iterates pages and calls into
//! the pipeline returned by [`pipeline_for`] rather than dispatching via
//! `match mode { Strict => …, Compat => … }`.
//!
//! Adding a future mode (a third extension, a lenient `.adoc` variant) is a
//! new constant row plus one arm in [`pipeline_for`] — no orchestrator edit.

use chrono::NaiveDate;

use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;
use crate::domain::source::{SourceFile, SourceMode};
use crate::infrastructure::parser::{parse_markdown_page, parse_page};
use crate::infrastructure::validate::compat::validate_compat_source_page;
use crate::infrastructure::validate::{validate_resolved_page, validate_source_page};

/// Bundle of per-mode validation entry points selected by [`pipeline_for`].
pub(crate) struct ModePipeline {
    /// Parse the raw source into a `PageAst`. ADR-0022: Strict for `.adoc`,
    /// Compat for `.md`.
    pub(crate) parse: fn(&SourceFile) -> (PageAst, Vec<Diagnostic>),
    /// Source-phase validation pass — `validate_source_page` for Strict and
    /// `validate_compat_source_page` for Compat. Each returns `Vec<Diagnostic>`
    /// so the orchestrator sees a single uniform diagnostic stream.
    pub(crate) validate_source_page: fn(&PageAst, &SourceFile) -> Vec<Diagnostic>,
    /// Resolved-phase validation policy.
    ///
    /// Compat mode produces no Knowledge Objects (ADR-0023), so the resolved
    /// phase has nothing to do — encoded as [`ResolvedPagePolicy::Empty`]
    /// rather than an `if mode == Strict` in the orchestrator.
    pub(crate) validate_resolved_page: ResolvedPagePolicy,
}

/// What the resolved-page phase does for a mode.
pub(crate) enum ResolvedPagePolicy {
    /// No-op — Compat has no Knowledge Objects to revisit.
    Empty,
    /// Run this resolved-page validation function.
    Validate(fn(&PageAst, &SourceFile, NaiveDate) -> Vec<Diagnostic>),
}

impl ResolvedPagePolicy {
    pub(crate) fn run(
        &self,
        page: &PageAst,
        source: &SourceFile,
        today: NaiveDate,
    ) -> Vec<Diagnostic> {
        match self {
            ResolvedPagePolicy::Empty => Vec::new(),
            ResolvedPagePolicy::Validate(check) => check(page, source, today),
        }
    }
}

const STRICT_PIPELINE: ModePipeline = ModePipeline {
    parse: parse_page,
    validate_source_page,
    validate_resolved_page: ResolvedPagePolicy::Validate(validate_resolved_page),
};

const COMPAT_PIPELINE: ModePipeline = ModePipeline {
    parse: parse_markdown_page,
    validate_source_page: validate_compat_source_page,
    validate_resolved_page: ResolvedPagePolicy::Empty,
};

/// Return the [`ModePipeline`] for `mode`. The data table is the entire
/// per-mode dispatch policy; the orchestrator only iterates and calls.
pub(crate) fn pipeline_for(mode: SourceMode) -> &'static ModePipeline {
    match mode {
        SourceMode::Strict => &STRICT_PIPELINE,
        SourceMode::Compat => &COMPAT_PIPELINE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// ADR-0023 invariant expressed as data: Compat has no resolved-page
    /// validation. Encoding this in the pipeline table rather than as an
    /// `if mode == Strict` in the orchestrator means a future commit cannot
    /// silently add resolved-page work to Compat without editing this row.
    #[test]
    fn compat_pipeline_skips_resolved_page_phase() {
        assert!(matches!(
            pipeline_for(SourceMode::Compat).validate_resolved_page,
            ResolvedPagePolicy::Empty
        ));
    }

    #[test]
    fn strict_pipeline_runs_resolved_page_validation() {
        assert!(matches!(
            pipeline_for(SourceMode::Strict).validate_resolved_page,
            ResolvedPagePolicy::Validate(_)
        ));
    }
}

use crate::domain::ast::{PageAst, WorkspaceAst};
use crate::domain::diagnostic::{CompatDiagnostic, Diagnostic};
use crate::domain::source::SourceFile;

pub(crate) trait ValidationRule {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>);
}

/// Validation rule that runs under V4 Compatibility Mode (`.md` sources).
///
/// The sink type is [`CompatDiagnostic`] rather than [`Diagnostic`] so the
/// warning-only invariant from ADR-0023 is enforced by the type system:
/// `CompatDiagnostic` has no `error`/`info` constructor, so a future commit
/// that tries to raise a compat code to `Severity::Error` is a type error.
/// The registry boundary in `infrastructure/validate/compat/mod.rs` unwraps
/// every diagnostic via [`CompatDiagnostic::into_diagnostic`] once, after
/// the rules have run.
pub(crate) trait CompatRule {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<CompatDiagnostic>);
}

/// Validation rule that operates on the whole `WorkspaceAst` aggregate rather
/// than a single page — for invariants that require cross-page context (page
/// ID uniqueness, link-target resolution, hierarchy checks). Mirrors
/// [`ValidationRule`] so adding a workspace-level rule is a new adapter, not
/// a branch inside the orchestrator.
pub(crate) trait WorkspaceRule {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>);
}

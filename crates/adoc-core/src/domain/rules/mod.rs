use crate::domain::ast::{PageAst, WorkspaceAst};
use crate::domain::diagnostic::Diagnostic;
use crate::domain::source::SourceFile;

pub(crate) trait ValidationRule {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>);
}

/// Validation rule that operates on the whole `WorkspaceAst` aggregate rather
/// than a single page — for invariants that require cross-page context (page
/// ID uniqueness, link-target resolution, hierarchy checks). Mirrors
/// [`ValidationRule`] so adding a workspace-level rule is a new adapter, not
/// a branch inside the orchestrator.
pub(crate) trait WorkspaceRule {
    fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>);
}

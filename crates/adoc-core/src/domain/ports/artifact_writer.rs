use crate::domain::ast::PageAst;
use crate::domain::diagnostic::Diagnostic;

/// Output port for compiler artifacts that aren't a single rendered string.
///
/// Each adapter declares its own `Output` shape — agent JSON today, and any
/// future structured artifact (RDF graph, Markdown index) — so adding a new
/// format is a new adapter rather than another edit to `compile.rs` or this
/// trait. Static dispatch is preserved per ADR-0006: callers pick the
/// concrete adapter at the call site.
pub(crate) trait ArtifactWriter {
    type Output;
    fn build(&self, pages: &[PageAst], diagnostics: &[Diagnostic]) -> Self::Output;
}

use crate::domain::diagnostic::Diagnostic;

/// Output port for compiler artifacts that aren't a single rendered string.
///
/// Each adapter declares its own `Output` shape — graph JSON today, and any
/// future structured artifact (Markdown index, binary sidecar) — so adding a new
/// format is a new adapter rather than another edit to `compile.rs` or this
/// trait. Static dispatch is preserved per ADR-0006: callers pick the
/// concrete adapter at the call site.
pub(crate) trait ArtifactWriter<Input: ?Sized> {
    type Output;
    fn build(&self, input: &Input, diagnostics: &[Diagnostic]) -> Self::Output;
}

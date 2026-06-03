use crate::domain::diagnostic::Diagnostic;

/// Output port for compiler artifacts that aren't a single rendered string.
///
/// Each adapter declares its own `Output` shape — graph JSON today, and any
/// future structured artifact (Markdown index, binary sidecar) — so adding a new
/// format is a new adapter rather than another edit to `compile.rs` or this
/// trait. Static dispatch is preserved per ADR-0006: callers pick the
/// concrete adapter at the call site.
///
/// Note: the main compile path calls `GraphJsonArtifact::build_for_date`
/// directly (V5.10+) to thread the `today` date; `build` remains for tests and
/// for structural completeness of the port abstraction.
#[allow(dead_code)]
pub(crate) trait ArtifactWriter<Input: ?Sized> {
    type Output;
    fn build(&self, input: &Input, diagnostics: &[Diagnostic]) -> Self::Output;
}

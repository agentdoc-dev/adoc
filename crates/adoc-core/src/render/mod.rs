pub(crate) mod html;

use crate::ast::PageAst;

/// Output port for compiled pages. compile_with_provider drives the renderer
/// through this trait so that adding a new format (e.g. Markdown export, plain
/// text) is a new adapter rather than an edit to the orchestrator.
pub(crate) trait Renderer {
    fn render(&self, pages: &[PageAst]) -> String;
}

pub(crate) use html::HtmlRenderer;

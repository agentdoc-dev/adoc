//! Shared AST walker for URL-bearing inline segments.
//!
//! Three validation rules — `UnsafeLinkForbidden` (strict), `UnsafeLinkDropped`
//! (compat), `UnsafeImageSrcDropped` (compat) — walk the same parse tree
//! looking for `InlineSegment::Link` and `InlineSegment::Image` URLs. They
//! used to duplicate the recursion three times; this module centralises it.
//!
//! Rules implement [`UrlVisitor`] and pick which segment kinds they care
//! about; the default implementations are no-ops. The walker descends through
//! every block kind a parser may produce (headings, paragraphs, lists, GFM
//! tables, footnote definitions, emphasis/strong/strikethrough wrappers).
//! Knowledge Object bodies and code blocks are skipped — body inlines are
//! resolved-phase concerns and live on [`KnowledgeObject`] aggregates, walked
//! by [`walk_inlines`] directly from those callers.
//!
//! [`KnowledgeObject`]: crate::domain::knowledge_object::KnowledgeObject

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::SourceSpan;
use crate::domain::inline::InlineSegment;

/// Receives a callback for every `Link` and `Image` inline encountered while
/// walking a page. Rules override only the methods they need.
pub(crate) trait UrlVisitor {
    fn on_link(&mut self, _text: &[InlineSegment], _url: &str, _span: &SourceSpan) {}
    fn on_image(&mut self, _alt: &[InlineSegment], _url: &str, _span: &SourceSpan) {}
}

/// Walk every block in `page`, invoking the visitor on each URL-bearing
/// inline segment.
pub(crate) fn walk_page<V: UrlVisitor>(page: &PageAst, visitor: &mut V) {
    for block in &page.blocks {
        walk_block(block, visitor);
    }
}

/// Walk a slice of inline segments directly. Used by resolved-phase rules
/// that already hold body inlines from a Knowledge Object aggregate.
pub(crate) fn walk_inlines<V: UrlVisitor>(inlines: &[InlineSegment], visitor: &mut V) {
    for segment in inlines {
        match segment {
            InlineSegment::Link { text, url, span } => {
                visitor.on_link(text, url, span);
                walk_inlines(text, visitor);
            }
            InlineSegment::Image { alt, url, span } => {
                visitor.on_image(alt, url, span);
                walk_inlines(alt, visitor);
            }
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => walk_inlines(inner, visitor),
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::QuarantinedHtml { .. }
            | InlineSegment::FootnoteReference { .. }
            | InlineSegment::UnknownExtension { .. } => {}
        }
    }
}

fn walk_block<V: UrlVisitor>(block: &BlockAst, visitor: &mut V) {
    match block {
        BlockAst::Heading(heading) => walk_inlines(&heading.inlines, visitor),
        BlockAst::Paragraph(paragraph) => walk_inlines(&paragraph.inlines, visitor),
        BlockAst::List(list) => {
            for item in &list.items {
                walk_inlines(&item.inlines, visitor);
            }
        }
        BlockAst::Table(table) => {
            for cell in &table.header {
                walk_inlines(&cell.inlines, visitor);
            }
            for row in &table.rows {
                for cell in row {
                    walk_inlines(&cell.inlines, visitor);
                }
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                walk_block(child, visitor);
            }
        }
        BlockAst::CodeBlock(_)
        | BlockAst::QuarantinedHtml(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_)
        | BlockAst::UnknownExtension(_)
        | BlockAst::ThematicBreak(_) => {}
    }
}

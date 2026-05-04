use std::path::PathBuf;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::PageId;
use crate::domain::inline::InlineSegment;

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct WorkspaceAst {
    pub(crate) pages: Vec<PageAst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageAst {
    pub(crate) id: PageId,
    pub(crate) title: Option<String>,
    pub(crate) source_path: PathBuf,
    pub(crate) blocks: Vec<BlockAst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum BlockAst {
    Heading(HeadingAst),
    Paragraph(ParagraphAst),
    List(ListAst),
    CodeBlock(CodeBlockAst),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct HeadingAst {
    pub(crate) level: u8,
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParagraphAst {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListAst {
    pub(crate) kind: ListKind,
    pub(crate) items: Vec<ListItem>,
    pub(crate) span: SourceSpan,
}

/// One item in a list — its inline content plus the source span of the line
/// it occupies. Reified per ADR-0007/-0006 so each item carries its own
/// position; future per-item rules walk `item.span` directly instead of
/// re-deriving offsets from the enclosing list.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListItem {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ListKind {
    Ordered,
    Unordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CodeBlockAst {
    pub(crate) language: Option<String>,
    pub(crate) code: String,
    pub(crate) span: SourceSpan,
}

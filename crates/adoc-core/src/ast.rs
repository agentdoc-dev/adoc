use std::path::PathBuf;

use crate::diagnostic::SourceSpan;
use crate::identity::PageId;
use crate::inline::InlineSegment;

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
    pub(crate) items: Vec<Vec<InlineSegment>>,
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

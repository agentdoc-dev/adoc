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
pub enum BlockAst {
    Heading(HeadingAst),
    Paragraph(ParagraphAst),
    List(ListAst),
    CodeBlock(CodeBlockAst),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HeadingAst {
    pub level: u8,
    pub inlines: Vec<InlineSegment>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphAst {
    pub inlines: Vec<InlineSegment>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAst {
    pub kind: ListKind,
    pub items: Vec<Vec<InlineSegment>>,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ListKind {
    Ordered,
    Unordered,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CodeBlockAst {
    pub language: Option<String>,
    pub code: String,
    pub span: SourceSpan,
}

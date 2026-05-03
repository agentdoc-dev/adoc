use std::path::PathBuf;

use crate::diagnostic::SourceSpan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceAst {
    pub pages: Vec<PageAst>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PageAst {
    pub id: String,
    pub title: Option<String>,
    pub source_path: PathBuf,
    pub blocks: Vec<BlockAst>,
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
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParagraphAst {
    pub text: String,
    pub span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListAst {
    pub kind: ListKind,
    pub items: Vec<String>,
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

use std::collections::BTreeMap;
use std::path::PathBuf;

use crate::domain::diagnostic::SourceSpan;
use crate::domain::identity::PageId;
use crate::domain::inline::InlineSegment;
use crate::domain::knowledge_object::KnowledgeObject;

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
    /// Resolved typed block. Produced by `resolve_knowledge_objects` from a
    /// `KnowledgeObjectPending`. Boxed so this large variant doesn't bloat
    /// the enum's stack footprint for the common prose blocks above.
    KnowledgeObject(Box<KnowledgeObject>),
    /// Transient parser output: a typed Knowledge Object block that has been
    /// read but not yet validated into an aggregate. The resolver stage
    /// replaces every Pending with either `KnowledgeObject(...)` (success) or
    /// drops it after emitting `schema.*`/`id.invalid` diagnostics. By the
    /// time the renderer or artifact emitter sees the AST, no Pending exists.
    KnowledgeObjectPending(Box<ParsedTypedBlock>),
    /// Block-level raw HTML found in Markdown source (V4 Compatibility Mode
    /// only). Never produced by the `.adoc` parser. The renderer wraps the
    /// stored source text in `<pre class="quarantined-html">` with HTML
    /// escaping; the graph emitter treats it as a prose block whose content
    /// is the original source text. The compat validator pipeline emits a
    /// `compat.raw_html_quarantined` warning per occurrence.
    QuarantinedHtml(QuarantinedHtmlAst),
    /// A Markdown thematic break (`---`, `***`, or `___` on its own line) from
    /// V4 Compatibility Mode source. Never produced by the `.adoc` parser.
    /// The renderer emits `<hr />`; the graph emitter projects it as a prose
    /// block carrying the original source text. No `compat.raw_html_quarantined`
    /// warning is emitted — a thematic break is valid Markdown, not raw HTML.
    ThematicBreak(ThematicBreakAst),
    /// GFM table from Markdown source (V4 Compatibility Mode only). The
    /// renderer walks `header`, `rows`, and `alignments` to emit a
    /// `<table>`; the graph emitter projects this to a single prose block
    /// whose `source_text` is the original Markdown table.
    Table(TableAst),
    /// GFM footnote definition from Markdown source (V4 Compatibility Mode
    /// only). The renderer emits an `<aside>` block keyed by `label`; the
    /// graph emitter projects this to a single prose block.
    FootnoteDefinition(FootnoteDefinitionAst),
    /// Block-level Markdown construct outside the V4 supported set (MDX
    /// component, Pandoc directive, math fence, custom attribute block).
    /// The renderer emits the original `source_text` inside an escaped
    /// `<code>` block; the compat validator emits one
    /// `compat.unknown_extension` warning per occurrence.
    UnknownExtension(UnknownExtensionAst),
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
///
/// `task_state` is populated only for GFM task list items (V4 Compatibility
/// Mode): `Some(true)` for `- [x]`, `Some(false)` for `- [ ]`, `None` for
/// plain list items.
///
/// `content` holds block-level children of a *loose* list item (a continuation
/// paragraph separated by a blank line, or an indented sub-list). Tight list
/// items leave this empty. The Markdown parser populates this field; the
/// `.adoc` parser always leaves it as `Vec::new()` since the `.adoc` grammar
/// does not support loose-list or nested-list syntax.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ListItem {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
    pub(crate) task_state: Option<bool>,
    pub(crate) content: Vec<BlockAst>,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct QuarantinedHtmlAst {
    pub(crate) source_text: String,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ThematicBreakAst {
    pub(crate) source_text: String,
    pub(crate) span: SourceSpan,
}

/// Column alignment for a GFM table column, derived from the alignment row
/// (`:---`, `:---:`, `---:`, `---`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum ColumnAlignment {
    Default,
    Left,
    Center,
    Right,
}

/// One cell in a GFM table. Cells in the header row carry the column
/// header inlines; cells in the body rows carry data inlines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableCell {
    pub(crate) inlines: Vec<InlineSegment>,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableAst {
    pub(crate) header: Vec<TableCell>,
    pub(crate) rows: Vec<Vec<TableCell>>,
    pub(crate) alignments: Vec<ColumnAlignment>,
    pub(crate) source_text: String,
    pub(crate) span: SourceSpan,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct FootnoteDefinitionAst {
    pub(crate) label: String,
    pub(crate) content: Vec<BlockAst>,
    pub(crate) source_text: String,
    pub(crate) span: SourceSpan,
}

/// Why a Markdown construct landed in `UnknownExtension`. Drives the
/// diagnostic message and lets future tooling distinguish the categories.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum UnknownExtensionKind {
    /// `<MyComponent prop="x" />` outside a code block. Distinguished from
    /// raw HTML by a PascalCase tag name (JSX/MDX convention).
    MdxComponent,
    /// `:::warning ... :::` Pandoc/extension directive at line start.
    PandocDirective,
    /// `{.class}` / `{#id}` custom attribute block.
    AttributeBlock,
    /// `$...$` inline math or `$$...$$` display math fence.
    MathFence,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct UnknownExtensionAst {
    pub(crate) source_text: String,
    pub(crate) span: SourceSpan,
    pub(crate) kind: UnknownExtensionKind,
}

/// Parser-produced shell of a typed block before validation. Carries the raw
/// text as parsed plus a record of duplicate field keys the parser observed,
/// so the validator can emit `schema.duplicate_field` diagnostics for
/// supported kinds. Lives in `domain` because `BlockAst` references it;
/// transient — never reaches the renderer or artifact emitter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct ParsedTypedBlock {
    pub(crate) kind_word: String,
    pub(crate) kind_word_span: SourceSpan,
    pub(crate) id_text: String,
    pub(crate) raw_fields: BTreeMap<String, String>,
    pub(crate) raw_field_spans: BTreeMap<String, SourceSpan>,
    pub(crate) duplicate_keys: Vec<String>,
    pub(crate) body_text: String,
    pub(crate) body_inlines: Vec<InlineSegment>,
    pub(crate) body_spans: Vec<SourceSpan>,
    pub(crate) content_spans: Vec<SourceSpan>,
    pub(crate) span: SourceSpan,
}

#[cfg(test)]
impl ParsedTypedBlock {
    pub(crate) fn test_body_inlines_from_text(text: &str) -> Vec<InlineSegment> {
        let mut inlines = Vec::new();
        for (index, line) in text.split('\n').enumerate() {
            if index > 0 {
                inlines.push(InlineSegment::Text("\n".to_string()));
            }
            if !line.is_empty() {
                inlines.push(InlineSegment::Text(line.to_string()));
            }
        }
        inlines
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;
    use std::path::PathBuf;

    use super::*;
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::knowledge_object::{
        KnowledgeObject,
        claim::{Claim, Evidence, Owner, Verification, VerifiedAt},
    };
    use crate::domain::values::NonEmpty;

    fn span() -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.adoc"),
            start: SourcePosition {
                line: 1,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line: 1,
                column: 8,
                offset: 7,
            },
        }
    }

    #[test]
    fn block_ast_supports_knowledge_object_and_pending_variants() {
        let parsed = ParsedTypedBlock {
            kind_word: "claim".to_string(),
            kind_word_span: span(),
            id_text: "billing.credits".to_string(),
            raw_fields: BTreeMap::new(),
            raw_field_spans: BTreeMap::new(),
            duplicate_keys: Vec::new(),
            body_text: "x".to_string(),
            body_inlines: ParsedTypedBlock::test_body_inlines_from_text("x"),
            body_spans: Vec::new(),
            content_spans: Vec::new(),
            span: span(),
        };
        let pending_block = BlockAst::KnowledgeObjectPending(Box::new(parsed));
        assert_eq!(pending_block, pending_block.clone());

        let claim = Claim::try_new(
            "billing.credits",
            Some("verified"),
            "x",
            BTreeMap::new(),
            Some(Verification::new(
                Owner::try_new("team").expect("owner"),
                VerifiedAt::try_new("2026-05-05").expect("verified_at"),
                NonEmpty::from_vec(vec![
                    Evidence::from_field("source", "source").expect("evidence"),
                ])
                .expect("non-empty evidence"),
            )),
            span(),
        )
        .expect("valid claim");
        let resolved_block = BlockAst::KnowledgeObject(Box::new(KnowledgeObject::Claim(claim)));
        assert_eq!(resolved_block, resolved_block.clone());
    }
}

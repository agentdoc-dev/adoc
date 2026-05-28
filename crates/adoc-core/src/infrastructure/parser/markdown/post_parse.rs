//! Post-parse rewrite: paragraphs that match Pandoc / attribute patterns
//! become `BlockAst::UnknownExtension`.
//!
//! `pulldown-cmark` parses `:::warning` and `{.class}` as plain paragraph
//! text. We rewrite those paragraphs to `UnknownExtension` blocks so the
//! renderer can quarantine them — the same recognition that the
//! `UnknownExtension` compat validator uses to emit diagnostics. Both
//! callers route through
//! [`crate::infrastructure::parser::extension_classifier::classify_line`] so
//! their behavior cannot drift apart.

use crate::domain::ast::{BlockAst, UnknownExtensionAst, UnknownExtensionKind};
use crate::domain::source::SourceFile;

use crate::infrastructure::parser::extension_classifier::{LineExtension, classify_line};

pub(super) fn rewrite_pandoc_and_attribute_paragraphs(
    blocks: &mut [BlockAst],
    source: &SourceFile,
) {
    rewrite_blocks(blocks, source);
}

/// Recursively rewrites `Paragraph` blocks that match Pandoc / attribute
/// patterns into `BlockAst::UnknownExtension`, descending into nested
/// containers (`List` item content and `FootnoteDefinition` content) so
/// that directives inside loose list items and footnote bodies are rewritten
/// with the same logic applied to top-level blocks.
fn rewrite_blocks(blocks: &mut [BlockAst], source: &SourceFile) {
    for block in blocks.iter_mut() {
        // Try to rewrite this block if it is a matching paragraph.
        if let Some(replacement) = paragraph_to_unknown_extension(block, source) {
            *block = replacement;
            // After rewriting there is nothing nested to recurse into.
            continue;
        }
        // Recurse into nested block containers.
        match block {
            BlockAst::List(list) => {
                for item in &mut list.items {
                    rewrite_blocks(&mut item.content, source);
                }
            }
            BlockAst::FootnoteDefinition(footnote) => {
                rewrite_blocks(&mut footnote.content, source);
            }
            _ => {}
        }
    }
}

fn paragraph_to_unknown_extension(block: &BlockAst, source: &SourceFile) -> Option<BlockAst> {
    let BlockAst::Paragraph(paragraph) = block else {
        return None;
    };
    let start_line = paragraph.span.start.line as usize;
    let end_line = paragraph.span.end.line as usize;
    let mut kind: Option<UnknownExtensionKind> = None;
    for (index, line) in source.text.lines().enumerate() {
        let line_number = index + 1;
        if line_number < start_line {
            continue;
        }
        if line_number > end_line {
            break;
        }
        match classify_line(line) {
            LineExtension::PandocDirective { .. } => {
                kind = Some(UnknownExtensionKind::PandocDirective);
                // Pandoc takes priority over any later attribute-block match.
                break;
            }
            LineExtension::AttributeBlock { .. } => {
                kind = Some(UnknownExtensionKind::AttributeBlock);
                // Don't break — Pandoc takes priority if both match on
                // different lines of the same paragraph.
            }
            LineExtension::None => {}
        }
    }
    let kind = kind?;
    let source_text = crate::domain::inline::to_source(&paragraph.inlines);
    Some(BlockAst::UnknownExtension(UnknownExtensionAst {
        source_text,
        span: paragraph.span.clone(),
        kind,
    }))
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
    use crate::domain::ast::{
        BlockAst, FootnoteDefinitionAst, ListAst, ListItem, ListKind, ParagraphAst,
        UnknownExtensionKind,
    };
    use crate::domain::diagnostic::{SourcePosition, SourceSpan};
    use crate::domain::inline::InlineSegment;
    use crate::infrastructure::parser::markdown::parse_markdown_page;

    fn span_at(line: u32) -> SourceSpan {
        SourceSpan {
            file: PathBuf::from("test.md"),
            start: SourcePosition {
                line,
                column: 1,
                offset: 0,
            },
            end: SourcePosition {
                line,
                column: 1,
                offset: 0,
            },
        }
    }

    fn source_for(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("/work/guide.md"),
            text.to_string(),
            PathBuf::from("team/guide.md"),
        )
    }

    /// Unit test: `rewrite_blocks` rewrites a `Paragraph` at the top level.
    /// This is the pre-existing behaviour; kept as a smoke test.
    #[test]
    fn top_level_pandoc_paragraph_is_rewritten() {
        let text = ":::warning\n";
        let source = source_for(text);
        let mut blocks = vec![BlockAst::Paragraph(ParagraphAst {
            inlines: vec![InlineSegment::Text(":::warning".to_string())],
            span: span_at(1),
        })];
        rewrite_blocks(&mut blocks, &source);
        assert!(
            matches!(
                &blocks[0],
                BlockAst::UnknownExtension(u) if u.kind == UnknownExtensionKind::PandocDirective
            ),
            "expected PandocDirective rewrite; got {:?}",
            blocks[0]
        );
    }

    /// Unit test: `rewrite_blocks` rewrites a `Paragraph` inside a list
    /// item's `content` (the finding-1 regression path).
    #[test]
    fn pandoc_paragraph_inside_list_item_content_is_rewritten() {
        // Source has the directive on line 3 (item marker line 1, blank line 2,
        // directive paragraph line 3).
        let text = "- item\n\n:::warning\n";
        let source = source_for(text);

        // Build an AST by hand: a List whose single item has a directive
        // paragraph in its content.
        let directive_para = BlockAst::Paragraph(ParagraphAst {
            inlines: vec![InlineSegment::Text(":::warning".to_string())],
            span: span_at(3),
        });
        let item = ListItem {
            inlines: vec![InlineSegment::Text("item".to_string())],
            span: span_at(1),
            task_state: None,
            content: vec![directive_para],
        };
        let mut blocks = vec![BlockAst::List(ListAst {
            kind: ListKind::Unordered,
            items: vec![item],
            span: span_at(1),
        })];

        rewrite_blocks(&mut blocks, &source);

        let list = match &blocks[0] {
            BlockAst::List(l) => l,
            other => panic!("expected List; got {other:?}"),
        };
        let child = &list.items[0].content[0];
        assert!(
            matches!(
                child,
                BlockAst::UnknownExtension(u) if u.kind == UnknownExtensionKind::PandocDirective
            ),
            "directive paragraph inside item.content must be rewritten to UnknownExtension(PandocDirective); got {child:?}"
        );
    }

    /// Unit test: `rewrite_blocks` rewrites an attribute-block `Paragraph`
    /// inside a list item's `content`.
    #[test]
    fn attribute_block_paragraph_inside_list_item_content_is_rewritten() {
        let text = "- item\n\n{.callout}\n";
        let source = source_for(text);

        let attr_para = BlockAst::Paragraph(ParagraphAst {
            inlines: vec![InlineSegment::Text("{.callout}".to_string())],
            span: span_at(3),
        });
        let item = ListItem {
            inlines: vec![InlineSegment::Text("item".to_string())],
            span: span_at(1),
            task_state: None,
            content: vec![attr_para],
        };
        let mut blocks = vec![BlockAst::List(ListAst {
            kind: ListKind::Unordered,
            items: vec![item],
            span: span_at(1),
        })];

        rewrite_blocks(&mut blocks, &source);

        let list = match &blocks[0] {
            BlockAst::List(l) => l,
            other => panic!("expected List; got {other:?}"),
        };
        let child = &list.items[0].content[0];
        assert!(
            matches!(
                child,
                BlockAst::UnknownExtension(u) if u.kind == UnknownExtensionKind::AttributeBlock
            ),
            "attribute-block paragraph inside item.content must be rewritten; got {child:?}"
        );
    }

    /// Unit test: `rewrite_blocks` rewrites a directive `Paragraph` inside a
    /// `FootnoteDefinition`'s `content`.
    #[test]
    fn pandoc_paragraph_inside_footnote_content_is_rewritten() {
        let text = "[^a]: note\n\n:::warning\n";
        let source = source_for(text);

        let directive_para = BlockAst::Paragraph(ParagraphAst {
            inlines: vec![InlineSegment::Text(":::warning".to_string())],
            span: span_at(3),
        });
        let mut blocks = vec![BlockAst::FootnoteDefinition(FootnoteDefinitionAst {
            label: "a".to_string(),
            content: vec![directive_para],
            source_text: "[^a]: note\n\n:::warning\n".to_string(),
            span: span_at(1),
        })];

        rewrite_blocks(&mut blocks, &source);

        let footnote = match &blocks[0] {
            BlockAst::FootnoteDefinition(f) => f,
            other => panic!("expected FootnoteDefinition; got {other:?}"),
        };
        let child = &footnote.content[0];
        assert!(
            matches!(
                child,
                BlockAst::UnknownExtension(u) if u.kind == UnknownExtensionKind::PandocDirective
            ),
            "directive paragraph inside footnote.content must be rewritten; got {child:?}"
        );
    }

    /// Integration test: parsing a `.md` source with a loose list where the
    /// continuation paragraph is a Pandoc directive must yield an
    /// `UnknownExtension(PandocDirective)` somewhere inside `item.content`.
    ///
    /// In a loose list pulldown-cmark wraps the first-item text in its own
    /// `Paragraph` (also inside `item.content`), so the directive paragraph
    /// is `item.content[1]`.  We search the whole content vec so the test
    /// does not depend on the exact index.
    #[test]
    fn loose_list_continuation_directive_is_rewritten_by_parser() {
        // A loose list item followed by a blank line and a `:::warning` paragraph
        // indented as item continuation.  pulldown-cmark emits the `:::warning`
        // paragraph as a child of Frame::Item; after post_parse the paragraph
        // must be rewritten to UnknownExtension(PandocDirective).
        let text = "- first item\n\n  :::warning\n";
        let source = source_for(text);
        let (page, _diagnostics) = parse_markdown_page(&source);

        let list = page
            .blocks
            .iter()
            .find_map(|b| match b {
                BlockAst::List(l) => Some(l),
                _ => None,
            })
            .expect("expected a List block");

        assert_eq!(list.items.len(), 1, "expected one list item");
        let item = &list.items[0];
        assert!(
            !item.content.is_empty(),
            "loose item must have non-empty content"
        );
        let has_directive = item.content.iter().any(|b| {
            matches!(
                b,
                BlockAst::UnknownExtension(u) if u.kind == UnknownExtensionKind::PandocDirective
            )
        });
        assert!(
            has_directive,
            "continuation directive paragraph must be rewritten to \
             UnknownExtension(PandocDirective) in item.content; got {:#?}",
            item.content
        );
    }
}

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
    for block in blocks.iter_mut() {
        let Some(replacement) = paragraph_to_unknown_extension(block, source) else {
            continue;
        };
        *block = replacement;
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

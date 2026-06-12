//! Byte-range layout extraction for patch apply (V6.4, ADR-0036).
//!
//! Apply never trusts spans from a previously written artifact — graph spans
//! are start-only and build-stale. This module re-parses one current `.adoc`
//! source file and converts the typed block's fresh parser spans into the
//! pure [`TypedBlockLayout`] the `domain::source_edit` planners consume.
//! Conversion uses `SourcePosition.offset` (bytes) exclusively; char-based
//! columns never leave the parser.

use crate::domain::ast::{BlockAst, ParsedTypedBlock};
use crate::domain::diagnostic::SourceSpan;
use crate::domain::source::SourceFile;
use crate::domain::source_edit::planner::TypedBlockLayout;

use super::parse_page;

/// Parse `source` and return the byte-range layout of the typed block whose
/// id is `target_id`, or `None` when no such block parses out of the current
/// text (the caller treats that as source drift).
pub(crate) fn typed_block_layout(source: &SourceFile, target_id: &str) -> Option<TypedBlockLayout> {
    let (page, _diagnostics) = parse_page(source);
    page.blocks.iter().find_map(|block| match block {
        BlockAst::KnowledgeObjectPending(pending) if pending.id_text == target_id => {
            Some(layout_from_pending(pending))
        }
        _ => None,
    })
}

fn layout_from_pending(pending: &ParsedTypedBlock) -> TypedBlockLayout {
    TypedBlockLayout {
        open_fence: byte_range(&pending.span),
        field_values: pending
            .raw_field_spans
            .iter()
            .map(|(key, span)| (key.clone(), byte_range(span)))
            .collect(),
        duplicate_keys: pending.duplicate_keys.clone(),
        body_lines: pending.body_spans.iter().map(byte_range).collect(),
        body_separator: pending.body_separator_span.as_ref().map(byte_range),
        close_fence: byte_range(&pending.close_fence_span),
    }
}

fn byte_range(span: &SourceSpan) -> std::ops::Range<usize> {
    span.start.offset as usize..span.end.offset as usize
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    fn source_file(text: &str) -> SourceFile {
        SourceFile::new_with_identity_path(
            PathBuf::from("docs/test.adoc"),
            text.to_string(),
            PathBuf::from("docs/test.adoc"),
        )
    }

    const TEXT: &str = "\
# Billing

::claim billing.credits
status: verified
owner: team-billing
--
Body line one.
Body line two.
::

Trailing prose.
";

    #[test]
    fn extracts_byte_ranges_for_target_block() {
        let source = source_file(TEXT);
        let layout = typed_block_layout(&source, "billing.credits").expect("block found");

        assert_eq!(&TEXT[layout.open_fence.clone()], "::claim billing.credits");
        assert_eq!(&TEXT[layout.field_values["status"].clone()], "verified");
        assert_eq!(&TEXT[layout.field_values["owner"].clone()], "team-billing");
        let separator = layout.body_separator.clone().expect("separator");
        assert_eq!(&TEXT[separator], "--");
        assert_eq!(layout.body_lines.len(), 2);
        assert_eq!(&TEXT[layout.body_lines[0].clone()], "Body line one.");
        assert_eq!(&TEXT[layout.body_lines[1].clone()], "Body line two.");
        assert_eq!(&TEXT[layout.close_fence.clone()], "::");
        assert!(layout.duplicate_keys.is_empty());
    }

    #[test]
    fn returns_none_for_missing_target() {
        let source = source_file(TEXT);
        assert!(typed_block_layout(&source, "no.such.id").is_none());
    }

    #[test]
    fn multibyte_prefix_keeps_ranges_on_byte_offsets() {
        let text = "# Café — naïve 🦀\n\n::claim a.b\nowner: caf\u{e9}\n--\nBody.\n::\n";
        let source = source_file(text);
        let layout = typed_block_layout(&source, "a.b").expect("block found");
        assert_eq!(&text[layout.open_fence.clone()], "::claim a.b");
        assert_eq!(&text[layout.field_values["owner"].clone()], "caf\u{e9}");
        assert_eq!(&text[layout.close_fence.clone()], "::");
    }
}

use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{CompatDiagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;

/// Compatibility-mode counterpart to `RawHtmlForbidden` (strict mode).
///
/// Walks every block and inline segment looking for `BlockAst::QuarantinedHtml`
/// and `InlineSegment::QuarantinedHtml`, both of which are produced only by
/// the Markdown parser. Emits one `compat.raw_html_quarantined` warning per
/// occurrence. The renderer is responsible for the actual quarantine
/// (wrapping the source text in `<pre class="quarantined-html">` or
/// `<code class="quarantined-html">`); this rule only reports.
pub(crate) struct RawHtmlQuarantine;

impl CompatRule for RawHtmlQuarantine {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<CompatDiagnostic>) {
        for block in &page.blocks {
            visit_block(block, sink);
        }
    }
}

fn visit_block(block: &BlockAst, sink: &mut Vec<CompatDiagnostic>) {
    match block {
        BlockAst::QuarantinedHtml(html) => {
            sink.push(quarantine_warning(html.span.clone()));
        }
        BlockAst::Heading(heading) => check_inlines(&heading.inlines, sink),
        BlockAst::Paragraph(paragraph) => check_inlines(&paragraph.inlines, sink),
        BlockAst::List(list) => {
            for item in &list.items {
                check_inlines(&item.inlines, sink);
                for child in &item.content {
                    visit_block(child, sink);
                }
            }
        }
        BlockAst::Table(table) => {
            for cell in &table.header {
                check_inlines(&cell.inlines, sink);
            }
            for row in &table.rows {
                for cell in row {
                    check_inlines(&cell.inlines, sink);
                }
            }
        }
        BlockAst::FootnoteDefinition(footnote) => {
            for child in &footnote.content {
                visit_block(child, sink);
            }
        }
        BlockAst::CodeBlock(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_)
        | BlockAst::UnknownExtension(_)
        // A thematic break is valid Markdown — never quarantined as raw HTML.
        | BlockAst::ThematicBreak(_) => {}
    }
}

fn check_inlines(inlines: &[InlineSegment], sink: &mut Vec<CompatDiagnostic>) {
    for segment in inlines {
        match segment {
            InlineSegment::QuarantinedHtml { span, .. } => {
                sink.push(quarantine_warning(span.clone()));
            }
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => {
                check_inlines(inner, sink);
            }
            InlineSegment::Link { text, .. } => check_inlines(text, sink),
            InlineSegment::Image { alt, .. } => check_inlines(alt, sink),
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::FootnoteReference { .. }
            | InlineSegment::UnknownExtension { .. } => {}
        }
    }
}

fn quarantine_warning(span: SourceSpan) -> CompatDiagnostic {
    CompatDiagnostic::warning(
        DiagnosticCode::CompatRawHtmlQuarantined,
        "Raw HTML in Markdown source was quarantined; rendered as escaped text instead of interpreted markup.",
    )
    .with_span(span)
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
    use crate::domain::source::SourceFile;
    use crate::language::parser::parse_markdown_page;

    fn validate(text: &str) -> Vec<Diagnostic> {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("/work/guide.md"),
            text.to_string(),
            PathBuf::from("team/guide.md"),
        );
        let (page, mut diagnostics) = parse_markdown_page(&source);
        diagnostics.extend(super::super::validate_compat_source_page(&page, &source));
        diagnostics
    }

    #[test]
    fn quarantine_rule_warns_on_block_raw_html() {
        let diagnostics = validate("Intro.\n\n<div>raw</div>\n\nOutro.\n");
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![DiagnosticCode::CompatRawHtmlQuarantined]);
        assert_eq!(
            diagnostics[0].severity,
            crate::domain::diagnostic::Severity::Warning
        );
    }

    #[test]
    fn quarantine_rule_warns_on_inline_raw_html() {
        // pulldown-cmark tokenizes paired tags as two separate InlineHtml
        // events (`<span>` and `</span>`); each tag is a quarantined token in
        // its own right, so two warnings are expected.
        let diagnostics = validate("Body <span>raw</span> after.\n");
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(
            codes,
            vec![
                DiagnosticCode::CompatRawHtmlQuarantined,
                DiagnosticCode::CompatRawHtmlQuarantined,
            ]
        );
    }

    #[test]
    fn quarantine_rule_is_silent_on_clean_prose() {
        let diagnostics = validate("# Title\n\nNormal prose.\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }

    #[test]
    fn quarantine_rule_does_not_warn_on_thematic_break() {
        // Regression test for the original bug: `---` on its own line (with
        // blank lines around it so pulldown-cmark sees a rule, not a setext
        // heading) must not emit compat.raw_html_quarantined.
        let diagnostics = validate("Before.\n\n---\n\nAfter.\n");
        let raw_html_diag_count = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatRawHtmlQuarantined)
            .count();
        assert_eq!(
            raw_html_diag_count, 0,
            "thematic break must not emit compat.raw_html_quarantined; got {diagnostics:?}"
        );
    }

    #[test]
    fn quarantine_rule_does_not_warn_on_thematic_break_triple_star() {
        let diagnostics = validate("Before.\n\n***\n\nAfter.\n");
        let raw_html_diag_count = diagnostics
            .iter()
            .filter(|d| d.code == DiagnosticCode::CompatRawHtmlQuarantined)
            .count();
        assert_eq!(
            raw_html_diag_count, 0,
            "thematic break (***) must not emit compat.raw_html_quarantined; got {diagnostics:?}"
        );
    }
}

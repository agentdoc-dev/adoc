use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;
use crate::domain::url_safety::is_url_safe;

/// Rejects link URLs whose scheme isn't on the strict-mode allowlist. Walks
/// every link in the page's AST (including nested labels and links inside
/// emphasis/strong wrappers) and emits one diagnostic per offender.
pub(crate) struct UnsafeLinkForbidden;

impl ValidationRule for UnsafeLinkForbidden {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            check_block(block, sink);
        }
    }
}

fn check_block(block: &BlockAst, sink: &mut Vec<Diagnostic>) {
    match block {
        BlockAst::Heading(heading) => check_inlines(&heading.inlines, sink),
        BlockAst::Paragraph(paragraph) => check_inlines(&paragraph.inlines, sink),
        BlockAst::List(list) => {
            for item in &list.items {
                check_inlines(&item.inlines, sink);
            }
        }
        BlockAst::CodeBlock(_) => {}
        // Knowledge Object body inlines are available only after resolution,
        // so `KnowledgeObjectBodyUnsafeLinksForbidden` handles them in the
        // resolved-page phase.
        BlockAst::KnowledgeObject(_) | BlockAst::KnowledgeObjectPending(_) => {}
        // Strict-mode rule, never reached for Markdown sources where
        // QuarantinedHtml and the V4 Markdown block variants originate.
        BlockAst::QuarantinedHtml(_)
        | BlockAst::Table(_)
        | BlockAst::FootnoteDefinition(_)
        | BlockAst::UnknownExtension(_) => {}
    }
}

pub(super) fn check_inlines(inlines: &[InlineSegment], sink: &mut Vec<Diagnostic>) {
    for segment in inlines {
        match segment {
            InlineSegment::Link { text, url, span } => {
                if !is_url_safe(url) {
                    sink.push(
                        Diagnostic::error(
                            DiagnosticCode::ParseUnsafeLink,
                            format!(
                                "Link URL scheme is not allowed in strict mode: {url}; use http, https, or mailto",
                            ),
                        )
                        .with_span(span.clone()),
                    );
                }
                check_inlines(text, sink);
            }
            InlineSegment::Emphasis(inner)
            | InlineSegment::Strong(inner)
            | InlineSegment::Strikethrough(inner) => {
                check_inlines(inner, sink);
            }
            InlineSegment::Image { .. } => {
                // Image variants are V4 compat-only; strict pipeline ignores.
            }
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::QuarantinedHtml { .. }
            | InlineSegment::FootnoteReference { .. }
            | InlineSegment::UnknownExtension { .. } => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
    use crate::domain::source::SourceFile;
    use crate::infrastructure::parser::parse_page;
    use crate::infrastructure::validate::validate_source_page;

    fn validate_text(text: &str) -> Vec<Diagnostic> {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("team/guide.adoc"),
        );
        let (page, mut diagnostics) = parse_page(&source);
        diagnostics.extend(validate_source_page(&page, &source));
        diagnostics
    }

    #[test]
    fn unsafe_link_rule_emits_diagnostic_for_javascript_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click](javascript:alert) here\n");

        assert_eq!(diagnostics.len(), 1, "expected one unsafe-link diagnostic");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, DiagnosticCode::ParseUnsafeLink);
        assert!(
            diagnostic.message.contains("javascript:alert"),
            "diagnostic message should quote the offending URL: {}",
            diagnostic.message
        );
        let span = diagnostic.span.as_ref().expect("diagnostic has span");
        assert_eq!(span.start.line, 3);
        assert_eq!(span.start.column, 5);
        assert_eq!(span.end.column, 30);
    }

    #[test]
    fn unsafe_link_rule_rejects_whitespace_prefixed_javascript_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click]( javascript:alert) here\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnsafeLink);
    }

    #[test]
    fn unsafe_link_rule_rejects_internal_tab_in_javascript_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click](j\tavascript:alert) here\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnsafeLink);
    }

    #[test]
    fn unsafe_link_rule_reports_correct_column_inside_emphasis() {
        let diagnostics = validate_text("# Page @doc(team.page)\n\n*[click](javascript:bad)*\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnsafeLink);
        let span = diagnostics[0].span.as_ref().expect("diagnostic has span");
        assert_eq!(
            span.start.column, 2,
            "link inside emphasis must report inner column"
        );
        assert_eq!(span.end.column, 25);
    }

    #[test]
    fn unsafe_link_rule_reports_correct_column_inside_strong() {
        let diagnostics = validate_text("# Page @doc(team.page)\n\n**[click](javascript:bad)**\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnsafeLink);
        let span = diagnostics[0].span.as_ref().expect("diagnostic has span");
        assert_eq!(
            span.start.column, 3,
            "link inside strong must report inner column past two-char marker"
        );
        assert_eq!(span.end.column, 26);
    }

    #[test]
    fn unsafe_link_rule_does_not_flag_relative_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [docs](./guide.html) for context\n");

        assert!(
            diagnostics.is_empty(),
            "relative URL should be safe: {diagnostics:?}"
        );
    }

    #[test]
    fn unsafe_link_rule_reports_link_column_in_indented_heading_padding() {
        let diagnostics = validate_text("##   [click](javascript:bad)\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseUnsafeLink);
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(
            span.start.column, 6,
            "extra spaces after # markers must shift the inline column accordingly"
        );
    }
}

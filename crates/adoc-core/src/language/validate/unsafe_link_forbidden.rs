use crate::domain::ast::PageAst;
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;
use crate::domain::url_safety::{allowed_schemes_summary, verdict};
use crate::language::validate::url_walker::{UrlVisitor, walk_inlines, walk_page};

/// Rejects link URLs whose scheme isn't on the strict-mode allowlist. Walks
/// every link in the page's AST (including nested labels and links inside
/// emphasis/strong wrappers) and emits one diagnostic per offender. Images
/// are ignored — they are V4 Compatibility Mode constructs handled by
/// `UnsafeImageSrcDropped` and never appear in `.adoc` source.
pub(crate) struct UnsafeLinkForbidden;

impl ValidationRule for UnsafeLinkForbidden {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        let mut visitor = StrictLinkVisitor { sink };
        walk_page(page, &mut visitor);
    }
}

/// Walk a Knowledge Object body's inlines with the strict link rule. Used by
/// `KnowledgeObjectBodyUnsafeLinksForbidden` so resolved-phase callers share
/// the verdict logic with the source-phase rule.
pub(super) fn check_body_inlines(inlines: &[InlineSegment], sink: &mut Vec<Diagnostic>) {
    let mut visitor = StrictLinkVisitor { sink };
    walk_inlines(inlines, &mut visitor);
}

struct StrictLinkVisitor<'a> {
    sink: &'a mut Vec<Diagnostic>,
}

impl UrlVisitor for StrictLinkVisitor<'_> {
    fn on_link(&mut self, _text: &[InlineSegment], url: &str, span: &SourceSpan) {
        if verdict(url).is_safe() {
            return;
        }
        self.sink.push(
            Diagnostic::error(
                DiagnosticCode::ParseUnsafeLink,
                format!(
                    "Link URL scheme is not allowed in strict mode: {url}; use {}",
                    allowed_schemes_summary(),
                ),
            )
            .with_span(span.clone()),
        );
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
    use crate::domain::source::SourceFile;
    use crate::language::parser::parse_page;
    use crate::language::validate::validate_source_page;

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

    #[test]
    fn unsafe_link_rule_help_lists_every_allowed_scheme() {
        // Regression gate: the rejection message must format from the
        // allowlist, never restate a hard-coded scheme list. If a future
        // change adds `tel:` to ALLOWED_SCHEMES, this assertion catches the
        // message drift automatically.
        use crate::domain::url_safety::ALLOWED_SCHEMES;
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click](javascript:alert)\n");
        let message = &diagnostics[0].message;
        for scheme in ALLOWED_SCHEMES {
            assert!(
                message.contains(scheme),
                "rejection message {message:?} must mention `{scheme}`",
            );
        }
    }
}

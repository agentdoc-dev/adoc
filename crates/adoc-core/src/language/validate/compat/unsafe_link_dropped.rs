use crate::domain::ast::PageAst;
use crate::domain::diagnostic::{CompatDiagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;
use crate::domain::url_safety::verdict;
use crate::language::validate::url_walker::{UrlVisitor, walk_page};

/// Compatibility-mode counterpart to `UnsafeLinkForbidden` (strict mode).
///
/// Walks every link in the page's AST and emits a
/// `compat.unsafe_link_dropped` warning when the URL fails the V4 safety
/// verdict (`javascript`, `data`, `vbscript`, any unknown scheme, or any
/// whitespace). The renderer is responsible for dropping the `href`
/// attribute; this rule only reports.
pub(crate) struct UnsafeLinkDropped;

impl CompatRule for UnsafeLinkDropped {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<CompatDiagnostic>) {
        let mut visitor = LinkVisitor { sink };
        walk_page(page, &mut visitor);
    }
}

struct LinkVisitor<'a> {
    sink: &'a mut Vec<CompatDiagnostic>,
}

impl UrlVisitor for LinkVisitor<'_> {
    fn on_link(&mut self, _text: &[InlineSegment], url: &str, span: &SourceSpan) {
        if verdict(url).is_safe() {
            return;
        }
        self.sink.push(
            CompatDiagnostic::warning(
                DiagnosticCode::CompatUnsafeLinkDropped,
                format!(
                    "Link href `{url}` uses an unsafe scheme; the href will be dropped from the rendered HTML."
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
    fn unsafe_link_rule_warns_on_javascript_scheme() {
        let diagnostics = validate("Click [here](javascript:alert(1)).\n");
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![DiagnosticCode::CompatUnsafeLinkDropped]);
        assert_eq!(
            diagnostics[0].severity,
            crate::domain::diagnostic::Severity::Warning
        );
    }

    #[test]
    fn unsafe_link_rule_is_silent_on_https_scheme() {
        let diagnostics = validate("Click [here](https://example.test).\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}

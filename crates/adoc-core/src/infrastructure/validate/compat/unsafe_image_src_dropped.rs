use crate::domain::ast::PageAst;
use crate::domain::diagnostic::{CompatDiagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::CompatRule;
use crate::domain::source::SourceFile;
use crate::domain::url_safety::verdict;
use crate::infrastructure::validate::url_walker::{UrlVisitor, walk_page};

/// Compat-mode rule that warns when a Markdown image embed uses an unsafe
/// URL scheme. The renderer drops the `src` attribute and keeps the alt text
/// visible; this rule only reports the diagnostic.
pub(crate) struct UnsafeImageSrcDropped;

impl CompatRule for UnsafeImageSrcDropped {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<CompatDiagnostic>) {
        let mut visitor = ImageVisitor { sink };
        walk_page(page, &mut visitor);
    }
}

struct ImageVisitor<'a> {
    sink: &'a mut Vec<CompatDiagnostic>,
}

impl UrlVisitor for ImageVisitor<'_> {
    fn on_image(&mut self, _alt: &[InlineSegment], url: &str, span: &SourceSpan) {
        if verdict(url).is_safe() {
            return;
        }
        self.sink.push(
            CompatDiagnostic::warning(
                DiagnosticCode::CompatUnsafeImageSrcDropped,
                format!(
                    "Image src `{url}` uses an unsafe scheme; the src will be dropped from the rendered HTML."
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
    use crate::infrastructure::parser::parse_markdown_page;

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
    fn unsafe_image_rule_warns_on_data_url_scheme() {
        let diagnostics = validate("![alt](data:image/svg+xml;base64,PHN2Zz48L3N2Zz4=)\n");
        let codes: Vec<_> = diagnostics.iter().map(|d| d.code).collect();
        assert_eq!(codes, vec![DiagnosticCode::CompatUnsafeImageSrcDropped]);
    }

    #[test]
    fn unsafe_image_rule_is_silent_on_https_scheme() {
        let diagnostics = validate("![ok](https://example.test/logo.png)\n");
        assert!(diagnostics.is_empty(), "{diagnostics:?}");
    }
}

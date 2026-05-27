use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::ValidationRule;
use crate::domain::source::SourceFile;

/// Compat-mode rule that warns when a Markdown image embed uses an unsafe
/// URL scheme. The renderer drops the `src` attribute and keeps the alt text
/// visible; this rule only reports the diagnostic.
pub(crate) struct UnsafeImageSrcDropped;

impl ValidationRule for UnsafeImageSrcDropped {
    fn check(&self, page: &PageAst, _source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            walk_block(block, sink);
        }
    }
}

fn walk_block(block: &BlockAst, sink: &mut Vec<Diagnostic>) {
    match block {
        BlockAst::Heading(heading) => walk_inlines(&heading.inlines, sink),
        BlockAst::Paragraph(paragraph) => walk_inlines(&paragraph.inlines, sink),
        BlockAst::List(list) => {
            for item in &list.items {
                walk_inlines(&item.inlines, sink);
            }
        }
        BlockAst::CodeBlock(_)
        | BlockAst::QuarantinedHtml(_)
        | BlockAst::KnowledgeObject(_)
        | BlockAst::KnowledgeObjectPending(_) => {}
    }
}

fn walk_inlines(inlines: &[InlineSegment], sink: &mut Vec<Diagnostic>) {
    for segment in inlines {
        match segment {
            InlineSegment::Image { alt, url, span } => {
                if !is_url_safe(url) {
                    sink.push(
                        Diagnostic::warning(
                            DiagnosticCode::CompatUnsafeImageSrcDropped,
                            format!(
                                "Image src `{url}` uses an unsafe scheme; the src will be dropped from the rendered HTML."
                            ),
                        )
                        .with_span(span.clone()),
                    );
                }
                walk_inlines(alt, sink);
            }
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
                walk_inlines(inner, sink);
            }
            InlineSegment::Link { text, .. } => walk_inlines(text, sink),
            InlineSegment::Text(_)
            | InlineSegment::Code(_)
            | InlineSegment::ObjectReference { .. }
            | InlineSegment::ObjectReferencePending { .. }
            | InlineSegment::QuarantinedHtml { .. } => {}
        }
    }
}

fn is_url_safe(url: &str) -> bool {
    if url.bytes().any(|byte| byte.is_ascii_whitespace()) {
        return false;
    }
    let Some(colon) = url.find(':') else {
        return true;
    };
    let scheme = &url[..colon];
    if scheme.is_empty() {
        return true;
    }
    if !scheme.starts_with(|character: char| character.is_ascii_alphabetic()) {
        return true;
    }
    if !scheme.chars().all(|character| {
        character.is_ascii_alphanumeric()
            || character == '+'
            || character == '-'
            || character == '.'
    }) {
        return true;
    }
    matches!(
        scheme.to_ascii_lowercase().as_str(),
        "http" | "https" | "mailto"
    )
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;
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

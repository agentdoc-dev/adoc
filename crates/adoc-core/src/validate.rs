//! Strict-mode validation pass.
//!
//! Each [`ValidationRule`] inspects a parsed page and appends diagnostics for
//! violations. The parser produces a syntactic AST; semantic checks (raw
//! HTML, unsafe link schemes) live here so they can be unit-tested at their
//! own interface and so the parser stays a tokenizer.
//!
//! The exception is `parse.unclosed_fence`: closure detection requires
//! streaming context (you only know a fence is unclosed once EOF is reached),
//! so that diagnostic remains in the parser. See ADR-0007 for the decision.

use crate::ast::{BlockAst, PageAst};
use crate::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::inline::InlineSegment;
use crate::source::{LineCursor, SourceFile};

pub(crate) trait ValidationRule {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>);
}

/// Run every strict-mode rule against `page`, appending diagnostics in source
/// order. The order of rules is chosen so that diagnostic emission matches
/// the order the parser used to produce: per line, raw HTML before the line's
/// inline content (so that a line containing both a raw tag and an unsafe
/// link reports the raw tag first).
pub(crate) fn validate_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    RawHtmlForbidden.check(page, source, &mut diagnostics);
    UnsafeLinkForbidden.check(page, source, &mut diagnostics);
    diagnostics
}

/// Rejects raw HTML in the source: any line that contains a recognizable HTML
/// opening tag at a tag boundary (start of line or after whitespace) yields a
/// `parse.raw_html` diagnostic. Inline `<` characters surrounded by prose
/// (e.g. `Vec<String>`) do not match.
pub(crate) struct RawHtmlForbidden;

impl ValidationRule for RawHtmlForbidden {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        let lines: Vec<&str> = source.text.lines().collect();
        for block in &page.blocks {
            match block {
                BlockAst::CodeBlock(_) => continue,
                BlockAst::Heading(heading) => {
                    flag_raw_html_in_span(&lines, source, &heading.span, sink)
                }
                BlockAst::Paragraph(paragraph) => {
                    flag_raw_html_in_span(&lines, source, &paragraph.span, sink)
                }
                BlockAst::List(list) => flag_raw_html_in_span(&lines, source, &list.span, sink),
            }
        }
    }
}

fn flag_raw_html_in_span(
    lines: &[&str],
    source: &SourceFile,
    span: &SourceSpan,
    sink: &mut Vec<Diagnostic>,
) {
    for line_number in span.start.line..=span.end.line {
        let Some(line) = lines.get(line_number.saturating_sub(1) as usize) else {
            continue;
        };
        let Some(matched) = find_raw_html(line) else {
            continue;
        };
        sink.push(
            Diagnostic::error(
                DiagnosticCode::ParseRawHtml,
                "Raw HTML is not allowed in strict mode; write AgentDoc Source prose instead",
            )
            .with_span(source.span_for_line_columns(
                line_number,
                matched.start_column,
                matched.end_column,
            )),
        );
    }
}

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
                check_inlines(item, sink);
            }
        }
        BlockAst::CodeBlock(_) => {}
    }
}

fn check_inlines(inlines: &[InlineSegment], sink: &mut Vec<Diagnostic>) {
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
            InlineSegment::Emphasis(inner) | InlineSegment::Strong(inner) => {
                check_inlines(inner, sink);
            }
            InlineSegment::Text(_) | InlineSegment::Code(_) => {}
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct RawHtmlMatch {
    start_column: u32,
    end_column: u32,
}

fn find_raw_html(line: &str) -> Option<RawHtmlMatch> {
    for (start_index, character) in line.char_indices() {
        if character != '<' {
            continue;
        }

        let is_tag_boundary = start_index == 0
            || line[..start_index]
                .chars()
                .last()
                .is_some_and(|character| character.is_whitespace());
        if !is_tag_boundary {
            continue;
        }

        let after_opening_bracket = &line[start_index + character.len_utf8()..];
        let Some(tag_end) = raw_html_tag_end(after_opening_bracket) else {
            continue;
        };
        let end_index = start_index + character.len_utf8() + tag_end;

        let line_start = LineCursor::at_line_start(0);
        return Some(RawHtmlMatch {
            start_column: line_start.column_after_chars(&line[..start_index]),
            end_column: line_start.column_after_chars(&line[..end_index]),
        });
    }

    None
}

fn raw_html_tag_end(value: &str) -> Option<usize> {
    let mut name_start = 0;
    if value.starts_with('/') {
        name_start = 1;
    }

    let first_character = value[name_start..].chars().next()?;
    if !first_character.is_ascii_alphabetic() {
        return None;
    }

    let mut name_end = name_start + first_character.len_utf8();
    for character in value[name_end..].chars() {
        if !character.is_ascii_alphanumeric() && character != '-' {
            break;
        }
        name_end += character.len_utf8();
    }

    let next_character = value[name_end..].chars().next()?;
    match next_character {
        '>' => Some(name_end + 1),
        '/' => value[name_end + 1..]
            .starts_with('>')
            .then_some(name_end + 2),
        character if character.is_whitespace() => value[name_end..]
            .find('>')
            .map(|relative_index| name_end + relative_index + 1),
        _ => None,
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
    use crate::parser::parse_page;

    fn validate_text(text: &str) -> Vec<Diagnostic> {
        let source = SourceFile::new_with_identity_path(
            PathBuf::from("guide.adoc"),
            text.to_string(),
            PathBuf::from("guide.adoc"),
        );
        let (page, mut diagnostics) = parse_page(&source);
        diagnostics.extend(validate_page(&page, &source));
        diagnostics
    }

    // --- predicate-level tests ---

    #[test]
    fn find_raw_html_matches_simple_block_tag() {
        let m = find_raw_html("<div>x</div>").expect("expected match");
        assert_eq!(m.start_column, 1);
        assert_eq!(m.end_column, 6);
    }

    #[test]
    fn find_raw_html_returns_none_for_inline_less_than() {
        assert!(find_raw_html("Vec<String>").is_none());
    }

    #[test]
    fn find_raw_html_skips_to_first_match_after_whitespace() {
        let m = find_raw_html("hello <span>x</span>").expect("expected match");
        assert_eq!(m.start_column, 7);
    }

    #[test]
    fn is_url_safe_accepts_http_https_and_mailto() {
        assert!(is_url_safe("https://example.test/path"));
        assert!(is_url_safe("http://example.test"));
        assert!(is_url_safe("mailto:hello@example.test"));
    }

    #[test]
    fn is_url_safe_accepts_relative_url() {
        assert!(is_url_safe("/docs/page.html"));
    }

    #[test]
    fn is_url_safe_rejects_javascript_scheme() {
        assert!(!is_url_safe("javascript:alert(1)"));
    }

    #[test]
    fn is_url_safe_rejects_url_with_internal_whitespace() {
        assert!(!is_url_safe("java\tscript:alert(1)"));
        assert!(!is_url_safe("javascript :alert(1)"));
    }

    // --- raw HTML rule (migrated from parser.rs) ---

    #[test]
    fn raw_html_rule_allows_angle_bracket_prose() {
        let diagnostics = validate_text(
            "# Technical Prose\n\nVec<String>, Map<K, V>, Result<T, E>, and compare a<b.\n",
        );
        assert!(
            diagnostics.is_empty(),
            "expected angle-bracket prose to validate cleanly, got {diagnostics:?}"
        );
    }

    #[test]
    fn raw_html_rule_rejects_inline_raw_html_tag() {
        let diagnostics =
            validate_text("# Unsafe @doc(team.unsafe)\n\nKeep <span>raw html</span> out.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 6);
    }

    #[test]
    fn raw_html_rule_rejects_unknown_raw_html_tag() {
        let diagnostics = validate_text("# Unsafe @doc(team.unsafe)\n\n<foo>bar</foo>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn raw_html_rule_rejects_custom_element_raw_html_tag() {
        let diagnostics =
            validate_text("# Unsafe @doc(team.unsafe)\n\n<my-component>x</my-component>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn raw_html_rule_skips_fenced_code_block() {
        let diagnostics = validate_text(
            "# Fenced HTML @doc(team.fenced)\n\n```html\n<div>example</div>\n```\n",
        );
        assert!(
            diagnostics.is_empty(),
            "expected raw HTML inside a fenced code block to be skipped, got {diagnostics:?}"
        );
    }

    #[test]
    fn raw_html_rule_flags_prose_when_fence_is_present_elsewhere() {
        let diagnostics = validate_text(
            "# Mixed @doc(team.mixed)\n\n```html\n<div>fenced</div>\n```\n\n<span>prose</span>\n",
        );

        assert_eq!(
            diagnostics.len(),
            1,
            "exactly one diagnostic for the prose-level <span>, got {diagnostics:?}"
        );
        assert_eq!(diagnostics[0].code, "parse.raw_html");
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.line, 7);
        assert_eq!(span.start.column, 1);
    }

    // --- unsafe link rule (migrated from inline.rs / parser.rs) ---

    #[test]
    fn unsafe_link_rule_emits_diagnostic_for_javascript_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click](javascript:alert) here\n");

        assert_eq!(diagnostics.len(), 1, "expected one unsafe-link diagnostic");
        let diagnostic = &diagnostics[0];
        assert_eq!(diagnostic.code, "parse.unsafe_link");
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
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
    }

    #[test]
    fn unsafe_link_rule_rejects_internal_tab_in_javascript_url() {
        let diagnostics =
            validate_text("# Page @doc(team.page)\n\nsee [click](j\tavascript:alert) here\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
    }

    #[test]
    fn unsafe_link_rule_reports_correct_column_inside_emphasis() {
        let diagnostics = validate_text("# Page @doc(team.page)\n\n*[click](javascript:bad)*\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
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
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
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
        assert_eq!(diagnostics[0].code, "parse.unsafe_link");
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(
            span.start.column, 6,
            "extra spaces after # markers must shift the inline column accordingly"
        );
    }
}

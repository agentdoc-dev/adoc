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

mod claim_unique_ids;
pub(crate) mod resolve_claims;
pub(crate) use resolve_claims::resolve_knowledge_objects;

use claim_unique_ids::KnowledgeObjectUniqueIds;

use crate::domain::ast::{BlockAst, PageAst, WorkspaceAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::inline::InlineSegment;
use crate::domain::rules::{ValidationRule, WorkspaceRule};
use crate::domain::scan::raw_html::find_raw_html;
use crate::domain::source::SourceFile;

/// Source-page rules run over the parsed page before pending Knowledge
/// Objects are resolved. They are allowed to inspect parser-owned source spans.
const SOURCE_PAGE_RULES: &[&dyn ValidationRule] = &[&RawHtmlForbidden, &UnsafeLinkForbidden];

/// Resolved-page rules run after pending Knowledge Objects have been converted
/// into typed aggregates. Empty until a rule needs typed `Claim` data.
const RESOLVED_PAGE_RULES: &[&dyn ValidationRule] = &[];

/// Workspace-level rules, applied in registration order after knowledge object
/// resolution and workspace assembly.
const WORKSPACE_RULES: &[&dyn WorkspaceRule] = &[&KnowledgeObjectUniqueIds];

/// Run every source-page rule against `page`. The orchestrator performs the
/// final source-position diagnostic sort before returning `CompileResult`.
pub(crate) fn validate_source_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    validate_page_with_rules(page, source, SOURCE_PAGE_RULES)
}

/// Run every resolved-page rule against `page` after Knowledge Object
/// resolution. The registry is intentionally empty in the first claim slice.
pub(crate) fn validate_resolved_page(page: &PageAst, source: &SourceFile) -> Vec<Diagnostic> {
    validate_page_with_rules(page, source, RESOLVED_PAGE_RULES)
}

fn validate_page_with_rules(
    page: &PageAst,
    source: &SourceFile,
    rules: &[&dyn ValidationRule],
) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in rules {
        rule.check(page, source, &mut diagnostics);
    }
    diagnostics
}

/// Run every workspace-level rule against `workspace`. Workspace rules run
/// after per-page validation, so per-page errors are already in the sink by
/// the time the orchestrator calls into here.
pub(crate) fn validate_workspace(workspace: &WorkspaceAst) -> Vec<Diagnostic> {
    let mut diagnostics = Vec::new();
    for rule in WORKSPACE_RULES {
        rule.check(workspace, &mut diagnostics);
    }
    diagnostics
}

/// Rejects raw HTML in the source: any line that contains a recognizable HTML
/// opening tag at a tag boundary (start of line or after whitespace) yields a
/// `parse.raw_html` diagnostic. Inline `<` characters surrounded by prose
/// (e.g. `Vec<String>`) do not match.
pub(crate) struct RawHtmlForbidden;

impl ValidationRule for RawHtmlForbidden {
    fn check(&self, page: &PageAst, source: &SourceFile, sink: &mut Vec<Diagnostic>) {
        for block in &page.blocks {
            match block {
                BlockAst::CodeBlock(_) => continue,
                BlockAst::Heading(heading) => flag_raw_html_in_span(source, &heading.span, sink),
                BlockAst::Paragraph(paragraph) => {
                    flag_raw_html_in_span(source, &paragraph.span, sink)
                }
                // Walk per-item spans, not `list.span`. `list.span` covers
                // only the first item's line; raw HTML in the 2nd+ items
                // would slip past if we used it. PR #20 review (P1).
                BlockAst::List(list) => {
                    for item in &list.items {
                        flag_raw_html_in_span(source, &item.span, sink);
                    }
                }
                // Resolved Knowledge Objects contain typed domain values.
                // Pending objects still carry parser-owned source spans that
                // source-page validators can scan before resolution.
                BlockAst::KnowledgeObject(_) => {}
                BlockAst::KnowledgeObjectPending(pending) => {
                    for span in &pending.content_spans {
                        flag_raw_html_in_span(source, span, sink);
                    }
                }
            }
        }
    }
}

fn flag_raw_html_in_span(source: &SourceFile, span: &SourceSpan, sink: &mut Vec<Diagnostic>) {
    for line_number in span.start.line..=span.end.line {
        let Some(line) = source.line_text(line_number) else {
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
                check_inlines(&item.inlines, sink);
            }
        }
        BlockAst::CodeBlock(_) => {}
        // Knowledge Object text is plain body/field text in v0.2. Inline
        // parsing for claim bodies lands in a later slice.
        // TODO(v0.5): once claim bodies parse inline object references, run
        // unsafe-link validation over those body inlines too.
        BlockAst::KnowledgeObject(_) | BlockAst::KnowledgeObjectPending(_) => {}
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
    use crate::infrastructure::parser::parse_page;

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

    // Raw-HTML scanner unit tests live alongside the scanner in
    // `crates/adoc-core/src/scan/raw_html.rs` (TB-8). Validator-level tests
    // exercise the rule via `validate_text` so the integration with the AST
    // walk and the per-block dispatch is covered here.

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
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 6);
    }

    #[test]
    fn raw_html_rule_rejects_unknown_raw_html_tag() {
        let diagnostics = validate_text("# Unsafe @doc(team.unsafe)\n\n<foo>bar</foo>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn raw_html_rule_rejects_custom_element_raw_html_tag() {
        let diagnostics =
            validate_text("# Unsafe @doc(team.unsafe)\n\n<my-component>x</my-component>\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 1);
    }

    #[test]
    fn raw_html_rule_skips_fenced_code_block() {
        let diagnostics =
            validate_text("# Fenced HTML @doc(team.fenced)\n\n```html\n<div>example</div>\n```\n");
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
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.line, 7);
        assert_eq!(span.start.column, 1);
    }

    #[test]
    fn raw_html_rule_rejects_raw_html_in_claim_field_value() {
        let diagnostics = validate_text(concat!(
            "# Claim @doc(team.claim)\n\n",
            "::claim billing.credits\n",
            "status: <span>verified</span>\n",
            "--\n",
            "The system credits users automatically.\n",
            "::\n",
        ));

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.line, 4);
        assert_eq!(span.start.column, 9);
    }

    #[test]
    fn raw_html_rule_rejects_raw_html_in_claim_body() {
        let diagnostics = validate_text(concat!(
            "# Claim @doc(team.claim)\n\n",
            "::claim billing.credits\n",
            "status: verified\n",
            "--\n",
            "Body <span>raw</span> text.\n",
            "::\n",
        ));

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        let span = diagnostics[0].span.as_ref().unwrap();
        assert_eq!(span.start.line, 6);
        assert_eq!(span.start.column, 6);
    }

    // --- unsafe link rule (migrated from inline.rs / parser.rs) ---

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

    // --- workspace-rule port ---

    fn workspace_with_titles(titles: &[&str]) -> WorkspaceAst {
        let pages = titles
            .iter()
            .map(|title| {
                let source = SourceFile::new_with_identity_path(
                    PathBuf::from(format!("{}.adoc", title)),
                    format!("# {title}\n"),
                    PathBuf::from(format!("{title}.adoc")),
                );
                let (page, _) = parse_page(&source);
                page
            })
            .collect();
        WorkspaceAst { pages }
    }

    struct SentinelWorkspaceRule;

    impl WorkspaceRule for SentinelWorkspaceRule {
        fn check(&self, workspace: &WorkspaceAst, sink: &mut Vec<Diagnostic>) {
            // Synthesise a diagnostic that proves the rule was invoked and
            // could see the workspace's contents.
            sink.push(Diagnostic::error(
                DiagnosticCode::ParseRawHtml,
                format!("workspace observed {} page(s)", workspace.pages.len()),
            ));
        }
    }

    #[test]
    fn workspace_rule_can_observe_workspace_pages_and_emit_diagnostic() {
        let workspace = workspace_with_titles(&["one", "two"]);
        let mut sink = Vec::new();

        SentinelWorkspaceRule.check(&workspace, &mut sink);

        assert_eq!(sink.len(), 1);
        assert_eq!(sink[0].message, "workspace observed 2 page(s)");
    }

    #[test]
    fn validate_workspace_emits_no_diagnostics_for_workspace_without_claim_duplicates() {
        let workspace = workspace_with_titles(&["alpha"]);

        let diagnostics = validate_workspace(&workspace);

        assert!(diagnostics.is_empty());
    }

    // --- P1 fix tests (PR #20 review): raw HTML in any list item flags ---

    #[test]
    fn raw_html_in_second_unordered_list_item_flags() {
        let diagnostics =
            validate_text("# Bug @doc(team.bug)\n\n- first item\n- second has <span>raw</span>\n");

        assert_eq!(
            diagnostics.len(),
            1,
            "expected one parse.raw_html for the 2nd item, got {diagnostics:?}"
        );
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        let span = diagnostics[0].span.as_ref().expect("diagnostic has span");
        assert_eq!(
            span.start.line, 4,
            "diagnostic must point at the 2nd item's line"
        );
    }

    #[test]
    fn raw_html_in_second_ordered_list_item_flags() {
        let diagnostics = validate_text(
            "# Bug @doc(team.bug)\n\n1. first item\n2. second has <span>raw</span>\n",
        );

        assert_eq!(
            diagnostics.len(),
            1,
            "expected one parse.raw_html for the 2nd item, got {diagnostics:?}"
        );
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        let span = diagnostics[0].span.as_ref().expect("diagnostic has span");
        assert_eq!(
            span.start.line, 4,
            "diagnostic must point at the 2nd item's line"
        );
    }

    #[test]
    fn raw_html_in_first_list_item_still_flags() {
        let diagnostics =
            validate_text("# Bug @doc(team.bug)\n\n- first has <span>raw</span>\n- second item\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.line, 3);
    }

    #[test]
    fn raw_html_in_third_list_item_flags() {
        let diagnostics = validate_text(
            "# Bug @doc(team.bug)\n\n- one\n- two\n- three has <span>raw</span>\n- four\n",
        );

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(
            diagnostics[0].span.as_ref().unwrap().start.line,
            5,
            "diagnostic must point at the 3rd item's line"
        );
    }

    #[test]
    fn raw_html_in_multiple_list_items_emits_one_diagnostic_per_offender() {
        let diagnostics = validate_text(
            "# Bug @doc(team.bug)\n\n- first <a>raw</a>\n- second <span>raw</span>\n",
        );

        assert_eq!(
            diagnostics.len(),
            2,
            "one diagnostic per offending item, got {diagnostics:?}"
        );
        assert!(
            diagnostics
                .iter()
                .all(|d| d.code == DiagnosticCode::ParseRawHtml)
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

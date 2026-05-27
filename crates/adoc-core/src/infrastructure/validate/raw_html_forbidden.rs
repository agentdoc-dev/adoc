use crate::domain::ast::{BlockAst, PageAst};
use crate::domain::diagnostic::{Diagnostic, DiagnosticCode, SourceSpan};
use crate::domain::rules::ValidationRule;
use crate::domain::scan::raw_html::find_raw_html;
use crate::domain::source::SourceFile;

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
                // Strict-mode rule, never reached for Markdown sources where
                // QuarantinedHtml originates. Kept exhaustive so adding the
                // variant elsewhere doesn't silently bypass strict checks.
                BlockAst::QuarantinedHtml(_) => {}
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

    // Raw-HTML scanner unit tests live alongside the scanner in
    // `crates/adoc-core/src/scan/raw_html.rs` (TB-8). Validator-level tests
    // exercise the rule via `validate_text` so the integration with the AST
    // walk and the per-block dispatch is covered here.

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
    fn raw_html_rule_rejects_adjacent_inline_raw_html_tag() {
        let diagnostics =
            validate_text("# Unsafe @doc(team.unsafe)\n\nKeep<span>raw html</span> out.\n");

        assert_eq!(diagnostics.len(), 1);
        assert_eq!(diagnostics[0].code, DiagnosticCode::ParseRawHtml);
        assert_eq!(diagnostics[0].span.as_ref().unwrap().start.column, 5);
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
}

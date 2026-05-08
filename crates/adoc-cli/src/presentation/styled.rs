use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::ExplainView;

use super::plain::{
    evidence_items, fields_items, has_evidence, has_fields, has_relations, relations_items,
};
use super::port::ExplainPresenter;
use super::style::chip::status_chip;
use super::style::kv::faint_label;
use super::style::palette::status_color;
use super::style::wikilink::highlight;

/// Styled presenter.  Produces the same line layout as [`PlainPresenter`] but
/// with ANSI decoration:
///
/// - All labels (`Object:`, `Kind:`, etc.) are rendered **faint** (dim).
/// - The `Status:` (or `Severity:`) value is wrapped in a coloured pill chip.
/// - Section headers (`Evidence:`, `Fields:`, `Relations:`) are also faint.
///
/// Body text, evidence items, fields items, and relation items are rendered as
/// plain text in this slice.  Wikilink highlighting and relation chips are
/// added in later slices.
///
/// # Factoring note
///
/// Section predicates and body-item helpers from `plain.rs` are reused to
/// avoid duplicating iteration logic.  The presenter owns the leading blank
/// line and the (faint) header for each section.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct StyledPresenter;

impl ExplainPresenter for StyledPresenter {
    fn present(&self, view: &ExplainView, out: &mut dyn io::Write) -> io::Result<()> {
        let mut buf = String::new();
        render_styled(&mut buf, view);
        out.write_all(buf.as_bytes())
    }
}

fn render_styled(output: &mut String, view: &ExplainView) {
    let record = &view.record;

    writeln!(output, "{} {}", faint_label("Object:"), record.id)
        .expect("writing to String cannot fail");
    writeln!(output, "{} {}", faint_label("Kind:"), record.kind)
        .expect("writing to String cannot fail");

    if let Some(status) = &record.status {
        let palette = status_color(Some(status.as_str()));
        let chip = status_chip(palette, status.as_str());
        if record.kind == "warning" {
            // Warnings use "Severity:" label but still get a chip.
            writeln!(output, "{} {chip}", faint_label("Severity:"))
                .expect("writing to String cannot fail");
        } else {
            writeln!(output, "{} {chip}", faint_label("Status:"))
                .expect("writing to String cannot fail");
        }
    }

    if let Some(owner) = &record.owner {
        writeln!(output, "{} {owner}", faint_label("Owner:"))
            .expect("writing to String cannot fail");
    }
    if let Some(verified_at) = &record.verified_at {
        writeln!(output, "{} {verified_at}", faint_label("Verified:"))
            .expect("writing to String cannot fail");
    }

    output.push('\n');
    writeln!(output, "{}", faint_label("Statement:")).expect("writing to String cannot fail");
    let highlighted = highlight(&record.body);
    output.push_str(&indent_body(&highlighted, "  "));
    if !record.body.ends_with('\n') {
        output.push('\n');
    }

    if has_evidence(record) {
        output.push('\n');
        writeln!(output, "{}", faint_label("Evidence:")).expect("writing to String cannot fail");
        evidence_items(output, record);
    }

    if has_fields(record) {
        output.push('\n');
        writeln!(output, "{}", faint_label("Fields:")).expect("writing to String cannot fail");
        fields_items(output, record);
    }

    output.push('\n');
    writeln!(
        output,
        "{} {}:{}:{}",
        faint_label("Source:"),
        record.source.path,
        record.source.line,
        record.source.column
    )
    .expect("writing to String cannot fail");

    if has_relations(&record.relations) {
        output.push('\n');
        writeln!(output, "{}", faint_label("Relations:")).expect("writing to String cannot fail");
        relations_items(output, &record.relations);
    }
}

/// Indent every line of `body` by `indent`.
///
/// A trailing newline on the original body is preserved; a body without one
/// does not gain one.  A trailing empty segment that follows the terminal `\n`
/// is NOT prefixed, to avoid producing a phantom indented blank line at the end.
///
/// # Examples
///
/// ```ignore
/// assert_eq!(indent_body("line1\nline2\n", "  "), "  line1\n  line2\n");
/// assert_eq!(indent_body("line1", "  "), "  line1");
/// assert_eq!(indent_body("", "  "), "");
/// ```
fn indent_body(body: &str, indent: &str) -> String {
    if body.is_empty() {
        return String::new();
    }
    let trailing_newline = body.ends_with('\n');
    let trimmed = body.strip_suffix('\n').unwrap_or(body);
    let mut out = String::with_capacity(body.len() + indent.len() * 4);
    for (i, line) in trimmed.split('\n').enumerate() {
        if i > 0 {
            out.push('\n');
        }
        out.push_str(indent);
        out.push_str(line);
    }
    if trailing_newline {
        out.push('\n');
    }
    out
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use adoc_core::{AgentJsonRelations, ExplainView, RetrievalRecord, RetrievalSource};

    use super::*;

    fn make_record(id: &str, kind: &str) -> RetrievalRecord {
        RetrievalRecord {
            id: id.to_string(),
            kind: kind.to_string(),
            status: None,
            owner: None,
            verified_at: None,
            body: "Body text.".to_string(),
            source: RetrievalSource {
                path: "docs/test.adoc".to_string(),
                line: 1,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        }
    }

    fn view_for(record: RetrievalRecord) -> ExplainView {
        ExplainView {
            record,
            related_statuses: BTreeMap::new(),
        }
    }

    fn render(view: &ExplainView) -> String {
        let mut buf = Vec::new();
        StyledPresenter.present(view, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    fn strip_ansi(s: &str) -> String {
        strip_ansi_escapes::strip_str(s)
    }

    // -----------------------------------------------------------------------
    // indent_body unit tests
    // -----------------------------------------------------------------------

    #[test]
    fn indent_body_empty_returns_empty() {
        assert_eq!(indent_body("", "  "), "");
    }

    #[test]
    fn indent_body_single_line_no_trailing_newline() {
        assert_eq!(indent_body("hello", "  "), "  hello");
    }

    #[test]
    fn indent_body_single_line_with_trailing_newline() {
        assert_eq!(indent_body("hello\n", "  "), "  hello\n");
    }

    #[test]
    fn indent_body_multi_line_with_trailing_newline() {
        assert_eq!(indent_body("line1\nline2\n", "  "), "  line1\n  line2\n");
    }

    #[test]
    fn indent_body_multi_line_without_trailing_newline() {
        assert_eq!(indent_body("line1\nline2", "  "), "  line1\n  line2");
    }

    #[test]
    fn indent_body_does_not_add_phantom_blank_line_after_terminal_newline() {
        // A body ending with `\n` should produce exactly one trailing newline,
        // not `  \n` (an indented empty line) after the last content line.
        let result = indent_body("one\ntwo\n", "  ");
        assert_eq!(result, "  one\n  two\n");
        assert!(
            !result.ends_with("  \n"),
            "phantom indented blank line must not appear"
        );
    }

    // -----------------------------------------------------------------------
    // Styled presenter tests
    // -----------------------------------------------------------------------

    #[test]
    fn styled_visible_text_matches_plain_layout() {
        let record = RetrievalRecord {
            id: "billing.policy".to_string(),
            kind: "decision".to_string(),
            status: Some("accepted".to_string()),
            owner: Some("architecture".to_string()),
            verified_at: None,
            body: "Refund policy is ledger-backed.\nManual credits are exceptions.".to_string(),
            source: RetrievalSource {
                path: "docs/decisions.adoc".to_string(),
                line: 7,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::from([
                ("scope".to_string(), "refunds".to_string()),
                ("decided_by".to_string(), "architecture".to_string()),
            ]),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let view = view_for(record);
        let text = strip_ansi(&render(&view));

        assert_eq!(
            text,
            concat!(
                "Object: billing.policy\n",
                "Kind: decision\n",
                "Status: [accepted]\n",
                "Owner: architecture\n",
                "\n",
                "Statement:\n",
                "  Refund policy is ledger-backed.\n",
                "  Manual credits are exceptions.\n",
                "\n",
                "Fields:\n",
                "- decided_by: architecture\n",
                "- scope: refunds\n",
                "\n",
                "Source: docs/decisions.adoc:7:1\n",
            )
        );
    }

    #[test]
    fn styled_uses_severity_label_for_warnings() {
        let mut record = make_record("team.warn", "warning");
        record.status = Some("high".to_string());
        let view = view_for(record);
        let text = strip_ansi(&render(&view));

        assert!(text.contains("Severity: [high]"));
        assert!(!text.contains("Status:"));
    }

    #[test]
    fn styled_status_line_contains_ansi_codes() {
        let mut record = make_record("billing.claim", "claim");
        record.status = Some("verified".to_string());
        let view = view_for(record);
        let raw = render(&view);

        // Raw output must contain at least one ANSI escape sequence.
        assert!(
            raw.contains('\x1b'),
            "expected ANSI escapes in styled output"
        );
        // Stripped output must not.
        let stripped = strip_ansi(&raw);
        assert!(!stripped.contains('\x1b'));
        assert!(stripped.contains("Status: [verified]"));
    }

    #[test]
    fn styled_section_headers_contain_ansi_faint_codes() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            owner: None,
            verified_at: None,
            body: "Credits decrement.".to_string(),
            source: RetrievalSource {
                path: "docs/billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            evidence: BTreeMap::from([("source".to_string(), "ledger".to_string())]),
            fields: BTreeMap::from([("scope".to_string(), "credits".to_string())]),
            relations: AgentJsonRelations {
                depends_on: vec!["billing.ledger".to_string()],
                supersedes: vec![],
                related_to: vec![],
            },
            search_match: None,
        };
        let view = view_for(record);
        let raw = render(&view);

        // ANSI faint is ESC[2m.  All three section headers must carry it.
        let faint_evidence = "\x1b[2mEvidence:\x1b[0m";
        let faint_fields = "\x1b[2mFields:\x1b[0m";
        let faint_relations = "\x1b[2mRelations:\x1b[0m";
        assert!(
            raw.contains(faint_evidence),
            "Evidence: header must be faint; raw={raw:?}"
        );
        assert!(
            raw.contains(faint_fields),
            "Fields: header must be faint; raw={raw:?}"
        );
        assert!(
            raw.contains(faint_relations),
            "Relations: header must be faint; raw={raw:?}"
        );

        // Stripped text must still read as plain layout.
        let stripped = strip_ansi(&raw);
        assert!(stripped.contains("Evidence:\n- source: ledger\n"));
        assert!(stripped.contains("Fields:\n- scope: credits\n"));
        assert!(stripped.contains("Relations:\n- depends_on: billing.ledger\n"));
    }
}

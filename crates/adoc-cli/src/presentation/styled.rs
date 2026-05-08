use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::ExplainView;
use owo_colors::OwoColorize as _;

use super::plain::{has_evidence, has_fields, has_relations};
use super::port::ExplainPresenter;
use super::style::chip::status_chip;
use super::style::footer::render_footer;
use super::style::humanise;
use super::style::key::cyan_key;
use super::style::kv::faint_label;
use super::style::palette::status_color;
use super::style::relations::relation_chip;
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
        // Footer: one blank line, then the provenance line (styled=true).
        buf.push('\n');
        render_footer(&mut buf, &view.render_meta, true);
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
    match (&record.verified_at, &view.expires) {
        (Some(verified_at), Some(info)) => {
            let coloured_paren = coloured_paren(info);
            writeln!(
                output,
                "{} {verified_at} · expires {} {coloured_paren}",
                faint_label("Verified:"),
                info.date
            )
            .expect("writing to String cannot fail");
        }
        (Some(verified_at), None) => {
            writeln!(output, "{} {verified_at}", faint_label("Verified:"))
                .expect("writing to String cannot fail");
        }
        (None, Some(info)) => {
            let coloured_paren = coloured_paren(info);
            writeln!(
                output,
                "{} {} {coloured_paren}",
                faint_label("Expires:"),
                info.date
            )
            .expect("writing to String cannot fail");
        }
        (None, None) => {}
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
        // Known keys rendered first in a fixed order, then any remaining keys
        // in BTreeMap (alphabetical) order — mirrors plain::evidence_items.
        let known = ["source", "test", "reviewed_by"];
        for field in known {
            if let Some(value) = record.evidence.get(field) {
                writeln!(output, "- {}: {value}", cyan_key(field))
                    .expect("writing to String cannot fail");
            }
        }
        for (field, value) in &record.evidence {
            if !known.contains(&field.as_str()) {
                writeln!(output, "- {}: {value}", cyan_key(field))
                    .expect("writing to String cannot fail");
            }
        }
    }

    if has_fields(record) {
        output.push('\n');
        writeln!(output, "{}", faint_label("Fields:")).expect("writing to String cannot fail");
        // BTreeMap iterates in sorted (alphabetical) key order — mirrors
        // plain::fields_items.
        for (field, value) in &record.fields {
            writeln!(output, "- {}: {value}", cyan_key(field))
                .expect("writing to String cannot fail");
        }
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
        let kinds: [(&str, &[String]); 3] = [
            ("depends_on", &record.relations.depends_on),
            ("supersedes", &record.relations.supersedes),
            ("related_to", &record.relations.related_to),
        ];
        for (kind, targets) in kinds {
            let cyan_kind = cyan_key(kind);
            for target in targets {
                let status_opt: Option<&str> = view
                    .related_statuses
                    .get(target)
                    .and_then(|opt| opt.as_deref());
                let palette = status_color(status_opt);
                if let Some(chip) = relation_chip(palette) {
                    writeln!(output, "- {cyan_kind}: {target} {chip}")
                        .expect("writing to String cannot fail");
                } else {
                    writeln!(output, "- {cyan_kind}: {target}")
                        .expect("writing to String cannot fail");
                }
            }
        }
    }
}

/// Format the expires parenthetical for `info`, colouring it red when the
/// record has already expired (`days_until < 0`).
fn coloured_paren(info: &adoc_core::ExpiresInfo) -> String {
    let humanised = humanise::format_diff(info.days_until);
    let paren = format!("({humanised})");
    if info.days_until < 0 {
        paren.red().to_string()
    } else {
        paren
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
    use std::path::PathBuf;
    use std::time::Duration;

    use adoc_core::{
        AgentJsonRelations, ExpiresInfo, ExplainView, RenderMeta, RetrievalRecord, RetrievalSource,
    };
    use chrono::NaiveDate;

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

    fn default_meta() -> RenderMeta {
        RenderMeta {
            artifact: PathBuf::from("docs.agent.json"),
            trust: None,
            duration: Duration::ZERO,
        }
    }

    fn view_for(record: RetrievalRecord) -> ExplainView {
        ExplainView {
            record,
            related_statuses: BTreeMap::new(),
            expires: None,
            render_meta: default_meta(),
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
                "\n",
                "✓ rendered from docs.agent.json · 0.00s\n",
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

    // -----------------------------------------------------------------------
    // Prove `highlight` runs in the styled pipeline: a body with [[…]] must
    // emit the cyan escape sequence in the raw (un-stripped) output.
    // Deleting the `highlight` call from `render_styled` would break this test.
    // -----------------------------------------------------------------------
    #[test]
    fn styled_body_passes_through_wikilink_highlight() {
        let mut record = make_record("billing.policy", "claim");
        record.body = "See [[billing.ledger]] for details.\n".to_string();
        let view = view_for(record);
        let raw = render(&view);
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mbilling.ledger\u{1b}[39m"),
            "expected bright-cyan-wrapped id in styled output, got: {:?}",
            raw
        );
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

    // -----------------------------------------------------------------------
    // Expires rendering tests (slice 6)
    // -----------------------------------------------------------------------

    fn expires_info(date: NaiveDate, days_until: i64) -> ExpiresInfo {
        ExpiresInfo { date, days_until }
    }

    #[test]
    fn styled_renders_verified_and_expires_suffix_when_both_present() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            88,
        ));
        let raw = render(&view);
        let stripped = strip_ansi(&raw);
        assert!(
            stripped.contains("Verified: 2026-05-06 · expires 2026-08-04 (in 88d)\n"),
            "expected combined verified+expires line in stripped output, got: {stripped:?}"
        );
    }

    #[test]
    fn styled_renders_standalone_expires_line_when_only_expires_present() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            88,
        ));
        let raw = render(&view);
        let stripped = strip_ansi(&raw);
        assert!(
            stripped.contains("Expires: 2026-08-04 (in 88d)\n"),
            "expected standalone expires line, got: {stripped:?}"
        );
        assert!(
            !stripped.contains("Verified:"),
            "should not contain Verified: label when verified_at absent"
        );
    }

    #[test]
    fn styled_renders_expired_parenthetical_in_red() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            -8,
        ));
        let raw = render(&view);
        // The parenthetical "(8d ago)" must be wrapped in red ANSI: ESC[31m…ESC[39m
        assert!(
            raw.contains("\u{1b}[31m(8d ago)\u{1b}[39m"),
            "expired parenthetical must be rendered in red; raw={raw:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Relation chip tests (slice 7)
    // -----------------------------------------------------------------------

    #[test]
    fn styled_relations_renders_contradicted_chip_after_supersedes_target() {
        let mut record = make_record("billing.credits", "claim");
        record.relations.supersedes = vec!["billing.credits.old-rule".to_string()];
        let mut view = view_for(record);
        view.related_statuses.insert(
            "billing.credits.old-rule".to_string(),
            Some("contradicted".to_string()),
        );
        let out = render(&view);
        // The CONTRADICTED chip must appear in the raw output after the target.
        assert!(
            out.contains("billing.credits.old-rule \u{1b}[30;41m[CONTRADICTED]\u{1b}[0m"),
            "expected CONTRADICTED chip after supersedes target, got: {out:?}"
        );
        // The visible text (stripped) must contain the target id on the line.
        let stripped = strip_ansi(&out);
        assert!(
            stripped.contains("- supersedes: billing.credits.old-rule"),
            "stripped output must show supersedes line, got: {stripped:?}"
        );
    }

    #[test]
    fn styled_relations_omits_chip_for_verified_target() {
        let mut record = make_record("billing.credits", "claim");
        record.relations.depends_on = vec!["billing.credits.ledger".to_string()];
        let mut view = view_for(record);
        view.related_statuses.insert(
            "billing.credits.ledger".to_string(),
            Some("verified".to_string()),
        );
        let out = render(&view);
        assert!(!out.contains("[CONTRADICTED]"));
        assert!(!out.contains("[DEPRECATED]"));
        // Check visible text via stripped output (kind is cyan in raw).
        let stripped = strip_ansi(&out);
        assert!(stripped.contains("- depends_on: billing.credits.ledger"));
    }

    #[test]
    fn styled_relations_omits_chip_for_unknown_target() {
        let mut record = make_record("billing.credits", "claim");
        record.relations.related_to = vec!["billing.credits.ghost".to_string()];
        let mut view = view_for(record);
        view.related_statuses
            .insert("billing.credits.ghost".to_string(), None);
        let out = render(&view);
        assert!(!out.contains("[CONTRADICTED]"));
        assert!(!out.contains("[DEPRECATED]"));
        // Check visible text via stripped output (kind is cyan in raw).
        let stripped = strip_ansi(&out);
        assert!(stripped.contains("- related_to: billing.credits.ghost"));
    }

    #[test]
    fn styled_renders_no_verified_or_expires_line_when_neither_present() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = None;
        let view = view_for(record);
        let stripped = strip_ansi(&render(&view));
        assert!(
            !stripped.contains("Verified:"),
            "expected no Verified line, got: {stripped:?}"
        );
        assert!(
            !stripped.contains("Expires:"),
            "expected no Expires line, got: {stripped:?}"
        );
    }

    #[test]
    fn styled_renders_future_expires_parenthetical_in_default_colour() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            88,
        ));
        let raw = render(&view);
        // Future expires must NOT contain red escape around the parenthetical.
        assert!(
            !raw.contains("\u{1b}[31m(in 88d)\u{1b}[39m"),
            "future parenthetical must not be in red; raw={raw:?}"
        );
        // The stripped text must still contain the parenthetical.
        let stripped = strip_ansi(&raw);
        assert!(stripped.contains("(in 88d)"));
    }

    // -----------------------------------------------------------------------
    // Footer rendering tests (slice 8)
    // -----------------------------------------------------------------------

    #[test]
    fn styled_footer_check_glyph_is_green() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.render_meta = RenderMeta {
            artifact: PathBuf::from("/tmp/adoc-retrieval-dist/docs.agent.json"),
            trust: Some("team".to_string()),
            duration: Duration::from_millis(60),
        };
        let raw = render(&view);
        // owo_colors emits ESC[32m for green fg and ESC[39m to reset.
        assert!(
            raw.contains("\u{1b}[32m✓\u{1b}[39m rendered from docs.agent.json"),
            "styled footer must have green ✓; got: {raw:?}"
        );
    }

    #[test]
    fn styled_footer_visible_text_contains_trust_and_duration() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.render_meta = RenderMeta {
            artifact: PathBuf::from("/tmp/docs.agent.json"),
            trust: Some("team".to_string()),
            duration: Duration::from_millis(60),
        };
        let stripped = strip_ansi(&render(&view));
        assert!(
            stripped.ends_with("\n✓ rendered from docs.agent.json · trust: team · 0.06s\n"),
            "stripped styled footer must match plain shape; got: {stripped:?}"
        );
    }

    #[test]
    fn styled_footer_preceded_by_blank_line() {
        let record = make_record("billing.credits", "claim");
        let view = view_for(record);
        let stripped = strip_ansi(&render(&view));
        assert!(
            stripped.contains("\n\n✓"),
            "footer must be preceded by a blank line, got: {stripped:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Slice 9: cyan keys for evidence, fields, and relation kinds
    // -----------------------------------------------------------------------

    /// The per-item key names in the Evidence section must be wrapped in
    /// truecolour cyan ANSI codes (`ESC[38;2;100;220;255m…ESC[39m`) in styled mode.
    #[test]
    fn styled_evidence_keys_render_in_cyan() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            owner: None,
            verified_at: None,
            body: "Credits.".to_string(),
            source: RetrievalSource {
                path: "docs/billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            evidence: BTreeMap::from([
                ("source".to_string(), "ledger".to_string()),
                ("test".to_string(), "cargo test credits".to_string()),
                ("reviewed_by".to_string(), "risk".to_string()),
            ]),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let view = view_for(record);
        let raw = render(&view);

        // Each known key must appear cyan-wrapped followed by colon.
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255msource\u{1b}[39m:"),
            "evidence 'source' key must be cyan; raw={raw:?}"
        );
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mtest\u{1b}[39m:"),
            "evidence 'test' key must be cyan; raw={raw:?}"
        );
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mreviewed_by\u{1b}[39m:"),
            "evidence 'reviewed_by' key must be cyan; raw={raw:?}"
        );

        // Visible text (stripped) must preserve the plain layout (no extra indent).
        let stripped = strip_ansi(&raw);
        assert!(stripped.contains("- source: ledger\n"));
        assert!(stripped.contains("- test: cargo test credits\n"));
        assert!(stripped.contains("- reviewed_by: risk\n"));
    }

    /// Custom evidence keys (not in the known set) must also be cyan-wrapped.
    #[test]
    fn styled_fields_keys_render_in_cyan() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "glossary".to_string(),
            status: None,
            owner: None,
            verified_at: None,
            body: "Credits are balance units.".to_string(),
            source: RetrievalSource {
                path: "docs/glossary.adoc".to_string(),
                line: 4,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::from([("canonical".to_string(), "billing credit".to_string())]),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let view = view_for(record);
        let raw = render(&view);

        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mcanonical\u{1b}[39m:"),
            "fields 'canonical' key must be cyan; raw={raw:?}"
        );

        // Visible text preserved.
        let stripped = strip_ansi(&raw);
        assert!(stripped.contains("- canonical: billing credit\n"));
    }

    /// All three relation kind names must be rendered in cyan.
    #[test]
    fn styled_relations_kinds_render_in_cyan() {
        let record = RetrievalRecord {
            id: "billing.policy".to_string(),
            kind: "decision".to_string(),
            status: Some("accepted".to_string()),
            owner: None,
            verified_at: None,
            body: "Policy body.".to_string(),
            source: RetrievalSource {
                path: "docs/decisions.adoc".to_string(),
                line: 7,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations {
                depends_on: vec!["billing.credits.ledger-source".to_string()],
                supersedes: vec!["billing.refunds.manual-credit".to_string()],
                related_to: vec!["billing.credits.decrement-after-success".to_string()],
            },
            search_match: None,
        };
        let view = view_for(record);
        let raw = render(&view);

        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mdepends_on\u{1b}[39m:"),
            "relation kind 'depends_on' must be cyan; raw={raw:?}"
        );
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255msupersedes\u{1b}[39m:"),
            "relation kind 'supersedes' must be cyan; raw={raw:?}"
        );
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mrelated_to\u{1b}[39m:"),
            "relation kind 'related_to' must be cyan; raw={raw:?}"
        );

        // Visible layout preserved.
        let stripped = strip_ansi(&raw);
        assert!(stripped.contains("- depends_on: billing.credits.ledger-source\n"));
        assert!(stripped.contains("- supersedes: billing.refunds.manual-credit\n"));
        assert!(stripped.contains("- related_to: billing.credits.decrement-after-success\n"));
    }

    /// Cyan kind name must be rendered independently of the CONTRADICTED chip:
    /// a target that is NOT contradicted (so no chip is appended) must still
    /// have its kind rendered in cyan.
    #[test]
    fn styled_no_chip_for_verified_target_still_uses_cyan_kind() {
        let mut record = make_record("billing.credits", "claim");
        record.relations.depends_on = vec!["billing.credits.ledger".to_string()];
        let mut view = view_for(record);
        view.related_statuses.insert(
            "billing.credits.ledger".to_string(),
            Some("verified".to_string()),
        );
        let raw = render(&view);

        // No chip.
        assert!(!raw.contains("[CONTRADICTED]"));
        // But kind is still cyan.
        assert!(
            raw.contains("\u{1b}[38;2;100;220;255mdepends_on\u{1b}[39m:"),
            "kind must be cyan even without a chip; raw={raw:?}"
        );
    }
}

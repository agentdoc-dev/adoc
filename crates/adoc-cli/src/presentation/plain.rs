use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::{AgentJsonRelations, RetrievalRecord};
use adoc_core::{ExpiresInfo, ExplainView};

use super::port::ExplainPresenter;
use super::style::footer::render_footer;
use super::style::humanise::format_diff;

/// Plain-text presenter.  Produces the same byte-for-byte output as the
/// former `TextRetrievalFormatter` in `adoc-core`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PlainPresenter;

impl ExplainPresenter for PlainPresenter {
    fn present(&self, view: &ExplainView, out: &mut dyn io::Write) -> io::Result<()> {
        let mut buf = String::new();
        render_record(&mut buf, &view.record, view.expires.as_ref());
        // Footer: one blank line, then the provenance line.
        buf.push('\n');
        render_footer(&mut buf, &view.render_meta, false);
        out.write_all(buf.as_bytes())
    }
}

/// Renders a single [`RetrievalRecord`] as plain text into `output`.
///
/// Shared between [`PlainPresenter`] (single-record explain path) and the
/// search command (multi-record path) so that both callers produce identical
/// bytes.
///
/// `expires` is `Some` when the record's `fields["expires_at"]` was parseable
/// as a `YYYY-MM-DD` date (populated by [`adoc_core::ExplainService`]).  Pass
/// `None` for the search path where no expiry computation is performed.
pub(crate) fn render_record(
    output: &mut String,
    record: &RetrievalRecord,
    expires: Option<&ExpiresInfo>,
) {
    writeln!(output, "Object: {}", record.id).expect("writing to String cannot fail");
    writeln!(output, "Kind: {}", record.kind).expect("writing to String cannot fail");
    if let Some(status) = &record.status {
        if record.kind == "warning" {
            writeln!(output, "Severity: {status}").expect("writing to String cannot fail");
        } else {
            writeln!(output, "Status: {status}").expect("writing to String cannot fail");
        }
    }
    if let Some(owner) = &record.owner {
        writeln!(output, "Owner: {owner}").expect("writing to String cannot fail");
    }
    match (&record.verified_at, expires) {
        (Some(verified_at), Some(info)) => {
            let humanised = format_diff(info.days_until);
            writeln!(
                output,
                "Verified: {verified_at} · expires {} ({humanised})",
                info.date
            )
            .expect("writing to String cannot fail");
        }
        (Some(verified_at), None) => {
            writeln!(output, "Verified: {verified_at}").expect("writing to String cannot fail");
        }
        (None, Some(info)) => {
            let humanised = format_diff(info.days_until);
            writeln!(output, "Expires: {} ({humanised})", info.date)
                .expect("writing to String cannot fail");
        }
        (None, None) => {}
    }

    output.push('\n');
    output.push_str("Statement:\n");
    output.push_str(&record.body);
    if !record.body.ends_with('\n') {
        output.push('\n');
    }

    if has_evidence(record) {
        output.push('\n');
        output.push_str("Evidence:\n");
        evidence_items(output, record);
    }

    if has_fields(record) {
        output.push('\n');
        output.push_str("Fields:\n");
        fields_items(output, record);
    }

    output.push('\n');
    writeln!(
        output,
        "Source: {}:{}:{}",
        record.source.path, record.source.line, record.source.column
    )
    .expect("writing to String cannot fail");

    if has_relations(&record.relations) {
        output.push('\n');
        output.push_str("Relations:\n");
        relations_items(output, &record.relations);
    }
}

// ---------------------------------------------------------------------------
// Section predicates — callers use these to decide whether to emit a header.
// ---------------------------------------------------------------------------

/// Returns `true` when the record has at least one evidence entry.
pub(crate) fn has_evidence(record: &RetrievalRecord) -> bool {
    !record.evidence.is_empty()
}

/// Returns `true` when the record has at least one custom field entry.
pub(crate) fn has_fields(record: &RetrievalRecord) -> bool {
    !record.fields.is_empty()
}

/// Returns `true` when the record has at least one relation in any category.
pub(crate) fn has_relations(relations: &AgentJsonRelations) -> bool {
    !relations.depends_on.is_empty()
        || !relations.supersedes.is_empty()
        || !relations.related_to.is_empty()
}

// ---------------------------------------------------------------------------
// Section body helpers — emit only the list items, no leading blank or header.
// ---------------------------------------------------------------------------

/// Appends evidence list items to `output`.  Does not emit a leading blank
/// line or the `Evidence:` header; the caller owns those.
pub(crate) fn evidence_items(output: &mut String, record: &RetrievalRecord) {
    let evidence_fields = ["source", "test", "reviewed_by"];
    for field in evidence_fields {
        if let Some(value) = record.evidence.get(field) {
            writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
        }
    }
    for (field, value) in &record.evidence {
        if !evidence_fields.contains(&field.as_str()) {
            writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
        }
    }
}

/// Appends fields list items to `output`.  Does not emit a leading blank line
/// or the `Fields:` header; the caller owns those.
pub(crate) fn fields_items(output: &mut String, record: &RetrievalRecord) {
    for (field, value) in &record.fields {
        writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
    }
}

/// Appends relation list items to `output`.  Does not emit a leading blank
/// line or the `Relations:` header; the caller owns those.
pub(crate) fn relations_items(output: &mut String, relations: &AgentJsonRelations) {
    render_relation_targets(output, "depends_on", &relations.depends_on);
    render_relation_targets(output, "supersedes", &relations.supersedes);
    render_relation_targets(output, "related_to", &relations.related_to);
}

fn render_relation_targets(output: &mut String, relation: &str, targets: &[String]) {
    for target in targets {
        writeln!(output, "- {relation}: {target}").expect("writing to String cannot fail");
    }
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
        PlainPresenter.present(view, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn plain_presenter_renders_record() {
        let record = make_record("team.id", "claim");
        let view = view_for(record);
        let text = render(&view);

        assert!(text.contains("Object: team.id"));
        assert!(text.contains("Kind: claim"));
        assert!(text.contains("Statement:\nBody text."));
        assert!(text.contains("Source: docs/test.adoc:1:1"));
    }

    #[test]
    fn plain_presenter_uses_severity_label_for_warnings() {
        let mut record = make_record("team.warn", "warning");
        record.status = Some("high".to_string());
        let view = view_for(record);
        let text = render(&view);

        assert!(text.contains("Severity: high"));
        assert!(!text.contains("Status:"));
    }

    #[test]
    fn plain_presenter_renders_statement_body_and_sorted_fields() {
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
        let text = render(&view);

        assert_eq!(
            text,
            concat!(
                "Object: billing.policy\n",
                "Kind: decision\n",
                "Status: accepted\n",
                "Owner: architecture\n",
                "\n",
                "Statement:\n",
                "Refund policy is ledger-backed.\n",
                "Manual credits are exceptions.\n",
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
    fn plain_presenter_renders_each_relation_target_on_its_own_line() {
        let record = RetrievalRecord {
            id: "billing.policy".to_string(),
            kind: "decision".to_string(),
            status: Some("accepted".to_string()),
            owner: None,
            verified_at: None,
            body: "Refund policy is ledger-backed.".to_string(),
            source: RetrievalSource {
                path: "docs/decisions.adoc".to_string(),
                line: 7,
                column: 1,
            },
            evidence: BTreeMap::new(),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations {
                depends_on: vec![
                    "billing.credits.ledger-source".to_string(),
                    "billing.refunds.audit-required".to_string(),
                ],
                supersedes: vec![
                    "billing.refunds.manual-credit".to_string(),
                    "billing.refunds.email-approval".to_string(),
                ],
                related_to: vec![
                    "billing.credits.decrement-after-success".to_string(),
                    "billing.credits.reconciliation".to_string(),
                ],
            },
            search_match: None,
        };
        let view = view_for(record);
        let text = render(&view);
        let relations = text
            .split_once("Relations:\n")
            .expect("relations block is rendered")
            .1;

        assert_eq!(
            relations,
            concat!(
                "- depends_on: billing.credits.ledger-source\n",
                "- depends_on: billing.refunds.audit-required\n",
                "- supersedes: billing.refunds.manual-credit\n",
                "- supersedes: billing.refunds.email-approval\n",
                "- related_to: billing.credits.decrement-after-success\n",
                "- related_to: billing.credits.reconciliation\n",
                "\n",
                "✓ rendered from docs.agent.json · 0.00s\n",
            )
        );
    }

    #[test]
    fn plain_presenter_renders_glossary_kind_metadata() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "glossary".to_string(),
            status: None,
            owner: None,
            verified_at: None,
            body: "Credits are account balance units.".to_string(),
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
        let text = render(&view);

        assert!(text.contains("Kind: glossary\n"));
        assert!(text.contains("Fields:\n- canonical: billing credit\n"));
    }

    #[test]
    fn plain_presenter_renders_unknown_evidence_keys_after_known_order() {
        let record = RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            owner: None,
            verified_at: None,
            body: "Credits decrement after successful payment.".to_string(),
            source: RetrievalSource {
                path: "docs/billing.adoc".to_string(),
                line: 9,
                column: 1,
            },
            evidence: BTreeMap::from([
                ("artifact".to_string(), "ledger.csv".to_string()),
                ("reviewed_by".to_string(), "risk".to_string()),
                ("source".to_string(), "ledger".to_string()),
                ("z_probe".to_string(), "trace".to_string()),
            ]),
            fields: BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        };
        let view = view_for(record);
        let text = render(&view);

        assert!(text.contains(concat!(
            "Evidence:\n",
            "- source: ledger\n",
            "- reviewed_by: risk\n",
            "- artifact: ledger.csv\n",
            "- z_probe: trace\n",
        )));
    }

    // -----------------------------------------------------------------------
    // Expires rendering tests (slice 6)
    // -----------------------------------------------------------------------

    fn expires_info(date: NaiveDate, days_until: i64) -> ExpiresInfo {
        ExpiresInfo { date, days_until }
    }

    #[test]
    fn plain_renders_verified_and_expires_suffix_when_both_present() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            88,
        ));
        let text = render(&view);
        assert!(
            text.contains("Verified: 2026-05-06 · expires 2026-08-04 (in 88d)\n"),
            "expected combined verified+expires line, got: {text:?}"
        );
    }

    #[test]
    fn plain_renders_only_verified_when_expires_absent() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let view = view_for(record);
        let text = render(&view);
        assert!(
            text.contains("Verified: 2026-05-06\n"),
            "expected bare verified line, got: {text:?}"
        );
        assert!(
            !text.contains("expires"),
            "should not contain expires when absent"
        );
    }

    #[test]
    fn plain_renders_standalone_expires_line_when_only_expires_present() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 8, 4).unwrap(),
            88,
        ));
        let text = render(&view);
        assert!(
            text.contains("Expires: 2026-08-04 (in 88d)\n"),
            "expected standalone expires line, got: {text:?}"
        );
        assert!(
            !text.contains("Verified:"),
            "should not contain Verified: when verified_at absent"
        );
    }

    #[test]
    fn plain_renders_no_expires_line_when_neither_present() {
        let record = make_record("billing.credits", "claim");
        let view = view_for(record);
        let text = render(&view);
        assert!(
            !text.contains("Verified:"),
            "should not contain Verified: line"
        );
        assert!(
            !text.contains("Expires:"),
            "should not contain Expires: line"
        );
    }

    #[test]
    fn plain_renders_expired_date_with_ago_suffix() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 4, 30).unwrap(),
            -8,
        ));
        let text = render(&view);
        assert!(
            text.contains("Verified: 2026-05-06 · expires 2026-04-30 (8d ago)\n"),
            "expected expired date with ago suffix, got: {text:?}"
        );
    }

    #[test]
    fn plain_renders_expires_today() {
        let mut record = make_record("billing.credits", "claim");
        record.verified_at = Some("2026-05-06".to_string());
        let mut view = view_for(record);
        view.expires = Some(expires_info(
            NaiveDate::from_ymd_opt(2026, 5, 8).unwrap(),
            0,
        ));
        let text = render(&view);
        assert!(
            text.contains("Verified: 2026-05-06 · expires 2026-05-08 (today)\n"),
            "expected today expiry, got: {text:?}"
        );
    }

    // -----------------------------------------------------------------------
    // Footer rendering tests (slice 8)
    // -----------------------------------------------------------------------

    #[test]
    fn plain_footer_emits_check_basename_and_duration() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.render_meta = RenderMeta {
            artifact: PathBuf::from("/tmp/adoc-retrieval-dist/docs.agent.json"),
            trust: Some("team".to_string()),
            duration: Duration::from_millis(60),
        };
        let text = render(&view);
        assert!(
            text.ends_with("\n✓ rendered from docs.agent.json · trust: team · 0.06s\n"),
            "plain footer line must end the output with trust and duration, got: {text:?}"
        );
    }

    #[test]
    fn plain_footer_omits_trust_when_absent() {
        let record = make_record("billing.credits", "claim");
        let mut view = view_for(record);
        view.render_meta = RenderMeta {
            artifact: PathBuf::from("/tmp/docs.agent.json"),
            trust: None,
            duration: Duration::from_millis(60),
        };
        let text = render(&view);
        assert!(
            text.ends_with("\n✓ rendered from docs.agent.json · 0.06s\n"),
            "plain footer must omit trust segment when trust is None, got: {text:?}"
        );
        assert!(
            !text.contains("trust:"),
            "footer must not contain 'trust:' when trust is None"
        );
    }

    #[test]
    fn plain_footer_preceded_by_blank_line() {
        let record = make_record("billing.credits", "claim");
        let view = view_for(record);
        let text = render(&view);
        assert!(
            text.contains("\n\n✓"),
            "footer must be preceded by a blank line, got: {text:?}"
        );
    }
}

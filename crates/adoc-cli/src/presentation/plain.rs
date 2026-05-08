use std::fmt::Write as FmtWrite;
use std::io;

use adoc_core::RetrievalEnvelope;
use adoc_core::{AgentJsonRelations, RetrievalRecord};

use super::port::ExplainPresenter;

/// Plain-text presenter.  Produces the same byte-for-byte output as the
/// former `TextRetrievalFormatter` in `adoc-core`.
#[derive(Debug, Clone, Copy, Default)]
pub(crate) struct PlainPresenter;

impl ExplainPresenter for PlainPresenter {
    fn present(&self, envelope: &RetrievalEnvelope, out: &mut dyn io::Write) -> io::Result<()> {
        let mut buf = String::new();

        for (index, record) in envelope.records.iter().enumerate() {
            if index > 0 {
                buf.push('\n');
            }
            render_record(&mut buf, record);
        }

        out.write_all(buf.as_bytes())
    }
}

fn render_record(output: &mut String, record: &RetrievalRecord) {
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
    if let Some(verified_at) = &record.verified_at {
        writeln!(output, "Verified: {verified_at}").expect("writing to String cannot fail");
    }

    output.push('\n');
    output.push_str("Statement:\n");
    output.push_str(&record.body);
    if !record.body.ends_with('\n') {
        output.push('\n');
    }

    render_evidence(output, record);
    render_fields(output, record);

    output.push('\n');
    writeln!(
        output,
        "Source: {}:{}:{}",
        record.source.path, record.source.line, record.source.column
    )
    .expect("writing to String cannot fail");

    render_relations(output, &record.relations);
}

fn render_evidence(output: &mut String, record: &RetrievalRecord) {
    let evidence_fields = ["source", "test", "reviewed_by"];
    if record.evidence.is_empty() {
        return;
    }

    output.push('\n');
    output.push_str("Evidence:\n");
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

fn render_fields(output: &mut String, record: &RetrievalRecord) {
    if record.fields.is_empty() {
        return;
    }

    output.push('\n');
    output.push_str("Fields:\n");
    for (field, value) in &record.fields {
        writeln!(output, "- {field}: {value}").expect("writing to String cannot fail");
    }
}

fn render_relations(output: &mut String, relations: &AgentJsonRelations) {
    if relations.depends_on.is_empty()
        && relations.supersedes.is_empty()
        && relations.related_to.is_empty()
    {
        return;
    }

    output.push('\n');
    output.push_str("Relations:\n");
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

    use adoc_core::{AgentJsonRelations, RetrievalEnvelope, RetrievalRecord, RetrievalSource};

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

    fn render(envelope: &RetrievalEnvelope) -> String {
        let mut buf = Vec::new();
        PlainPresenter.present(envelope, &mut buf).unwrap();
        String::from_utf8(buf).unwrap()
    }

    #[test]
    fn plain_presenter_renders_record() {
        let record = make_record("team.id", "claim");
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let text = render(&envelope);

        assert!(text.contains("Object: team.id"));
        assert!(text.contains("Kind: claim"));
        assert!(text.contains("Statement:\nBody text."));
        assert!(text.contains("Source: docs/test.adoc:1:1"));
    }

    #[test]
    fn plain_presenter_uses_severity_label_for_warnings() {
        let mut record = make_record("team.warn", "warning");
        record.status = Some("high".to_string());
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let text = render(&envelope);

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
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let text = render(&envelope);

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
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let text = render(&envelope);
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
            )
        );
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
        let envelope = RetrievalEnvelope::new(vec![record], Vec::new());
        let text = render(&envelope);

        assert!(text.contains(concat!(
            "Evidence:\n",
            "- source: ledger\n",
            "- reviewed_by: risk\n",
            "- artifact: ledger.csv\n",
            "- z_probe: trace\n",
        )));
    }
}

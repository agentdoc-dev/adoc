use std::path::PathBuf;

use adoc_core::{
    AgentJsonRelations, DiagnosticCode, JsonRetrievalFormatter, RetrievalEnvelope,
    RetrievalFormatter, RetrievalInput, RetrievalRecord, RetrievalSource, TextRetrievalFormatter,
    explain_object, load_retrieval_session,
};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn write_temp_artifact(name: &str, contents: &str) -> PathBuf {
    let path = std::env::temp_dir().join(format!(
        "adoc-retrieval-{name}-{}.agent.json",
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .expect("clock is after epoch")
            .as_nanos()
    ));
    std::fs::write(&path, contents).expect("temp artifact can be written");
    path
}

#[test]
fn explain_object_returns_record_for_id_in_loaded_agent_artifact() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean load, got {:?}",
        result.diagnostics
    );
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.verified-credits");

    assert!(
        explained.diagnostics.is_empty(),
        "expected clean explain, got {:?}",
        explained.diagnostics
    );
    assert_eq!(explained.records.len(), 1);
    let record = &explained.records[0];
    assert_eq!(record.id, "billing.verified-credits");
    assert_eq!(record.kind, "claim");
    assert_eq!(record.status.as_deref(), Some("verified"));
    assert_eq!(record.owner.as_deref(), Some("team-billing"));
    assert_eq!(record.verified_at.as_deref(), Some("2026-05-05"));
    assert_eq!(
        record.source.path,
        "tests/fixtures/claim/valid_verified_claim_with_all_evidence/input.adoc"
    );
    assert_eq!(
        record.evidence.get("source").map(String::as_str),
        Some("payments ledger")
    );
}

#[test]
fn explain_object_serializes_record_without_search_match_block() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.verified-credits");
    let value = serde_json::to_value(&explained.records[0]).expect("record serializes");

    assert!(value.get("match").is_none());
    assert!(value.get("retrieval").is_none());
}

#[test]
fn retrieval_envelope_serializes_stable_schema_with_records_and_diagnostics() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");
    let explained = explain_object(&session, "billing.verified-credits");

    let value =
        serde_json::to_value(RetrievalEnvelope::from(explained)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"][0]["id"], "billing.verified-credits");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert!(value["records"][0].get("match").is_none());
    assert!(value["records"][0].get("retrieval").is_none());
}

#[test]
fn explain_object_reports_unknown_id_without_loading_source() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.missing");

    assert!(explained.records.is_empty());
    assert_eq!(explained.diagnostics.len(), 1);
    assert_eq!(
        explained.diagnostics[0].code,
        DiagnosticCode::RetrievalObjectNotFound
    );
    assert_eq!(
        explained.diagnostics[0].object_id.as_deref(),
        Some("billing.missing")
    );
}

#[test]
fn load_retrieval_session_rejects_duplicate_object_ids_inside_artifact() {
    let path = write_temp_artifact(
        "duplicate",
        r#"{
          "schema_version": "adoc.agent.v0",
          "pages": [],
          "objects": [
            {
              "id": "billing.duplicate",
              "kind": "claim",
              "status": "draft",
              "body": "First.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 1, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            },
            {
              "id": "billing.duplicate",
              "kind": "claim",
              "status": "draft",
              "body": "Second.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 2, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            }
          ],
          "diagnostics": []
        }"#,
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: path.clone(),
    });

    std::fs::remove_file(path).expect("temp artifact removed");
    assert!(result.session.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::IdDuplicateInArtifact
    );
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("billing.duplicate")
    );
}

#[test]
fn text_retrieval_formatter_renders_statement_body_and_sorted_fields() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
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
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::from([
                ("scope".to_string(), "refunds".to_string()),
                ("decided_by".to_string(), "architecture".to_string()),
            ]),
            relations: AgentJsonRelations::default(),
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

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
fn text_retrieval_formatter_renders_glossary_kind_metadata() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
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
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::from([(
                "canonical".to_string(),
                "billing credit".to_string(),
            )]),
            relations: AgentJsonRelations::default(),
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

    assert!(text.contains("Kind: glossary\n"));
    assert!(text.contains("Fields:\n- canonical: billing credit\n"));
}

#[test]
fn text_retrieval_formatter_renders_unknown_evidence_keys_after_known_order() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
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
            evidence: std::collections::BTreeMap::from([
                ("artifact".to_string(), "ledger.csv".to_string()),
                ("reviewed_by".to_string(), "risk".to_string()),
                ("source".to_string(), "ledger".to_string()),
                ("z_probe".to_string(), "trace".to_string()),
            ]),
            fields: std::collections::BTreeMap::new(),
            relations: AgentJsonRelations::default(),
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

    assert!(text.contains(concat!(
        "Evidence:\n",
        "- source: ledger\n",
        "- reviewed_by: risk\n",
        "- artifact: ledger.csv\n",
        "- z_probe: trace\n",
    )));
}

#[test]
fn json_retrieval_formatter_preserves_envelope_shape() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
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
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::new(),
            relations: AgentJsonRelations::default(),
        }],
        Vec::new(),
    );

    let rendered = JsonRetrievalFormatter
        .render(&envelope)
        .expect("JSON retrieval render succeeds");
    let expected = serde_json::to_string_pretty(&envelope).expect("envelope serializes");

    assert_eq!(rendered, expected);
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("rendered JSON parses");
    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert!(value["records"][0].get("match").is_none());
    assert!(value["records"][0].get("retrieval").is_none());
}

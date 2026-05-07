use std::path::PathBuf;

use adoc_core::{
    AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan, DiagnosticCode,
    JsonRetrievalFormatter, RetrievalEnvelope, RetrievalFormatter, RetrievalInput, RetrievalMatch,
    RetrievalRecord, RetrievalSource, SearchFilters, SearchResult, TextRetrievalFormatter,
    explain_object, load_retrieval_session,
};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn write_temp_artifact(name: &str, contents: &str) -> tempfile::NamedTempFile {
    let artifact = tempfile::Builder::new()
        .prefix(&format!("adoc-retrieval-{name}-"))
        .suffix(".agent.json")
        .tempfile()
        .expect("temp artifact can be created");
    std::fs::write(artifact.path(), contents).expect("temp artifact can be written");
    artifact
}

fn retrieval_filter_object(
    id: &str,
    kind: &str,
    status: Option<&str>,
    owner: Option<&str>,
    source_path: &str,
) -> AgentJsonObject {
    let mut fields = std::collections::BTreeMap::new();
    if let Some(owner) = owner {
        fields.insert("owner".to_string(), owner.to_string());
    }

    AgentJsonObject {
        id: id.to_string(),
        kind: kind.to_string(),
        status: status.map(str::to_string),
        body: "Filter fixture body.".to_string(),
        page_id: "team.page".to_string(),
        source_span: AgentJsonSourceSpan {
            path: source_path.to_string(),
            line: 1,
            column: 1,
        },
        fields,
        relations: AgentJsonRelations::default(),
    }
}

#[test]
fn search_filter_matches_case_insensitive_substrings_on_object_metadata() {
    let object = retrieval_filter_object(
        "billing.verified-credits",
        "claim",
        Some("verified"),
        Some("Team-Billing"),
        "docs/billing/credits.adoc",
    );
    let filters = SearchFilters {
        kind: Some("CLA".to_string()),
        status: Some("VERI".to_string()),
        owner: Some("billing".to_string()),
        source_path: Some("CREDITS.ADOC".to_string()),
    };

    assert!(filters.matches(&object));
}

#[test]
fn search_filter_rejects_missing_status_or_owner_when_filter_is_supplied() {
    let object = retrieval_filter_object(
        "glossary.credit",
        "glossary",
        None,
        None,
        "docs/glossary.adoc",
    );

    assert!(
        !SearchFilters {
            kind: None,
            status: Some("verified".to_string()),
            owner: None,
            source_path: None,
        }
        .matches(&object)
    );
    assert!(
        !SearchFilters {
            kind: None,
            status: None,
            owner: Some("team".to_string()),
            source_path: None,
        }
        .matches(&object)
    );
}

#[test]
fn search_filter_validation_checks_each_supplied_filter_independently() {
    let objects = vec![
        retrieval_filter_object(
            "billing.claim",
            "claim",
            Some("draft"),
            Some("team-a"),
            "docs/billing.adoc",
        ),
        retrieval_filter_object(
            "architecture.decision",
            "decision",
            Some("accepted"),
            Some("team-b"),
            "docs/architecture.adoc",
        ),
    ];
    let filters = SearchFilters {
        kind: Some("claim".to_string()),
        status: None,
        owner: Some("team-b".to_string()),
        source_path: None,
    };

    assert!(filters.validate_against(&objects).is_empty());
    assert!(objects.iter().all(|object| !filters.matches(object)));
}

#[test]
fn search_filter_validation_reports_each_supplied_filter_with_no_independent_match() {
    let objects = vec![retrieval_filter_object(
        "billing.claim",
        "claim",
        Some("draft"),
        Some("team-a"),
        "docs/billing.adoc",
    )];
    let filters = SearchFilters {
        kind: Some("decision".to_string()),
        status: Some("verified".to_string()),
        owner: Some("team-b".to_string()),
        source_path: Some("architecture".to_string()),
    };

    let diagnostics = filters.validate_against(&objects);

    assert_eq!(diagnostics.len(), 4);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DiagnosticCode::SearchInvalidFilter)
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.help.as_deref()
                == Some(DiagnosticCode::SearchInvalidFilter.default_help()))
    );
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
fn retrieval_record_serializes_lexical_search_match_contract() {
    let object = AgentJsonObject {
        id: "billing.verified-credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        body: "Credits are verified.".to_string(),
        page_id: "team.billing".to_string(),
        source_span: AgentJsonSourceSpan {
            path: "billing.adoc".to_string(),
            line: 5,
            column: 1,
        },
        fields: std::collections::BTreeMap::new(),
        relations: AgentJsonRelations::default(),
    };

    let record = RetrievalRecord::from_object_with_match(&object, RetrievalMatch::lexical(1));
    let value = serde_json::to_value(&record).expect("record serializes");

    assert_eq!(
        value["match"],
        serde_json::json!({
            "mode": "lexical",
            "lexical_rank": 1
        })
    );
    assert!(value.get("retrieval").is_none());
}

#[test]
fn retrieval_envelope_can_be_created_from_search_result() {
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
        evidence: std::collections::BTreeMap::new(),
        fields: std::collections::BTreeMap::new(),
        relations: AgentJsonRelations::default(),
        search_match: Some(RetrievalMatch::lexical(1)),
    };
    let result = SearchResult {
        records: vec![record],
        diagnostics: Vec::new(),
    };

    let value = serde_json::to_value(RetrievalEnvelope::from(result)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"][0]["match"]["mode"], "lexical");
    assert_eq!(value["records"][0]["match"]["lexical_rank"], 1);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
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
    let artifact = write_temp_artifact(
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
        artifact_path: artifact.path().to_path_buf(),
    });

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
            search_match: None,
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
fn text_retrieval_formatter_renders_each_relation_target_on_its_own_line() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
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
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::new(),
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
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");
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
            search_match: None,
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
            search_match: None,
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
            search_match: None,
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

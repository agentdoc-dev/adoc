use std::path::PathBuf;

use adoc_core::{DiagnosticCode, RetrievalInput, explain_object, load_retrieval_session};

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

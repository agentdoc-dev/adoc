use adoc_core::{
    SourceAssertionInput, SourceBindingCoordinates, SourceBindingInput, SourceProvenanceError,
    build_source_assertion, build_source_binding, validate_source_assertion,
    validate_source_binding,
};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const ASSERTION_BYTES: &[u8] = b"Enterprise refunds are allowed for 14 days.";

fn coordinates() -> SourceBindingCoordinates {
    SourceBindingCoordinates {
        connector: "github".to_string(),
        source: "agentdoc-dev/policies".to_string(),
        revision: Some("0123456789abcdef".to_string()),
        path: "docs/refunds.adoc".to_string(),
        anchor: "policy.refunds.enterprise-window".to_string(),
        source_revision_digest: format!("sha256:{}", "1".repeat(64)),
    }
}

#[test]
fn standalone_source_binding_reuses_the_graph_v6_shape() {
    let binding = build_source_binding(SourceBindingInput {
        source_binding_id: "binding-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        coordinates: coordinates(),
    })
    .expect("source binding builds");
    let document = binding.to_canonical_json().expect("binding serializes");

    assert_eq!(
        validate_source_binding(document.as_bytes()).expect("binding validates"),
        binding
    );
    assert_eq!(
        serde_json::to_value(binding).expect("binding serializes")["binding"],
        serde_json::json!({
            "connector": "github",
            "source": "agentdoc-dev/policies",
            "revision": "0123456789abcdef",
            "path": "docs/refunds.adoc",
            "anchor": "policy.refunds.enterprise-window",
            "source_revision_digest": format!("sha256:{}", "1".repeat(64)),
        })
    );
}

#[test]
fn source_binding_rejects_an_explicit_null_revision() {
    let binding = build_source_binding(SourceBindingInput {
        source_binding_id: "binding-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        coordinates: coordinates(),
    })
    .expect("source binding builds");
    let mut document = serde_json::to_value(binding).expect("binding serializes");
    document["binding"]["revision"] = serde_json::Value::Null;

    assert!(matches!(
        validate_source_binding(document.to_string().as_bytes()),
        Err(SourceProvenanceError::InvalidDocument { .. })
    ));
}

#[test]
fn source_assertion_digest_roundtrip() {
    let assertion = build_source_assertion(SourceAssertionInput {
        source_assertion_id: "assertion-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        source_binding_id: "binding-001".to_string(),
        source_acl_snapshot_id: "snapshot-001".to_string(),
        extractor: "adoc".to_string(),
        extractor_version: "0.4.0".to_string(),
        media_type: "text/plain; charset=utf-8".to_string(),
        exact_bytes: ASSERTION_BYTES,
    })
    .expect("source assertion builds");
    let document = assertion.to_canonical_json().expect("assertion serializes");

    assert_eq!(
        validate_source_assertion(document.as_bytes(), ASSERTION_BYTES)
            .expect("assertion validates"),
        assertion
    );
    assert_eq!(
        assertion.content_digest(),
        format!(
            "sha256:{}",
            Sha256::digest(ASSERTION_BYTES)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    );
}

#[test]
fn source_assertion_rejects_bytes_from_another_claim() {
    let assertion = build_source_assertion(SourceAssertionInput {
        source_assertion_id: "assertion-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        source_binding_id: "binding-001".to_string(),
        source_acl_snapshot_id: "snapshot-001".to_string(),
        extractor: "adoc".to_string(),
        extractor_version: "0.4.0".to_string(),
        media_type: "text/plain; charset=utf-8".to_string(),
        exact_bytes: ASSERTION_BYTES,
    })
    .expect("source assertion builds");

    assert_eq!(
        validate_source_assertion(
            assertion
                .to_canonical_json()
                .expect("assertion serializes")
                .as_bytes(),
            b"Enterprise refunds are allowed for 30 days.",
        ),
        Err(SourceProvenanceError::DigestMismatch)
    );
}

#[test]
fn source_provenance_serialization_matches_published_schemas() {
    let binding = build_source_binding(SourceBindingInput {
        source_binding_id: "binding-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        coordinates: coordinates(),
    })
    .expect("source binding builds");
    let assertion = build_source_assertion(SourceAssertionInput {
        source_assertion_id: "assertion-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        source_record_id: "source-record-001".to_string(),
        source_binding_id: "binding-001".to_string(),
        source_acl_snapshot_id: "snapshot-001".to_string(),
        extractor: "adoc".to_string(),
        extractor_version: "0.4.0".to_string(),
        media_type: "text/plain; charset=utf-8".to_string(),
        exact_bytes: ASSERTION_BYTES,
    })
    .expect("source assertion builds");

    for (name, value) in [
        (
            "adoc.source_binding.v0.schema.json",
            serde_json::to_value(binding).expect("binding serializes"),
        ),
        (
            "adoc.source_assertion.v0.schema.json",
            serde_json::to_value(assertion).expect("assertion serializes"),
        ),
    ] {
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema")
            .join(name);
        let schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(schema_path).expect("source provenance schema is published"),
        )
        .expect("source provenance schema is JSON");
        let validator = jsonschema::validator_for(&schema).expect("schema compiles");

        assert!(validator.is_valid(&value), "{name} accepts typed output");
    }
}

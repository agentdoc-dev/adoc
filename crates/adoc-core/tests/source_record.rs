use adoc_core::{
    RetentionClass, SourceArtifact, SourceRecordError, SourceRecordInput, build_source_record,
    validate_source_record,
};
use chrono::{TimeZone, Utc};
use sha2::{Digest, Sha256};
use std::fs;
use std::path::PathBuf;

const EXACT_BYTES: &[u8] = b"policy: retain exact candidate input\n\0binary-safe\n";

fn input() -> SourceRecordInput<'static> {
    SourceRecordInput {
        source_record_id: "source-record-001".to_string(),
        workspace_id: "workspace-001".to_string(),
        connector_id: "connector-001".to_string(),
        source: SourceArtifact {
            provider: "github".to_string(),
            kind: "file".to_string(),
            external_id: "repository-42:docs/policy.adoc".to_string(),
            external_version: "0123456789abcdef".to_string(),
        },
        observed_at: Utc
            .with_ymd_and_hms(2026, 8, 25, 10, 0, 0)
            .single()
            .expect("valid observation time"),
        media_type: "text/plain; charset=utf-8".to_string(),
        retention_class: RetentionClass::ExactCandidateInput,
        exact_bytes: EXACT_BYTES,
    }
}

#[test]
fn source_record_digest_roundtrip() {
    let record = build_source_record(input()).expect("source record builds");
    let document = record.to_canonical_json().expect("record serializes");
    let validated =
        validate_source_record(document.as_bytes(), EXACT_BYTES).expect("record validates");

    assert_eq!(validated, record);
    assert_eq!(record.content_length_bytes(), EXACT_BYTES.len() as u64);
    assert_eq!(
        record.content_digest(),
        format!(
            "sha256:{}",
            Sha256::digest(EXACT_BYTES)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        )
    );
}

#[test]
fn source_record_serialization_matches_published_schema() {
    let record = build_source_record(input()).expect("source record builds");
    let mut instance = serde_json::to_value(record).expect("record serializes");
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/agent/v0/schema/adoc.source_record.v0.schema.json");
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(schema_path).expect("source record schema is published"),
    )
    .expect("source record schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("source record schema compiles");

    assert!(validator.is_valid(&instance));
    instance["unexpected"] = serde_json::json!(true);
    assert!(!validator.is_valid(&instance));
}

#[test]
fn source_record_rejects_digest_or_length_mismatch() {
    let record = build_source_record(input()).expect("source record builds");
    let mut document = serde_json::to_value(record).expect("record serializes");
    document["content_digest"] = serde_json::json!(format!("sha256:{}", "0".repeat(64)));

    assert_eq!(
        validate_source_record(
            serde_json::to_string(&document)
                .expect("document serializes")
                .as_bytes(),
            EXACT_BYTES,
        ),
        Err(SourceRecordError::DigestMismatch)
    );

    let record = build_source_record(input()).expect("source record builds");
    let mut document = serde_json::to_value(record).expect("record serializes");
    document["content_length_bytes"] = serde_json::json!(EXACT_BYTES.len() + 1);
    assert_eq!(
        validate_source_record(
            serde_json::to_string(&document)
                .expect("document serializes")
                .as_bytes(),
            EXACT_BYTES,
        ),
        Err(SourceRecordError::ContentLengthMismatch)
    );
}

#[test]
fn source_record_rejects_unknown_version_with_remediation() {
    let record = build_source_record(input()).expect("source record builds");
    let mut document = serde_json::to_value(record).expect("record serializes");
    document["schema_version"] = serde_json::json!("adoc.source_record.v99");

    let error = validate_source_record(
        serde_json::to_string(&document)
            .expect("document serializes")
            .as_bytes(),
        EXACT_BYTES,
    )
    .expect_err("unknown version is rejected");
    assert_eq!(
        error,
        SourceRecordError::UnsupportedVersion {
            version: "adoc.source_record.v99".to_string()
        }
    );
    assert!(
        error
            .remediation()
            .contains("supported source-record version")
    );
}

#[test]
fn source_record_rejects_observation_years_outside_the_schema_range() {
    for year in [-1, 10_000] {
        let mut input = input();
        input.observed_at = Utc
            .with_ymd_and_hms(year, 1, 1, 0, 0, 0)
            .single()
            .expect("chrono supports the out-of-contract year");

        assert!(matches!(
            build_source_record(input),
            Err(SourceRecordError::InvalidDocument { message })
                if message.contains("four-digit UTC year")
        ));
    }
}

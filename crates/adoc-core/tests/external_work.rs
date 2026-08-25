use std::{collections::BTreeMap, fs, path::PathBuf};

use adoc_core::{
    CapabilityRequirement, ContractRequirement, ExactRevision, ExternalWorkError,
    WorkChangeRequest, WorkRequest, WorkRequestInput, WorkResultInput, WorkRuntime, WorkSource,
    WorkloadAuthorization, build_work_request, build_work_result, validate_work_request,
    validate_work_result,
};
use chrono::{TimeZone, Utc};
use sha2::{Digest, Sha256};

const OUTPUT_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const OTHER_DIGEST: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const WORKSPACE_ID: &str = "10000000-0000-0000-0000-000000000401";
const REPOSITORY_ID: &str = "30000000-0000-0000-0000-000000000401";
const WORKLOAD_PRINCIPAL_ID: &str = "20000000-0000-0000-0000-000000000401";

fn revision(value: &str) -> ExactRevision {
    ExactRevision {
        system: "git".to_string(),
        value: value.to_string(),
    }
}

fn request_input() -> WorkRequestInput {
    WorkRequestInput {
        request_id: "request-001".to_string(),
        nonce: "request-nonce-001".to_string(),
        workspace_id: WORKSPACE_ID.to_string(),
        repository_id: REPOSITORY_ID.to_string(),
        source: WorkSource {
            provider: "github".to_string(),
            external_repository_id: "42".to_string(),
        },
        revision: revision("head-sha"),
        change_request: WorkChangeRequest {
            system: "github_pull_request".to_string(),
            id: "165".to_string(),
        },
        contracts: vec![
            ContractRequirement {
                schema_version: "adoc.work_result.v0".to_string(),
            },
            ContractRequirement {
                schema_version: "adoc.semantic_assessment.v0".to_string(),
            },
        ],
        capabilities: vec![CapabilityRequirement {
            name: "code_change_assessment".to_string(),
            version: "1".to_string(),
        }],
        expires_at: Utc
            .with_ymd_and_hms(2026, 8, 26, 12, 0, 0)
            .single()
            .expect("valid expiry"),
        workload: WorkloadAuthorization {
            principal_id: WORKLOAD_PRINCIPAL_ID.to_string(),
            subject: "repo:agentdoc-dev/adoc:environment:production".to_string(),
            audience: "https://cloud.agentdoc.dev/work-results".to_string(),
        },
    }
}

fn result_input(request: &WorkRequest) -> WorkResultInput {
    WorkResultInput {
        request_id: request.request_id().to_string(),
        request_digest: request.request_digest().to_string(),
        workspace_id: request.workspace_id().to_string(),
        repository_id: request.repository_id().to_string(),
        revision: request.revision().clone(),
        completion_nonce: "completion-nonce-001".to_string(),
        worker: request.workload().clone(),
        runtime: WorkRuntime {
            name: "adoc".to_string(),
            version: "0.4.0".to_string(),
        },
        output_digests: BTreeMap::from([
            ("semantic_assessment".to_string(), OUTPUT_DIGEST.to_string()),
            ("validation_receipt".to_string(), OUTPUT_DIGEST.to_string()),
        ]),
    }
}

#[test]
fn work_request_digest_and_round_trip_are_stable() {
    let first = build_work_request(request_input()).expect("request builds");
    let mut reordered = request_input();
    reordered.contracts.reverse();
    let second = build_work_request(reordered).expect("request builds");

    assert_eq!(first, second);
    let bytes = first.to_canonical_json().expect("request serializes");
    assert_eq!(
        first,
        validate_work_request(bytes.as_bytes()).expect("canonical request validates")
    );
    assert!(first.request_digest().starts_with("sha256:"));
    let mut digest_input = serde_json::to_value(&first).expect("request serializes");
    digest_input
        .as_object_mut()
        .expect("request object")
        .remove("request_digest");
    let expected = format!(
        "sha256:{}",
        Sha256::digest(serde_json::to_vec(&digest_input).expect("canonical JSON"))
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    assert_eq!(first.request_digest(), expected);
    assert_eq!(
        first.request_digest(),
        "sha256:9ce0a75ac46186aec3ae1b4a16ad8d49c0e0cc6a59e1b03a53a6fe84ff55824f"
    );
}

#[test]
fn wire_requirements_must_already_use_canonical_order() {
    let request = build_work_request(request_input()).expect("request builds");
    let mut document = serde_json::to_value(request).expect("request serializes");
    document["contracts"]
        .as_array_mut()
        .expect("contracts array")
        .reverse();

    validate_work_request(&serde_json::to_vec(&document).expect("serializes"))
        .expect_err("wire requirements cannot rely on validator normalization");
}

#[test]
fn work_request_expiry_uses_canonical_utc_seconds() {
    let mut input = request_input();
    input.expires_at = Utc
        .with_ymd_and_hms(2026, 8, 26, 12, 0, 0)
        .single()
        .expect("valid expiry")
        + chrono::Duration::milliseconds(1);

    build_work_request(input).expect_err("subsecond expiry is not cross-runtime canonical");
}

#[test]
fn work_request_rejects_noncanonical_expiry_spellings() {
    let request = build_work_request(request_input()).expect("request builds");
    let canonical = request.to_canonical_json().expect("request serializes");

    for expiry in ["2026-08-26T12:00:00.000Z", "2026-08-26T13:00:00+01:00"] {
        let document = canonical.replace("2026-08-26T12:00:00Z", expiry);
        validate_work_request(document.as_bytes())
            .expect_err("equivalent but noncanonical expiry text must be rejected");
    }
}

#[test]
fn unknown_work_envelope_version_is_rejected_with_remediation() {
    let request = build_work_request(request_input()).expect("request builds");
    let mut document = serde_json::to_value(&request).expect("request serializes");
    document["schema_version"] = serde_json::json!("adoc.work_request.v99");
    document
        .as_object_mut()
        .expect("request object")
        .remove("request_id");
    document["future_field"] = serde_json::json!(true);

    let error = validate_work_request(
        serde_json::to_vec(&document)
            .expect("document serializes")
            .as_slice(),
    )
    .expect_err("unknown request version is rejected");
    assert!(error.to_string().contains("adoc.work_request.v99"));
    assert!(
        error
            .remediation()
            .contains("supported work-request version")
    );

    let result = build_work_result(result_input(&request), &request).expect("result builds");
    let mut document = serde_json::to_value(result).expect("result serializes");
    document["schema_version"] = serde_json::json!("adoc.work_result.v99");
    document
        .as_object_mut()
        .expect("result object")
        .remove("result_digest");
    document["future_field"] = serde_json::json!(true);
    let error = validate_work_result(
        serde_json::to_vec(&document)
            .expect("document serializes")
            .as_slice(),
        &request,
    )
    .expect_err("unknown result version is rejected before exact v0 decoding");
    assert!(matches!(
        error,
        ExternalWorkError::UnsupportedVersion {
            envelope: "work-result",
            version
        } if version == "adoc.work_result.v99"
    ));
}

#[test]
fn work_requirements_are_ascii_for_cross_runtime_ordering() {
    let mut contract = request_input();
    contract.contracts[0].schema_version = "adoc.\u{10000}.v0".to_string();
    build_work_request(contract).expect_err("non-ASCII contract versions are rejected");

    let mut capability = request_input();
    capability.capabilities[0].name = "code_\u{e000}_assessment".to_string();
    build_work_request(capability).expect_err("non-ASCII capability fields are rejected");
}

#[test]
fn work_result_is_bound_to_the_exact_request() {
    let request = build_work_request(request_input()).expect("request builds");
    let result = build_work_result(result_input(&request), &request).expect("result builds");
    let bytes = result.to_canonical_json().expect("result serializes");
    assert_eq!(
        result,
        validate_work_result(bytes.as_bytes(), &request).expect("result validates")
    );
    assert_eq!(
        result.result_digest(),
        "sha256:548cd60cbca8128ba87cb4516744e81c07b6a027ab09b8dd0d08a5b76de347e2"
    );

    for boundary in ["request", "repository", "revision", "workspace"] {
        let mut other_request = request_input();
        match boundary {
            "request" => other_request.request_id = "request-002".to_string(),
            "repository" => other_request.repository_id = "repository-002".to_string(),
            "revision" => other_request.revision = revision("other-sha"),
            "workspace" => other_request.workspace_id = "workspace-002".to_string(),
            _ => unreachable!(),
        }
        let other = build_work_request(other_request).expect("other request builds");
        let error = validate_work_result(bytes.as_bytes(), &other)
            .expect_err("cross-boundary result substitution is rejected");
        assert!(
            error
                .to_string()
                .contains("does not match the exact request")
        );
    }
}

#[test]
fn work_result_rejects_a_forged_digest_or_worker_identity() {
    let request = build_work_request(request_input()).expect("request builds");
    let result = build_work_result(result_input(&request), &request).expect("result builds");
    let mut forged_digest = serde_json::to_value(&result).expect("result serializes");
    forged_digest["result_digest"] = serde_json::json!(OUTPUT_DIGEST);
    let mut forged_worker = serde_json::to_value(&result).expect("result serializes");
    forged_worker["worker"]["subject"] = serde_json::json!("other-worker");

    for document in [forged_digest, forged_worker] {
        validate_work_result(
            serde_json::to_vec(&document)
                .expect("document serializes")
                .as_slice(),
            &request,
        )
        .expect_err("forged result is rejected");
    }
}

#[test]
fn output_digest_names_have_one_cross_runtime_order() {
    let request = build_work_request(request_input()).expect("request builds");
    let mut input = result_input(&request);
    input.output_digests = BTreeMap::from([
        ("2".to_string(), OUTPUT_DIGEST.to_string()),
        ("10".to_string(), OTHER_DIGEST.to_string()),
    ]);

    build_work_result(input, &request)
        .expect_err("integer-like keys have runtime-specific JSON ordering");
}

#[test]
fn duplicate_output_digest_keys_are_rejected_before_normalization() {
    let request = build_work_request(request_input()).expect("request builds");
    let result = build_work_result(result_input(&request), &request).expect("result builds");
    let canonical = result.to_canonical_json().expect("result serializes");
    let needle = format!("\"semantic_assessment\": \"{OUTPUT_DIGEST}\"");
    let replacement = format!("\"semantic_assessment\": \"{OTHER_DIGEST}\",\n    {needle}");
    let duplicated = canonical.replacen(&needle, &replacement, 1);

    validate_work_result(duplicated.as_bytes(), &request)
        .expect_err("duplicate digest names must not be collapsed by map deserialization");
}

#[test]
fn serialized_external_work_envelopes_match_the_published_schemas() {
    let request = build_work_request(request_input()).expect("request builds");
    let result = build_work_result(result_input(&request), &request).expect("result builds");

    for (name, instance) in [
        (
            "adoc.work_request.v0.schema.json",
            serde_json::to_value(request).expect("request serializes"),
        ),
        (
            "adoc.work_result.v0.schema.json",
            serde_json::to_value(result).expect("result serializes"),
        ),
    ] {
        let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../docs/agent/v0/schema")
            .join(name);
        let schema: serde_json::Value = serde_json::from_str(
            &fs::read_to_string(schema_path).expect("published schema is readable"),
        )
        .expect("published schema is JSON");
        let validator = jsonschema::validator_for(&schema).expect("published schema compiles");
        let errors = validator
            .iter_errors(&instance)
            .map(|error| error.to_string())
            .collect::<Vec<_>>();
        assert!(errors.is_empty(), "{name} validation failed: {errors:#?}");
    }
}

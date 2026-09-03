//! E5.3 — deterministic four-mode gate result (`adoc.gate_result.v0`).

use std::fs;
use std::path::PathBuf;

use adoc_core::{GateMode, GateOutcome, GateReason, GateResult, validate_gate_result};
use serde_json::{Value, json};

const HEAD: &str = "0123456789abcdef0123456789abcdef01234567";
const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/agent/v0/schema/adoc.gate_result.v0.schema.json");
    serde_json::from_str(&fs::read_to_string(path).expect("schema is readable"))
        .expect("schema is JSON")
}

#[test]
fn same_facts_same_conclusion_bytes() {
    let first = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![B.to_string(), A.to_string(), B.to_string()],
        None,
        GateOutcome::Pass,
        vec![],
    )
    .expect("gate result builds");
    let second = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![A.to_string(), B.to_string()],
        None,
        GateOutcome::Pass,
        vec![],
    )
    .expect("same facts build");

    assert_eq!(
        first.to_canonical_json().expect("serializes"),
        second.to_canonical_json().expect("serializes")
    );
    assert_eq!(first.effective_mode(), Some(GateMode::Advisory));
    let value: Value = serde_json::from_str(&first.to_canonical_json().expect("serializes"))
        .expect("gate result is JSON");
    assert_eq!(
        value,
        json!({
            "schema_version": "adoc.gate_result.v0",
            "head_sha": HEAD,
            "policy_version": "gate-policy-v1",
            "input_digests": [A, B],
            "effective_mode": "advisory",
            "result": "pass",
            "reasons": []
        })
    );
}

#[test]
fn record_matches_the_published_closed_schema() {
    let record = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![A.to_string()],
        Some("approval_required".to_string()),
        GateOutcome::Block,
        vec![
            GateReason::SemanticInvalid,
            GateReason::PromotionUnapproved,
            GateReason::ProviderFailedNoFallback,
            GateReason::ProposalMissing,
            GateReason::ProposalHashMismatch,
            GateReason::CloudUnavailable,
            GateReason::AuditPersistenceFailed,
            GateReason::AssessmentStale,
            GateReason::AssessmentMissing,
            GateReason::ApprovalMissing,
            GateReason::ApprovalInvalidated,
            GateReason::SemanticInvalid,
        ],
    )
    .expect("gate result builds");
    let instance: Value = serde_json::from_str(&record.to_canonical_json().expect("serializes"))
        .expect("gate result is JSON");
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");
    let errors: Vec<_> = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect();

    assert!(errors.is_empty(), "schema validation failed: {errors:#?}");
    assert_eq!(
        schema()["properties"]["reasons"]["items"]["enum"],
        json!([
            "gate.approval_invalidated",
            "gate.approval_missing",
            "gate.assessment_missing",
            "gate.assessment_stale",
            "gate.audit_persistence_failed",
            "gate.cloud_unavailable",
            "gate.mode_unknown",
            "gate.promotion_unapproved",
            "gate.proposal_hash_mismatch",
            "gate.proposal_missing",
            "gate.provider_failed_no_fallback",
            "gate.semantic_invalid"
        ])
    );
    assert_eq!(
        instance["reasons"],
        json!([
            "gate.approval_invalidated",
            "gate.approval_missing",
            "gate.assessment_missing",
            "gate.assessment_stale",
            "gate.audit_persistence_failed",
            "gate.cloud_unavailable",
            "gate.promotion_unapproved",
            "gate.proposal_hash_mismatch",
            "gate.proposal_missing",
            "gate.provider_failed_no_fallback",
            "gate.semantic_invalid"
        ])
    );

    let mut publish_failure = instance;
    publish_failure["reasons"] = json!(["gate.check_publish_failed"]);
    assert!(
        !validator.is_valid(&publish_failure),
        "E5.4 publication failures are not E5.3 gate-result reasons"
    );
}

#[test]
fn unknown_mode_blocks_without_falling_back() {
    let unknown = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![],
        Some("typo_mode".to_string()),
        GateOutcome::Block,
        vec![GateReason::ModeUnknown],
    )
    .expect("unknown mode produces a typed blocking result");
    assert_eq!(unknown.effective_mode(), None);

    for configured in ["", " advisory ", "\t"] {
        let invalid_configuration = GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            Some(configured.to_string()),
            GateOutcome::Block,
            vec![GateReason::ModeUnknown],
        )
        .expect("every present non-mode string is an unknown-mode result");
        let instance: Value = serde_json::from_str(
            &invalid_configuration
                .to_canonical_json()
                .expect("serializes"),
        )
        .expect("gate result is JSON");
        assert!(
            jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&instance),
            "schema must represent unknown configured mode {configured:?}"
        );
    }

    assert!(
        GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            Some("advisory".to_string()),
            GateOutcome::Pass,
            vec![GateReason::ModeUnknown],
        )
        .is_err(),
        "gate.mode_unknown cannot label a known mode"
    );

    let known_mode_with_unknown_reason = json!({
        "schema_version": "adoc.gate_result.v0",
        "head_sha": HEAD,
        "policy_version": "gate-policy-v1",
        "input_digests": [],
        "configured_mode": "advisory",
        "effective_mode": "advisory",
        "result": "pass",
        "reasons": ["gate.mode_unknown"]
    });
    assert!(
        !jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&known_mode_with_unknown_reason),
        "published schema must match the domain's known-mode invariant"
    );

    let mut unset_mode_with_unknown_reason = known_mode_with_unknown_reason;
    unset_mode_with_unknown_reason
        .as_object_mut()
        .expect("gate result object")
        .remove("configured_mode");
    assert!(
        !jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&unset_mode_with_unknown_reason),
        "unset advisory mode is not an unknown-mode error"
    );
}

#[test]
fn blocking_result_always_names_a_registered_reason() {
    assert!(
        GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            Some("assessment_required".to_string()),
            GateOutcome::Block,
            vec![],
        )
        .is_err(),
        "domain must reject a silent block"
    );

    let silent_block = json!({
        "schema_version": "adoc.gate_result.v0",
        "head_sha": HEAD,
        "policy_version": "gate-policy-v1",
        "input_digests": [],
        "configured_mode": "assessment_required",
        "effective_mode": "assessment_required",
        "result": "block",
        "reasons": []
    });
    assert!(
        !jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&silent_block),
        "published schema must reject a silent block"
    );
}

#[test]
fn validated_result_exposes_only_typed_gate_facts() {
    for (mode, effective) in [
        ("advisory", GateMode::Advisory),
        ("assessment_required", GateMode::AssessmentRequired),
        ("proposal_required", GateMode::ProposalRequired),
        ("approval_required", GateMode::ApprovalRequired),
    ] {
        let record = GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![A.to_string()],
            Some(mode.to_string()),
            GateOutcome::Block,
            vec![GateReason::SemanticInvalid],
        )
        .expect("known mode builds");

        assert_eq!(record.head_sha(), HEAD);
        assert_eq!(record.policy_version(), "gate-policy-v1");
        assert_eq!(record.input_digests(), [A]);
        assert_eq!(record.configured_mode(), Some(mode));
        assert_eq!(record.effective_mode(), Some(effective));
        assert_eq!(record.result(), GateOutcome::Block);
        assert_eq!(record.reasons(), [GateReason::SemanticInvalid]);
    }
}

#[test]
fn wire_validation_rejects_noncanonical_derived_fields() {
    let canonical = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![A.to_string(), B.to_string()],
        Some("proposal_required".to_string()),
        GateOutcome::Block,
        vec![GateReason::ProposalMissing],
    )
    .expect("gate result builds")
    .to_canonical_json()
    .expect("serializes");
    assert!(validate_gate_result(canonical.as_bytes()).is_ok());

    let mut drifted: Value = serde_json::from_str(&canonical).expect("gate result is JSON");
    drifted["effective_mode"] = json!("advisory");
    assert!(
        validate_gate_result(&serde_json::to_vec(&drifted).expect("serializes")).is_err(),
        "effective mode is derived, never caller authority"
    );
}

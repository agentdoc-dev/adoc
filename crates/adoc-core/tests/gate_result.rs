//! E5.3 — deterministic four-mode gate result (`adoc.gate_result.v0`).

use std::fs;
use std::path::PathBuf;

use adoc_core::{
    GateMode, GateOutcome, GateReason, GateResult, GateResultError, validate_gate_result,
};
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
fn every_reason_variant_matches_the_published_enum_in_canonical_order() {
    const ALL: [GateReason; 12] = [
        GateReason::ApprovalInvalidated,
        GateReason::ApprovalMissing,
        GateReason::AssessmentMissing,
        GateReason::AssessmentStale,
        GateReason::AuditPersistenceFailed,
        GateReason::CloudUnavailable,
        GateReason::ModeUnknown,
        GateReason::PromotionUnapproved,
        GateReason::ProposalHashMismatch,
        GateReason::ProposalMissing,
        GateReason::ProviderFailedNoFallback,
        GateReason::SemanticInvalid,
    ];

    for reason in ALL {
        match reason {
            GateReason::ApprovalInvalidated
            | GateReason::ApprovalMissing
            | GateReason::AssessmentMissing
            | GateReason::AssessmentStale
            | GateReason::AuditPersistenceFailed
            | GateReason::CloudUnavailable
            | GateReason::ModeUnknown
            | GateReason::PromotionUnapproved
            | GateReason::ProposalHashMismatch
            | GateReason::ProposalMissing
            | GateReason::ProviderFailedNoFallback
            | GateReason::SemanticInvalid => {}
        }
    }

    let wire = serde_json::to_value(ALL).expect("reasons serialize");
    let mut ascending = wire.clone();
    ascending
        .as_array_mut()
        .expect("reasons are an array")
        .sort_by(|left, right| left.as_str().cmp(&right.as_str()));

    assert_eq!(wire, ascending);
    assert_eq!(wire, schema()["properties"]["reasons"]["items"]["enum"]);
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

    for configured in ["", " advisory ", "advisory-mode"] {
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
        assert!(
            validate_gate_result(
                &serde_json::to_vec(&instance).expect("unknown-mode result serializes")
            )
            .is_ok(),
            "wire validation must preserve safe unknown configured mode {configured:?}"
        );
    }

    assert_eq!(
        GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            Some("advisory".to_string()),
            GateOutcome::Block,
            vec![GateReason::ModeUnknown],
        ),
        Err(GateResultError::InvalidUnknownMode),
        "gate.mode_unknown cannot label a known mode"
    );

    let known_mode_with_unknown_reason = json!({
        "schema_version": "adoc.gate_result.v0",
        "head_sha": HEAD,
        "policy_version": "gate-policy-v1",
        "input_digests": [],
        "configured_mode": "advisory",
        "effective_mode": "advisory",
        "result": "block",
        "reasons": ["gate.mode_unknown"]
    });
    assert_eq!(
        validate_gate_result(
            &serde_json::to_vec(&known_mode_with_unknown_reason).expect("serializes")
        ),
        Err(GateResultError::InvalidUnknownMode),
        "wire validation must preserve the unknown-mode diagnostic"
    );
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
fn configured_mode_is_bounded_and_control_free() {
    for configured in [
        "line\nbreak".to_string(),
        "trailing\n".to_string(),
        "escape\u{1b}".to_string(),
        "next\u{85}line".to_string(),
        "x".repeat(129),
    ] {
        assert_eq!(
            GateResult::new(
                HEAD.to_string(),
                "gate-policy-v1".to_string(),
                vec![],
                Some(configured.clone()),
                GateOutcome::Block,
                vec![GateReason::ModeUnknown],
            ),
            Err(GateResultError::InvalidField {
                field: "configured_mode".to_string(),
            })
        );

        let invalid = json!({
            "schema_version": "adoc.gate_result.v0",
            "head_sha": HEAD,
            "policy_version": "gate-policy-v1",
            "input_digests": [],
            "configured_mode": configured,
            "effective_mode": null,
            "result": "block",
            "reasons": ["gate.mode_unknown"]
        });
        assert_eq!(
            validate_gate_result(&serde_json::to_vec(&invalid).expect("serializes")),
            Err(GateResultError::InvalidField {
                field: "configured_mode".to_string(),
            }),
            "wire validation must reject unsafe configured mode exactly"
        );
        assert!(
            !jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&invalid),
            "published schema must reject unsafe configured mode"
        );
    }

    let boundary = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![],
        Some("é".repeat(128)),
        GateOutcome::Block,
        vec![GateReason::ModeUnknown],
    )
    .expect("128 Unicode scalar values are admitted");
    let bytes = boundary
        .to_canonical_json()
        .expect("boundary result serializes");
    let instance: Value = serde_json::from_str(&bytes).expect("boundary result is JSON");
    assert!(validate_gate_result(bytes.as_bytes()).is_ok());
    assert!(
        jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&instance),
        "schema length uses Unicode scalar values, not UTF-8 bytes"
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
fn passing_result_never_names_a_blocking_reason() {
    assert!(
        GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            Some("advisory".to_string()),
            GateOutcome::Pass,
            vec![GateReason::SemanticInvalid],
        )
        .is_err(),
        "domain must reject a pass with a blocking reason"
    );

    let contradictory_pass = json!({
        "schema_version": "adoc.gate_result.v0",
        "head_sha": HEAD,
        "policy_version": "gate-policy-v1",
        "input_digests": [],
        "configured_mode": "advisory",
        "effective_mode": "advisory",
        "result": "pass",
        "reasons": ["gate.semantic_invalid"]
    });
    assert!(
        validate_gate_result(&serde_json::to_vec(&contradictory_pass).expect("serializes"))
            .is_err(),
        "wire validation must reject a pass with a blocking reason"
    );
    assert!(
        !jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&contradictory_pass),
        "published schema must reject a pass with a blocking reason"
    );
}

#[test]
fn advisory_mode_is_pass_only() {
    for configured_mode in [None, Some("advisory".to_string())] {
        assert_eq!(
            GateResult::new(
                HEAD.to_string(),
                "gate-policy-v1".to_string(),
                vec![],
                configured_mode.clone(),
                GateOutcome::Block,
                vec![GateReason::SemanticInvalid],
            ),
            Err(GateResultError::InvalidField {
                field: "result".to_string(),
            }),
            "effective advisory mode cannot block"
        );

        let mut advisory_block = json!({
            "schema_version": "adoc.gate_result.v0",
            "head_sha": HEAD,
            "policy_version": "gate-policy-v1",
            "input_digests": [],
            "configured_mode": configured_mode,
            "effective_mode": "advisory",
            "result": "block",
            "reasons": ["gate.semantic_invalid"]
        });
        if advisory_block["configured_mode"].is_null() {
            advisory_block
                .as_object_mut()
                .expect("gate result object")
                .remove("configured_mode");
        }

        assert_eq!(
            validate_gate_result(&serde_json::to_vec(&advisory_block).expect("serializes")),
            Err(GateResultError::InvalidField {
                field: "result".to_string(),
            }),
            "wire validation cannot turn advisory diagnostics into a block"
        );
        assert!(
            !jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&advisory_block),
            "published schema must reject advisory blocks"
        );
    }
}

#[test]
fn strict_mode_pass_requires_validated_input_evidence() {
    for mode in [
        "assessment_required",
        "proposal_required",
        "approval_required",
    ] {
        assert_eq!(
            GateResult::new(
                HEAD.to_string(),
                "gate-policy-v1".to_string(),
                vec![],
                Some(mode.to_string()),
                GateOutcome::Pass,
                vec![],
            ),
            Err(GateResultError::InvalidField {
                field: "input_digests".to_string(),
            }),
            "{mode} cannot pass without validated input evidence"
        );

        let evidence_free_pass = json!({
            "schema_version": "adoc.gate_result.v0",
            "head_sha": HEAD,
            "policy_version": "gate-policy-v1",
            "input_digests": [],
            "configured_mode": mode,
            "effective_mode": mode,
            "result": "pass",
            "reasons": []
        });
        assert_eq!(
            validate_gate_result(&serde_json::to_vec(&evidence_free_pass).expect("serializes")),
            Err(GateResultError::InvalidField {
                field: "input_digests".to_string(),
            }),
            "wire validation must reject evidence-free {mode} pass exactly"
        );
        assert!(
            !jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&evidence_free_pass),
            "published schema must reject evidence-free {mode} pass"
        );

        let evidenced_pass = GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![A.to_string()],
            Some(mode.to_string()),
            GateOutcome::Pass,
            vec![],
        )
        .expect("strict mode passes with validated input evidence");
        let evidenced_bytes = evidenced_pass
            .to_canonical_json()
            .expect("evidenced pass serializes");
        let evidenced_instance: Value =
            serde_json::from_str(&evidenced_bytes).expect("evidenced pass is JSON");
        assert!(validate_gate_result(evidenced_bytes.as_bytes()).is_ok());
        assert!(
            jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&evidenced_instance),
            "published schema must accept evidenced {mode} pass"
        );
    }

    for configured_mode in [None, Some("advisory".to_string())] {
        let advisory = GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![],
            configured_mode,
            GateOutcome::Pass,
            vec![],
        )
        .expect("advisory pass may be evidence-free");
        let bytes = advisory
            .to_canonical_json()
            .expect("advisory pass serializes");
        let instance: Value = serde_json::from_str(&bytes).expect("advisory pass is JSON");
        assert!(validate_gate_result(bytes.as_bytes()).is_ok());
        assert!(
            jsonschema::validator_for(&schema())
                .expect("schema compiles")
                .is_valid(&instance),
            "published schema must accept evidence-free advisory pass"
        );
    }
    let missing = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![],
        Some("assessment_required".to_string()),
        GateOutcome::Block,
        vec![GateReason::AssessmentMissing],
    )
    .expect("fail-closed strict-mode block may be evidence-free");
    let bytes = missing
        .to_canonical_json()
        .expect("missing-evidence block serializes");
    let instance: Value = serde_json::from_str(&bytes).expect("missing-evidence block is JSON");
    assert!(validate_gate_result(bytes.as_bytes()).is_ok());
    assert!(
        jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&instance),
        "published schema must accept evidence-free fail-closed block"
    );
}

#[test]
fn validated_result_exposes_only_typed_gate_facts() {
    for (mode, effective, result, reasons) in [
        ("advisory", GateMode::Advisory, GateOutcome::Pass, vec![]),
        (
            "assessment_required",
            GateMode::AssessmentRequired,
            GateOutcome::Block,
            vec![GateReason::SemanticInvalid],
        ),
        (
            "proposal_required",
            GateMode::ProposalRequired,
            GateOutcome::Block,
            vec![GateReason::SemanticInvalid],
        ),
        (
            "approval_required",
            GateMode::ApprovalRequired,
            GateOutcome::Block,
            vec![GateReason::SemanticInvalid],
        ),
    ] {
        let record = GateResult::new(
            HEAD.to_string(),
            "gate-policy-v1".to_string(),
            vec![A.to_string()],
            Some(mode.to_string()),
            result,
            reasons.clone(),
        )
        .expect("known mode builds");

        assert_eq!(record.head_sha(), HEAD);
        assert_eq!(record.policy_version(), "gate-policy-v1");
        assert_eq!(record.input_digests(), [A]);
        assert_eq!(record.configured_mode(), Some(mode));
        assert_eq!(record.effective_mode(), Some(effective));
        assert_eq!(record.result(), result);
        assert_eq!(record.reasons(), reasons);
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

    let canonical_value: Value = serde_json::from_str(&canonical).expect("gate result is JSON");

    let mut drifted_effective_mode = canonical_value.clone();
    drifted_effective_mode["effective_mode"] = json!("advisory");
    assert!(
        validate_gate_result(&serde_json::to_vec(&drifted_effective_mode).expect("serializes"))
            .is_err(),
        "effective mode is derived, never caller authority"
    );

    let mut unsorted_digests = canonical_value.clone();
    unsorted_digests["input_digests"] = json!([B, A]);
    let mut duplicate_reasons = canonical_value.clone();
    duplicate_reasons["reasons"] = json!(["gate.proposal_missing", "gate.proposal_missing"]);
    let mut explicit_null_mode = canonical_value.clone();
    explicit_null_mode["configured_mode"] = Value::Null;
    explicit_null_mode["effective_mode"] = json!("advisory");

    for (what, document) in [
        ("unsorted input_digests", unsorted_digests),
        ("duplicate reasons", duplicate_reasons),
        ("explicit null configured_mode", explicit_null_mode),
    ] {
        assert!(
            validate_gate_result(&serde_json::to_vec(&document).expect("serializes")).is_err(),
            "{what} is not a canonical gate result"
        );
    }

    let mut unknown_member = canonical_value.clone();
    unknown_member["evaluated_at"] = json!("2026-09-04T00:00:00Z");
    assert!(
        validate_gate_result(&serde_json::to_vec(&unknown_member).expect("serializes")).is_err(),
        "unknown wire members fail closed"
    );

    let mut wrong_version = canonical_value;
    wrong_version["schema_version"] = json!("adoc.gate_result.v99");
    assert_eq!(
        validate_gate_result(&serde_json::to_vec(&wrong_version).expect("serializes")),
        Err(GateResultError::UnsupportedVersion {
            version: "adoc.gate_result.v99".to_string(),
        })
    );
}

#[test]
fn wire_validation_rejects_duplicate_object_members_before_normalization() {
    let canonical = GateResult::new(
        HEAD.to_string(),
        "gate-policy-v1".to_string(),
        vec![A.to_string()],
        Some("proposal_required".to_string()),
        GateOutcome::Block,
        vec![GateReason::ProposalMissing],
    )
    .expect("gate result builds")
    .to_canonical_json()
    .expect("serializes");
    let duplicated = canonical.replacen(
        "\"result\": \"block\"",
        "\"result\": \"pass\",\n  \"result\": \"block\"",
        1,
    );

    validate_gate_result(duplicated.as_bytes())
        .expect_err("duplicate result members must not collapse to the last value");
}

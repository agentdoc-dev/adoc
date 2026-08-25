use adoc_core::{
    ExecutorAuthority, ExecutorConfiguration, ExecutorQualificationError,
    ExecutorQualificationExpectedBindings, QualificationLayer, RequalificationTrigger,
    validate_executor_qualification,
};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

const D1: &str = "sha256:1111111111111111111111111111111111111111111111111111111111111111";
const D2: &str = "sha256:2222222222222222222222222222222222222222222222222222222222222222";
const D3: &str = "sha256:3333333333333333333333333333333333333333333333333333333333333333";
const D4: &str = "sha256:4444444444444444444444444444444444444444444444444444444444444444";
const D5: &str = "sha256:5555555555555555555555555555555555555555555555555555555555555555";
const D6: &str = "sha256:6666666666666666666666666666666666666666666666666666666666666666";
const D7: &str = "sha256:7777777777777777777777777777777777777777777777777777777777777777";
const D8: &str = "sha256:8888888888888888888888888888888888888888888888888888888888888888";
const D9: &str = "sha256:9999999999999999999999999999999999999999999999999999999999999999";
const DA: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DB: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DC: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

fn model_record() -> Value {
    json!({
        "schema_version": "adoc.executor_qualification.v0",
        "qualification_id": "qual-code-assessment-codex-v1",
        "capability": {"name": "code_change_assessment", "version": "1"},
        "subject": {
            "kind": "model",
            "provider": "codex",
            "executor_digest": D1,
            "model_digest": D2,
            "config_digest": D3,
            "configuration": {
                "model_revision_digest": D2,
                "quantization_digest": D4,
                "system_prompt_task_digest": D5,
                "context_strategy_digest": D6,
                "output_constraints_digest": D7,
                "toolset_digest": D8,
                "inference_parameters_digest": D9,
                "safety_configuration_digest": DA,
                "adapter_implementation_digest": DB
            }
        },
        "protocol": {"valid": true, "version": "semantic-executor-v1"},
        "agentdoc_evaluation": {
            "kind": "capability",
            "qualified": true,
            "evidence_ref": "qualification-suite:code-assessment-v1"
        },
        "organization_approval": {
            "approved": true,
            "scope": ["repo:billing"],
            "risk": ["high"],
            "deployment": ["customer_worker"],
            "policy_digest": DC
        },
        "runtime_policy": {
            "eligible": true,
            "operation_digest": DA,
            "policy_digest": DC
        }
    })
}

fn validate(value: &Value) -> adoc_core::ExecutorQualification {
    validate_executor_qualification(&serde_json::to_vec(value).expect("fixture serializes"))
        .expect("qualification validates")
}

fn configuration(value: &Value) -> ExecutorConfiguration {
    serde_json::from_value(value["subject"].clone()).expect("configuration fixture parses")
}

fn expected(value: &Value) -> ExecutorQualificationExpectedBindings {
    let bytes = serde_json::to_vec(value).expect("fixture serializes");
    ExecutorQualificationExpectedBindings {
        qualification_id: value["qualification_id"]
            .as_str()
            .expect("qualification id")
            .to_string(),
        record_digest: format!(
            "sha256:{}",
            Sha256::digest(bytes)
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        ),
        capability_name: value["capability"]["name"]
            .as_str()
            .expect("capability name")
            .to_string(),
        capability_version: value["capability"]["version"]
            .as_str()
            .expect("capability version")
            .to_string(),
        protocol_version: value["protocol"]["version"]
            .as_str()
            .expect("protocol version")
            .to_string(),
        requested_scope: value["organization_approval"]["scope"][0]
            .as_str()
            .expect("scope")
            .to_string(),
        requested_risk: value["organization_approval"]["risk"][0]
            .as_str()
            .expect("risk")
            .to_string(),
        requested_deployment: value["organization_approval"]["deployment"][0]
            .as_str()
            .expect("deployment")
            .to_string(),
        organization_policy_digest: value["organization_approval"]["policy_digest"]
            .as_str()
            .expect("organization policy")
            .to_string(),
        runtime_policy_digest: value["runtime_policy"]["policy_digest"]
            .as_str()
            .expect("runtime policy")
            .to_string(),
    }
}

#[test]
fn protocol_valid_but_unqualified_output_is_advisory_only() {
    let mut value = model_record();
    value["agentdoc_evaluation"]["qualified"] = json!(false);
    value["organization_approval"]["approved"] = json!(false);
    value["runtime_policy"]["eligible"] = json!(false);
    let record = validate(&value);

    let eligibility = record.evaluate(&configuration(&model_record()), DA, &expected(&value));

    assert_eq!(eligibility.authority(), ExecutorAuthority::AdvisoryOnly);
    assert_eq!(
        eligibility.missing_layers(),
        &[
            QualificationLayer::AgentDocEvaluated,
            QualificationLayer::OrganizationApproved,
            QualificationLayer::RuntimePolicyEligible,
        ]
    );
}

#[test]
fn every_layer_is_required_for_gate_authority() {
    let base = model_record();
    let current = configuration(&base);
    assert_eq!(
        validate(&base)
            .evaluate(&current, DA, &expected(&base))
            .authority(),
        ExecutorAuthority::GateAuthoritative
    );

    for (path, missing) in [
        ("protocol", QualificationLayer::ProtocolValid),
        ("agentdoc_evaluation", QualificationLayer::AgentDocEvaluated),
        (
            "organization_approval",
            QualificationLayer::OrganizationApproved,
        ),
        ("runtime_policy", QualificationLayer::RuntimePolicyEligible),
    ] {
        let mut value = base.clone();
        let field = if path == "protocol" {
            "valid"
        } else if path == "agentdoc_evaluation" {
            "qualified"
        } else if path == "organization_approval" {
            "approved"
        } else {
            "eligible"
        };
        value[path][field] = json!(false);
        let eligibility = validate(&value).evaluate(&current, DA, &expected(&value));
        assert!(eligibility.missing_layers().contains(&missing));
        assert_ne!(
            eligibility.authority(),
            ExecutorAuthority::GateAuthoritative
        );
    }
}

#[test]
fn every_material_model_configuration_change_names_a_requalification_trigger() {
    let value = model_record();
    let record = validate(&value);

    for (field, trigger) in [
        (
            "model_revision_digest",
            RequalificationTrigger::ModelRevision,
        ),
        ("quantization_digest", RequalificationTrigger::Quantization),
        (
            "system_prompt_task_digest",
            RequalificationTrigger::SystemPromptOrTaskDefinition,
        ),
        (
            "context_strategy_digest",
            RequalificationTrigger::ContextStrategy,
        ),
        (
            "output_constraints_digest",
            RequalificationTrigger::OutputConstraints,
        ),
        ("toolset_digest", RequalificationTrigger::ToolAvailability),
        (
            "inference_parameters_digest",
            RequalificationTrigger::InferenceParameters,
        ),
        (
            "safety_configuration_digest",
            RequalificationTrigger::SafetyConfiguration,
        ),
        (
            "adapter_implementation_digest",
            RequalificationTrigger::AdapterImplementation,
        ),
    ] {
        let mut current = value["subject"].clone();
        current["configuration"][field] = json!(DC);
        let current: ExecutorConfiguration =
            serde_json::from_value(current).expect("configuration parses");
        let eligibility = record.evaluate(&current, DA, &expected(&value));
        assert_eq!(eligibility.authority(), ExecutorAuthority::AdvisoryOnly);
        assert_eq!(eligibility.requalification_triggers(), &[trigger]);
    }
}

#[test]
fn an_inference_temperature_change_requires_requalification() {
    let value = model_record();
    let record = validate(&value);
    let mut current = value["subject"].clone();
    current["configuration"]["inference_parameters_digest"] = json!(DC);
    current["config_digest"] = json!(DC);
    let eligibility = record.evaluate(
        &serde_json::from_value(current).expect("configuration parses"),
        DA,
        &expected(&value),
    );

    assert_eq!(eligibility.authority(), ExecutorAuthority::AdvisoryOnly);
    assert!(
        eligibility
            .requalification_triggers()
            .contains(&RequalificationTrigger::InferenceParameters)
    );
}

#[test]
fn runtime_eligibility_is_bound_to_the_exact_operation() {
    let value = model_record();
    let eligibility = validate(&value).evaluate(&configuration(&value), DB, &expected(&value));

    assert_eq!(eligibility.authority(), ExecutorAuthority::AdvisoryOnly);
    assert_eq!(
        eligibility.missing_layers(),
        &[QualificationLayer::RuntimePolicyEligible]
    );
}

#[test]
fn self_declared_qualification_cannot_create_gate_authority() {
    let trusted = model_record();
    let mut self_declared = trusted.clone();
    self_declared["qualification_id"] = json!("attacker-declared");

    assert_ne!(
        validate(&self_declared)
            .evaluate(&configuration(&trusted), DA, &expected(&trusted))
            .authority(),
        ExecutorAuthority::GateAuthoritative
    );
}

#[test]
fn qualification_is_bound_to_the_requested_capability() {
    let trusted = model_record();
    let mut other_capability = trusted.clone();
    other_capability["capability"] = json!({"name": "contradiction_analysis", "version": "1"});

    assert_ne!(
        validate(&other_capability)
            .evaluate(&configuration(&trusted), DA, &expected(&trusted))
            .authority(),
        ExecutorAuthority::GateAuthoritative
    );
}

#[test]
fn qualification_is_bound_to_current_approval_policies() {
    let trusted = model_record();
    let mut stale = trusted.clone();
    stale["organization_approval"]["policy_digest"] = json!(DB);
    stale["runtime_policy"]["policy_digest"] = json!(DB);

    assert_ne!(
        validate(&stale)
            .evaluate(&configuration(&trusted), DA, &expected(&trusted))
            .authority(),
        ExecutorAuthority::GateAuthoritative
    );
}

#[test]
fn qualification_is_bound_to_the_requested_approval_dimensions() {
    let value = model_record();

    for dimension in ["scope", "risk", "deployment"] {
        let mut expected = expected(&value);
        match dimension {
            "scope" => expected.requested_scope = "repo:other".to_string(),
            "risk" => expected.requested_risk = "low".to_string(),
            "deployment" => expected.requested_deployment = "public_provider".to_string(),
            _ => unreachable!(),
        }
        assert_ne!(
            validate(&value)
                .evaluate(&configuration(&value), DA, &expected)
                .authority(),
            ExecutorAuthority::GateAuthoritative,
            "{dimension} must match the trusted request"
        );
    }
}

#[test]
fn qualification_is_bound_to_the_current_protocol_version() {
    let value = model_record();
    let mut expected = expected(&value);
    expected.protocol_version = "semantic-executor-v2".to_string();

    let eligibility = validate(&value).evaluate(&configuration(&value), DA, &expected);
    assert_eq!(eligibility.authority(), ExecutorAuthority::AdvisoryOnly);
    assert!(
        eligibility
            .missing_layers()
            .contains(&QualificationLayer::ProtocolValid)
    );
}

#[test]
fn human_qualification_uses_authenticated_permission_policy_not_benchmarks() {
    let value = json!({
        "schema_version": "adoc.executor_qualification.v0",
        "qualification_id": "qual-human-reviewer-v1",
        "capability": {"name": "code_change_assessment", "version": "1"},
        "subject": {
            "kind": "human",
            "principal_id": "principal:reviewer-1",
            "executor_digest": D1,
            "config_digest": D3,
            "permission_policy_digest": DC
        },
        "protocol": {"valid": true, "version": "semantic-executor-v1"},
        "agentdoc_evaluation": {
            "kind": "authenticated_permission",
            "qualified": true,
            "principal_id": "principal:reviewer-1",
            "permission_policy_digest": DC
        },
        "organization_approval": {
            "approved": true,
            "scope": ["repo:billing"],
            "risk": ["high"],
            "deployment": ["human_review"],
            "policy_digest": DC
        },
        "runtime_policy": {
            "eligible": true,
            "operation_digest": DA,
            "policy_digest": DC
        }
    });

    let record = validate(&value);
    assert_eq!(
        record
            .evaluate(&configuration(&value), DA, &expected(&value))
            .authority(),
        ExecutorAuthority::GateAuthoritative
    );
    assert!(
        !record
            .to_canonical_json()
            .expect("serializes")
            .contains("benchmark")
    );
}

#[test]
fn qualification_receipt_preserves_exact_executor_model_and_config_digests() {
    let record = validate(&model_record());
    let json = record.to_canonical_json().expect("serializes");

    assert!(json.contains(D1));
    assert!(json.contains(D2));
    assert!(json.contains(D3));
}

#[test]
fn schema_validity_cannot_forge_a_human_permission_binding() {
    let mut value = model_record();
    value["subject"] = json!({
        "kind": "human",
        "principal_id": "principal:reviewer-1",
        "executor_digest": D1,
        "config_digest": D3,
        "permission_policy_digest": DC
    });
    value["agentdoc_evaluation"] = json!({
        "kind": "authenticated_permission",
        "qualified": true,
        "principal_id": "principal:different-reviewer",
        "permission_policy_digest": DC
    });

    assert!(matches!(
        validate_executor_qualification(&serde_json::to_vec(&value).expect("serializes")),
        Err(ExecutorQualificationError::InvalidDocument { .. })
    ));
}

#[test]
fn unknown_fields_and_non_digest_configuration_are_rejected() {
    let mut unknown = model_record();
    unknown["claimed_gate_authority"] = json!(true);
    let mut malformed = model_record();
    malformed["subject"]["config_digest"] = json!("not-a-digest");

    for value in [unknown, malformed] {
        assert!(matches!(
            validate_executor_qualification(&serde_json::to_vec(&value).expect("serializes")),
            Err(ExecutorQualificationError::InvalidDocument { .. })
        ));
    }
}

#[test]
fn organization_approval_sets_serialize_canonically() {
    let mut value = model_record();
    value["organization_approval"]["scope"] = json!(["repo:z", "repo:a"]);
    value["organization_approval"]["risk"] = json!(["low", "high"]);
    let first = validate(&value).to_canonical_json().expect("serializes");
    value["organization_approval"]["scope"] = json!(["repo:a", "repo:z"]);
    value["organization_approval"]["risk"] = json!(["high", "low"]);

    assert_eq!(
        first,
        validate(&value).to_canonical_json().expect("serializes")
    );
}

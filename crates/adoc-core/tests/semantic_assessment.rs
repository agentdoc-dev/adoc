use adoc_core::{
    CapabilityPolicy, CapabilityPolicyRule, CitationHandle, ContextClass, ContextRequirement,
    ContextUnavailability, ContextUnavailabilityKind, DiagnosticCode, ExactRevision,
    KnowledgeBasis, SemanticContext, SemanticContextBasis, SemanticContextInput,
    SemanticContextItem, SemanticContextSelection, SemanticMateriality, UnavailabilityOutcome,
    UnavailabilityReason, build_semantic_context, validate_semantic_assessment,
};
use chrono::NaiveDate;
use serde_json::json;
use std::{fs, path::PathBuf};

const ASSESSMENT_DIGEST: &str =
    "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const GRAPH_DIGEST: &str =
    "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn revision(value: &str) -> ExactRevision {
    ExactRevision {
        system: "git".to_string(),
        value: value.to_string(),
    }
}

fn context_input() -> SemanticContextInput {
    SemanticContextInput {
        evaluation_date: NaiveDate::from_ymd_opt(2026, 8, 24).expect("valid date"),
        subject_revision: revision("head-sha"),
        source_revision: revision("head-sha"),
        base_revision: revision("base-sha"),
        head_revision: revision("head-sha"),
        basis: SemanticContextBasis {
            assessment_digest: ASSESSMENT_DIGEST.to_string(),
            knowledge_basis: KnowledgeBasis::GraphArtifact {
                digest: GRAPH_DIGEST.to_string(),
            },
        },
        selection: SemanticContextSelection {
            algorithm: "changed-only".to_string(),
            version: "1".to_string(),
            authorized_scope: vec!["repo:billing".to_string()],
        },
        capability_policy: CapabilityPolicy {
            version: "semantic-context-policy-v1".to_string(),
            rules: [
                UnavailabilityReason::Permission,
                UnavailabilityReason::Retention,
                UnavailabilityReason::SourceOutage,
                UnavailabilityReason::Truncation,
                UnavailabilityReason::ResourceLimit,
            ]
            .into_iter()
            .map(|reason| CapabilityPolicyRule {
                reason,
                outcome: UnavailabilityOutcome::Insufficient,
            })
            .collect(),
        },
        context_classes: vec![ContextClass {
            class_id: "changed_knowledge".to_string(),
            requirement: ContextRequirement::Required,
            byte_budget: 4096,
        }],
        items: vec![
            SemanticContextItem {
                handle_id: "hunk-a".to_string(),
                class_id: "changed_knowledge".to_string(),
                scope_ref: "repo:billing".to_string(),
                handle: CitationHandle::DiffHunk {
                    changed_source_id: "src/billing.rs".to_string(),
                    hunk_digest: ASSESSMENT_DIGEST.to_string(),
                },
                content: json!({"diff": "+ durable billing behavior"}),
                truncated: false,
            },
            SemanticContextItem {
                handle_id: "object-a".to_string(),
                class_id: "changed_knowledge".to_string(),
                scope_ref: "repo:billing".to_string(),
                handle: CitationHandle::KnowledgeObject {
                    object_id: "billing.policy".to_string(),
                    semantic_hash: GRAPH_DIGEST.to_string(),
                },
                content: json!({"body": "Current billing policy."}),
                truncated: false,
            },
        ],
        unavailability: Vec::new(),
    }
}

fn context() -> SemanticContext {
    build_semantic_context(context_input()).expect("semantic context builds")
}

fn assessment_json(context: &SemanticContext) -> serde_json::Value {
    json!({
        "schema_version": "adoc.semantic_assessment.v0",
        "context_digest": context.context_digest(),
        "base_revision": {"system": "git", "value": "base-sha"},
        "head_revision": {"system": "git", "value": "head-sha"},
        "identity": {"provider": "codex", "model": "gpt-5"},
        "materiality_policy_version": "adoc.materiality.v0",
        "scope": {"handle_ids": ["object-a", "hunk-a"]},
        "findings": [{
            "finding_id": "finding-001",
            "classification": "extends_existing_knowledge",
            "affected_objects": [{
                "object_id": "billing.policy",
                "content_hash": GRAPH_DIGEST
            }],
            "citations": ["object-a", "hunk-a"],
            "materiality": "material",
            "proposed_disposition": "update_existing",
            "candidate_updates": [{
                "object_id": "billing.policy",
                "body": "Updated billing policy.",
                "fields": {}
            }],
            "unresolved_questions": [],
            "explanation": "The new branch changes durable billing behavior."
        }]
    })
}

#[test]
fn semantic_assessment_round_trip_is_deterministic_and_timestamp_free() {
    let context = context();
    let first = assessment_json(&context);
    let mut second = first.clone();
    second["scope"]["handle_ids"] = json!(["hunk-a", "object-a"]);
    second["findings"][0]["citations"] = json!(["hunk-a", "object-a"]);

    let first = validate_semantic_assessment(
        serde_json::to_vec(&first)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("assessment validates");
    let second = validate_semantic_assessment(
        serde_json::to_vec(&second)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("assessment validates");

    let first = first.to_canonical_json().expect("assessment serializes");
    assert_eq!(first, second.to_canonical_json().expect("serializes"));
    assert!(!first.contains("timestamp"));
    assert!(!first.contains("created_at"));
}

#[test]
fn semantic_assessment_rejects_anonymous_output() {
    let context = context();

    for identity in [json!(null), json!({"provider": "codex"})] {
        let mut document = assessment_json(&context);
        document["identity"] = identity;
        let error = validate_semantic_assessment(
            serde_json::to_vec(&document)
                .expect("fixture serializes")
                .as_slice(),
            &context,
        )
        .expect_err("anonymous output is rejected");

        assert_eq!(
            error.diagnostic_code(),
            DiagnosticCode::AssessmentSemanticIdentityMissing
        );
    }
}

#[test]
fn semantic_assessment_rejects_every_citation_outside_the_exact_context() {
    let context = context();
    let mut fabricated_handle = assessment_json(&context);
    fabricated_handle["scope"]["handle_ids"] = json!(["fabricated-hunk", "object-a"]);
    fabricated_handle["findings"][0]["citations"] = json!(["fabricated-hunk", "object-a"]);

    let mut fabricated_object = assessment_json(&context);
    fabricated_object["findings"][0]["affected_objects"][0]["object_id"] =
        json!("billing.fabricated");

    let mut wrong_context = assessment_json(&context);
    wrong_context["context_digest"] = json!(ASSESSMENT_DIGEST);

    for document in [fabricated_handle, fabricated_object, wrong_context] {
        let error = validate_semantic_assessment(
            serde_json::to_vec(&document)
                .expect("fixture serializes")
                .as_slice(),
            &context,
        )
        .expect_err("citation outside exact context is rejected");

        assert_eq!(
            error.diagnostic_code(),
            DiagnosticCode::AssessmentSemanticCitationInvalid
        );
    }
}

#[test]
fn update_candidates_must_target_a_cited_affected_object() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"][0]["candidate_updates"][0]["object_id"] = json!("admin.secrets");

    let error = validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("an update outside the cited affected set is rejected");

    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticCitationInvalid
    );
}

#[test]
fn human_review_candidates_must_target_a_cited_affected_object() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"][0]["proposed_disposition"] = json!("needs_human_review");
    document["findings"][0]["candidate_updates"][0]["object_id"] = json!("admin.secrets");

    let error = validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("a human-review candidate outside the cited affected set is rejected");

    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticCitationInvalid
    );
}

#[test]
fn create_knowledge_cannot_smuggle_an_unbound_candidate_target() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"][0]["proposed_disposition"] = json!("create_knowledge");
    document["findings"][0]["candidate_updates"][0]["object_id"] = json!("admin.secrets");

    validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("create_knowledge cannot authorize an unbound candidate target");
}

#[test]
fn candidate_body_is_required_even_when_it_is_null() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"][0]["candidate_updates"][0]["body"] = json!(null);
    document["findings"][0]["candidate_updates"][0]["fields"] = json!({"owner": "team-billing"});

    validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("an explicit null body remains valid when fields are present");

    document["findings"][0]["candidate_updates"][0]
        .as_object_mut()
        .expect("candidate object")
        .remove("body");

    validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("the published nullable body field must still be present");
}

#[test]
fn semantic_assessment_rejects_a_different_revision_identity() {
    let context = context();

    for field in ["base_revision", "head_revision"] {
        let mut document = assessment_json(&context);
        document[field]["value"] = json!("other-sha");
        let error = validate_semantic_assessment(
            serde_json::to_vec(&document)
                .expect("fixture serializes")
                .as_slice(),
            &context,
        )
        .expect_err("mismatched revision is rejected");

        assert_eq!(
            error.diagnostic_code(),
            DiagnosticCode::AssessmentSemanticRevisionMismatch
        );
    }
}

#[test]
fn semantic_assessment_rejects_context_with_required_omissions() {
    let mut input = context_input();
    input.items.retain(|item| item.handle_id != "object-a");
    input.unavailability.push(ContextUnavailability {
        record_id: "redacted-object-a".to_string(),
        class_id: "changed_knowledge".to_string(),
        kind: ContextUnavailabilityKind::Redaction,
        reason: UnavailabilityReason::Permission,
    });
    let context = build_semantic_context(input).expect("incomplete context is recorded");

    let error = validate_semantic_assessment(
        serde_json::to_vec(&assessment_json(&context))
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("required omissions make the assessment invalid");

    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticCitationInvalid
    );
}

#[test]
fn every_semantic_assessment_wire_code_is_producible() {
    let context = context();
    let mut cases = Vec::new();

    let mut document = assessment_json(&context);
    document["unexpected"] = json!(true);
    cases.push((document, DiagnosticCode::AssessmentSemanticSchemaInvalid));

    let mut document = assessment_json(&context);
    document["schema_version"] = json!("adoc.semantic_assessment.v99");
    cases.push((
        document,
        DiagnosticCode::AssessmentSemanticVersionUnsupported,
    ));

    let mut document = assessment_json(&context);
    document["findings"][0]["citations"] = json!(["fabricated-hunk"]);
    document["scope"]["handle_ids"] = json!(["fabricated-hunk"]);
    cases.push((document, DiagnosticCode::AssessmentSemanticCitationInvalid));

    let mut document = assessment_json(&context);
    document["findings"][0]["classification"] = json!("probably_fine");
    cases.push((
        document,
        DiagnosticCode::AssessmentSemanticClassificationUnknown,
    ));

    let mut document = assessment_json(&context);
    document["head_revision"]["value"] = json!("other-head");
    cases.push((document, DiagnosticCode::AssessmentSemanticRevisionMismatch));

    let mut document = assessment_json(&context);
    document["identity"] = json!(null);
    cases.push((document, DiagnosticCode::AssessmentSemanticIdentityMissing));

    for (document, expected) in cases {
        let error = validate_semantic_assessment(
            serde_json::to_vec(&document)
                .expect("fixture serializes")
                .as_slice(),
            &context,
        )
        .expect_err("corrupted fixture is rejected");
        assert_eq!(error.diagnostic_code(), expected);
    }
}

#[test]
fn typed_materiality_is_derived_without_reading_explanatory_prose() {
    let context = context();
    let material = validate_semantic_assessment(
        serde_json::to_vec(&assessment_json(&context))
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("material fixture validates");
    assert_eq!(
        material.findings()[0].materiality(),
        SemanticMateriality::Material
    );

    let mut immaterial = assessment_json(&context);
    immaterial["findings"][0]["classification"] = json!("consistent");
    immaterial["findings"][0]["materiality"] = json!("immaterial");
    immaterial["findings"][0]["proposed_disposition"] = json!("no_change_required");
    immaterial["findings"][0]["candidate_updates"] = json!([]);
    immaterial["findings"][0]["explanation"] =
        json!("Free-form words cannot set materiality or gate authority.");
    let immaterial = validate_semantic_assessment(
        serde_json::to_vec(&immaterial)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("immaterial fixture validates");
    assert_eq!(
        immaterial.findings()[0].materiality(),
        SemanticMateriality::Immaterial
    );
    assert!(immaterial.allows_no_change_required());

    let mut false_claim = assessment_json(&context);
    false_claim["findings"][0]["materiality"] = json!("immaterial");
    let error = validate_semantic_assessment(
        serde_json::to_vec(&false_claim)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("producer cannot set materiality contrary to typed facts");
    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticSchemaInvalid
    );
}

#[test]
fn insufficient_evidence_cannot_claim_no_change_required() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"][0]["classification"] = json!("insufficient_evidence");
    document["findings"][0]["materiality"] = json!("undetermined");
    document["findings"][0]["proposed_disposition"] = json!("no_change_required");
    document["findings"][0]["candidate_updates"] = json!([]);

    let error = validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("insufficient evidence cannot produce a negative verdict");
    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticSchemaInvalid
    );
}

#[test]
fn serialized_semantic_assessment_matches_the_published_schema() {
    let context = context();
    let assessment = validate_semantic_assessment(
        serde_json::to_vec(&assessment_json(&context))
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("assessment validates");
    let instance = serde_json::to_value(&assessment).expect("assessment serializes");
    let schema_path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/agent/v0/schema/adoc.semantic_assessment.v0.schema.json");
    let schema: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(schema_path).expect("published schema is readable"),
    )
    .expect("published schema is JSON");
    let validator = jsonschema::validator_for(&schema).expect("published schema compiles");
    let errors = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();

    assert!(errors.is_empty(), "schema validation failed: {errors:#?}");
}

#[test]
fn human_submission_uses_the_identical_contract_boundary() {
    let context = context();
    let model_document = assessment_json(&context);
    let mut human_document = model_document.clone();
    human_document["identity"] = json!({"provider": "human", "model": "structured-assessment-v0"});

    let model = validate_semantic_assessment(
        serde_json::to_vec(&model_document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("model submission validates");
    let human = validate_semantic_assessment(
        serde_json::to_vec(&human_document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect("human submission validates");

    assert_eq!(human.identity().provider, "human");
    assert_eq!(model.findings(), human.findings());
    assert_eq!(
        model.allows_no_change_required(),
        human.allows_no_change_required()
    );
}

#[test]
fn empty_findings_cannot_masquerade_as_a_complete_assessment() {
    let context = context();
    let mut document = assessment_json(&context);
    document["findings"] = json!([]);

    let error = validate_semantic_assessment(
        serde_json::to_vec(&document)
            .expect("fixture serializes")
            .as_slice(),
        &context,
    )
    .expect_err("an empty assessment is not complete");
    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::AssessmentSemanticSchemaInvalid
    );
}

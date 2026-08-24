use adoc_core::{
    CapabilityPolicy, CapabilityPolicyRule, CitationHandle, ContextClass, ContextRequirement,
    ContextUnavailability, ContextUnavailabilityKind, DiagnosticCode, ExactRevision,
    KnowledgeBasis, SemanticContext, SemanticContextBasis, SemanticContextInput,
    SemanticContextItem, SemanticContextSelection, UnavailabilityOutcome, UnavailabilityReason,
    build_semantic_context, validate_semantic_assessment,
};
use chrono::NaiveDate;
use serde_json::json;

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
        "scope": {"handle_ids": ["object-a", "hunk-a"]},
        "findings": [{
            "finding_id": "finding-001",
            "classification": "extends_existing_knowledge",
            "affected_objects": [{
                "object_id": "billing.policy",
                "content_hash": GRAPH_DIGEST
            }],
            "citations": ["object-a", "hunk-a"],
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

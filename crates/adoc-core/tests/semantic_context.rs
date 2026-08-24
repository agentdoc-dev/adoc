use adoc_core::{
    CapabilityPolicy, CapabilityPolicyRule, CitationHandle, ContextClass, ContextRequirement,
    ContextUnavailability, ContextUnavailabilityKind, DiagnosticCode, ExactRevision,
    GraphCitationObject, KnowledgeBasis, SemanticContextBasis, SemanticContextInput,
    SemanticContextItem, SemanticContextOutcome, SemanticContextSelection,
    SemanticContextValidationBasis, UnavailabilityOutcome, UnavailabilityReason,
    build_semantic_context, validate_semantic_context,
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

fn item(handle_id: &str, object_id: &str, semantic_hash: &str) -> SemanticContextItem {
    SemanticContextItem {
        handle_id: handle_id.to_string(),
        class_id: "changed_knowledge".to_string(),
        scope_ref: "repo:billing".to_string(),
        handle: CitationHandle::KnowledgeObject {
            object_id: object_id.to_string(),
            semantic_hash: semantic_hash.to_string(),
        },
        content: json!({"body": format!("context for {object_id}")}),
        truncated: false,
    }
}

fn input(items: Vec<SemanticContextItem>) -> SemanticContextInput {
    SemanticContextInput {
        evaluation_date: NaiveDate::from_ymd_opt(2026, 8, 24).expect("valid date"),
        subject_revision: revision("subject-sha"),
        source_revision: revision("source-sha"),
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
        items,
        unavailability: Vec::new(),
    }
}

fn validation_basis() -> SemanticContextValidationBasis {
    SemanticContextValidationBasis {
        evaluation_date: NaiveDate::from_ymd_opt(2026, 8, 24).expect("valid date"),
        graph_artifact_digest: Some(GRAPH_DIGEST.to_string()),
        graph_objects: vec![
            GraphCitationObject {
                object_id: "billing.alpha".to_string(),
                semantic_hash: ASSESSMENT_DIGEST.to_string(),
                has_source_binding: true,
                evidence_count: 1,
            },
            GraphCitationObject {
                object_id: "billing.beta".to_string(),
                semantic_hash: GRAPH_DIGEST.to_string(),
                has_source_binding: true,
                evidence_count: 1,
            },
        ],
    }
}

#[test]
fn semantic_context_round_trip_is_digest_stable_and_order_independent() {
    let first = build_semantic_context(input(vec![
        item("handle-b", "billing.beta", GRAPH_DIGEST),
        item("handle-a", "billing.alpha", ASSESSMENT_DIGEST),
    ]))
    .expect("valid semantic context");
    let second = build_semantic_context(input(vec![
        item("handle-a", "billing.alpha", ASSESSMENT_DIGEST),
        item("handle-b", "billing.beta", GRAPH_DIGEST),
    ]))
    .expect("valid semantic context");

    assert_eq!(first.context_digest(), second.context_digest());
    assert_eq!(
        first.to_canonical_json().expect("serializes"),
        second.to_canonical_json().expect("serializes")
    );
    assert!(
        !first
            .to_canonical_json()
            .expect("serializes")
            .contains("timestamp")
    );

    let serialized = first.to_canonical_json().expect("serializes");
    let validated = validate_semantic_context(serialized.as_bytes(), &validation_basis())
        .expect("serialized context validates");
    assert_eq!(
        validated.to_canonical_json().expect("serializes"),
        serialized
    );
}

#[test]
fn semantic_context_rejects_unknown_citation_handle_kinds() {
    let context = build_semantic_context(input(vec![item(
        "handle-a",
        "billing.alpha",
        ASSESSMENT_DIGEST,
    )]))
    .expect("valid semantic context");
    let serialized = context.to_canonical_json().expect("serializes");
    let unknown = serialized.replace("knowledge_object", "future_handle");

    let error = validate_semantic_context(unknown.as_bytes(), &validation_basis())
        .expect_err("unknown kind rejected");
    assert!(
        error
            .to_string()
            .contains("unknown variant `future_handle`"),
        "unexpected error: {error}"
    );
}

#[test]
fn semantic_context_rejects_unknown_contract_versions_exactly() {
    let context = build_semantic_context(input(vec![item(
        "handle-a",
        "billing.alpha",
        ASSESSMENT_DIGEST,
    )]))
    .expect("valid semantic context");
    let serialized = context.to_canonical_json().expect("serializes");

    for version in ["adoc.semantic_context.v1", "adoc.semantic_context.v99"] {
        let unknown = serialized.replace("adoc.semantic_context.v0", version);
        let error = validate_semantic_context(unknown.as_bytes(), &validation_basis())
            .expect_err("unknown version rejected");
        assert_eq!(
            error.diagnostic_code(),
            DiagnosticCode::SchemaUnsupportedVersion
        );
    }
}

#[test]
fn semantic_context_round_trips_every_closed_citation_handle_kind() {
    let handles = vec![
        CitationHandle::DiffHunk {
            changed_source_id: "docs/billing.adoc".to_string(),
            hunk_digest: ASSESSMENT_DIGEST.to_string(),
        },
        CitationHandle::SourceAssertion {
            source_assertion_id: "assertion-1".to_string(),
            source_record_id: "record-1".to_string(),
        },
        CitationHandle::SourceBinding {
            object_id: "billing.ready".to_string(),
        },
        CitationHandle::Evidence {
            object_id: "billing.ready".to_string(),
            evidence_index: 0,
        },
    ];
    let items = handles
        .into_iter()
        .enumerate()
        .map(|(index, handle)| SemanticContextItem {
            handle_id: format!("handle-{index}"),
            class_id: "changed_knowledge".to_string(),
            scope_ref: "repo:billing".to_string(),
            handle,
            content: json!({"text": "inert context"}),
            truncated: false,
        })
        .collect();

    let context = build_semantic_context(input(items)).expect("all closed handles are valid");
    let serialized = context.to_canonical_json().expect("serializes");
    let mut basis = validation_basis();
    basis.graph_objects.push(GraphCitationObject {
        object_id: "billing.ready".to_string(),
        semantic_hash: ASSESSMENT_DIGEST.to_string(),
        has_source_binding: true,
        evidence_count: 1,
    });
    let validated =
        validate_semantic_context(serialized.as_bytes(), &basis).expect("round trip validates");

    assert_eq!(
        validated.to_canonical_json().expect("serializes"),
        serialized
    );
    for kind in [
        "diff_hunk",
        "source_assertion",
        "source_binding",
        "evidence",
    ] {
        assert!(serialized.contains(&format!("\"kind\": \"{kind}\"")));
    }
}

#[test]
fn truncated_required_context_is_ineligible_for_no_change_required() {
    let mut semantic_input = input(vec![item("handle-a", "billing.alpha", ASSESSMENT_DIGEST)]);
    semantic_input.context_classes = vec![ContextClass {
        class_id: "changed_knowledge".to_string(),
        requirement: ContextRequirement::Required,
        byte_budget: 1024,
    }];
    semantic_input.items[0].class_id = "changed_knowledge".to_string();
    semantic_input.items[0].truncated = true;

    let context = build_semantic_context(semantic_input).expect("incomplete context is recordable");

    assert_eq!(context.outcome(), SemanticContextOutcome::Insufficient);
    assert!(!context.allows_no_change_required());
    assert!(
        context
            .to_canonical_json()
            .expect("serializes")
            .contains("\"complete\": false")
    );
}

#[test]
fn optional_truncation_is_reported_without_blocking_readiness() {
    let mut semantic_input = input(vec![item(
        "required-item",
        "billing.alpha",
        ASSESSMENT_DIGEST,
    )]);
    semantic_input.context_classes.push(ContextClass {
        class_id: "related_knowledge".to_string(),
        requirement: ContextRequirement::Optional,
        byte_budget: 1024,
    });
    let mut optional_item = item("optional-item", "billing.beta", GRAPH_DIGEST);
    optional_item.class_id = "related_knowledge".to_string();
    optional_item.truncated = true;
    semantic_input.items.push(optional_item);

    let context = build_semantic_context(semantic_input).expect("optional loss is recordable");

    assert_eq!(context.outcome(), SemanticContextOutcome::Ready);
    assert!(context.allows_no_change_required());
    assert!(
        context
            .to_canonical_json()
            .expect("serializes")
            .contains("\"truncated\": true")
    );
}

#[test]
fn semantic_context_rejects_content_over_its_declared_budget() {
    let mut semantic_input = input(vec![item("handle-a", "billing.alpha", ASSESSMENT_DIGEST)]);
    semantic_input.context_classes[0].byte_budget = 1;

    let error = build_semantic_context(semantic_input).expect_err("over-budget context rejected");
    assert!(error.to_string().contains("exceeds its 1-byte budget"));
}

#[test]
fn semantic_context_rejects_forged_derived_coverage() {
    let context = build_semantic_context(input(vec![item(
        "handle-a",
        "billing.alpha",
        ASSESSMENT_DIGEST,
    )]))
    .expect("valid context");
    let serialized = context.to_canonical_json().expect("serializes");
    let forged = serialized.replacen("\"complete\": true", "\"complete\": false", 1);

    let error = validate_semantic_context(forged.as_bytes(), &validation_basis())
        .expect_err("forgery rejected");
    assert_eq!(
        error.to_string(),
        "semantic context coverage or outcome does not match its items"
    );
}

#[test]
fn every_required_unavailability_reason_obeys_the_capability_policy() {
    for reason in [
        UnavailabilityReason::Permission,
        UnavailabilityReason::Retention,
        UnavailabilityReason::SourceOutage,
        UnavailabilityReason::Truncation,
        UnavailabilityReason::ResourceLimit,
    ] {
        let mut semantic_input = input(vec![item("handle-a", "billing.alpha", ASSESSMENT_DIGEST)]);
        semantic_input.unavailability = vec![ContextUnavailability {
            record_id: "unavailable-1".to_string(),
            class_id: "changed_knowledge".to_string(),
            kind: ContextUnavailabilityKind::Omission,
            reason,
        }];
        semantic_input.capability_policy.rules = semantic_input
            .capability_policy
            .rules
            .into_iter()
            .map(|rule| CapabilityPolicyRule {
                reason: rule.reason,
                outcome: if rule.reason == reason {
                    UnavailabilityOutcome::Failed
                } else {
                    UnavailabilityOutcome::Insufficient
                },
            })
            .collect();

        let failed = build_semantic_context(semantic_input).expect("failure is recordable");
        assert_eq!(
            failed.outcome(),
            SemanticContextOutcome::Failed,
            "{reason:?}"
        );
        assert!(!failed.allows_no_change_required());
    }
}

#[test]
fn semantic_context_outcomes_map_to_stable_diagnostics() {
    assert_eq!(
        SemanticContextOutcome::Insufficient.diagnostic_code(),
        Some(DiagnosticCode::SemanticContextInsufficientContext)
    );
    assert_eq!(
        SemanticContextOutcome::Failed.diagnostic_code(),
        Some(DiagnosticCode::SemanticContextFailed)
    );
    assert_eq!(SemanticContextOutcome::Ready.diagnostic_code(), None);
}

#[test]
fn semantic_context_sorts_authorized_scope_and_rejects_items_outside_it() {
    let mut semantic_input = input(vec![item("handle-a", "billing.alpha", ASSESSMENT_DIGEST)]);
    semantic_input.selection = SemanticContextSelection {
        algorithm: "changed-and-related".to_string(),
        version: "1".to_string(),
        authorized_scope: vec!["repo:billing".to_string(), "repo:accounts".to_string()],
    };
    semantic_input.items[0].scope_ref = "repo:billing".to_string();

    let context = build_semantic_context(semantic_input.clone()).expect("authorized item");
    let serialized = context.to_canonical_json().expect("serializes");
    assert!(
        serialized.find("repo:accounts").expect("accounts scope")
            < serialized.find("repo:billing").expect("billing scope")
    );

    semantic_input.items[0].scope_ref = "repo:other".to_string();
    let error = build_semantic_context(semantic_input).expect_err("scope escape rejected");
    assert!(error.to_string().contains("outside authorized scope"));
}

#[test]
fn graph_backed_context_rejects_unresolved_source_binding_coordinates() {
    let mut binding = item("binding", "billing.ready", ASSESSMENT_DIGEST);
    binding.handle = CitationHandle::SourceBinding {
        object_id: "billing.ready".to_string(),
    };
    let context = build_semantic_context(input(vec![binding])).expect("context builds");
    let serialized = context.to_canonical_json().expect("serializes");
    let basis = SemanticContextValidationBasis {
        evaluation_date: NaiveDate::from_ymd_opt(2026, 8, 24).expect("valid date"),
        graph_artifact_digest: Some(GRAPH_DIGEST.to_string()),
        graph_objects: vec![GraphCitationObject {
            object_id: "billing.ready".to_string(),
            semantic_hash: ASSESSMENT_DIGEST.to_string(),
            has_source_binding: false,
            evidence_count: 0,
        }],
    };

    let error = validate_semantic_context(serialized.as_bytes(), &basis)
        .expect_err("missing binding rejected");
    assert_eq!(
        error.diagnostic_code(),
        DiagnosticCode::SemanticContextBasisMismatch
    );
}

#[test]
fn semantic_context_validator_has_no_connector_or_network_dependencies() {
    let source = std::fs::read_to_string(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("src/domain/semantic_context.rs"),
    )
    .expect("semantic context domain source is readable")
    .to_ascii_lowercase();

    for forbidden in [
        "crate::infrastructure",
        "reqwest",
        "github",
        "gitlab",
        "slack",
        "confluence",
    ] {
        assert!(
            !source.contains(forbidden),
            "semantic context validation must not depend on {forbidden}"
        );
    }
}

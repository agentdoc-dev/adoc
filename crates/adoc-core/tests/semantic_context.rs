use adoc_core::{
    CitationHandle, ContextClass, ContextRequirement, ExactRevision, KnowledgeBasis,
    SemanticContextBasis, SemanticContextInput, SemanticContextItem, SemanticContextOutcome,
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
        context_classes: vec![ContextClass {
            class_id: "changed_knowledge".to_string(),
            requirement: ContextRequirement::Required,
            byte_budget: 4096,
        }],
        items,
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
    let validated =
        validate_semantic_context(serialized.as_bytes()).expect("serialized context validates");
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

    let error = validate_semantic_context(unknown.as_bytes()).expect_err("unknown kind rejected");
    assert!(
        error
            .to_string()
            .contains("unknown variant `future_handle`"),
        "unexpected error: {error}"
    );
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
            handle,
            content: json!({"text": "inert context"}),
            truncated: false,
        })
        .collect();

    let context = build_semantic_context(input(items)).expect("all closed handles are valid");
    let serialized = context.to_canonical_json().expect("serializes");
    let validated = validate_semantic_context(serialized.as_bytes()).expect("round trip validates");

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

    let error = validate_semantic_context(forged.as_bytes()).expect_err("forgery rejected");
    assert_eq!(
        error.to_string(),
        "semantic context coverage or outcome does not match its items"
    );
}

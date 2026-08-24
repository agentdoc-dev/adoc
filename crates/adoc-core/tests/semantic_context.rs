use adoc_core::{
    CitationHandle, ExactRevision, KnowledgeBasis, SemanticContextBasis, SemanticContextInput,
    SemanticContextItem, build_semantic_context, validate_semantic_context,
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
        handle: CitationHandle::KnowledgeObject {
            object_id: object_id.to_string(),
            semantic_hash: semantic_hash.to_string(),
        },
        content: json!({"body": format!("context for {object_id}")}),
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

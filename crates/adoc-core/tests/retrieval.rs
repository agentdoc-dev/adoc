use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    DiagnosticCode, GraphDirection, GraphRelationKind, RetrievalEntry, RetrievalEnvelope,
    RetrievalInput, RetrievalMatch, RetrievalRecord, RetrievalRelations, RetrievalSession,
    RetrievalSource, SearchFilters, SearchMode, SearchQuery, SearchRecordScope, SearchResult,
    load_retrieval_session, search, why_object,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

fn workspace_fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("..")
        .join("..")
        .join("tests")
        .join("fixtures")
        .join(relative)
}

fn write_temp_artifact(name: &str, contents: &str) -> tempfile::NamedTempFile {
    let artifact = tempfile::Builder::new()
        .prefix(&format!("adoc-retrieval-{name}-"))
        .suffix(".graph.json")
        .tempfile()
        .expect("temp artifact can be created");
    std::fs::write(artifact.path(), contents).expect("temp artifact can be written");
    artifact
}

fn write_temp_search_artifact(name: &str, contents: &str) -> tempfile::NamedTempFile {
    let artifact = tempfile::Builder::new()
        .prefix(&format!("adoc-retrieval-{name}-"))
        .suffix(".search.json")
        .tempfile()
        .expect("temp search artifact can be created");
    std::fs::write(artifact.path(), contents).expect("temp search artifact can be written");
    artifact
}

fn load_session_from_objects(objects: Vec<Value>) -> RetrievalSession {
    let graph_json = graph_json_from_objects(objects, Vec::new());
    let artifact = write_temp_artifact("search", &graph_json);
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean search fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("search fixture session loads")
}

fn load_session_from_objects_with_vectors(
    objects: Vec<Value>,
    vectors: Vec<(&str, Vec<f32>)>,
) -> RetrievalSession {
    let graph_json = graph_json_from_objects(objects, Vec::new());
    let artifact = write_temp_artifact("hybrid-graph", &graph_json);
    let search_document = serde_json::json!({
        "schema_version": "adoc.search.v1",
        "model": {
            "id": "bge-small-en-v1.5",
            "provider": "fastembed",
            "dim": 384
        },
        "graph_artifact_hash": sha256_prefixed(graph_json.as_bytes()),
        "embeddings": vectors
            .into_iter()
            .map(|(id, vector)| serde_json::json!({
                "id": id,
                "entry_kind": "knowledge_object",
                "content_hash": "sha256:test",
                "vector": vector
            }))
            .collect::<Vec<_>>()
    });
    let search_artifact = write_temp_search_artifact(
        "hybrid-search",
        &serde_json::to_string_pretty(&search_document).expect("search fixture serializes"),
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: Some(search_artifact.path().to_path_buf()),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean hybrid fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("hybrid fixture session loads")
}

fn load_session_from_objects_with_graph(
    objects: Vec<Value>,
    edges: Vec<Value>,
) -> RetrievalSession {
    let graph_json = graph_json_from_objects(objects, edges);
    let artifact = write_temp_artifact("graph-search", &graph_json);

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean graph-backed fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("graph-backed fixture session loads")
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    use std::fmt::Write;

    let digest = Sha256::digest(bytes);
    let mut output = String::from("sha256:");
    for byte in digest {
        let _ = write!(output, "{byte:02x}");
    }
    output
}

fn graph_json_from_objects(objects: Vec<Value>, edges: Vec<Value>) -> String {
    let document = json!({
          "schema_version": "adoc.graph.v5",
    "repository_identity": null,
          "nodes": objects,
          "edges": edges,
          "diagnostics": []
      });
    let canonical: CanonicalGraphDocument =
        serde_json::from_value(document).expect("graph fixture has canonical shape");
    serde_json::to_string_pretty(&canonical).expect("search fixture serializes to graph JSON")
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalGraphDocument {
    schema_version: String,
    repository_identity: Option<Value>,
    nodes: Vec<CanonicalGraphNode>,
    edges: Vec<CanonicalGraphEdge>,
    diagnostics: Vec<Value>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum CanonicalGraphNode {
    Page(CanonicalPageNode),
    Heading(CanonicalBlockNode),
    Paragraph(CanonicalBlockNode),
    List(CanonicalBlockNode),
    CodeBlock(CanonicalBlockNode),
    KnowledgeObject(CanonicalKnowledgeObject),
}

/// V1.7.2: mirrors `GraphPageNode` serialization order so prose fixtures
/// round-trip byte-identically for the graph_artifact_hash check.
#[derive(Debug, Serialize, Deserialize)]
struct CanonicalPageNode {
    id: String,
    order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    title: Option<String>,
    source_path: String,
}

/// V1.7.2: mirrors `GraphBlockNode` serialization order.
#[derive(Debug, Serialize, Deserialize)]
struct CanonicalBlockNode {
    id: String,
    page_id: String,
    order: u32,
    #[serde(skip_serializing_if = "Option::is_none")]
    level: Option<u8>,
    #[serde(skip_serializing_if = "Option::is_none")]
    text: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    language: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    code: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    items: Vec<String>,
    source_span: CanonicalSourceSpan,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalKnowledgeObject {
    id: String,
    kind: String,
    content_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    status: Option<String>,
    body: String,
    page_id: String,
    source_span: CanonicalSourceSpan,
    fields: std::collections::BTreeMap<String, String>,
    relations: CanonicalRelations,
    /// V5.8: typed evidence array.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    evidence: Vec<CanonicalEvidence>,
}

/// V5.8: evidence entry in the graph node's `evidence` array.
#[derive(Debug, Serialize, Deserialize)]
struct CanonicalEvidence {
    kind: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    value: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    reference: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalSourceSpan {
    path: String,
    line: u32,
    column: u32,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalRelations {
    depends_on: Vec<String>,
    supersedes: Vec<String>,
    related_to: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CanonicalGraphEdge {
    kind: String,
    source: String,
    target: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    relation: Option<GraphRelationKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    order: Option<u32>,
}

fn relation_edge(source: &str, relation: GraphRelationKind, target: &str) -> Value {
    json!({
        "kind": "relation",
        "source": source,
        "target": target,
        "relation": relation.as_str()
    })
}

fn load_workspace_fixture_session(relative: &str) -> RetrievalSession {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: workspace_fixture_path(relative),
        search_artifact_path: None,
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("fixture retrieval session loads")
}

fn retrieval_filter_object(
    id: &str,
    kind: &str,
    status: Option<&str>,
    owner: Option<&str>,
    source_path: &str,
) -> Value {
    let mut fields = serde_json::Map::new();
    if let Some(owner) = owner {
        fields.insert("owner".to_string(), json!(owner));
    }

    let mut object = json!({
        "type": "knowledge_object",
        "id": id,
        "kind": kind,
        "content_hash": format!("sha256:{id}"),
        "body": "Filter fixture body.",
        "page_id": "team.page",
        "source_span": {
            "path": source_path,
            "line": 1,
            "column": 1
        },
        "fields": fields,
        "relations": {
            "depends_on": [],
            "supersedes": [],
            "related_to": []
        }
    });
    if let Some(status) = status {
        object["status"] = json!(status);
    }
    object
}

fn retrieval_search_object(
    id: &str,
    kind: &str,
    status: Option<&str>,
    owner: Option<&str>,
    source_path: &str,
    body: &str,
) -> Value {
    let mut object = retrieval_filter_object(id, kind, status, owner, source_path);
    object["body"] = json!(body);
    object
}

fn verified_claim_graph_artifact() -> tempfile::NamedTempFile {
    let mut fields = std::collections::BTreeMap::new();
    fields.insert("owner".to_string(), "team-billing".to_string());
    fields.insert("verified_at".to_string(), "2026-05-05".to_string());
    // V5.8: evidence is in the typed "evidence" array, not in "fields".
    let object = json!({
        "type": "knowledge_object",
        "id": "billing.verified-credits",
        "kind": "claim",
        "content_hash": "sha256:billing.verified-credits",
        "status": "verified",
        "body": "Credits are verified by the payments ledger.",
        "page_id": "team.billing",
        "source_span": {
            "path": "tests/fixtures/claim/valid_verified_claim_with_all_evidence/input.adoc",
            "line": 5,
            "column": 1
        },
        "fields": fields,
        "relations": {
            "depends_on": [],
            "supersedes": [],
            "related_to": []
        },
        "evidence": [
            { "kind": "source_code", "value": "payments ledger" },
            { "kind": "test", "value": "cargo test billing_credits" },
            { "kind": "human_review", "value": "qa-team" }
        ]
    });
    write_temp_artifact(
        "verified-claim",
        &graph_json_from_objects(vec![object], Vec::new()),
    )
}

fn lexical_query(text: &str, top: usize, filters: SearchFilters) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        mode: SearchMode::Lexical,
        filters,
        top: NonZeroUsize::new(top).expect("test search top is non-zero"),
        query_vector: None,
        scope: SearchRecordScope::default(),
    }
}

fn hybrid_query(
    text: &str,
    query_vector: Vec<f32>,
    top: usize,
    filters: SearchFilters,
) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        mode: SearchMode::Hybrid,
        filters,
        top: NonZeroUsize::new(top).expect("test search top is non-zero"),
        query_vector: Some(query_vector),
        scope: SearchRecordScope::default(),
    }
}

fn semantic_query(
    text: &str,
    query_vector: Vec<f32>,
    top: usize,
    filters: SearchFilters,
) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        mode: SearchMode::Semantic,
        filters,
        top: NonZeroUsize::new(top).expect("test search top is non-zero"),
        query_vector: Some(query_vector),
        scope: SearchRecordScope::default(),
    }
}

fn search_ids(result: &SearchResult) -> Vec<&str> {
    result.records.iter().map(|record| record.id()).collect()
}

fn search_ranks(result: &SearchResult) -> Vec<u32> {
    result
        .records
        .iter()
        .map(|record| {
            record
                .search_match()
                .expect("search result records include lexical match metadata")
                .result_rank
        })
        .collect()
}

fn search_lexical_ranks(result: &SearchResult) -> Vec<Option<u32>> {
    result
        .records
        .iter()
        .map(|record| {
            record
                .search_match()
                .expect("search result records include lexical match metadata")
                .lexical_rank
        })
        .collect()
}

#[test]
fn hybrid_match_serializes_rrf_score_and_omits_missing_rank_fields() {
    let record = RetrievalRecord {
        id: "billing.hybrid".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        severity: None,
        trust: None,
        content_hash: "sha256:billing.hybrid".to_string(),
        owner: None,
        verified_at: None,
        body: "Hybrid result.".to_string(),
        source: RetrievalSource {
            path: "docs/billing.adoc".to_string(),
            line: 1,
            column: 1,
        },
        evidence: std::collections::BTreeMap::new(),
        fields: std::collections::BTreeMap::new(),
        relations: RetrievalRelations::default(),
        search_match: Some(RetrievalMatch::hybrid(1, 0.0312, Some(2), None)),
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
        resolved_questions: Vec::new(),
    };

    let value = serde_json::to_value(RetrievalEnvelope::new(
        vec![RetrievalEntry::KnowledgeObject(record)],
        Vec::new(),
    ))
    .expect("retrieval envelope serializes");
    let search_match = value["records"][0]["match"]
        .as_object()
        .expect("match block is an object");

    assert_eq!(search_match["mode"], "hybrid");
    assert_eq!(search_match["result_rank"], 1);
    assert_eq!(search_match["rrf_score"], 0.0312);
    assert_eq!(search_match["lexical_rank"], 2);
    assert!(
        !search_match.contains_key("vector_rank"),
        "missing rank fields must be omitted, got {search_match:?}"
    );
    assert!(
        !search_match.contains_key("cosine_score"),
        "hybrid records must not include cosine_score, got {search_match:?}"
    );
}

fn assert_top_3_contains(session: &RetrievalSession, query: &str, expected_id: &str) {
    let result = search(session, lexical_query(query, 3, SearchFilters::default()));

    assert!(
        result.diagnostics.is_empty(),
        "expected clean search for {query:?}, got {:?}",
        result.diagnostics
    );
    let ids = search_ids(&result);
    assert!(
        ids.contains(&expected_id),
        "expected {expected_id} in top 3 for {query:?}, got {ids:?}"
    );
}

#[test]
fn hybrid_search_fuses_lexical_and_vector_results_and_reports_match_metadata() {
    let session = load_session_from_objects_with_vectors(
        vec![
            retrieval_search_object(
                "billing.lexical-only",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "target target target",
            ),
            retrieval_search_object(
                "billing.blended",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "target",
            ),
            retrieval_search_object(
                "billing.semantic-only",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "unrelated",
            ),
        ],
        vec![
            ("billing.lexical-only", vec![0.0, 1.0]),
            ("billing.blended", vec![1.0, 0.0]),
            ("billing.semantic-only", vec![1.0, 0.0]),
        ],
    );

    let result = search(
        &session,
        hybrid_query("target", vec![1.0, 0.0], 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(
        search_ids(&result).first().copied(),
        Some("billing.blended")
    );
    let search_match = result.records[0]
        .search_match()
        .expect("hybrid result has match metadata");
    assert_eq!(search_match.mode, SearchMode::Hybrid);
    assert!(search_match.rrf_score.is_some());
    assert_eq!(search_match.lexical_rank, Some(2));
    assert_eq!(search_match.vector_rank, Some(1));
    assert_eq!(search_match.cosine_score, None);
}

#[test]
fn hybrid_search_filters_after_ranking_and_preserves_full_pool_ranks() {
    let session = load_session_from_objects_with_vectors(
        vec![
            retrieval_search_object(
                "billing.top",
                "claim",
                None,
                Some("team-a"),
                "docs/billing.adoc",
                "target target target",
            ),
            retrieval_search_object(
                "billing.keep",
                "claim",
                None,
                Some("team-b"),
                "docs/billing.adoc",
                "target",
            ),
        ],
        vec![
            ("billing.top", vec![1.0, 0.0]),
            ("billing.keep", vec![0.8, 0.2]),
        ],
    );

    let result = search(
        &session,
        hybrid_query(
            "target",
            vec![1.0, 0.0],
            1,
            SearchFilters {
                owner: Some("team-b".to_string()),
                ..SearchFilters::default()
            },
        ),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.keep"]);
    let search_match = result.records[0].search_match().unwrap();
    assert_eq!(search_match.mode, SearchMode::Hybrid);
    assert_eq!(search_match.lexical_rank, Some(2));
    assert_eq!(search_match.vector_rank, Some(2));
}

/// V1.7.1 review follow-up: pins ride above the `top` budget in hybrid mode —
/// a prefix-pinned Knowledge Object and the best-scored hit both return at
/// `top = 1`.
#[test]
fn hybrid_search_pins_ride_above_the_scored_budget() {
    let session = load_session_from_objects_with_vectors(
        vec![
            retrieval_search_object(
                "billing.credits",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "Prefix target.",
            ),
            retrieval_search_object(
                "support.winner",
                "claim",
                None,
                Some("team-support"),
                "docs/support.adoc",
                "Vector winner.",
            ),
        ],
        vec![
            ("billing.credits", vec![0.0, 1.0]),
            ("support.winner", vec![1.0, 0.0]),
        ],
    );

    let result = search(
        &session,
        hybrid_query(
            "billing.credits",
            vec![1.0, 0.0],
            1,
            SearchFilters::default(),
        ),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.credits", "support.winner"]);
}

#[test]
fn lexical_search_indexes_v0_evidence_fields() {
    let mut object = retrieval_search_object(
        "billing.evidence",
        "claim",
        Some("verified"),
        Some("team-billing"),
        "docs/billing.adoc",
        "Credits require review.",
    );
    // V5.8: evidence is in the typed evidence array, not fields.
    object["evidence"] = json!([
        { "kind": "source_code", "value": "refund runbook" },
        { "kind": "test", "value": "cargo test refunds" },
        { "kind": "human_review", "value": "qa-billing" }
    ]);
    let session = load_session_from_objects(vec![object]);

    let result = search(
        &session,
        lexical_query("refund runbook", 1, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.evidence"]);
}

#[test]
fn retrieval_metadata_classification_is_shared_by_filters_records_and_lexical_index() {
    let mut object = retrieval_search_object(
        "billing.metadata",
        "claim",
        Some("verified"),
        Some("team-billing"),
        "docs/billing.adoc",
        "Credits require review.",
    );
    object["fields"]["verified_at"] = json!("2026-05-05");
    object["fields"]["expires_at"] = json!("2026-06-01");
    // V5.8: evidence is in the typed evidence array.
    object["evidence"] = json!([
        { "kind": "human_review", "value": "qa-billing" }
    ]);
    let session = load_session_from_objects(vec![object]);

    let result = search(
        &session,
        lexical_query(
            "qa-billing",
            1,
            SearchFilters {
                owner: Some("team-billing".to_string()),
                ..SearchFilters::default()
            },
        ),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.metadata"]);
    let record = result.records[0]
        .as_knowledge_object()
        .expect("knowledge object record");
    assert_eq!(record.owner.as_deref(), Some("team-billing"));
    assert_eq!(record.verified_at.as_deref(), Some("2026-05-05"));
    // V5.8: reviewed_by maps to human_review EvidenceKind.
    assert_eq!(
        record.evidence.get("human_review").map(String::as_str),
        Some("qa-billing")
    );
    assert_eq!(
        record.fields.get("expires_at").map(String::as_str),
        Some("2026-06-01")
    );
}

#[test]
fn retrieval_session_rejects_malformed_graph_artifacts_through_graph_index_validation() {
    let graph_json = graph_json_from_objects(
        vec![
            retrieval_search_object(
                "billing.duplicate",
                "claim",
                Some("draft"),
                None,
                "docs/billing.adoc",
                "First.",
            ),
            retrieval_search_object(
                "billing.duplicate",
                "claim",
                Some("draft"),
                None,
                "docs/billing.adoc",
                "Second.",
            ),
        ],
        Vec::new(),
    );
    let artifact = write_temp_artifact("duplicate", &graph_json);

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(result.session.is_none());
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::IdDuplicateInArtifact
    );
}

#[test]
fn semantic_search_pins_id_prefix_matches_before_vector_hits() {
    let session = load_session_from_objects_with_vectors(
        vec![
            retrieval_search_object(
                "billing.credits",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "Prefix target.",
            ),
            retrieval_search_object(
                "support.vector",
                "claim",
                None,
                Some("team-support"),
                "docs/support.adoc",
                "Vector winner.",
            ),
        ],
        vec![
            ("billing.credits", vec![0.0, 1.0]),
            ("support.vector", vec![1.0, 0.0]),
        ],
    );

    let result = search(
        &session,
        semantic_query(
            "billing.credits",
            vec![1.0, 0.0],
            1,
            SearchFilters::default(),
        ),
    );

    assert!(result.diagnostics.is_empty());
    // The pin rides above the top=1 budget; the vector winner keeps the
    // single scored slot (V1.7.1 review follow-up).
    assert_eq!(search_ids(&result), ["billing.credits", "support.vector"]);
    let search_match = result.records[0].search_match().unwrap();
    assert_eq!(search_match.mode, SearchMode::Semantic);
    assert_eq!(search_match.vector_rank, Some(2));
}

#[test]
fn semantic_search_pins_id_prefix_matches_without_vector_hit() {
    let session = load_session_from_objects_with_vectors(
        vec![
            retrieval_search_object(
                "billing.new-object",
                "claim",
                None,
                Some("team-billing"),
                "docs/billing.adoc",
                "New object missing from partial search sidecar.",
            ),
            retrieval_search_object(
                "support.vector",
                "claim",
                None,
                Some("team-support"),
                "docs/support.adoc",
                "Vector winner.",
            ),
        ],
        vec![("support.vector", vec![1.0, 0.0])],
    );

    let result = search(
        &session,
        semantic_query(
            "billing.new-object",
            vec![1.0, 0.0],
            1,
            SearchFilters::default(),
        ),
    );

    assert!(result.diagnostics.is_empty());
    // The pin rides above the top=1 budget even without a vector hit of its
    // own (V1.7.1 review follow-up).
    assert_eq!(
        search_ids(&result),
        ["billing.new-object", "support.vector"]
    );
    let search_match = result.records[0].search_match().unwrap();
    assert_eq!(search_match.mode, SearchMode::Semantic);
    assert_eq!(search_match.vector_rank, None);
    assert_eq!(search_match.cosine_score, None);
}

#[test]
fn hybrid_search_requires_query_vector_when_vector_index_is_loaded() {
    let session = load_session_from_objects_with_vectors(
        vec![retrieval_search_object(
            "billing.vector",
            "claim",
            None,
            Some("team-billing"),
            "docs/billing.adoc",
            "target",
        )],
        vec![("billing.vector", vec![1.0, 0.0])],
    );

    let result = search(
        &session,
        SearchQuery {
            text: "target".to_string(),
            mode: SearchMode::Hybrid,
            filters: SearchFilters::default(),
            top: NonZeroUsize::new(1).unwrap(),
            query_vector: None,
            scope: SearchRecordScope::default(),
        },
    );

    assert!(result.records.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::EmbedComputeFailed
    );
}

#[test]
fn retrieval_search_billing_pilot_subset_returns_benchmark_matches_in_top_3() {
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.graph.json");

    assert_top_3_contains(&session, "credit ledger", "billing.credits.ledger-source");
    assert_top_3_contains(&session, "refund audit", "billing.refunds.audit-required");
    assert_top_3_contains(
        &session,
        "entitlement payment",
        "billing.entitlements.sync-after-payment",
    );
}

#[test]
fn retrieval_search_billing_pilot_subset_pins_exact_and_prefix_ids() {
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.graph.json");

    let exact = search(
        &session,
        lexical_query(
            "billing.credits.decrement-after-success",
            3,
            SearchFilters::default(),
        ),
    );
    assert!(exact.diagnostics.is_empty());
    assert_eq!(
        search_ids(&exact).first().copied(),
        Some("billing.credits.decrement-after-success")
    );

    // Pins ride above the `top` budget (V1.7.1 review follow-up): the four
    // prefix pins return in addition to the four scored slots.
    let prefix = search(
        &session,
        lexical_query("billing.credits", 4, SearchFilters::default()),
    );
    assert!(prefix.diagnostics.is_empty());
    assert_eq!(
        search_ids(&prefix)[..4],
        [
            "billing.credits",
            "billing.credits.nonnegative",
            "billing.credits.ledger-source",
            "billing.credits.decrement-after-success"
        ]
    );
    assert_eq!(prefix.records.len(), 8);
    assert_eq!(search_ranks(&prefix), [1, 2, 3, 4, 5, 6, 7, 8]);
}

#[test]
fn retrieval_search_billing_pilot_subset_covers_filters_and_tie_ordering() {
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.graph.json");

    let filtered = search(
        &session,
        lexical_query(
            "ledger",
            3,
            SearchFilters {
                kind: Some("decision".to_string()),
                status: Some("accepted".to_string()),
                owner: Some("team-billing".to_string()),
                source_path: Some("03-decisions.adoc".to_string()),
                ..SearchFilters::default()
            },
        ),
    );
    assert!(filtered.diagnostics.is_empty());
    assert_eq!(
        search_ids(&filtered).first().copied(),
        Some("billing.decision.ledger-first")
    );

    let ties = search(
        &session,
        lexical_query("tie rank", 3, SearchFilters::default()),
    );
    assert!(ties.diagnostics.is_empty());
    assert_eq!(
        search_ids(&ties),
        ["billing.tie.alpha", "billing.tie.beta", "billing.tie.gamma"]
    );
    assert_eq!(search_ranks(&ties), [1, 2, 3]);
}

#[test]
fn retrieval_search_empty_fixture_returns_no_matches_without_diagnostics() {
    let session = load_workspace_fixture_session("v1_2_search/empty.graph.json");

    let result = search(
        &session,
        lexical_query("credit ledger", 3, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn search_filter_matches_case_insensitive_substrings_on_object_metadata() {
    let session = load_session_from_objects(vec![retrieval_filter_object(
        "billing.verified-credits",
        "claim",
        Some("verified"),
        Some("Team-Billing"),
        "docs/billing/credits.adoc",
    )]);
    let filters = SearchFilters {
        kind: Some("CLA".to_string()),
        status: Some("VERI".to_string()),
        owner: Some("billing".to_string()),
        source_path: Some("CREDITS.ADOC".to_string()),
        ..SearchFilters::default()
    };

    let result = search(&session, lexical_query("", 10, filters));

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.verified-credits"]);
}

#[test]
fn search_pins_exact_object_id_as_rank_one() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits",
            "claim",
            Some("verified"),
            Some("team-billing"),
            "docs/billing.adoc",
            "Short exact ID body.",
        ),
        retrieval_search_object(
            "support.credits-heavy",
            "claim",
            Some("verified"),
            Some("team-support"),
            "docs/support.adoc",
            "billing credits billing credits billing credits billing credits",
        ),
    ]);

    let result = search(
        &session,
        lexical_query("billing.credits", 5, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(result.records[0].id(), "billing.credits");
    assert_eq!(
        result.records[0].search_match(),
        Some(&RetrievalMatch::lexical(1, Some(2)))
    );
}

#[test]
fn search_pins_id_prefix_matches_by_length_then_lexical_before_bm25() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits.b",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix match beta.",
        ),
        retrieval_search_object(
            "support.heavy",
            "claim",
            None,
            None,
            "docs/support.adoc",
            "billing credit billing credit billing credit billing credit billing credit",
        ),
        retrieval_search_object(
            "billing.credit",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix match exact.",
        ),
        retrieval_search_object(
            "billing.credits.a",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix match alpha.",
        ),
        retrieval_search_object(
            "billing.credits",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix match plural.",
        ),
    ]);

    let result = search(
        &session,
        lexical_query("billing.credit", 5, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(
        search_ids(&result),
        [
            "billing.credit",
            "billing.credits",
            "billing.credits.a",
            "billing.credits.b",
            "support.heavy"
        ]
    );
    assert_eq!(search_ranks(&result), [1, 2, 3, 4, 5]);
}

#[test]
fn search_result_rank_tracks_pins_while_lexical_rank_is_omitted_for_pinned_only_hits() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "No matching prefix token.",
        ),
        retrieval_search_object(
            "support.heavy",
            "claim",
            None,
            None,
            "docs/support.adoc",
            "billing billing billing",
        ),
    ]);

    let result = search(&session, lexical_query("bil", 2, SearchFilters::default()));

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), ["billing.credits"]);
    assert_eq!(search_ranks(&result), [1]);
    assert_eq!(search_lexical_ranks(&result), [None]);
}

#[test]
fn search_id_prefix_pins_are_case_sensitive_raw_prefix_matches() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix target.",
        ),
        retrieval_search_object(
            "support.heavy",
            "claim",
            None,
            None,
            "docs/support.adoc",
            "billing credits billing credits billing credits billing credits",
        ),
    ]);

    let lowercase = search(
        &session,
        lexical_query("billing.credits", 2, SearchFilters::default()),
    );
    let uppercase = search(
        &session,
        lexical_query("Billing.Credits", 2, SearchFilters::default()),
    );

    assert!(lowercase.diagnostics.is_empty());
    assert!(uppercase.diagnostics.is_empty());
    assert_eq!(
        search_ids(&lowercase).first().copied(),
        Some("billing.credits")
    );
    assert_eq!(
        search_ids(&uppercase).first().copied(),
        Some("support.heavy")
    );
}

#[test]
fn search_id_prefix_pins_namespace_queries_before_bm25_hits() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.refunds",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix target refunds.",
        ),
        retrieval_search_object(
            "billing.credits",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "Prefix target credits.",
        ),
        retrieval_search_object(
            "support.heavy",
            "claim",
            None,
            None,
            "docs/support.adoc",
            "billing billing billing billing",
        ),
    ]);

    let result = search(
        &session,
        lexical_query("billing.", 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(
        search_ids(&result),
        ["billing.credits", "billing.refunds", "support.heavy"]
    );
    assert_eq!(search_ranks(&result), [1, 2, 3]);
}

#[test]
fn search_is_deterministic_when_repeated_on_same_session() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits.depth",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "credits credits ledger",
        ),
        retrieval_search_object(
            "billing.credits.single",
            "claim",
            None,
            None,
            "docs/billing.adoc",
            "credits",
        ),
        retrieval_search_object(
            "support.credits",
            "claim",
            None,
            None,
            "docs/support.adoc",
            "credits support",
        ),
    ]);
    let query = lexical_query("credits", 3, SearchFilters::default());

    let first = search(&session, query.clone());
    let second = search(&session, query);

    assert!(first.diagnostics.is_empty());
    assert_eq!(first.records, second.records);
    assert_eq!(first.diagnostics, second.diagnostics);
}

#[test]
fn search_applies_filters_individually_and_combined_before_ranking() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.verified-credits",
            "claim",
            Some("verified"),
            Some("team-billing"),
            "docs/billing.adoc",
            "credits billing verified",
        ),
        retrieval_search_object(
            "billing.draft-credits",
            "claim",
            Some("draft"),
            Some("team-billing"),
            "docs/billing.adoc",
            "credits billing draft",
        ),
        retrieval_search_object(
            "architecture.credit-decision",
            "decision",
            Some("accepted"),
            Some("team-architecture"),
            "docs/architecture.adoc",
            "credits architecture decision",
        ),
        retrieval_search_object(
            "support.credit-warning",
            "warning",
            Some("high"),
            Some("team-support"),
            "docs/support.adoc",
            "credits support warning",
        ),
    ]);

    let by_kind = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                kind: Some("claim".to_string()),
                ..SearchFilters::default()
            },
        ),
    );
    let by_status = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                status: Some("verified".to_string()),
                ..SearchFilters::default()
            },
        ),
    );
    let by_owner = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                owner: Some("architecture".to_string()),
                ..SearchFilters::default()
            },
        ),
    );
    let by_source = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                source_path: Some("support".to_string()),
                ..SearchFilters::default()
            },
        ),
    );
    let combined = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                kind: Some("claim".to_string()),
                status: Some("draft".to_string()),
                owner: Some("team-billing".to_string()),
                source_path: Some("billing.adoc".to_string()),
                ..SearchFilters::default()
            },
        ),
    );

    assert_eq!(
        search_ids(&by_kind),
        ["billing.draft-credits", "billing.verified-credits"]
    );
    assert_eq!(search_ids(&by_status), ["billing.verified-credits"]);
    assert_eq!(search_ids(&by_owner), ["architecture.credit-decision"]);
    assert_eq!(search_ids(&by_source), ["support.credit-warning"]);
    assert_eq!(search_ids(&combined), ["billing.draft-credits"]);
    assert!(combined.diagnostics.is_empty());
}

#[test]
fn search_related_to_filters_candidates_without_changing_unfiltered_search() {
    let session = load_session_from_objects_with_graph(
        vec![
            retrieval_search_object(
                "billing.root",
                "claim",
                Some("draft"),
                None,
                "docs/graph.adoc",
                "root target",
            ),
            retrieval_search_object(
                "billing.alpha",
                "claim",
                Some("draft"),
                None,
                "docs/graph.adoc",
                "target target",
            ),
            retrieval_search_object(
                "billing.beta",
                "claim",
                Some("draft"),
                None,
                "docs/graph.adoc",
                "target target target",
            ),
            retrieval_search_object(
                "billing.gamma",
                "claim",
                Some("draft"),
                None,
                "docs/graph.adoc",
                "target",
            ),
        ],
        vec![
            relation_edge(
                "billing.root",
                GraphRelationKind::DependsOn,
                "billing.alpha",
            ),
            relation_edge(
                "billing.root",
                GraphRelationKind::DependsOn,
                "billing.gamma",
            ),
        ],
    );

    let unfiltered = search(
        &session,
        lexical_query("target", 10, SearchFilters::default()),
    );
    let filtered = search(
        &session,
        lexical_query(
            "target",
            10,
            SearchFilters {
                related_to: Some("billing.root".to_string()),
                relation: Some(GraphRelationKind::DependsOn),
                direction: Some(GraphDirection::Outgoing),
                ..SearchFilters::default()
            },
        ),
    );

    assert!(unfiltered.diagnostics.is_empty());
    assert!(filtered.diagnostics.is_empty());
    assert!(
        search_ids(&unfiltered).contains(&"billing.beta"),
        "unfiltered search should retain unrelated objects"
    );
    assert_eq!(search_ids(&filtered), ["billing.alpha", "billing.gamma"]);
}

#[test]
fn search_returns_empty_without_diagnostics_for_valid_filters_with_empty_intersection() {
    let session = load_session_from_objects(vec![
        retrieval_search_object(
            "billing.credits",
            "claim",
            Some("verified"),
            Some("team-billing"),
            "docs/billing.adoc",
            "credits billing",
        ),
        retrieval_search_object(
            "architecture.credits",
            "decision",
            Some("accepted"),
            Some("team-architecture"),
            "docs/architecture.adoc",
            "credits architecture",
        ),
    ]);

    let result = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                kind: Some("claim".to_string()),
                owner: Some("team-architecture".to_string()),
                ..SearchFilters::default()
            },
        ),
    );

    assert!(result.records.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn search_returns_invalid_filter_diagnostics_without_records() {
    let session = load_session_from_objects(vec![retrieval_search_object(
        "billing.credits",
        "claim",
        Some("verified"),
        Some("team-billing"),
        "docs/billing.adoc",
        "credits billing",
    )]);

    let result = search(
        &session,
        lexical_query(
            "credits",
            10,
            SearchFilters {
                kind: Some("decision".to_string()),
                ..SearchFilters::default()
            },
        ),
    );

    assert!(result.records.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::SearchInvalidFilter
    );
}

#[test]
fn search_returns_empty_without_diagnostics_for_empty_artifact_and_no_matches() {
    let empty_session = load_session_from_objects(Vec::new());
    let empty_result = search(
        &empty_session,
        lexical_query("credits", 10, SearchFilters::default()),
    );

    assert!(empty_result.records.is_empty());
    assert!(empty_result.diagnostics.is_empty());

    let populated_session = load_session_from_objects(vec![retrieval_search_object(
        "billing.credits",
        "claim",
        None,
        None,
        "docs/billing.adoc",
        "credits billing",
    )]);

    let no_match = search(
        &populated_session,
        lexical_query("refunds", 10, SearchFilters::default()),
    );

    assert!(no_match.records.is_empty());
    assert!(no_match.diagnostics.is_empty());
}

#[test]
fn search_filter_rejects_missing_status_or_owner_when_filter_is_supplied() {
    let session = load_session_from_objects(vec![retrieval_filter_object(
        "glossary.credit",
        "glossary",
        None,
        None,
        "docs/glossary.adoc",
    )]);

    let missing_status = search(
        &session,
        lexical_query(
            "",
            10,
            SearchFilters {
                kind: None,
                status: Some("verified".to_string()),
                owner: None,
                source_path: None,
                ..SearchFilters::default()
            },
        ),
    );
    let missing_owner = search(
        &session,
        lexical_query(
            "",
            10,
            SearchFilters {
                kind: None,
                status: None,
                owner: Some("team".to_string()),
                source_path: None,
                ..SearchFilters::default()
            },
        ),
    );

    assert_eq!(
        missing_status.diagnostics[0].code,
        DiagnosticCode::SearchInvalidFilter
    );
    assert_eq!(
        missing_owner.diagnostics[0].code,
        DiagnosticCode::SearchInvalidFilter
    );
}

#[test]
fn search_filter_validation_checks_each_supplied_filter_independently() {
    let objects = vec![
        retrieval_filter_object(
            "billing.claim",
            "claim",
            Some("draft"),
            Some("team-a"),
            "docs/billing.adoc",
        ),
        retrieval_filter_object(
            "architecture.decision",
            "decision",
            Some("accepted"),
            Some("team-b"),
            "docs/architecture.adoc",
        ),
    ];
    let filters = SearchFilters {
        kind: Some("claim".to_string()),
        status: None,
        owner: Some("team-b".to_string()),
        source_path: None,
        ..SearchFilters::default()
    };

    let session = load_session_from_objects(objects);
    let result = search(&session, lexical_query("", 10, filters));

    assert!(result.diagnostics.is_empty());
    assert!(result.records.is_empty());
}

#[test]
fn search_filter_validation_reports_each_supplied_filter_with_no_independent_match() {
    let objects = vec![retrieval_filter_object(
        "billing.claim",
        "claim",
        Some("draft"),
        Some("team-a"),
        "docs/billing.adoc",
    )];
    let filters = SearchFilters {
        kind: Some("decision".to_string()),
        status: Some("verified".to_string()),
        owner: Some("team-b".to_string()),
        source_path: Some("architecture".to_string()),
        ..SearchFilters::default()
    };

    let session = load_session_from_objects(objects);
    let diagnostics = search(&session, lexical_query("", 10, filters)).diagnostics;

    assert_eq!(diagnostics.len(), 4);
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.code == DiagnosticCode::SearchInvalidFilter)
    );
    assert!(
        diagnostics
            .iter()
            .all(|diagnostic| diagnostic.help.as_deref()
                == Some(DiagnosticCode::SearchInvalidFilter.default_help()))
    );
}

#[test]
fn why_object_returns_record_for_id_in_loaded_graph_artifact() {
    let artifact = verified_claim_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean load, got {:?}",
        result.diagnostics
    );
    let session = result.session.expect("retrieval session loads");

    let why_result = why_object(&session, "billing.verified-credits");

    assert!(
        why_result.diagnostics.is_empty(),
        "expected clean why result, got {:?}",
        why_result.diagnostics
    );
    assert_eq!(why_result.records.len(), 1);
    let record = &why_result.records[0];
    assert_eq!(record.id, "billing.verified-credits");
    assert_eq!(record.kind, "claim");
    assert_eq!(record.status.as_deref(), Some("verified"));
    assert_eq!(record.owner.as_deref(), Some("team-billing"));
    assert_eq!(record.verified_at.as_deref(), Some("2026-05-05"));
    assert_eq!(
        record.source.path,
        "tests/fixtures/claim/valid_verified_claim_with_all_evidence/input.adoc"
    );
    // V5.8: source maps to source_code EvidenceKind.
    assert_eq!(
        record.evidence.get("source_code").map(String::as_str),
        Some("payments ledger")
    );
}

#[test]
fn why_object_serializes_record_without_search_match_block() {
    let artifact = verified_claim_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    let session = result.session.expect("retrieval session loads");

    let why_result = why_object(&session, "billing.verified-credits");
    let value = serde_json::to_value(&why_result.records[0]).expect("record serializes");

    assert!(value.get("match").is_none());
    assert!(value.get("retrieval").is_none());
}

#[test]
fn why_object_lists_answered_questions_resolving_the_target() {
    // V6.5.3: `why` on an answering object surfaces the answered questions
    // whose `resolved_by` names it; unrelated objects omit the field entirely.
    let mut question = retrieval_filter_object(
        "billing.trial-credit-expiration",
        "question",
        Some("answered"),
        None,
        "questions.adoc",
    );
    question["fields"]["resolved_by"] = json!("billing.credits-expire");
    let session = load_session_from_objects(vec![
        retrieval_filter_object(
            "billing.credits-expire",
            "claim",
            Some("draft"),
            None,
            "billing.adoc",
        ),
        retrieval_filter_object(
            "billing.unrelated",
            "claim",
            Some("draft"),
            None,
            "billing.adoc",
        ),
        question,
    ]);

    let why_result = why_object(&session, "billing.credits-expire");

    assert!(
        why_result.diagnostics.is_empty(),
        "expected clean why result, got {:?}",
        why_result.diagnostics
    );
    assert_eq!(why_result.records.len(), 1);
    assert_eq!(
        why_result.records[0].resolved_questions,
        ["billing.trial-credit-expiration"]
    );

    let unrelated = why_object(&session, "billing.unrelated");
    let value = serde_json::to_value(&unrelated.records[0]).expect("record serializes");
    assert!(
        value.get("resolved_questions").is_none(),
        "empty resolved_questions must be omitted from serialization"
    );
}

#[test]
fn retrieval_envelope_serializes_stable_schema_with_records_and_diagnostics() {
    let artifact = verified_claim_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    let session = result.session.expect("retrieval session loads");
    let why_result = why_object(&session, "billing.verified-credits");

    let value =
        serde_json::to_value(RetrievalEnvelope::from(why_result)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    assert_eq!(value["records"][0]["id"], "billing.verified-credits");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert!(value["records"][0].get("match").is_none());
    assert!(value["records"][0].get("retrieval").is_none());
}

#[test]
fn retrieval_record_serializes_lexical_search_match_contract() {
    let record = RetrievalRecord {
        id: "billing.verified-credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        severity: None,
        trust: None,
        content_hash: "sha256:billing.verified-credits".to_string(),
        owner: None,
        verified_at: None,
        body: "Credits are verified.".to_string(),
        source: RetrievalSource {
            path: "billing.adoc".to_string(),
            line: 5,
            column: 1,
        },
        evidence: std::collections::BTreeMap::new(),
        fields: std::collections::BTreeMap::new(),
        relations: RetrievalRelations::default(),
        search_match: Some(RetrievalMatch::lexical(1, Some(1))),
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
        resolved_questions: Vec::new(),
    };
    let value = serde_json::to_value(&record).expect("record serializes");

    assert_eq!(
        value["match"],
        serde_json::json!({
            "mode": "lexical",
            "result_rank": 1,
            "lexical_rank": 1
        })
    );
    assert!(value.get("retrieval").is_none());
}

#[test]
fn retrieval_envelope_can_be_created_from_search_result() {
    let record = RetrievalRecord {
        id: "billing.credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        severity: None,
        trust: None,
        content_hash: "sha256:billing.credits".to_string(),
        owner: None,
        verified_at: None,
        body: "Credits decrement after successful payment.".to_string(),
        source: RetrievalSource {
            path: "docs/billing.adoc".to_string(),
            line: 9,
            column: 1,
        },
        evidence: std::collections::BTreeMap::new(),
        fields: std::collections::BTreeMap::new(),
        relations: RetrievalRelations::default(),
        search_match: Some(RetrievalMatch::lexical(1, Some(1))),
        effective_status: None,
        effective_reason: None,
        evidence_quality: None,
        resolved_questions: Vec::new(),
    };
    let result = SearchResult {
        records: vec![RetrievalEntry::KnowledgeObject(record)],
        diagnostics: Vec::new(),
    };

    let value = serde_json::to_value(RetrievalEnvelope::from(result)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    assert_eq!(value["records"][0]["match"]["mode"], "lexical");
    assert_eq!(value["records"][0]["match"]["result_rank"], 1);
    assert_eq!(value["records"][0]["match"]["lexical_rank"], 1);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

#[test]
fn why_object_reports_unknown_id_without_loading_source() {
    let artifact = verified_claim_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    let session = result.session.expect("retrieval session loads");

    let why_result = why_object(&session, "billing.missing");

    assert!(why_result.records.is_empty());
    assert_eq!(why_result.diagnostics.len(), 1);
    assert_eq!(
        why_result.diagnostics[0].code,
        DiagnosticCode::RetrievalObjectNotFound
    );
    assert_eq!(
        why_result.diagnostics[0].object_id.as_deref(),
        Some("billing.missing")
    );
}

#[test]
fn why_object_reports_invalid_id_without_lookup() {
    let artifact = verified_claim_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    let session = result.session.expect("retrieval session loads");

    let why_result = why_object(&session, "bad");

    assert!(why_result.records.is_empty());
    assert_eq!(why_result.diagnostics.len(), 1);
    assert_eq!(why_result.diagnostics[0].code, DiagnosticCode::IdInvalid);
    assert_eq!(why_result.diagnostics[0].object_id.as_deref(), Some("bad"));
}

#[test]
fn load_retrieval_session_rejects_invalid_object_ids_inside_artifact() {
    let artifact = write_temp_artifact(
        "invalid-object-id",
        r#"{
          "schema_version": "adoc.graph.v5",
  "repository_identity": null,
          "nodes": [
            {
              "type": "knowledge_object",
              "id": "bad",
              "kind": "claim",
              "content_hash": "sha256:bad",
              "status": "draft",
              "body": "Invalid artifact object ID.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 1, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            }
          ],
          "edges": [],
          "diagnostics": []
        }"#,
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(result.session.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(result.diagnostics[0].code, DiagnosticCode::IdInvalid);
    assert_eq!(result.diagnostics[0].object_id.as_deref(), Some("bad"));
}

#[test]
fn load_retrieval_session_rejects_duplicate_object_ids_inside_artifact() {
    let artifact = write_temp_artifact(
        "duplicate",
        r#"{
          "schema_version": "adoc.graph.v5",
  "repository_identity": null,
          "nodes": [
            {
              "type": "knowledge_object",
              "id": "billing.duplicate",
              "kind": "claim",
              "content_hash": "sha256:billing.duplicate.first",
              "status": "draft",
              "body": "First.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 1, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            },
            {
              "type": "knowledge_object",
              "id": "billing.duplicate",
              "kind": "claim",
              "content_hash": "sha256:billing.duplicate.second",
              "status": "draft",
              "body": "Second.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 2, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            }
          ],
          "edges": [],
          "diagnostics": []
        }"#,
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });

    assert!(result.session.is_none());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::IdDuplicateInArtifact
    );
    assert_eq!(
        result.diagnostics[0].object_id.as_deref(),
        Some("billing.duplicate")
    );
}

// ---------------------------------------------------------------------------
// V4.3 migration-hint diagnostic
// ---------------------------------------------------------------------------

fn prose_only_graph_artifact(prose_blocks: usize) -> tempfile::NamedTempFile {
    let mut nodes: Vec<Value> = Vec::with_capacity(1 + prose_blocks);
    nodes.push(json!({
        "type": "page",
        "id": "compat.page",
        "order": 0,
        "title": "Compat Page",
        "source_path": "docs/compat.md"
    }));
    for index in 0..prose_blocks {
        nodes.push(json!({
            "type": "paragraph",
            "id": format!("compat.page#p{}", index),
            "page_id": "compat.page",
            "order": index as u32,
            "text": "Markdown prose lives here.",
            "source_span": {
                "path": "docs/compat.md",
                "line": (index + 1) as u32,
                "column": 1
            }
        }));
    }
    let document = json!({
          "schema_version": "adoc.graph.v5",
    "repository_identity": null,
          "nodes": nodes,
          "edges": [],
          "diagnostics": []
      });
    write_temp_artifact(
        "prose-only",
        &serde_json::to_string_pretty(&document).expect("prose-only fixture serializes"),
    )
}

fn empty_graph_artifact() -> tempfile::NamedTempFile {
    let document = json!({
          "schema_version": "adoc.graph.v5",
    "repository_identity": null,
          "nodes": [],
          "edges": [],
          "diagnostics": []
      });
    write_temp_artifact(
        "empty-graph",
        &serde_json::to_string_pretty(&document).expect("empty graph fixture serializes"),
    )
}

fn load_prose_only_session(prose_blocks: usize) -> RetrievalSession {
    let artifact = prose_only_graph_artifact(prose_blocks);
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    assert!(
        result.diagnostics.is_empty(),
        "prose-only fixture should load cleanly, got {:?}",
        result.diagnostics
    );
    result.session.expect("prose-only session loads")
}

fn load_empty_graph_session() -> RetrievalSession {
    let artifact = empty_graph_artifact();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    assert!(
        result.diagnostics.is_empty(),
        "empty-graph fixture should load cleanly, got {:?}",
        result.diagnostics
    );
    result.session.expect("empty-graph session loads")
}

#[test]
fn lexical_search_emits_migration_hint_when_only_prose_blocks() {
    let session = load_prose_only_session(2);

    let result = search(
        &session,
        lexical_query("refund", 10, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration
    );
    assert!(
        result.diagnostics[0].message.contains("Knowledge Objects"),
        "diagnostic message should reference Knowledge Objects, got {:?}",
        result.diagnostics[0].message
    );
}

#[test]
fn empty_query_lexical_search_emits_migration_hint_for_prose_only_project() {
    // Per the V4.3 "skip empty-query short-circuit" decision, the empty-query
    // branch should still emit the hint when the graph holds prose-only nodes.
    let session = load_prose_only_session(1);

    let result = search(&session, lexical_query("", 10, SearchFilters::default()));

    assert!(result.records.is_empty());
    assert_eq!(result.diagnostics.len(), 1);
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration
    );
}

#[test]
fn hybrid_search_falls_back_to_lexical_and_emits_single_migration_hint() {
    // No search artifact ⇒ hybrid falls through to lexical via `_impl`.
    // The hint must fire exactly once (no double-emission across fallback).
    let session = load_prose_only_session(1);

    let result = search(
        &session,
        hybrid_query("anything", vec![0.0; 4], 5, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert_eq!(
        result.diagnostics.len(),
        1,
        "hybrid→lexical fallback must not double-emit the migration hint"
    );
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration
    );
}

#[test]
fn search_does_not_emit_migration_hint_when_kos_present() {
    let session = load_session_from_objects(vec![retrieval_search_object(
        "billing.credits",
        "claim",
        Some("draft"),
        None,
        "docs/billing.adoc",
        "Credits apply after payment.",
    )]);

    // Query that matches nothing in the body — records will be empty but a KO
    // exists, so the hint must not fire.
    let result = search(
        &session,
        lexical_query("zzz-no-match-zzz", 10, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert!(
        !result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration),
        "migration hint must not fire when Knowledge Objects exist, got {:?}",
        result.diagnostics
    );
}

#[test]
fn search_does_not_emit_migration_hint_for_empty_graph() {
    let session = load_empty_graph_session();

    let result = search(
        &session,
        lexical_query("anything", 10, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert!(
        result.diagnostics.is_empty(),
        "migration hint must not fire for a fully empty graph, got {:?}",
        result.diagnostics
    );
}

#[test]
fn migration_hint_appears_in_retrieval_envelope() {
    let session = load_prose_only_session(1);
    let result = search(
        &session,
        lexical_query("anything", 10, SearchFilters::default()),
    );

    let envelope: RetrievalEnvelope = result.into();
    let value = serde_json::to_value(&envelope).expect("envelope serializes");
    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    assert_eq!(value["records"].as_array().unwrap().len(), 0);
    let diagnostics = value["diagnostics"].as_array().expect("diagnostics array");
    assert_eq!(diagnostics.len(), 1);
    assert_eq!(
        diagnostics[0]["code"],
        "retrieval.no_knowledge_objects_consider_migration"
    );
    assert_eq!(diagnostics[0]["severity"], "warning");
}

// ── V6.5.1: api Knowledge Object retrieval coverage ─────────────────────────

/// The retrieval pipeline is kind-generic by construction: an `api` node's
/// body is BM25-findable and its record echoes kind, lifecycle status, and
/// the method/path riding the fields map.
#[test]
fn api_object_is_lexically_findable_with_method_and_path_fields() {
    let mut api = retrieval_search_object(
        "billing.consume-credit",
        "api",
        Some("verified"),
        Some("backend-platform"),
        "docs/billing-api.adoc",
        "Consumes one or more credits for a completed generation job.",
    );
    api["fields"]["method"] = json!("POST");
    api["fields"]["path"] = json!("/api/billing/credits/consume");

    let session = load_session_from_objects(vec![api]);

    let result = search(
        &session,
        lexical_query("generation job credits", 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), vec!["billing.consume-credit"]);
    let record = result.records[0]
        .as_knowledge_object()
        .expect("knowledge object record");
    assert_eq!(record.kind, "api");
    assert_eq!(record.status.as_deref(), Some("verified"));
    assert_eq!(
        record.fields.get("method").map(String::as_str),
        Some("POST")
    );
    assert_eq!(
        record.fields.get("path").map(String::as_str),
        Some("/api/billing/credits/consume")
    );
}

// ── V6.5.2: observation Knowledge Object retrieval coverage ─────────────────

/// An `observation` node's body is BM25-findable and its record echoes kind,
/// the `observed` lifecycle status, and the sample_size/observed_at scalars
/// riding the fields map. The numbers themselves are metadata, not meaning —
/// they are echoed on the record but not indexed.
#[test]
fn observation_object_is_lexically_findable_with_metadata_fields() {
    let mut observation = retrieval_search_object(
        "onboarding.credit-confusion",
        "observation",
        Some("observed"),
        None,
        "docs/onboarding.adoc",
        "Users often misunderstand credit usage before their first generation.",
    );
    observation["fields"]["sample_size"] = json!("37");
    observation["fields"]["observed_at"] = json!("2026-04-30");

    let session = load_session_from_objects(vec![observation]);

    let result = search(
        &session,
        lexical_query("misunderstand credit usage", 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), vec!["onboarding.credit-confusion"]);
    let record = result.records[0]
        .as_knowledge_object()
        .expect("knowledge object record");
    assert_eq!(record.kind, "observation");
    assert_eq!(record.status.as_deref(), Some("observed"));
    assert_eq!(
        record.fields.get("sample_size").map(String::as_str),
        Some("37")
    );
    assert_eq!(
        record.fields.get("observed_at").map(String::as_str),
        Some("2026-04-30")
    );
}

// ── V6.5.3: question Knowledge Object retrieval coverage ────────────────────

/// The retrieval pipeline is kind-generic by construction: a `question`
/// node's body is BM25-findable and its record echoes kind and lifecycle
/// status.
#[test]
fn question_object_body_is_lexically_findable() {
    let question = retrieval_search_object(
        "billing.trial-credit-expiration",
        "question",
        Some("open"),
        Some("product-growth"),
        "docs/billing-questions.adoc",
        "Should unused trial credits expire after 30 days or remain available indefinitely?",
    );

    let session = load_session_from_objects(vec![question]);

    let result = search(
        &session,
        lexical_query("trial credits expire", 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), vec!["billing.trial-credit-expiration"]);
    let record = result.records[0]
        .as_knowledge_object()
        .expect("knowledge object record");
    assert_eq!(record.kind, "question");
    assert_eq!(record.status.as_deref(), Some("open"));
}

// ── V6.5.4: task Knowledge Object retrieval coverage ────────────────────────

/// The retrieval pipeline is kind-generic by construction: a `task` node's
/// body is BM25-findable and its record echoes kind, lifecycle status, and
/// the owner/due riding the fields map.
#[test]
fn task_object_is_lexically_findable_with_owner_and_due_fields() {
    let mut task = retrieval_search_object(
        "billing.update-support-runbook",
        "task",
        Some("open"),
        Some("support-ops"),
        "docs/billing-tasks.adoc",
        "Update the support runbook to mention refund behavior after persistence failure.",
    );
    task["fields"]["due"] = json!("2026-05-20");

    let session = load_session_from_objects(vec![task]);

    let result = search(
        &session,
        lexical_query("support runbook refund", 3, SearchFilters::default()),
    );

    assert!(result.diagnostics.is_empty());
    assert_eq!(search_ids(&result), vec!["billing.update-support-runbook"]);
    let record = result.records[0]
        .as_knowledge_object()
        .expect("knowledge object record");
    assert_eq!(record.kind, "task");
    assert_eq!(record.status.as_deref(), Some("open"));
    // `owner` is a typed record slot, not a generic fields entry.
    assert_eq!(record.owner.as_deref(), Some("support-ops"));
    assert_eq!(
        record.fields.get("due").map(String::as_str),
        Some("2026-05-20")
    );
}

/// V1.7.1 (ADR-0040): a raw graph document carrying prose blocks — the
/// canonical builders above are Knowledge-Object-only by design, and prose
/// nodes ride the artifact's own serde shape.
fn prose_symmetry_graph_json(extension: &str) -> String {
    let path = format!("docs/guide.{extension}");
    serde_json::to_string_pretty(&json!({
        "schema_version": "adoc.graph.v5",
  "repository_identity": null,
        "nodes": [
            {
                "type": "page",
                "id": "guides.page",
                "order": 0,
                "source_path": path
            },
            {
                "type": "heading",
                "id": "guides.page#block-0000",
                "page_id": "guides.page",
                "order": 0,
                "level": 1,
                "text": "Billing basics",
                "source_span": { "path": path, "line": 1, "column": 1 }
            },
            {
                "type": "paragraph",
                "id": "guides.page#block-0001",
                "page_id": "guides.page",
                "order": 1,
                "text": "Credits are consumed when a generation job completes, not when it starts.",
                "source_span": { "path": path, "line": 3, "column": 1 }
            },
            {
                "type": "paragraph",
                "id": "guides.page#block-0002",
                "page_id": "guides.page",
                "order": 2,
                "text": "Refunds are handled manually by support.",
                "source_span": { "path": path, "line": 5, "column": 1 }
            },
            {
                "type": "code_block",
                "id": "guides.page#block-0003",
                "page_id": "guides.page",
                "order": 3,
                "language": "shell",
                "code": "adoc build --provider fastembed",
                "source_span": { "path": path, "line": 7, "column": 1 }
            },
            {
                "type": "list",
                "id": "guides.page#block-0004",
                "page_id": "guides.page",
                "order": 4,
                "text": "ordered",
                "items": [
                    "Rotate the signing key quarterly.",
                    "Revoke the previous key after rotation."
                ],
                "source_span": { "path": path, "line": 11, "column": 1 }
            }
        ],
        "edges": [],
        "diagnostics": []
    }))
    .expect("prose fixture serializes")
}

fn load_prose_session(extension: &str) -> RetrievalSession {
    let artifact = write_temp_artifact(
        &format!("prose-symmetry-{extension}"),
        &prose_symmetry_graph_json(extension),
    );
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    assert!(
        result.diagnostics.is_empty(),
        "expected clean prose fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("prose fixture session loads")
}

/// V1.7.1 roadmap acceptance — the symmetry rule: identical prose in a
/// `.adoc` file and a `.md` file ranks identically; only `source.path`
/// differs. V1.7.3 extends the property across every prose block kind the
/// graph carries: heading, paragraph, list, and code block.
#[test]
fn identical_prose_ranks_identically_across_adoc_and_md_sources() {
    let adoc_session = load_prose_session("adoc");
    let md_session = load_prose_session("md");

    for query in [
        "credits consumed",
        "refunds",
        "billing basics",
        "rotate the signing key",
        "provider fastembed",
    ] {
        let adoc_result = search(
            &adoc_session,
            lexical_query(query, 10, SearchFilters::default()),
        );
        let md_result = search(
            &md_session,
            lexical_query(query, 10, SearchFilters::default()),
        );

        assert_eq!(
            search_ids(&adoc_result),
            search_ids(&md_result),
            "result order must be source-mode-independent for {query:?}"
        );
        assert_eq!(
            search_lexical_ranks(&adoc_result),
            search_lexical_ranks(&md_result),
            "lexical ranks must be source-mode-independent for {query:?}"
        );
        for (adoc_entry, md_entry) in adoc_result.records.iter().zip(&md_result.records) {
            let (RetrievalEntry::Prose(adoc_record), RetrievalEntry::Prose(md_record)) =
                (adoc_entry, md_entry)
            else {
                panic!("prose-only fixtures must return prose records");
            };
            assert!(adoc_record.source.path.ends_with(".adoc"));
            assert!(md_record.source.path.ends_with(".md"));
            assert_eq!(adoc_record.text, md_record.text);
            assert_eq!(adoc_record.heading_context, md_record.heading_context);
        }
    }
}

/// V1.7.1 (ADR-0040): the prose record wire shape — `record_type`
/// discriminator, the Knowledge Object `match`/`source` shapes (the ADR
/// normalizes the roadmap's illustrative JSON), heading context present.
#[test]
fn prose_record_serializes_with_record_type_and_normalized_match_shape() {
    let session = load_prose_session("md");
    let result = search(
        &session,
        lexical_query("credits consumed", 10, SearchFilters::default()),
    );

    let value = serde_json::to_value(RetrievalEnvelope::from(result)).expect("envelope serializes");
    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    let record = &value["records"][0];
    assert_eq!(record["record_type"], "prose");
    assert_eq!(record["id"], "guides.page#block-0001");
    assert_eq!(record["page_id"], "guides.page");
    assert_eq!(record["block_kind"], "paragraph");
    assert_eq!(record["heading_context"], "Billing basics");
    assert_eq!(record["source"]["path"], "docs/guide.md");
    assert_eq!(record["source"]["line"], 3);
    assert_eq!(record["source"]["column"], 1);
    assert_eq!(record["match"]["mode"], "lexical");
    assert_eq!(record["match"]["result_rank"], 1);
    assert!(
        record.get("search_match").is_none(),
        "the match block serializes under `match`, not `search_match` (ADR-0040)"
    );
    assert!(
        record.get("content_hash").is_none() && record.get("relations").is_none(),
        "prose records carry no content_hash or relations"
    );

    // A block before any heading omits heading_context entirely.
    let heading_hit = search(
        &session,
        lexical_query("billing basics", 10, SearchFilters::default()),
    );
    let heading_value =
        serde_json::to_value(RetrievalEnvelope::from(heading_hit)).expect("envelope serializes");
    let heading_record = heading_value["records"]
        .as_array()
        .expect("records array")
        .iter()
        .find(|r| r["id"] == "guides.page#block-0000")
        .expect("heading block is findable")
        .clone();
    assert!(
        heading_record.get("heading_context").is_none(),
        "a top-level heading has no ancestor context, got {heading_record:?}"
    );
}

/// V1.7.2 (ADR-0040): a prose session backed by a v1 search artifact whose
/// prose entries carry vectors, so semantic and hybrid ranking can score
/// prose blocks.
fn load_prose_session_with_vectors(vectors: Vec<(&str, Vec<f32>)>) -> RetrievalSession {
    let canonical: CanonicalGraphDocument = serde_json::from_str(&prose_symmetry_graph_json("md"))
        .expect("prose fixture has canonical shape");
    let graph_json =
        serde_json::to_string_pretty(&canonical).expect("prose fixture serializes canonically");
    let artifact = write_temp_artifact("prose-vectors-graph", &graph_json);
    let search_document = json!({
        "schema_version": "adoc.search.v1",
        "model": {
            "id": "bge-small-en-v1.5",
            "provider": "fastembed",
            "dim": 384
        },
        "graph_artifact_hash": sha256_prefixed(graph_json.as_bytes()),
        "embeddings": vectors
            .into_iter()
            .map(|(id, vector)| json!({
                "id": id,
                "entry_kind": if id.contains('#') { "prose" } else { "knowledge_object" },
                "content_hash": "sha256:test",
                "vector": vector
            }))
            .collect::<Vec<_>>()
    });
    let search_artifact = write_temp_search_artifact(
        "prose-vectors-search",
        &serde_json::to_string_pretty(&search_document).expect("search fixture serializes"),
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: Some(search_artifact.path().to_path_buf()),
    });
    assert!(
        result.diagnostics.is_empty(),
        "expected clean prose vector fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("prose vector session loads")
}

/// V1.7.2 (ADR-0040): prose ids coming out of the vector index resolve to
/// prose records instead of panicking on the Knowledge-Object lookup.
#[test]
fn semantic_search_returns_prose_records_ranked_by_cosine() {
    let session = load_prose_session_with_vectors(vec![
        ("guides.page#block-0001", vec![1.0, 0.0]),
        ("guides.page#block-0002", vec![0.0, 1.0]),
    ]);

    let result = search(
        &session,
        semantic_query(
            "credit spending",
            vec![1.0, 0.0],
            10,
            SearchFilters::default(),
        ),
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    assert_eq!(
        search_ids(&result),
        vec!["guides.page#block-0001", "guides.page#block-0002"]
    );
    let RetrievalEntry::Prose(record) = &result.records[0] else {
        panic!("semantic prose hit must resolve to a prose record");
    };
    let search_match = record
        .search_match
        .as_ref()
        .expect("semantic hit carries match metadata");
    assert_eq!(search_match.mode, SearchMode::Semantic);
    assert_eq!(search_match.vector_rank, Some(1));
    let cosine = search_match
        .cosine_score
        .expect("semantic hit carries cosine score");
    assert!(
        (cosine - 1.0).abs() < 1e-6,
        "expected cosine 1.0, got {cosine}"
    );
}

/// V1.7.2: prose-only semantic search is un-gated now that prose vectors
/// exist in adoc.search.v1.
#[test]
fn prose_only_semantic_search_returns_prose_without_scope_conflict() {
    let session = load_prose_session_with_vectors(vec![
        ("guides.page#block-0001", vec![1.0, 0.0]),
        ("guides.page#block-0002", vec![0.0, 1.0]),
    ]);

    let query = SearchQuery {
        text: "credit spending".to_string(),
        mode: SearchMode::Semantic,
        filters: SearchFilters::default(),
        top: NonZeroUsize::new(10).expect("test search top is non-zero"),
        query_vector: Some(vec![0.0, 1.0]),
        scope: SearchRecordScope::ProseOnly,
    };
    let result = search(&session, query);

    assert!(
        result.diagnostics.is_empty(),
        "prose-only semantic search must not report a scope conflict: {:?}",
        result.diagnostics
    );
    assert_eq!(
        search_ids(&result),
        vec!["guides.page#block-0002", "guides.page#block-0001"]
    );
}

/// V1.7.2: objects-only semantic search keeps prose vectors out of the pool.
#[test]
fn objects_only_semantic_search_excludes_prose_vectors() {
    let session = load_prose_session_with_vectors(vec![("guides.page#block-0001", vec![1.0, 0.0])]);

    let query = SearchQuery {
        text: "credit spending".to_string(),
        mode: SearchMode::Semantic,
        filters: SearchFilters::default(),
        top: NonZeroUsize::new(10).expect("test search top is non-zero"),
        query_vector: Some(vec![1.0, 0.0]),
        scope: SearchRecordScope::ObjectsOnly,
    };
    let result = search(&session, query);

    // The V4.3 migration hint fires: an objects-only search over a
    // prose-only project finds no Knowledge Objects, by design.
    assert_eq!(
        result
            .diagnostics
            .iter()
            .map(|diagnostic| diagnostic.code)
            .collect::<Vec<_>>(),
        vec![DiagnosticCode::RetrievalNoKnowledgeObjectsConsiderMigration]
    );
    assert!(
        result.records.is_empty(),
        "the fixture has no Knowledge Objects, so objects-only semantic search returns nothing"
    );
}

/// V1.7.2: hybrid fusion scores prose on both lexical and vector rank once
/// prose vectors exist.
#[test]
fn hybrid_search_prose_hits_carry_vector_ranks() {
    let session = load_prose_session_with_vectors(vec![
        ("guides.page#block-0001", vec![1.0, 0.0]),
        ("guides.page#block-0002", vec![0.0, 1.0]),
    ]);

    let result = search(
        &session,
        hybrid_query(
            "credits consumed",
            vec![1.0, 0.0],
            10,
            SearchFilters::default(),
        ),
    );

    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let RetrievalEntry::Prose(record) = &result.records[0] else {
        panic!("hybrid prose hit must resolve to a prose record");
    };
    assert_eq!(record.id, "guides.page#block-0001");
    let search_match = record
        .search_match
        .as_ref()
        .expect("hybrid hit carries match metadata");
    assert_eq!(
        search_match.vector_rank,
        Some(1),
        "prose must enter the vector ranking in hybrid mode (V1.7.2)"
    );
    assert_eq!(search_match.lexical_rank, Some(1));
}

/// ADR-0040: code stays lexical-only. The v1 artifact never embeds code
/// blocks, yet they remain in the blended candidate pool — a code-block hit
/// fuses into hybrid results on lexical rank alone and never surfaces in
/// semantic mode.
#[test]
fn code_block_prose_ranks_lexically_in_hybrid_but_never_semantically() {
    let session = load_prose_session_with_vectors(vec![
        ("guides.page#block-0001", vec![1.0, 0.0]),
        ("guides.page#block-0002", vec![0.0, 1.0]),
    ]);

    let hybrid = search(
        &session,
        hybrid_query(
            "provider fastembed",
            vec![1.0, 0.0],
            10,
            SearchFilters::default(),
        ),
    );
    assert!(hybrid.diagnostics.is_empty(), "{:?}", hybrid.diagnostics);
    let code_hit = hybrid
        .records
        .iter()
        .find_map(|entry| match entry {
            RetrievalEntry::Prose(record) if record.id == "guides.page#block-0003" => Some(record),
            _ => None,
        })
        .expect("code block must fuse into hybrid results on its lexical hit");
    let search_match = code_hit
        .search_match
        .as_ref()
        .expect("hybrid hit carries match metadata");
    assert_eq!(search_match.lexical_rank, Some(1));
    assert_eq!(
        search_match.vector_rank, None,
        "code blocks are never embedded, so a hybrid code hit carries no vector rank (ADR-0040)"
    );

    let semantic = search(
        &session,
        semantic_query(
            "provider fastembed",
            vec![1.0, 0.0],
            10,
            SearchFilters::default(),
        ),
    );
    assert!(
        semantic.diagnostics.is_empty(),
        "{:?}",
        semantic.diagnostics
    );
    assert!(
        !search_ids(&semantic).contains(&"guides.page#block-0003"),
        "semantic search must never surface a code block: it has no vector (ADR-0040)"
    );
}

/// V1.7.3 (ADR-0040 §Consequences): the blend is honest by rank alone. In a
/// mixed corpus, a query answered by a Knowledge Object ranks the object
/// first, and a query answered only by prose ranks the prose first — same
/// session, no weights, no scope flags.
#[test]
fn blended_lexical_search_ranks_knowledge_objects_and_prose_by_honest_rank() {
    let ko = retrieval_search_object(
        "mixed.claims.settlement-once",
        "claim",
        Some("verified"),
        Some("team-billing"),
        "docs/mixed.adoc",
        "Each captured payment settlement writes exactly one ledger movement.",
    );
    let graph_json = graph_json_from_objects(
        vec![
            json!({
                "type": "page",
                "id": "mixed.page",
                "order": 0,
                "source_path": "docs/mixed.md"
            }),
            json!({
                "type": "heading",
                "id": "mixed.page#block-0000",
                "page_id": "mixed.page",
                "order": 0,
                "level": 1,
                "text": "Failover operations",
                "source_span": { "path": "docs/mixed.md", "line": 1, "column": 1 }
            }),
            json!({
                "type": "paragraph",
                "id": "mixed.page#block-0001",
                "page_id": "mixed.page",
                "order": 1,
                "text": "The standby health dashboard is the source of truth before failover.",
                "source_span": { "path": "docs/mixed.md", "line": 3, "column": 1 }
            }),
            ko,
        ],
        Vec::new(),
    );
    let artifact = write_temp_artifact("mixed-blend", &graph_json);
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
        search_artifact_path: None,
    });
    assert!(result.diagnostics.is_empty(), "{:?}", result.diagnostics);
    let session = result.session.expect("mixed fixture session loads");

    let ko_first = search(
        &session,
        lexical_query("settlement ledger movement", 10, SearchFilters::default()),
    );
    assert_eq!(
        search_ids(&ko_first).first().copied(),
        Some("mixed.claims.settlement-once"),
        "a query answered by a Knowledge Object must rank it first in the blend"
    );

    let prose_first = search(
        &session,
        lexical_query("standby health dashboard", 10, SearchFilters::default()),
    );
    assert_eq!(
        search_ids(&prose_first).first().copied(),
        Some("mixed.page#block-0001"),
        "a query answered only by prose must rank the prose block first in the blend"
    );
    assert!(
        prose_first.diagnostics.is_empty(),
        "a mixed corpus never fires the migration hint: {:?}",
        prose_first.diagnostics
    );
}

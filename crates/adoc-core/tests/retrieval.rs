use std::num::NonZeroUsize;
use std::path::PathBuf;

use adoc_core::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan, DiagnosticCode,
    JsonRetrievalFormatter, RetrievalEnvelope, RetrievalFormatter, RetrievalInput, RetrievalMatch,
    RetrievalRecord, RetrievalSession, RetrievalSource, SearchFilters, SearchMode, SearchQuery,
    SearchResult, TextRetrievalFormatter, explain_object, load_retrieval_session, search,
};

fn fixture_path(relative: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("fixtures")
        .join(relative)
}

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
        .suffix(".agent.json")
        .tempfile()
        .expect("temp artifact can be created");
    std::fs::write(artifact.path(), contents).expect("temp artifact can be written");
    artifact
}

fn load_session_from_objects(objects: Vec<AgentJsonObject>) -> RetrievalSession {
    let document = AgentJsonDocument {
        schema_version: "adoc.agent.v0".to_string(),
        pages: Vec::new(),
        objects,
        diagnostics: Vec::new(),
    };
    let artifact = write_temp_artifact(
        "search",
        &document
            .to_pretty_json()
            .expect("search fixture serializes to agent JSON"),
    );
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean search fixture load, got {:?}",
        result.diagnostics
    );
    result.session.expect("search fixture session loads")
}

fn load_workspace_fixture_session(relative: &str) -> RetrievalSession {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: workspace_fixture_path(relative),
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
) -> AgentJsonObject {
    let mut fields = std::collections::BTreeMap::new();
    if let Some(owner) = owner {
        fields.insert("owner".to_string(), owner.to_string());
    }

    AgentJsonObject {
        id: id.to_string(),
        kind: kind.to_string(),
        status: status.map(str::to_string),
        body: "Filter fixture body.".to_string(),
        page_id: "team.page".to_string(),
        source_span: AgentJsonSourceSpan {
            path: source_path.to_string(),
            line: 1,
            column: 1,
        },
        fields,
        relations: AgentJsonRelations::default(),
    }
}

fn retrieval_search_object(
    id: &str,
    kind: &str,
    status: Option<&str>,
    owner: Option<&str>,
    source_path: &str,
    body: &str,
) -> AgentJsonObject {
    let mut object = retrieval_filter_object(id, kind, status, owner, source_path);
    object.body = body.to_string();
    object
}

fn lexical_query(text: &str, top: usize, filters: SearchFilters) -> SearchQuery {
    SearchQuery {
        text: text.to_string(),
        mode: SearchMode::Lexical,
        filters,
        top: NonZeroUsize::new(top).expect("test search top is non-zero"),
    }
}

fn search_ids(result: &SearchResult) -> Vec<&str> {
    result
        .records
        .iter()
        .map(|record| record.id.as_str())
        .collect()
}

fn search_ranks(result: &SearchResult) -> Vec<u32> {
    result
        .records
        .iter()
        .map(|record| {
            record
                .search_match
                .as_ref()
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
                .search_match
                .as_ref()
                .expect("search result records include lexical match metadata")
                .lexical_rank
        })
        .collect()
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
fn retrieval_search_billing_pilot_subset_returns_benchmark_matches_in_top_3() {
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.agent.json");

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
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.agent.json");

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

    let prefix = search(
        &session,
        lexical_query("billing.credits", 4, SearchFilters::default()),
    );
    assert!(prefix.diagnostics.is_empty());
    assert_eq!(
        search_ids(&prefix),
        [
            "billing.credits",
            "billing.credits.nonnegative",
            "billing.credits.ledger-source",
            "billing.credits.decrement-after-success"
        ]
    );
    assert_eq!(search_ranks(&prefix), [1, 2, 3, 4]);
}

#[test]
fn retrieval_search_billing_pilot_subset_covers_filters_and_tie_ordering() {
    let session = load_workspace_fixture_session("v1_2_search/pilot_subset.agent.json");

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
    let session = load_workspace_fixture_session("v1_2_search/empty.agent.json");

    let result = search(
        &session,
        lexical_query("credit ledger", 3, SearchFilters::default()),
    );

    assert!(result.records.is_empty());
    assert!(result.diagnostics.is_empty());
}

#[test]
fn search_filter_matches_case_insensitive_substrings_on_object_metadata() {
    let object = retrieval_filter_object(
        "billing.verified-credits",
        "claim",
        Some("verified"),
        Some("Team-Billing"),
        "docs/billing/credits.adoc",
    );
    let filters = SearchFilters {
        kind: Some("CLA".to_string()),
        status: Some("VERI".to_string()),
        owner: Some("billing".to_string()),
        source_path: Some("CREDITS.ADOC".to_string()),
    };

    assert!(filters.matches(&object));
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
    assert_eq!(result.records[0].id, "billing.credits");
    assert_eq!(
        result.records[0].search_match,
        Some(RetrievalMatch::lexical(1, Some(2)))
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
    let object = retrieval_filter_object(
        "glossary.credit",
        "glossary",
        None,
        None,
        "docs/glossary.adoc",
    );

    assert!(
        !SearchFilters {
            kind: None,
            status: Some("verified".to_string()),
            owner: None,
            source_path: None,
        }
        .matches(&object)
    );
    assert!(
        !SearchFilters {
            kind: None,
            status: None,
            owner: Some("team".to_string()),
            source_path: None,
        }
        .matches(&object)
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
    };

    assert!(filters.validate_against(&objects).is_empty());
    assert!(objects.iter().all(|object| !filters.matches(object)));
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
    };

    let diagnostics = filters.validate_against(&objects);

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
fn explain_object_returns_record_for_id_in_loaded_agent_artifact() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean load, got {:?}",
        result.diagnostics
    );
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.verified-credits");

    assert!(
        explained.diagnostics.is_empty(),
        "expected clean explain, got {:?}",
        explained.diagnostics
    );
    assert_eq!(explained.records.len(), 1);
    let record = &explained.records[0];
    assert_eq!(record.id, "billing.verified-credits");
    assert_eq!(record.kind, "claim");
    assert_eq!(record.status.as_deref(), Some("verified"));
    assert_eq!(record.owner.as_deref(), Some("team-billing"));
    assert_eq!(record.verified_at.as_deref(), Some("2026-05-05"));
    assert_eq!(
        record.source.path,
        "tests/fixtures/claim/valid_verified_claim_with_all_evidence/input.adoc"
    );
    assert_eq!(
        record.evidence.get("source").map(String::as_str),
        Some("payments ledger")
    );
}

#[test]
fn explain_object_serializes_record_without_search_match_block() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.verified-credits");
    let value = serde_json::to_value(&explained.records[0]).expect("record serializes");

    assert!(value.get("match").is_none());
    assert!(value.get("retrieval").is_none());
}

#[test]
fn retrieval_envelope_serializes_stable_schema_with_records_and_diagnostics() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");
    let explained = explain_object(&session, "billing.verified-credits");

    let value =
        serde_json::to_value(RetrievalEnvelope::from(explained)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"][0]["id"], "billing.verified-credits");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert!(value["records"][0].get("match").is_none());
    assert!(value["records"][0].get("retrieval").is_none());
}

#[test]
fn retrieval_record_serializes_lexical_search_match_contract() {
    let object = AgentJsonObject {
        id: "billing.verified-credits".to_string(),
        kind: "claim".to_string(),
        status: Some("verified".to_string()),
        body: "Credits are verified.".to_string(),
        page_id: "team.billing".to_string(),
        source_span: AgentJsonSourceSpan {
            path: "billing.adoc".to_string(),
            line: 5,
            column: 1,
        },
        fields: std::collections::BTreeMap::new(),
        relations: AgentJsonRelations::default(),
    };

    let record =
        RetrievalRecord::from_object_with_match(&object, RetrievalMatch::lexical(1, Some(1)));
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
        relations: AgentJsonRelations::default(),
        search_match: Some(RetrievalMatch::lexical(1, Some(1))),
    };
    let result = SearchResult {
        records: vec![record],
        diagnostics: Vec::new(),
    };

    let value = serde_json::to_value(RetrievalEnvelope::from(result)).expect("envelope serializes");

    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"][0]["match"]["mode"], "lexical");
    assert_eq!(value["records"][0]["match"]["result_rank"], 1);
    assert_eq!(value["records"][0]["match"]["lexical_rank"], 1);
    assert_eq!(value["diagnostics"], serde_json::json!([]));
}

#[test]
fn explain_object_reports_unknown_id_without_loading_source() {
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture_path(
            "claim/valid_verified_claim_with_all_evidence/expected.agent.json",
        ),
    });
    let session = result.session.expect("retrieval session loads");

    let explained = explain_object(&session, "billing.missing");

    assert!(explained.records.is_empty());
    assert_eq!(explained.diagnostics.len(), 1);
    assert_eq!(
        explained.diagnostics[0].code,
        DiagnosticCode::RetrievalObjectNotFound
    );
    assert_eq!(
        explained.diagnostics[0].object_id.as_deref(),
        Some("billing.missing")
    );
}

#[test]
fn load_retrieval_session_rejects_duplicate_object_ids_inside_artifact() {
    let artifact = write_temp_artifact(
        "duplicate",
        r#"{
          "schema_version": "adoc.agent.v0",
          "pages": [],
          "objects": [
            {
              "id": "billing.duplicate",
              "kind": "claim",
              "status": "draft",
              "body": "First.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 1, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            },
            {
              "id": "billing.duplicate",
              "kind": "claim",
              "status": "draft",
              "body": "Second.",
              "page_id": "billing.page",
              "source_span": { "path": "billing.adoc", "line": 2, "column": 1 },
              "fields": {},
              "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
            }
          ],
          "diagnostics": []
        }"#,
    );

    let result = load_retrieval_session(RetrievalInput {
        artifact_path: artifact.path().to_path_buf(),
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

#[test]
fn text_retrieval_formatter_renders_statement_body_and_sorted_fields() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
            id: "billing.policy".to_string(),
            kind: "decision".to_string(),
            status: Some("accepted".to_string()),
            owner: Some("architecture".to_string()),
            verified_at: None,
            body: "Refund policy is ledger-backed.\nManual credits are exceptions.".to_string(),
            source: RetrievalSource {
                path: "docs/decisions.adoc".to_string(),
                line: 7,
                column: 1,
            },
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::from([
                ("scope".to_string(), "refunds".to_string()),
                ("decided_by".to_string(), "architecture".to_string()),
            ]),
            relations: AgentJsonRelations::default(),
            search_match: None,
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

    assert_eq!(
        text,
        concat!(
            "Object: billing.policy\n",
            "Kind: decision\n",
            "Status: accepted\n",
            "Owner: architecture\n",
            "\n",
            "Statement:\n",
            "Refund policy is ledger-backed.\n",
            "Manual credits are exceptions.\n",
            "\n",
            "Fields:\n",
            "- decided_by: architecture\n",
            "- scope: refunds\n",
            "\n",
            "Source: docs/decisions.adoc:7:1\n",
        )
    );
}

#[test]
fn text_retrieval_formatter_renders_each_relation_target_on_its_own_line() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
            id: "billing.policy".to_string(),
            kind: "decision".to_string(),
            status: Some("accepted".to_string()),
            owner: None,
            verified_at: None,
            body: "Refund policy is ledger-backed.".to_string(),
            source: RetrievalSource {
                path: "docs/decisions.adoc".to_string(),
                line: 7,
                column: 1,
            },
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::new(),
            relations: AgentJsonRelations {
                depends_on: vec![
                    "billing.credits.ledger-source".to_string(),
                    "billing.refunds.audit-required".to_string(),
                ],
                supersedes: vec![
                    "billing.refunds.manual-credit".to_string(),
                    "billing.refunds.email-approval".to_string(),
                ],
                related_to: vec![
                    "billing.credits.decrement-after-success".to_string(),
                    "billing.credits.reconciliation".to_string(),
                ],
            },
            search_match: None,
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");
    let relations = text
        .split_once("Relations:\n")
        .expect("relations block is rendered")
        .1;

    assert_eq!(
        relations,
        concat!(
            "- depends_on: billing.credits.ledger-source\n",
            "- depends_on: billing.refunds.audit-required\n",
            "- supersedes: billing.refunds.manual-credit\n",
            "- supersedes: billing.refunds.email-approval\n",
            "- related_to: billing.credits.decrement-after-success\n",
            "- related_to: billing.credits.reconciliation\n",
        )
    );
}

#[test]
fn text_retrieval_formatter_renders_glossary_kind_metadata() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "glossary".to_string(),
            status: None,
            owner: None,
            verified_at: None,
            body: "Credits are account balance units.".to_string(),
            source: RetrievalSource {
                path: "docs/glossary.adoc".to_string(),
                line: 4,
                column: 1,
            },
            evidence: std::collections::BTreeMap::new(),
            fields: std::collections::BTreeMap::from([(
                "canonical".to_string(),
                "billing credit".to_string(),
            )]),
            relations: AgentJsonRelations::default(),
            search_match: None,
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

    assert!(text.contains("Kind: glossary\n"));
    assert!(text.contains("Fields:\n- canonical: billing credit\n"));
}

#[test]
fn text_retrieval_formatter_renders_unknown_evidence_keys_after_known_order() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            owner: None,
            verified_at: None,
            body: "Credits decrement after successful payment.".to_string(),
            source: RetrievalSource {
                path: "docs/billing.adoc".to_string(),
                line: 9,
                column: 1,
            },
            evidence: std::collections::BTreeMap::from([
                ("artifact".to_string(), "ledger.csv".to_string()),
                ("reviewed_by".to_string(), "risk".to_string()),
                ("source".to_string(), "ledger".to_string()),
                ("z_probe".to_string(), "trace".to_string()),
            ]),
            fields: std::collections::BTreeMap::new(),
            relations: AgentJsonRelations::default(),
            search_match: None,
        }],
        Vec::new(),
    );

    let text = TextRetrievalFormatter
        .render(&envelope)
        .expect("text retrieval render succeeds");

    assert!(text.contains(concat!(
        "Evidence:\n",
        "- source: ledger\n",
        "- reviewed_by: risk\n",
        "- artifact: ledger.csv\n",
        "- z_probe: trace\n",
    )));
}

#[test]
fn json_retrieval_formatter_preserves_envelope_shape() {
    let envelope = RetrievalEnvelope::new(
        vec![RetrievalRecord {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
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
            relations: AgentJsonRelations::default(),
            search_match: None,
        }],
        Vec::new(),
    );

    let rendered = JsonRetrievalFormatter
        .render(&envelope)
        .expect("JSON retrieval render succeeds");
    let expected = serde_json::to_string_pretty(&envelope).expect("envelope serializes");

    assert_eq!(rendered, expected);
    let value: serde_json::Value = serde_json::from_str(&rendered).expect("rendered JSON parses");
    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert!(value["records"][0].get("match").is_none());
    assert!(value["records"][0].get("retrieval").is_none());
}

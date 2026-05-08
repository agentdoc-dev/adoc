mod support;

use support::v1_4::{V1_4Pilot, build_v1_4_pilot};

#[cfg(feature = "fastembed-it")]
use support::v1_4::build_v1_4_pilot_with_fastembed;

fn run_search_json(pilot: &V1_4Pilot, query: &str, semantic: bool) -> serde_json::Value {
    let mut args: Vec<String> = vec![
        "search".into(),
        query.into(),
        "--artifact".into(),
        pilot.agent_path.to_string_lossy().into_owned(),
        "--search-artifact".into(),
        pilot.search_path.to_string_lossy().into_owned(),
        "--format".into(),
        "json".into(),
        "--top".into(),
        "5".into(),
    ];
    if semantic {
        args.push("--semantic".into());
    }
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(&args)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .output()
        .expect("adoc search runs");
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
        panic!(
            "stdout is JSON: {e}; stderr={}",
            String::from_utf8_lossy(&output.stderr)
        )
    })
}

#[test]
fn semantic_search_reports_vector_rank_and_cosine_in_json_envelope() {
    let pilot = build_v1_4_pilot();
    let envelope = run_search_json(&pilot, "ledger entries for credits", true);
    assert_eq!(envelope["schema_version"], "adoc.retrieval.v0");
    let first = &envelope["records"][0];
    assert_eq!(first["match"]["mode"], "semantic");
    assert!(
        first["match"]["vector_rank"].is_number(),
        "vector_rank must be a number: {first}"
    );
    assert!(
        first["match"]["cosine_score"].is_number(),
        "cosine_score must be a number: {first}"
    );
}

#[test]
fn semantic_search_without_artifact_errors_out() {
    let pilot = build_v1_4_pilot();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            pilot.agent_path.to_str().unwrap(),
            "--search-artifact",
            "/nonexistent.search.json",
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .output()
        .expect("adoc runs");
    assert_eq!(output.status.code(), Some(2));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert!(
        envelope["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "search.artifact_missing"),
        "expected search.artifact_missing; got {envelope}"
    );
}

#[test]
fn semantic_search_without_artifact_does_not_pay_embed_model_load_cost() {
    let pilot = build_v1_4_pilot();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            pilot.agent_path.to_str().unwrap(),
            "--search-artifact",
            "/nonexistent.search.json",
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "force-load-fail")
        .output()
        .expect("adoc runs");
    assert_eq!(output.status.code(), Some(2));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let codes: Vec<&str> = envelope["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .map(|d| d["code"].as_str().unwrap_or(""))
        .collect();
    assert!(
        codes.contains(&"search.artifact_missing"),
        "expected search.artifact_missing; got {codes:?}"
    );
    assert!(
        !codes.contains(&"embed.model_load_failed"),
        "gate must short-circuit before embed_query; got {codes:?}"
    );
}

#[test]
fn lexical_default_unchanged_when_search_artifact_present() {
    let pilot = build_v1_4_pilot();
    let envelope = run_search_json(&pilot, "credits", false);
    let first = &envelope["records"][0];
    assert_eq!(first["match"]["mode"], "lexical");
    let m = first["match"].as_object().unwrap();
    assert!(
        m.get("vector_rank").is_none_or(|v| v.is_null()),
        "vector_rank should be absent or null for lexical: {first}"
    );
    assert!(
        m.get("cosine_score").is_none_or(|v| v.is_null()),
        "cosine_score should be absent or null for lexical: {first}"
    );
}

/// Smoke check on JSON envelope serialization stability for the in-memory
/// provider. Determinism on the production fastembed path is asserted in
/// `paraphrase_recall::fastembed_semantic_results_are_deterministic_across_two_runs`.
#[test]
fn semantic_results_are_deterministic_across_two_runs() {
    let pilot = build_v1_4_pilot();
    let one = run_search_json(&pilot, "ledger entries for credits", true);
    let two = run_search_json(&pilot, "ledger entries for credits", true);
    assert_eq!(one["records"], two["records"]);
}

#[test]
fn search_model_mismatch_disables_semantic_in_cli() {
    let mismatch = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../adoc-core/tests/fixtures/v1_4_semantic/model_mismatch.search.json");
    let pilot = build_v1_4_pilot();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            pilot.agent_path.to_str().unwrap(),
            "--search-artifact",
            mismatch.to_str().unwrap(),
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(2));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let diags = envelope["diagnostics"].as_array().unwrap();
    assert!(
        diags.iter().any(|d| d["code"] == "search.model_mismatch"),
        "expected search.model_mismatch; got {envelope}"
    );
}

#[test]
fn embed_failure_diagnostic_includes_default_help() {
    let pilot = build_v1_4_pilot();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "anything",
            "--artifact",
            pilot.agent_path.to_str().unwrap(),
            "--search-artifact",
            pilot.search_path.to_str().unwrap(),
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "force-load-fail")
        .output()
        .expect("adoc runs");
    assert_eq!(output.status.code(), Some(2));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    let embed_diag = envelope["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|d| d["code"] == "embed.model_load_failed")
        .expect("embed.model_load_failed must surface when load fails after gate");
    let help = embed_diag["help"]
        .as_str()
        .expect("manual CLI Diagnostic must include the per-code default help");
    assert!(!help.is_empty(), "default_help must be a non-empty string");
}

#[cfg(feature = "fastembed-it")]
mod paraphrase_recall {
    use super::*;

    const PARAPHRASES: &[(&str, &str)] = &[
        ("money tracking entries", "billing.credits.ledger-source"),
        ("returning charges audit", "billing.refunds.audit-required"),
        ("dlq retry behaviour", "ops.dlq.retry-policy"),
    ];

    fn run_search_json_no_env(pilot: &V1_4Pilot, query: &str) -> serde_json::Value {
        let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
            .args([
                "search",
                query,
                "--artifact",
                pilot.agent_path.to_str().unwrap(),
                "--search-artifact",
                pilot.search_path.to_str().unwrap(),
                "--semantic",
                "--format",
                "json",
                "--top",
                "3",
            ])
            // No ADOC_TEST_EMBEDDING_PROVIDER — real fastembed for both corpus and query.
            .output()
            .expect("adoc search runs");
        serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
            panic!(
                "stdout is JSON: {e}; stderr={}",
                String::from_utf8_lossy(&output.stderr)
            )
        })
    }

    #[test]
    fn fastembed_semantic_results_are_deterministic_across_two_runs() {
        let pilot = build_v1_4_pilot_with_fastembed();
        let one = run_search_json_no_env(&pilot, "ledger entries for credits");
        let two = run_search_json_no_env(&pilot, "ledger entries for credits");
        assert_eq!(one["records"], two["records"]);
    }

    #[test]
    fn paraphrase_recall_succeeds_in_top_3_under_semantic_with_fastembed() {
        let pilot = build_v1_4_pilot_with_fastembed();
        for (query, expected) in PARAPHRASES {
            let envelope = run_search_json_no_env(&pilot, query);
            let ids: Vec<&str> = envelope["records"]
                .as_array()
                .expect("records array")
                .iter()
                .filter_map(|record| record["id"].as_str())
                .take(3)
                .collect();
            assert!(
                ids.iter().any(|id| id == expected),
                "semantic search missed `{expected}` for query `{query}`. got={ids:?}"
            );
        }
    }
}

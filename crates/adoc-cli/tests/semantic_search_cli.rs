mod support;

use support::v1_4::{V1_4Pilot, build_v1_4_pilot};

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

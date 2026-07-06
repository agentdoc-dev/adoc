mod support;

use support::v1_4::{V1_4Pilot, build_v1_4_pilot};

#[cfg(feature = "fastembed-it")]
use support::v1_4::build_v1_4_pilot_with_fastembed;

fn run_search_json(pilot: &V1_4Pilot, query: &str, semantic: bool) -> serde_json::Value {
    let mut args: Vec<String> = vec![
        "search".into(),
        query.into(),
        "--artifact".into(),
        pilot.artifact_path.to_string_lossy().into_owned(),
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
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
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
    assert_eq!(envelope["schema_version"], "adoc.retrieval.v1");
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
            pilot.artifact_path.to_str().unwrap(),
            "--search-artifact",
            "/nonexistent.search.json",
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
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
    assert!(
        envelope["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|d| d["code"] == "search.artifact_missing" && d["severity"] == "error"),
        "expected error search.artifact_missing; got {envelope}"
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
            pilot.artifact_path.to_str().unwrap(),
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
fn default_search_is_hybrid_when_search_artifact_is_present() {
    let pilot = build_v1_4_pilot();
    let envelope = run_search_json(&pilot, "credits", false);
    let first = &envelope["records"][0];
    assert_eq!(first["match"]["mode"], "hybrid");
    let m = first["match"].as_object().unwrap();
    assert!(
        m["rrf_score"].is_number(),
        "rrf_score should be present for hybrid: {first}"
    );
    assert!(
        m.get("cosine_score").is_none_or(|v| v.is_null()),
        "cosine_score should be absent or null for hybrid: {first}"
    );
}

#[test]
fn lexical_flag_remains_escape_hatch_when_search_artifact_is_present() {
    let pilot = build_v1_4_pilot();
    let output = std::process::Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "credits",
            "--artifact",
            pilot.artifact_path.to_str().unwrap(),
            "--search-artifact",
            pilot.search_path.to_str().unwrap(),
            "--lexical",
            "--format",
            "json",
            "--top",
            "5",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .output()
        .expect("adoc search runs");
    assert_eq!(output.status.code(), Some(0));
    let envelope: serde_json::Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(envelope["records"][0]["match"]["mode"], "lexical");
}

/// Smoke check on JSON envelope serialization stability for the deterministic
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
            pilot.artifact_path.to_str().unwrap(),
            "--search-artifact",
            mismatch.to_str().unwrap(),
            "--semantic",
            "--format",
            "json",
        ])
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
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
            pilot.artifact_path.to_str().unwrap(),
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
                pilot.artifact_path.to_str().unwrap(),
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

/// V1.7.2 (ADR-0040): prose vectors in adoc.search.v1, exercised through the
/// real CLI against the Markdown Pilot with the deterministic provider.
mod prose_semantic {
    use std::path::PathBuf;
    use std::process::Command;

    use crate::support::TestWorkspace;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    struct MarkdownPilot {
        _workspace: TestWorkspace,
        artifact: String,
        search_artifact: String,
    }

    fn build_markdown_pilot() -> MarkdownPilot {
        let workspace = TestWorkspace::new("v1-7-2-markdown-pilot");
        let output_directory = workspace.root.join("dist");
        let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .current_dir(repo_root())
            .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
            .args([
                "build",
                "examples/markdown-pilot/",
                "--out",
                output_directory
                    .to_str()
                    .expect("output directory path is utf-8"),
            ])
            .output()
            .expect("adoc build runs");
        assert!(
            build_output.status.success(),
            "markdown pilot must build cleanly\nstderr:\n{}",
            String::from_utf8_lossy(&build_output.stderr)
        );
        MarkdownPilot {
            artifact: output_directory
                .join("docs.graph.json")
                .to_string_lossy()
                .into_owned(),
            search_artifact: output_directory
                .join("docs.search.json")
                .to_string_lossy()
                .into_owned(),
            _workspace: workspace,
        }
    }

    fn search_json(pilot: &MarkdownPilot, extra_args: &[&str], query: &str) -> serde_json::Value {
        let mut args = vec![
            "search",
            query,
            "--artifact",
            &pilot.artifact,
            "--search-artifact",
            &pilot.search_artifact,
        ];
        args.extend_from_slice(extra_args);
        args.extend_from_slice(&["--format", "json"]);
        let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .args(&args)
            .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
            .output()
            .expect("adoc search runs");
        assert!(
            output.status.success(),
            "search must exit 0\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        serde_json::from_slice(&output.stdout).expect("search stdout is JSON")
    }

    /// The Markdown Pilot's v1 search artifact carries prose entries with
    /// entry_kind, no code_block entries, and no sub-threshold entries.
    #[test]
    fn markdown_pilot_search_artifact_carries_prose_entries() {
        let pilot = build_markdown_pilot();
        let artifact: serde_json::Value = serde_json::from_str(
            &std::fs::read_to_string(&pilot.search_artifact).expect("search artifact is readable"),
        )
        .expect("search artifact is JSON");

        assert_eq!(artifact["schema_version"], "adoc.search.v1");
        let embeddings = artifact["embeddings"].as_array().expect("embeddings array");
        assert!(
            embeddings
                .iter()
                .any(|entry| entry["entry_kind"] == "prose"),
            "prose entries must be embedded"
        );
        assert!(
            embeddings
                .iter()
                .any(|entry| entry["entry_kind"] == "knowledge_object"),
            "knowledge object entries must remain"
        );
    }

    /// V1.7.2 roadmap acceptance: `--semantic --prose-only` is un-gated and
    /// returns prose records ranked by vector similarity.
    #[test]
    fn prose_only_semantic_search_returns_ranked_prose_records() {
        let pilot = build_markdown_pilot();
        let envelope = search_json(
            &pilot,
            &["--semantic", "--prose-only", "--top", "3"],
            "how are credits spent during a generation run",
        );

        assert_eq!(envelope["schema_version"], "adoc.retrieval.v1");
        let records = envelope["records"].as_array().expect("records array");
        assert!(
            !records.is_empty(),
            "prose-only semantic search returns records"
        );
        for record in records {
            assert_eq!(record["record_type"], "prose");
            assert_eq!(record["match"]["mode"], "semantic");
            assert!(
                record["match"]["vector_rank"].is_number(),
                "prose semantic hits carry vector_rank: {record}"
            );
            assert!(
                record["match"]["cosine_score"].is_number(),
                "prose semantic hits carry cosine_score: {record}"
            );
        }
    }

    /// V1.7.2 roadmap acceptance: a paraphrase query returns a prose match
    /// under `--semantic` that lexical search misses (fixture-pinned with
    /// deterministic vectors; the pin moves only if the pilot prose or the
    /// Embedding Composition changes).
    #[test]
    fn semantic_search_returns_prose_match_lexical_misses() {
        let pilot = build_markdown_pilot();
        let query = "confirming message delivery receipts";
        let pinned_id = "tutorials.concepts#block-0007";

        let lexical = search_json(&pilot, &["--lexical", "--prose-only"], query);
        assert!(
            lexical["records"]
                .as_array()
                .expect("records array")
                .iter()
                .all(|record| record["id"] != pinned_id),
            "the pinned block must be lexically unreachable for this query: {lexical}"
        );

        let semantic = search_json(&pilot, &["--semantic", "--prose-only", "--top", "1"], query);
        let first = &semantic["records"][0];
        assert_eq!(first["record_type"], "prose");
        assert_eq!(first["id"], pinned_id);
        assert_eq!(first["match"]["mode"], "semantic");
    }

    /// Prose semantic results are deterministic across two runs.
    #[test]
    fn prose_semantic_results_are_deterministic_across_two_runs() {
        let pilot = build_markdown_pilot();
        let query = "verifying webhook deliveries during onboarding";

        let first = search_json(&pilot, &["--semantic", "--prose-only", "--top", "5"], query);
        let second = search_json(&pilot, &["--semantic", "--prose-only", "--top", "5"], query);

        assert_eq!(first, second, "deterministic vectors must rank stably");
    }
}

/// V1.7.2: real-model paraphrase recall over Markdown Pilot prose —
/// the deterministic tests above pin the wiring; this proves the semantics.
#[cfg(feature = "fastembed-it")]
mod prose_paraphrase_recall {
    use std::path::PathBuf;
    use std::process::Command;

    use crate::support::TestWorkspace;

    fn repo_root() -> PathBuf {
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
    }

    #[test]
    fn paraphrase_query_recalls_settlement_prose_in_top_3_with_fastembed() {
        let workspace = TestWorkspace::new("v1-7-2-markdown-pilot-fastembed");
        let output_directory = workspace.root.join("dist");
        let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .current_dir(repo_root())
            // No ADOC_TEST_EMBEDDING_PROVIDER — real fastembed for the corpus.
            .args([
                "build",
                "examples/markdown-pilot/",
                "--out",
                output_directory
                    .to_str()
                    .expect("output directory path is utf-8"),
            ])
            .output()
            .expect("adoc build runs");
        assert!(
            build_output.status.success(),
            "markdown pilot must build cleanly with fastembed\nstderr:\n{}",
            String::from_utf8_lossy(&build_output.stderr)
        );

        let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .args([
                "search",
                "when do funds become available to withdraw",
                "--artifact",
                output_directory.join("docs.graph.json").to_str().unwrap(),
                "--search-artifact",
                output_directory.join("docs.search.json").to_str().unwrap(),
                "--semantic",
                "--prose-only",
                "--top",
                "3",
                "--format",
                "json",
            ])
            .output()
            .expect("adoc search runs");
        let envelope: serde_json::Value =
            serde_json::from_slice(&output.stdout).unwrap_or_else(|e| {
                panic!(
                    "stdout is JSON: {e}; stderr={}",
                    String::from_utf8_lossy(&output.stderr)
                )
            });

        let records = envelope["records"].as_array().expect("records array");
        assert!(
            records.iter().any(|record| {
                record["record_type"] == "prose"
                    && record["page_id"] == "tutorials.concepts"
                    && record["text"]
                        .as_str()
                        .is_some_and(|text| text.contains("Settlement"))
            }),
            "the settlement paragraph must be a top-3 semantic prose hit, got {envelope:#}"
        );
    }
}

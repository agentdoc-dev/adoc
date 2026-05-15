use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;

mod support;

use support::TestWorkspace;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

fn is_billing_pilot_adoc_path(path: &str) -> bool {
    let path = Path::new(path);

    path.extension()
        .is_some_and(|extension| extension == "adoc")
        && path
            .parent()
            .and_then(Path::file_name)
            .is_some_and(|directory| directory == "billing-pilot")
}

#[test]
fn billing_pilot_checks_builds_and_exposes_useful_artifacts() {
    let repo_root = repo_root();
    let example_path = "examples/billing-pilot";

    let check_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args(["check", example_path])
        .output()
        .expect("adoc check runs");

    assert!(
        check_output.status.success(),
        "expected billing pilot to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&check_output.stdout),
        String::from_utf8_lossy(&check_output.stderr)
    );

    let workspace = TestWorkspace::new("billing-pilot-build");
    let output_directory = workspace.root.join("dist");
    let build_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .args([
            "build",
            example_path,
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        build_output.status.success(),
        "expected billing pilot to build cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&build_output.stdout),
        String::from_utf8_lossy(&build_output.stderr)
    );

    let html = std::fs::read_to_string(output_directory.join("docs.html"))
        .expect("billing pilot HTML is written");
    assert!(html.contains("<article data-page-id=\"billing.claims\">"));
    assert!(html.contains("class=\"claim claim--verified\""));
    assert!(html.contains("class=\"warning warning--high\""));
    assert!(html.contains("href=\"#billing.credits\""));
    assert!(html.contains("<dt>depends_on</dt>"));

    assert!(!output_directory.join("docs.agent.json").exists());
    let graph_json_text = std::fs::read_to_string(output_directory.join("docs.graph.json"))
        .expect("billing pilot graph JSON is written");
    let graph_json: Value =
        serde_json::from_str(&graph_json_text).expect("graph JSON is valid JSON");
    assert_eq!(graph_json["schema_version"], "adoc.graph.v1");
    let nodes = graph_json["nodes"]
        .as_array()
        .expect("graph JSON nodes is an array");
    let objects: Vec<&Value> = nodes
        .iter()
        .filter(|node| node["type"] == "knowledge_object")
        .collect();

    assert!(
        objects.len() >= 30,
        "expected at least 30 Knowledge Objects, got {}",
        objects.len()
    );
    for kind in ["claim", "decision", "warning", "glossary"] {
        assert!(
            objects.iter().any(|object| object["kind"] == kind),
            "expected at least one {kind} object"
        );
    }

    let verified_claim_count = objects
        .iter()
        .filter(|object| object["kind"] == "claim" && object["status"] == "verified")
        .count();
    assert!(
        verified_claim_count >= 8,
        "expected at least 8 verified claims, got {verified_claim_count}"
    );

    let refund_claim = objects
        .iter()
        .find(|object| object["id"] == "billing.refunds.issue-credit")
        .expect("refund issue-credit claim is present");
    assert_eq!(refund_claim["fields"]["owner"], "team-billing");
    assert_eq!(refund_claim["fields"]["verified_at"], "2026-05-06");
    assert!(
        refund_claim["body"]
            .as_str()
            .expect("body is a string")
            .contains("[[billing.credits]]"),
        "graph JSON body should preserve citeable object-reference source text"
    );
    assert_eq!(
        refund_claim["relations"]["depends_on"],
        serde_json::json!([
            "billing.credits.ledger-source",
            "billing.refunds.audit-required"
        ])
    );

    let decrement_claim = objects
        .iter()
        .find(|object| object["id"] == "billing.credits.decrement-after-success")
        .expect("decrement-after-success claim is present");
    assert_eq!(decrement_claim["kind"], "claim");
    assert_eq!(decrement_claim["status"], "verified");
    assert_eq!(decrement_claim["fields"]["owner"], "team-billing");
    assert_eq!(decrement_claim["fields"]["verified_at"], "2026-05-06");
    assert_eq!(
        decrement_claim["relations"]["depends_on"],
        serde_json::json!(["billing.credits.ledger-source"])
    );

    let search_json_text = std::fs::read_to_string(output_directory.join("docs.search.json"))
        .expect("billing pilot search JSON is written");
    let search_json: Value =
        serde_json::from_str(&search_json_text).expect("search JSON is valid JSON");
    assert_eq!(search_json["schema_version"], "adoc.search.v0");
    assert_eq!(search_json["model"]["id"], "in-memory");
    assert_eq!(search_json["model"]["provider"], "test");
    assert_eq!(search_json["model"]["dim"], 384);
    assert_eq!(
        search_json["embeddings"]
            .as_array()
            .expect("search embeddings is an array")
            .len(),
        objects.len(),
        "search artifact should carry one embedding per Knowledge Object"
    );

    let artifact_path = output_directory.join("docs.graph.json");
    let artifact_arg = artifact_path
        .to_str()
        .expect("artifact path is UTF-8")
        .to_owned();
    let why_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args([
            "why",
            "billing.credits.decrement-after-success",
            "--artifact",
            &artifact_arg,
        ])
        .output()
        .expect("adoc why runs");

    assert!(
        why_output.status.success(),
        "expected billing pilot why to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&why_output.stdout),
        String::from_utf8_lossy(&why_output.stderr)
    );
    let why_stdout = String::from_utf8_lossy(&why_output.stdout);
    assert!(why_stdout.contains("Object: billing.credits.decrement-after-success"));
    assert!(why_stdout.contains("Kind: claim"));
    assert!(why_stdout.contains("Status: verified"));
    assert!(why_stdout.contains("Owner: team-billing"));
    assert!(why_stdout.contains("Verified: 2026-05-06"));
    assert!(why_stdout.contains("Evidence:"));
    assert!(why_stdout.contains("- source: billing service credit application trace 2026-05-05"));
    assert!(why_stdout.contains("- test: cargo test billing_credit_decrement_after_success"));
    assert!(why_stdout.contains("- reviewed_by: qa-billing"));
    assert!(why_stdout.contains("Relations:"));
    assert!(why_stdout.contains("- depends_on: billing.credits.ledger-source"));

    let why_json_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args([
            "why",
            "billing.credits.decrement-after-success",
            "--artifact",
            &artifact_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc why --format json runs");

    assert!(
        why_json_output.status.success(),
        "expected billing pilot why JSON to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&why_json_output.stdout),
        String::from_utf8_lossy(&why_json_output.stderr)
    );
    assert!(
        why_json_output.stderr.is_empty(),
        "success JSON mode should not emit stderr diagnostics"
    );
    let why_json: Value =
        serde_json::from_slice(&why_json_output.stdout).expect("why stdout is JSON");
    assert_eq!(why_json["schema_version"], "adoc.retrieval.v0");
    assert_eq!(why_json["diagnostics"], serde_json::json!([]));
    assert_eq!(
        why_json["records"][0]["id"],
        "billing.credits.decrement-after-success"
    );
    assert_eq!(why_json["records"][0]["kind"], "claim");
    assert_eq!(why_json["records"][0]["status"], "verified");
    assert_eq!(why_json["records"][0]["owner"], "team-billing");

    for object in objects {
        let source_span = &object["source_span"];
        assert!(
            source_span["path"]
                .as_str()
                .is_some_and(is_billing_pilot_adoc_path),
            "object should expose source path for citations: {object}"
        );
        assert!(
            source_span["line"].as_u64().is_some_and(|line| line > 0),
            "object should expose source line for citations: {object}"
        );
        assert!(
            source_span["column"]
                .as_u64()
                .is_some_and(|column| column > 0),
            "object should expose source column for citations: {object}"
        );
    }
}

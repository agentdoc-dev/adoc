#[allow(dead_code)]
mod support;

use std::path::{Path, PathBuf};
use std::process::Command;

use serde_json::Value;
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

    let agent_json_text = std::fs::read_to_string(output_directory.join("docs.agent.json"))
        .expect("billing pilot agent JSON is written");
    let agent_json: Value =
        serde_json::from_str(&agent_json_text).expect("agent JSON is valid JSON");
    let objects = agent_json["objects"]
        .as_array()
        .expect("agent JSON objects is an array");

    assert!(
        objects.len() >= 20,
        "expected at least 20 Knowledge Objects, got {}",
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
        verified_claim_count >= 5,
        "expected at least 5 verified claims, got {verified_claim_count}"
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
        "agent JSON body should preserve citeable object-reference source text"
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

    let artifact_path = output_directory.join("docs.agent.json");
    let artifact_arg = artifact_path
        .to_str()
        .expect("artifact path is UTF-8")
        .to_owned();
    let explain_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args([
            "explain",
            "billing.credits.decrement-after-success",
            "--artifact",
            &artifact_arg,
        ])
        .output()
        .expect("adoc explain runs");

    assert!(
        explain_output.status.success(),
        "expected billing pilot explain to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&explain_output.stdout),
        String::from_utf8_lossy(&explain_output.stderr)
    );
    let explain_stdout = String::from_utf8_lossy(&explain_output.stdout);
    assert!(explain_stdout.contains("Object: billing.credits.decrement-after-success"));
    assert!(explain_stdout.contains("Kind: claim"));
    assert!(explain_stdout.contains("Status: verified"));
    assert!(explain_stdout.contains("Owner: team-billing"));
    assert!(explain_stdout.contains("Verified: 2026-05-06"));
    assert!(explain_stdout.contains("Evidence:"));
    assert!(
        explain_stdout.contains("- source: billing service credit application trace 2026-05-05")
    );
    assert!(explain_stdout.contains("- test: cargo test billing_credit_decrement_after_success"));
    assert!(explain_stdout.contains("- reviewed_by: qa-billing"));
    assert!(explain_stdout.contains("Relations:"));
    assert!(explain_stdout.contains("- depends_on: billing.credits.ledger-source"));

    let explain_json_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&repo_root)
        .args([
            "explain",
            "billing.credits.decrement-after-success",
            "--artifact",
            &artifact_arg,
            "--format",
            "json",
        ])
        .output()
        .expect("adoc explain --format json runs");

    assert!(
        explain_json_output.status.success(),
        "expected billing pilot explain JSON to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&explain_json_output.stdout),
        String::from_utf8_lossy(&explain_json_output.stderr)
    );
    assert!(
        explain_json_output.stderr.is_empty(),
        "success JSON mode should not emit stderr diagnostics"
    );
    let explain_json: Value =
        serde_json::from_slice(&explain_json_output.stdout).expect("explain stdout is JSON");
    assert_eq!(explain_json["schema_version"], "adoc.retrieval.v0");
    assert_eq!(explain_json["diagnostics"], serde_json::json!([]));
    assert_eq!(
        explain_json["records"][0]["id"],
        "billing.credits.decrement-after-success"
    );
    assert_eq!(explain_json["records"][0]["kind"], "claim");
    assert_eq!(explain_json["records"][0]["status"], "verified");
    assert_eq!(explain_json["records"][0]["owner"], "team-billing");

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

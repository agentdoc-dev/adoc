mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, fixture_path};

fn copy_valid_artifact(workspace: &TestWorkspace, relative_path: &str) {
    let artifact = fs::read_to_string(fixture_path("v1_1_explain/valid_artifact.agent.json"))
        .expect("fixture artifact is readable");
    workspace.write(relative_path, &artifact);
}

#[test]
fn explain_defaults_to_dist_agent_json_and_text_format() {
    let workspace = TestWorkspace::new("explain-defaults");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.refunds.issue-credit"])
        .output()
        .expect("adoc explain runs");

    assert!(
        output.status.success(),
        "expected explain to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Object: billing.refunds.issue-credit"));
    assert!(stdout.contains("Kind: claim"));
    assert!(stdout.contains("Status: verified"));
    assert!(stdout.contains("Owner: team-billing"));
    assert!(stdout.contains("Verified: 2026-05-06"));
    assert!(
        stdout.contains("Statement: Refund credits are issued from the ledger after approval.")
    );
    assert!(stdout.contains("Evidence:"));
    assert!(stdout.contains("- source: ledger-export"));
    assert!(stdout.contains("- test: cargo test refunds"));
    assert!(stdout.contains("- reviewed_by: risk-team"));
    assert!(!stdout.contains("  source: ledger-export"));
    assert!(stdout.contains("Source: docs/refunds.adoc:12:3"));
    assert!(stdout.contains("Relations:"));
    assert!(stdout.contains("- depends_on: billing.credits.ledger-source"));
    assert!(stdout.contains("- supersedes: billing.refunds.manual-credit"));
    assert!(stdout.contains("- related_to: billing.refunds.audit-required"));
    assert!(!stdout.contains("  depends_on: billing.credits.ledger-source"));
}

#[test]
fn explain_uses_explicit_artifact_and_omits_unavailable_fields() {
    let workspace = TestWorkspace::new("explain-explicit-artifact");
    copy_valid_artifact(&workspace, "custom/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "explain",
            "billing.refunds.fraud-window",
            "--artifact",
            "custom/docs.agent.json",
            "--format",
            "text",
        ])
        .output()
        .expect("adoc explain runs");

    assert!(
        output.status.success(),
        "expected explain to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Object: billing.refunds.fraud-window"));
    assert!(stdout.contains("Kind: warning"));
    assert!(stdout.contains("Severity: high"));
    assert!(
        stdout
            .contains("Statement: Refund attempts above the risk threshold require manual review.")
    );
    assert!(stdout.contains("Evidence:"));
    assert!(stdout.contains("- source: risk-runbook"));
    assert!(!stdout.contains("  source: risk-runbook"));
    assert!(stdout.contains("Source: docs/refunds.adoc:28:1"));
    assert!(!stdout.contains("Owner:"));
    assert!(!stdout.contains("Verified:"));
    assert!(!stdout.contains("Relations:"));
}

#[test]
fn explain_object_not_found_exits_3() {
    let workspace = TestWorkspace::new("explain-not-found");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.missing"])
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(3));
    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stdout.is_empty(), "not-found diagnostics should use stderr");
    assert!(stderr.contains("retrieval.object_not_found"));
    assert!(stderr.contains("billing.missing"));
}

#[test]
fn explain_artifact_errors_exit_2() {
    let workspace = TestWorkspace::new("explain-artifact-missing");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.refunds.issue-credit"])
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(2));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("io.artifact_missing"));
    assert!(stderr.contains("dist/docs.agent.json"));
}

#[test]
fn explain_format_json_is_rejected_for_tracer_3() {
    let workspace = TestWorkspace::new("explain-json-rejected");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "explain",
            "billing.refunds.issue-credit",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("unsupported explain format: json"));
}

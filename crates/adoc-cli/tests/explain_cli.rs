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
    insta::assert_snapshot!("explain_plain", stdout);
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
            "plain",
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
        stdout.contains(
            "Statement:\nRefund attempts above the risk threshold require manual review."
        )
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
fn explain_text_renders_decision_and_glossary_metadata() {
    let workspace = TestWorkspace::new("explain-decision-glossary");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let decision_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.refunds.policy"])
        .output()
        .expect("adoc explain decision runs");

    assert!(
        decision_output.status.success(),
        "expected decision explain to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&decision_output.stdout),
        String::from_utf8_lossy(&decision_output.stderr)
    );
    let decision_stdout = String::from_utf8_lossy(&decision_output.stdout);
    assert!(decision_stdout.contains("Kind: decision"));
    assert!(decision_stdout.contains("Fields:\n- decided_by: architecture\n- scope: refunds"));
    assert!(decision_stdout.contains("Statement:\nRefund credits are issued only after approval."));

    let glossary_output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.refunds.credit"])
        .output()
        .expect("adoc explain glossary runs");

    assert!(
        glossary_output.status.success(),
        "expected glossary explain to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&glossary_output.stdout),
        String::from_utf8_lossy(&glossary_output.stderr)
    );
    let glossary_stdout = String::from_utf8_lossy(&glossary_output.stdout);
    assert!(glossary_stdout.contains("Kind: glossary"));
    assert!(glossary_stdout.contains("Fields:\n- canonical: refund credit"));
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
fn explain_format_json_emits_retrieval_envelope() {
    let workspace = TestWorkspace::new("explain-json-success");
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

    assert!(
        output.status.success(),
        "expected explain JSON to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "success JSON mode should not emit stderr diagnostics"
    );

    let stdout = String::from_utf8_lossy(&output.stdout);
    insta::assert_snapshot!("explain_json", stdout);
}

#[test]
fn explain_format_json_object_not_found_exits_3_with_envelope() {
    let workspace = TestWorkspace::new("explain-json-not-found");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["explain", "billing.missing", "--format", "json"])
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(3));
    assert!(
        output.stderr.is_empty(),
        "JSON diagnostics should be emitted in stdout envelope"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"], serde_json::json!([]));
    assert_eq!(
        value["diagnostics"][0]["code"],
        "retrieval.object_not_found"
    );
    assert_eq!(value["diagnostics"][0]["object_id"], "billing.missing");
}

#[test]
fn explain_format_json_artifact_errors_exit_2_with_envelope() {
    let workspace = TestWorkspace::new("explain-json-artifact-errors");
    let cases = [
        ("malformed_artifact.agent.json", "io.artifact_malformed"),
        (
            "unsupported_version.agent.json",
            "schema.unsupported_version",
        ),
        ("duplicate_id.agent.json", "id.duplicate_in_artifact"),
    ];

    for (fixture, expected_code) in cases {
        let artifact = fixture_path(&format!("v1_1_explain/{fixture}"));
        let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .current_dir(&workspace.root)
            .args([
                "explain",
                "billing.refunds.issue-credit",
                "--artifact",
                artifact.to_str().expect("fixture path is UTF-8"),
                "--format",
                "json",
            ])
            .output()
            .expect("adoc explain runs");

        assert_eq!(
            output.status.code(),
            Some(2),
            "expected artifact error exit for {fixture}"
        );
        assert!(
            output.stderr.is_empty(),
            "JSON diagnostics should be emitted in stdout envelope for {fixture}"
        );
        let value: serde_json::Value =
            serde_json::from_slice(&output.stdout).expect("stdout is JSON");
        assert_eq!(value["schema_version"], "adoc.retrieval.v0");
        assert_eq!(value["records"], serde_json::json!([]));
        assert_eq!(value["diagnostics"][0]["code"], expected_code);
    }
}

#[test]
fn top_level_help_exits_0_and_lists_commands() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .arg("--help")
        .output()
        .expect("adoc --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "help should render to stdout, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: adoc"));
    assert!(stdout.contains("check"));
    assert!(stdout.contains("build"));
    assert!(stdout.contains("explain"));
}

#[test]
fn top_level_version_exits_0_and_prints_version() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .arg("--version")
        .output()
        .expect("adoc --version runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "version should render to stdout, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains(env!("CARGO_PKG_VERSION")));
}

#[test]
fn explain_help_exits_0_and_lists_defaults() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["explain", "--help"])
        .output()
        .expect("adoc explain --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "command help should render to stdout, stderr was:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Usage: adoc explain [OPTIONS] <OBJECT_ID>"));
    assert!(stdout.contains("--artifact <ARTIFACT>"));
    assert!(stdout.contains("dist/docs.agent.json"));
    assert!(stdout.contains("--format <FORMAT>"));
    assert!(stdout.contains("plain"));
    assert!(stdout.contains("json"));
}

#[test]
fn explain_unsupported_format_exits_1_with_parse_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "explain",
            "billing.refunds.issue-credit",
            "--format",
            "yaml",
        ])
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "parse errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'yaml'"));
    assert!(stderr.contains("possible values:"));
}

#[test]
fn explain_missing_object_id_exits_1_with_parse_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .arg("explain")
        .output()
        .expect("adoc explain runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "parse errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("required arguments were not provided"));
    assert!(stderr.contains("<OBJECT_ID>"));
}

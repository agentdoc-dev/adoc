mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, fixture_path};

fn copy_search_artifact(workspace: &TestWorkspace, relative_path: &str) {
    let artifact = fs::read_to_string(fixture_path("v1_1_explain/valid_artifact.agent.json"))
        .expect("fixture artifact is readable");
    workspace.write(relative_path, &artifact);
}

fn pilot_subset_artifact() -> String {
    fixture_path("v1_2_search/pilot_subset.agent.json")
        .to_str()
        .expect("fixture path is UTF-8")
        .to_string()
}

fn empty_search_artifact() -> String {
    fixture_path("v1_2_search/empty.agent.json")
        .to_str()
        .expect("fixture path is UTF-8")
        .to_string()
}

fn search_object_ids(stdout: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(stdout)
        .lines()
        .filter_map(|line| line.strip_prefix("Object: ").map(str::to_string))
        .collect()
}

fn assert_search_top_3_contains(query: &str, expected_id: &str) {
    let artifact = pilot_subset_artifact();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["search", query, "--artifact", &artifact, "--top", "3"])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    let ids = search_object_ids(&output.stdout);
    assert!(
        ids.iter().any(|id| id == expected_id),
        "expected {expected_id} in top 3 for {query:?}, got {ids:?}"
    );
}

#[test]
fn search_cli_billing_pilot_subset_returns_benchmark_matches_in_top_3() {
    assert_search_top_3_contains("credit ledger", "billing.credits.ledger-source");
    assert_search_top_3_contains("refund audit", "billing.refunds.audit-required");
    assert_search_top_3_contains(
        "entitlement payment",
        "billing.entitlements.sync-after-payment",
    );
}

#[test]
fn search_cli_billing_pilot_subset_supports_exact_id_prefix_id_and_filters() {
    let artifact = pilot_subset_artifact();

    let exact = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "billing.credits.decrement-after-success",
            "--artifact",
            &artifact,
            "--top",
            "1",
        ])
        .output()
        .expect("adoc search runs");
    assert!(exact.status.success());
    assert_eq!(
        search_object_ids(&exact.stdout),
        ["billing.credits.decrement-after-success"]
    );

    let prefix = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "billing.credits",
            "--artifact",
            &artifact,
            "--top",
            "3",
        ])
        .output()
        .expect("adoc search runs");
    assert!(prefix.status.success());
    assert_eq!(
        search_object_ids(&prefix.stdout),
        [
            "billing.credits",
            "billing.credits.nonnegative",
            "billing.credits.ledger-source"
        ]
    );

    let filtered = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "ledger",
            "--artifact",
            &artifact,
            "--kind",
            "decision",
            "--status",
            "accepted",
            "--owner",
            "team-billing",
            "--source-path",
            "03-decisions.adoc",
            "--top",
            "1",
        ])
        .output()
        .expect("adoc search runs");
    assert!(filtered.status.success());
    assert_eq!(
        search_object_ids(&filtered.stdout),
        ["billing.decision.ledger-first"]
    );
}

#[test]
fn search_cli_empty_fixture_prints_no_matches() {
    let artifact = empty_search_artifact();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "credit ledger",
            "--artifact",
            &artifact,
            "--top",
            "3",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected empty search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "(no matches)\n");
}

#[test]
fn search_cli_defaults_to_dist_agent_json_and_text_format() {
    let workspace = TestWorkspace::new("search-defaults");
    copy_search_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger"])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "successful text search should not emit diagnostics"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Object: billing.refunds.issue-credit"));
    assert!(stdout.contains("Kind: claim"));
    assert!(
        stdout.contains("Statement:\nRefund credits are issued from the ledger after approval.")
    );
}

#[test]
fn search_cli_uses_explicit_artifact_path() {
    let workspace = TestWorkspace::new("search-explicit-artifact");
    copy_search_artifact(&workspace, "custom/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "search",
            "risk",
            "--artifact",
            "custom/docs.agent.json",
            "--format",
            "text",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Object: billing.refunds.fraud-window"));
    assert!(stdout.contains("Kind: warning"));
}

#[test]
fn search_cli_unsupported_format_exits_1_with_parse_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["search", "ledger", "--format", "yaml"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "parse errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value 'yaml'"));
    assert!(stderr.contains("possible values: text, json"));
}

#[test]
fn search_cli_top_zero_exits_1_with_parse_error() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["search", "ledger", "--top", "0"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "parse errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("invalid value '0'"));
    assert!(stderr.contains("--top <TOP>"));
}

#[test]
fn search_cli_missing_artifact_exits_2() {
    let workspace = TestWorkspace::new("search-artifact-missing");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(2));
    assert!(
        output.stdout.is_empty(),
        "artifact diagnostics in text mode should render to stderr"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("io.artifact_missing"));
    assert!(stderr.contains("dist/docs.agent.json"));
}

#[test]
fn search_cli_empty_result_exits_0_and_prints_no_matches() {
    let workspace = TestWorkspace::new("search-empty");
    copy_search_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "chargebacks"])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected empty search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_eq!(String::from_utf8_lossy(&output.stdout), "(no matches)\n");
}

#[test]
fn search_cli_invalid_filter_exits_1_and_prints_stderr_in_text_mode() {
    let workspace = TestWorkspace::new("search-invalid-filter-text");
    copy_search_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--kind", "runbook"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "invalid filter diagnostics in text mode should render to stderr"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("search.invalid_filter"));
    assert!(stderr.contains("kind=runbook"));
}

#[test]
fn search_cli_json_success_includes_envelope_records_diagnostics_and_match_metadata() {
    let workspace = TestWorkspace::new("search-json-success");
    copy_search_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "search",
            "ledger",
            "--format",
            "json",
            "--kind",
            "claim",
            "--status",
            "verified",
            "--owner",
            "team-billing",
            "--source-path",
            "refunds.adoc",
            "--top",
            "1",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected search JSON to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        output.stderr.is_empty(),
        "success JSON mode should not emit stderr diagnostics"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_eq!(value["records"][0]["id"], "billing.refunds.issue-credit");
    assert_eq!(value["records"][0]["match"]["mode"], "lexical");
    assert_eq!(value["records"][0]["match"]["lexical_rank"], 1);
}

#[test]
fn search_cli_json_invalid_filter_exits_1_with_envelope_diagnostics_and_no_stderr() {
    let workspace = TestWorkspace::new("search-json-invalid-filter");
    copy_search_artifact(&workspace, "dist/docs.agent.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--kind", "runbook", "--format", "json"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "JSON diagnostics should be emitted in stdout envelope"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.retrieval.v0");
    assert_eq!(value["records"], serde_json::json!([]));
    assert_eq!(value["diagnostics"][0]["code"], "search.invalid_filter");
    assert!(
        value["diagnostics"][0]["message"]
            .as_str()
            .expect("diagnostic message is a string")
            .contains("kind=runbook")
    );
}

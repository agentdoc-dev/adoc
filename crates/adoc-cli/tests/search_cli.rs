mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, fixture_path, workspace_fixture_path};

fn copy_search_artifact(workspace: &TestWorkspace, relative_path: &str) {
    let artifact = fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json"))
        .expect("fixture artifact is readable");
    workspace.write(relative_path, &artifact);
}

fn pilot_subset_artifact() -> String {
    workspace_fixture_path("v1_2_search/pilot_subset.graph.json")
        .to_str()
        .expect("fixture path is UTF-8")
        .to_string()
}

fn empty_search_artifact() -> String {
    workspace_fixture_path("v1_2_search/empty.graph.json")
        .to_str()
        .expect("fixture path is UTF-8")
        .to_string()
}

fn artifact_with_diagnostic(severity: &str) -> String {
    let artifact = fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json"))
        .expect("fixture artifact is readable");
    let mut value: serde_json::Value =
        serde_json::from_str(&artifact).expect("fixture artifact is JSON");
    value["diagnostics"] = serde_json::json!([
        {
            "code": "parse.raw_html",
            "severity": severity,
            "message": "artifact carried diagnostic",
            "span": null,
            "object_id": null,
            "help": "inspect source"
        }
    ]);
    serde_json::to_string_pretty(&value).expect("artifact serializes")
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
        .args([
            "search",
            query,
            "--artifact",
            &artifact,
            "--lexical",
            "--top",
            "3",
        ])
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
            "--lexical",
            "--top",
            "1",
        ])
        .output()
        .expect("adoc search runs");
    assert!(exact.status.success());
    // Pins ride above --top (ADR-0040): the exact-id pin plus one scored hit.
    let exact_ids = search_object_ids(&exact.stdout);
    assert_eq!(exact_ids[0], "billing.credits.decrement-after-success");
    assert_eq!(
        exact_ids.len(),
        2,
        "one scored hit keeps the --top 1 budget, got {exact_ids:?}"
    );

    let prefix = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "billing.credits",
            "--artifact",
            &artifact,
            "--lexical",
            "--top",
            "3",
        ])
        .output()
        .expect("adoc search runs");
    assert!(prefix.status.success());
    // All four prefix pins return in addition to the three scored slots.
    let prefix_ids = search_object_ids(&prefix.stdout);
    assert_eq!(
        prefix_ids[..4],
        [
            "billing.credits",
            "billing.credits.nonnegative",
            "billing.credits.ledger-source",
            "billing.credits.decrement-after-success"
        ]
    );
    assert_eq!(
        prefix_ids.len(),
        7,
        "three scored hits keep the --top 3 budget, got {prefix_ids:?}"
    );

    let filtered = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "ledger",
            "--artifact",
            &artifact,
            "--lexical",
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
            "--lexical",
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
fn search_cli_defaults_to_dist_graph_json_and_text_format() {
    let workspace = TestWorkspace::new("search-defaults");
    copy_search_artifact(&workspace, "dist/docs.graph.json");

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
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(
        stderr.matches("search.artifact_missing").count(),
        1,
        "default hybrid fallback should emit one search.artifact_missing warning, stderr:\n{stderr}"
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
    copy_search_artifact(&workspace, "custom/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "search",
            "risk",
            "--artifact",
            "custom/docs.graph.json",
            "--lexical",
            "--format",
            "plain",
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
fn search_cli_styled_color_always_emits_ansi_codes() {
    let workspace = TestWorkspace::new("search-styled-color-always");
    copy_search_artifact(&workspace, "dist/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env_remove("NO_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE")
        .args([
            "search",
            "ledger",
            "--lexical",
            "--format",
            "styled",
            "--color",
            "always",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected styled search to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains('\x1b'),
        "styled search output must contain ANSI escapes"
    );
    assert!(stdout.contains("billing.refunds.issue-credit"));
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
    assert!(stderr.contains("possible values:"));
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
    assert!(stderr.contains("dist/docs.graph.json"));
}

#[test]
fn search_cli_empty_result_exits_0_and_prints_no_matches() {
    let workspace = TestWorkspace::new("search-empty");
    copy_search_artifact(&workspace, "dist/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "chargebacks", "--lexical"])
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
    copy_search_artifact(&workspace, "dist/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--kind", "runbook", "--lexical"])
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
    copy_search_artifact(&workspace, "dist/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "search",
            "ledger",
            "--lexical",
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
    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_eq!(value["records"][0]["id"], "billing.refunds.issue-credit");
    assert_eq!(value["records"][0]["match"]["mode"], "lexical");
    assert_eq!(value["records"][0]["match"]["result_rank"], 1);
    assert_eq!(value["records"][0]["match"]["lexical_rank"], 1);
}

#[test]
fn search_cli_json_invalid_filter_exits_1_with_envelope_diagnostics_and_no_stderr() {
    let workspace = TestWorkspace::new("search-json-invalid-filter");
    copy_search_artifact(&workspace, "dist/docs.graph.json");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "search",
            "ledger",
            "--kind",
            "runbook",
            "--lexical",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stderr.is_empty(),
        "JSON diagnostics should be emitted in stdout envelope"
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.retrieval.v1");
    assert_eq!(value["records"], serde_json::json!([]));
    assert_eq!(value["diagnostics"][0]["code"], "search.invalid_filter");
    assert!(
        value["diagnostics"][0]["message"]
            .as_str()
            .expect("diagnostic message is a string")
            .contains("kind=runbook")
    );
}

#[test]
fn search_cli_json_success_includes_loaded_artifact_warnings() {
    let workspace = TestWorkspace::new("search-json-artifact-warning");
    workspace.write("dist/docs.graph.json", &artifact_with_diagnostic("warning"));

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--lexical", "--format", "json"])
        .output()
        .expect("adoc search runs");

    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["diagnostics"][0]["severity"], "warning");
    assert_eq!(value["diagnostics"][0]["code"], "parse.raw_html");
    assert!(
        !value["records"]
            .as_array()
            .expect("records array")
            .is_empty()
    );
}

#[test]
fn search_cli_text_success_prints_loaded_artifact_warnings_to_stderr() {
    let workspace = TestWorkspace::new("search-text-artifact-warning");
    workspace.write("dist/docs.graph.json", &artifact_with_diagnostic("warning"));

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--lexical"])
        .output()
        .expect("adoc search runs");

    assert!(output.status.success());
    assert!(String::from_utf8_lossy(&output.stdout).contains("Object:"));
    assert!(String::from_utf8_lossy(&output.stderr).contains("warning[parse.raw_html]"));
}

#[test]
fn search_cli_loaded_artifact_errors_exit_2() {
    let workspace = TestWorkspace::new("search-artifact-error");
    workspace.write("dist/docs.graph.json", &artifact_with_diagnostic("error"));

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--lexical", "--format", "json"])
        .output()
        .expect("adoc search runs");

    assert_eq!(output.status.code(), Some(2));
    assert!(output.stderr.is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["records"], serde_json::json!([]));
    assert_eq!(value["diagnostics"][0]["severity"], "error");
    assert_eq!(value["diagnostics"][0]["code"], "parse.raw_html");
}

// ---------------------------------------------------------------------------
// V1.7.1 (ADR-0040): blended prose search over the Markdown Pilot.
// ---------------------------------------------------------------------------

fn repo_root() -> std::path::PathBuf {
    std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("cli crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

/// Builds `examples/markdown-pilot/` into the workspace and returns the graph
/// artifact path.
fn build_markdown_pilot(workspace: &TestWorkspace) -> String {
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
    output_directory
        .join("docs.graph.json")
        .to_str()
        .expect("artifact path is utf-8")
        .to_string()
}

fn search_pilot_json(artifact: &str, extra_args: &[&str], query: &str) -> serde_json::Value {
    let mut args = vec!["search", query, "--artifact", artifact];
    args.extend_from_slice(extra_args);
    args.extend_from_slice(&["--format", "json"]);
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(&args)
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

/// V1.7.1 roadmap acceptance: a query matching only `.md` tutorial prose
/// returns a `record_type: "prose"` match with the correct block id,
/// `heading_context`, and source path, exit 0.
#[test]
fn search_cli_blended_returns_md_prose_record_with_heading_context() {
    let workspace = TestWorkspace::new("md-pilot-blended-prose");
    let artifact = build_markdown_pilot(&workspace);

    let envelope = search_pilot_json(&artifact, &["--lexical"], "delivery log");

    assert_eq!(envelope["schema_version"], "adoc.retrieval.v1");
    let records = envelope["records"].as_array().expect("records array");
    let prose = records
        .iter()
        .find(|record| record["record_type"] == "prose")
        .expect("a prose record for .md-only tutorial text");
    let id = prose["id"].as_str().expect("prose id is a string");
    assert!(
        id.starts_with("tutorials.getting-started#block-"),
        "prose id must be the page-scoped positional block id, got {id}"
    );
    assert_eq!(prose["page_id"], "tutorials.getting-started");
    assert_eq!(prose["block_kind"], "paragraph");
    assert_eq!(
        prose["heading_context"],
        "Getting Started with Acme Payments > Step 3 — Verify a webhook"
    );
    assert!(
        prose["source"]["path"]
            .as_str()
            .expect("source path is a string")
            .ends_with("tutorials/getting-started.md"),
        "prose source must point at the .md file, got {:?}",
        prose["source"]
    );
    assert_eq!(prose["match"]["mode"], "lexical");
}

#[test]
fn search_cli_objects_only_suppresses_prose_records() {
    let workspace = TestWorkspace::new("md-pilot-objects-only");
    let artifact = build_markdown_pilot(&workspace);

    let envelope = search_pilot_json(&artifact, &["--lexical", "--objects-only"], "refund");

    let records = envelope["records"].as_array().expect("records array");
    assert!(!records.is_empty(), "refund matches Knowledge Objects");
    assert!(
        records
            .iter()
            .all(|record| record["record_type"] == "knowledge_object"),
        "--objects-only must return Knowledge Objects only, got {records:#}",
        records = envelope["records"]
    );
}

#[test]
fn search_cli_prose_only_suppresses_knowledge_objects() {
    let workspace = TestWorkspace::new("md-pilot-prose-only");
    let artifact = build_markdown_pilot(&workspace);

    let envelope = search_pilot_json(&artifact, &["--lexical", "--prose-only"], "refund");

    let records = envelope["records"].as_array().expect("records array");
    assert!(!records.is_empty(), "refund matches .md prose");
    assert!(
        records
            .iter()
            .all(|record| record["record_type"] == "prose"),
        "--prose-only must return prose records only, got {records:#}",
        records = envelope["records"]
    );
}

/// Object ID pins stay on top of the blended list (ADR-0040).
#[test]
fn search_cli_exact_object_id_pins_knowledge_object_first_in_blended() {
    let workspace = TestWorkspace::new("md-pilot-pin");
    let artifact = build_markdown_pilot(&workspace);

    let envelope = search_pilot_json(&artifact, &["--lexical"], "billing.refunds.issue-credit");

    assert_eq!(envelope["records"][0]["record_type"], "knowledge_object");
    assert_eq!(envelope["records"][0]["id"], "billing.refunds.issue-credit");
}

/// `--prose-only` conflicts with Knowledge Object metadata filters at the
/// argument layer — prose has no kind to filter on.
#[test]
fn search_cli_prose_only_conflicts_with_kind_filter() {
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args(["search", "refund", "--prose-only", "--kind", "claim"])
        .output()
        .expect("adoc search runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "conflicting flags must fail as a usage error (house convention: exit 1)\nstderr:\n{}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stderr)
            .contains("'--prose-only' cannot be used with '--kind"),
        "clap must name the conflicting pair"
    );
}

#[test]
fn search_cli_plain_format_renders_prose_record_with_context() {
    let workspace = TestWorkspace::new("md-pilot-plain-prose");
    let artifact = build_markdown_pilot(&workspace);

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .args([
            "search",
            "delivery log",
            "--artifact",
            &artifact,
            "--lexical",
            "--prose-only",
            "--format",
            "plain",
        ])
        .output()
        .expect("adoc search runs");

    assert!(output.status.success());
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("Prose: tutorials.getting-started#block-"),
        "plain output labels prose hits distinctly from Object IDs, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Context: Getting Started with Acme Payments > Step 3 — Verify a webhook"),
        "plain output shows the heading breadcrumb, got:\n{stdout}"
    );
    assert!(stdout.contains("Text:"), "plain output has a Text section");
    assert!(
        stdout.contains("Source: ") && stdout.contains("tutorials/getting-started.md"),
        "plain output cites the .md source span, got:\n{stdout}"
    );
}

mod support;

use std::process::Command;

use support::TestWorkspace;

fn git(workspace: &TestWorkspace, args: &[&str]) -> String {
    let output = Command::new("git")
        .current_dir(&workspace.root)
        .args(args)
        .output()
        .expect("git runs");
    assert!(
        output.status.success(),
        "git {args:?} failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8(output.stdout)
        .expect("UTF-8 git output")
        .trim()
        .to_string()
}

fn repo() -> TestWorkspace {
    let workspace = TestWorkspace::new("assess-changes-complete");
    git(&workspace, &["init", "--initial-branch=main"]);
    git(&workspace, &["config", "user.email", "test@agentdoc.dev"]);
    git(&workspace, &["config", "user.name", "AgentDoc Test"]);
    git(&workspace, &["config", "commit.gpgsign", "false"]);
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    workspace.write(
        "docs/billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n\n",
            "::claim billing.credits\n",
            "status: verified\n",
            "owner: billing-platform\n",
            "verified_at: 2026-07-01\n",
            "source: src/billing.rs\n",
            "impacts: [src/billing.rs]\n",
            "--\nCredits settle after payment.\n::\n",
        ),
    );
    workspace.write("src/billing.rs", "pub fn settle() {}\n");
    git(&workspace, &["add", "-A"]);
    git(&workspace, &["commit", "-m", "initial"]);
    workspace
}

#[test]
fn assess_changes_emits_one_complete_body_free_record_per_changed_path() {
    let workspace = repo();
    workspace.write("src/billing.rs", "pub fn settle() { charge(); }\n");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "assess-changes",
            "--base",
            "HEAD",
            "--as-of",
            "2026-07-22",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc assess-changes runs");
    assert!(
        output.status.success(),
        "assessment failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assessment JSON");
    assert_eq!(value["schema_version"], "adoc.change_assessment.v0");
    assert_eq!(value["completeness"], "complete");
    assert_eq!(value["outcome"], "review_required");
    assert_eq!(value["evaluation_date"], "2026-07-22");
    assert_eq!(value["paths"]["value"].as_array().expect("paths").len(), 1);
    assert_eq!(value["paths"]["value"][0]["classification"], "covered");
    assert_eq!(value["objects"]["value"][0]["id"], "billing.credits");
    assert!(value["objects"]["value"][0].get("body").is_none());
}

#[test]
fn same_pr_exclusion_is_prospective_and_cannot_hide_code() {
    let workspace = repo();
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\nassessment:\n  exclude_paths:\n    - src/\n",
    );
    workspace.write("src/new.rs", "pub fn new_behavior() {}\n");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["assess-changes", "--base", "HEAD", "--format", "json"])
        .output()
        .expect("adoc assess-changes runs");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assessment JSON");
    let paths = value["paths"]["value"].as_array().expect("paths");
    let source = paths
        .iter()
        .find(|path| path["path"] == "src/new.rs")
        .expect("source path is assessed");
    assert_eq!(source["classification"], "uncovered");
    assert_eq!(value["policy_changes"]["changed"], true);
}

#[test]
fn unresolved_base_emits_error_not_evaluated_envelope_and_exits_two() {
    let workspace = repo();
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "assess-changes",
            "--base",
            "missing-ref",
            "--format",
            "json",
        ])
        .output()
        .expect("adoc assess-changes runs");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("error JSON");
    assert_eq!(value["completeness"], "error");
    assert_eq!(value["outcome"], "not_evaluated");
    assert_eq!(value["paths"]["status"], "unavailable");
    assert_eq!(value["diagnostics"][0]["code"], "assessment.ref_unresolved");
}

#[test]
fn invalid_head_emits_error_invalid_without_fake_empty_graph_sections() {
    let workspace = repo();
    workspace.write("docs/billing.adoc", "::claim broken\n::\n");
    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["assess-changes", "--base", "HEAD", "--format", "json"])
        .output()
        .expect("adoc assess-changes runs");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("error JSON");
    assert_eq!(value["completeness"], "error");
    assert_eq!(value["outcome"], "invalid");
    assert_eq!(value["knowledge_snapshot"]["status"], "unavailable");
    assert_eq!(value["objects"]["status"], "unavailable");
}

#[test]
fn invalid_comparison_base_retains_head_graph_in_partial_envelope() {
    let workspace = repo();
    workspace.write("agentdoc.config.yaml", "version: [broken\n");
    git(&workspace, &["add", "agentdoc.config.yaml"]);
    git(&workspace, &["commit", "-m", "broken base config"]);
    let base = git(&workspace, &["rev-parse", "HEAD"]);
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["assess-changes", "--base", &base, "--format", "json"])
        .output()
        .expect("adoc assess-changes runs");

    assert_eq!(output.status.code(), Some(2));
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("partial JSON");
    assert_eq!(value["completeness"], "partial");
    assert_eq!(value["outcome"], "not_evaluated");
    assert_eq!(value["knowledge_snapshot"]["status"], "available");
    assert_eq!(value["knowledge_changes"]["status"], "unavailable");
    assert_eq!(value["objects"]["value"][0]["changed_in_pr"], "unknown");
}

#[test]
fn deleting_authoritative_knowledge_keeps_a_body_free_review_tombstone() {
    let workspace = repo();
    std::fs::remove_file(workspace.root.join("docs/billing.adoc")).expect("delete source");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args(["assess-changes", "--base", "HEAD", "--format", "json"])
        .output()
        .expect("adoc assess-changes runs");
    assert!(output.status.success());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("assessment JSON");
    assert_eq!(value["outcome"], "review_required");
    let deleted = &value["knowledge_changes"]["value"]["deleted"][0];
    assert_eq!(deleted["id"], "billing.credits");
    assert_eq!(deleted["kind"], "claim");
    assert_eq!(deleted["authority"], "authoritative");
    assert!(deleted["base_content_hash"].as_str().is_some());
    assert!(deleted.get("body").is_none());
    assert_eq!(value["proof_obligations"][0]["kind"], "claim");
}

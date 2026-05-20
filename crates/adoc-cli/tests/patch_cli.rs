mod support;

use support::{TestWorkspace, adoc_command, stderr, stdout};

fn build_patch_workspace(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    workspace.write(
        "docs/billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits apply after payment.\n",
            "::\n",
            "\n",
            "::claim billing.old-credits\n",
            "status: draft\n",
            "--\n",
            "Old credits behavior.\n",
            "::\n",
        ),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("adoc build runs");
    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    workspace
}

fn content_hash(workspace: &TestWorkspace, id: &str) -> String {
    let graph_text = std::fs::read_to_string(workspace.root.join("dist/docs.graph.json"))
        .expect("graph artifact is readable");
    let graph: serde_json::Value =
        serde_json::from_str(&graph_text).expect("graph artifact is JSON");
    graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["type"] == "knowledge_object" && node["id"] == id)
        .and_then(|node| node["content_hash"].as_str())
        .expect("node content_hash")
        .to_string()
}

fn replace_body_patch(target: &str, base_hash: &str) -> serde_json::Value {
    serde_json::json!({
        "schema_version": "adoc.patch.v0",
        "op": "replace_body",
        "target": target,
        "base_hash": base_hash,
        "changes": { "body": "Credits apply after ledger commit." },
        "reason": "Updated after ledger refactor.",
        "proposer": { "type": "agent", "id": "code-review-agent" }
    })
}

fn create_accepted_decision_patch(include_decided_by: bool) -> serde_json::Value {
    let mut fields = serde_json::Map::new();
    if include_decided_by {
        fields.insert(
            "decided_by".to_string(),
            serde_json::json!("architecture-review"),
        );
    }

    serde_json::json!({
        "schema_version": "adoc.patch.v0",
        "op": "create_object",
        "target": "billing.accepted-policy",
        "changes": {
            "kind": "decision",
            "status": "accepted",
            "body": "Use ledger-backed credits for customer adjustments.",
            "fields": fields,
            "placement": {
                "page_id": "team.billing",
                "after": "billing.credits"
            }
        },
        "reason": "Record accepted billing policy."
    })
}

#[test]
fn patch_check_plain_reports_valid_review_diff() {
    let workspace = build_patch_workspace("patch-plain");
    let hash = content_hash(&workspace, "billing.credits");
    workspace.write(
        "patch.json",
        &serde_json::to_string_pretty(&replace_body_patch("billing.credits", &hash))
            .expect("patch serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "patch.json", "--format", "plain"])
        .output()
        .expect("adoc patch runs");

    assert!(
        output.status.success(),
        "expected patch check to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stderr(&output).is_empty());
    let stdout = stdout(&output);
    assert!(stdout.contains("Status: valid (accepted for review)"));
    assert!(stdout.contains("Target: billing.credits"));
    assert!(stdout.contains("Operation: replace_body"));
    assert!(stdout.contains("Diffs:"));
    assert!(stdout.contains("- body:"));
}

#[test]
fn patch_check_json_uses_patch_check_envelope() {
    let workspace = build_patch_workspace("patch-json");
    let hash = content_hash(&workspace, "billing.credits");
    workspace.write(
        "patch.json",
        &serde_json::to_string_pretty(&replace_body_patch("billing.credits", &hash))
            .expect("patch serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    assert!(
        output.status.success(),
        "expected patch JSON to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stderr(&output).is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.patch.check.v0");
    assert_eq!(value["valid"], true);
    assert_eq!(value["target"], "billing.credits");
    assert_eq!(value["diffs"][0]["field"], "body");
}

#[test]
fn patch_check_exit_codes_distinguish_validation_io_missing_and_stale_hash() {
    let workspace = build_patch_workspace("patch-exit-codes");
    let hash = content_hash(&workspace, "billing.credits");

    let mut invalid = replace_body_patch("billing.credits", &hash);
    invalid["reason"] = serde_json::json!("");
    workspace.write(
        "invalid.json",
        &serde_json::to_string_pretty(&invalid).expect("patch serializes"),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "invalid.json"])
        .output()
        .expect("adoc patch invalid runs");
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("patch.invalid_document"));

    workspace.write("malformed.json", "{");
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "malformed.json"])
        .output()
        .expect("adoc patch malformed runs");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("io.artifact_malformed"));

    workspace.write(
        "missing-target.json",
        &serde_json::to_string_pretty(&replace_body_patch("billing.missing", &hash))
            .expect("patch serializes"),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "missing-target.json"])
        .output()
        .expect("adoc patch missing target runs");
    assert_eq!(output.status.code(), Some(3));
    assert!(stderr(&output).contains("graph.object_not_found"));

    workspace.write(
        "stale.json",
        &serde_json::to_string_pretty(&replace_body_patch("billing.credits", "sha256:stale"))
            .expect("patch serializes"),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "stale.json"])
        .output()
        .expect("adoc patch stale hash runs");
    assert_eq!(output.status.code(), Some(4));
    assert!(stderr(&output).contains("patch.base_hash_mismatch"));
}

#[test]
fn patch_check_accepts_create_object_accepted_decision_with_decided_by() {
    let workspace = build_patch_workspace("patch-create-accepted-decision-valid");
    workspace.write(
        "patch.json",
        &serde_json::to_string_pretty(&create_accepted_decision_patch(true))
            .expect("patch serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    assert!(
        output.status.success(),
        "expected accepted decision patch to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stderr(&output).is_empty());
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.patch.check.v0");
    assert_eq!(value["valid"], true);
    assert_eq!(value["operation"], "create_object");
    assert_eq!(value["target"], "billing.accepted-policy");
    assert_eq!(value["diffs"][0]["new"]["kind"], "decision");
    assert_eq!(
        value["diffs"][0]["new"]["fields"]["decided_by"],
        "architecture-review"
    );
}

#[test]
fn patch_check_rejects_create_object_accepted_decision_without_decided_by() {
    let workspace = build_patch_workspace("patch-create-accepted-decision-invalid");
    workspace.write(
        "patch.json",
        &serde_json::to_string_pretty(&create_accepted_decision_patch(false))
            .expect("patch serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "patch.json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("patch.validation_failed"));
}

#[test]
fn patch_check_reports_artifact_load_failure_for_missing_graph_content_hash() {
    let workspace = build_patch_workspace("patch-missing-graph-content-hash");
    let mut graph: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(workspace.root.join("dist/docs.graph.json"))
            .expect("graph artifact is readable"),
    )
    .expect("graph artifact is JSON");
    let nodes = graph["nodes"].as_array_mut().expect("nodes array");
    let node = nodes
        .iter_mut()
        .find(|node| node["type"] == "knowledge_object" && node["id"] == "billing.credits")
        .expect("billing.credits node exists");
    node.as_object_mut()
        .expect("knowledge object node is object")
        .remove("content_hash");
    workspace.write(
        "dist/docs.graph.json",
        &serde_json::to_string_pretty(&graph).expect("graph serializes"),
    );
    workspace.write(
        "patch.json",
        &serde_json::to_string_pretty(&replace_body_patch("billing.credits", "sha256:unused"))
            .expect("patch serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--check", "patch.json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("io.artifact_malformed"));
}

#[test]
fn patch_help_lists_check_and_artifact_flags() {
    let output = adoc_command()
        .args(["patch", "--help"])
        .output()
        .expect("adoc patch --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(stderr(&output).is_empty());
    let stdout = stdout(&output);
    assert!(stdout.contains("Usage: adoc patch [OPTIONS] --check <PATCH_JSON>"));
    assert!(stdout.contains("--check <PATCH_JSON>"));
    assert!(stdout.contains("--artifact <ARTIFACT>"));
    assert!(stdout.contains("adoc patch --check patch.json"));
}

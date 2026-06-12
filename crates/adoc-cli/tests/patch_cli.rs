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
fn patch_help_lists_check_apply_and_artifact_flags() {
    let output = adoc_command()
        .args(["patch", "--help"])
        .output()
        .expect("adoc patch --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(stderr(&output).is_empty());
    let stdout = stdout(&output);
    assert!(stdout.contains("--check <PATCH_JSON>"));
    assert!(stdout.contains("--apply <PATCH_JSON_OR_@->"));
    assert!(stdout.contains("--artifact <ARTIFACT>"));
    assert!(stdout.contains("adoc patch --check patch.json"));
    assert!(stdout.contains("adoc patch --apply patch.json"));
}

// ---------------------------------------------------------------------------
// V6.4 TB1 — `adoc patch --apply`
// ---------------------------------------------------------------------------

/// Apply needs config-driven docs-root resolution: `content_hash` payloads
/// embed source paths, so the artifact must be built through the same
/// (config) chain apply recompiles through.
fn build_apply_workspace(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    workspace.write(
        "agentdoc.config.yaml",
        concat!(
            "version: 1\n",
            "mode: strict\n",
            "docs_path: docs\n",
            "outputs:\n",
            "  dir: dist\n",
            "embeddings:\n",
            "  provider: none\n",
        ),
    );
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
        .args(["build"])
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

#[test]
fn patch_apply_json_rewrites_body_and_exits_zero() {
    let workspace = build_apply_workspace("patch-apply-happy");
    let base_hash = content_hash(&workspace, "billing.credits");
    let original =
        std::fs::read_to_string(workspace.root.join("docs/billing.adoc")).expect("source readable");
    workspace.write(
        "patch.json",
        &replace_body_patch("billing.credits", &base_hash).to_string(),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--apply", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(0), "stderr:\n{}", stderr(&output));
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.patch.apply.v0");
    assert_eq!(envelope["applied"], true);
    assert_eq!(envelope["target"], "billing.credits");
    assert_eq!(envelope["operation"], "replace_body");
    assert_eq!(envelope["check"]["valid"], true);
    assert_eq!(envelope["post_check"]["ran"], true);
    assert_eq!(envelope["post_check"]["error_count"], 0);
    assert_eq!(envelope["artifacts_stale"], true);
    assert_eq!(envelope["trace"]["interface"], "cli");
    assert_eq!(envelope["trace"]["proposer"]["kind"], "agent");
    let written = envelope["written_files"]
        .as_array()
        .expect("written_files array");
    assert_eq!(written.len(), 1);
    assert!(
        written[0]["path"]
            .as_str()
            .expect("path string")
            .ends_with("docs/billing.adoc")
    );

    let rewritten =
        std::fs::read_to_string(workspace.root.join("docs/billing.adoc")).expect("source readable");
    assert_eq!(
        rewritten,
        original.replace(
            "Credits apply after payment.",
            "Credits apply after ledger commit."
        ),
        "only the body line changes"
    );
}

#[test]
fn patch_apply_refusal_exits_one_and_writes_nothing() {
    let workspace = build_apply_workspace("patch-apply-refusal");
    let original =
        std::fs::read_to_string(workspace.root.join("docs/billing.adoc")).expect("source readable");
    workspace.write(
        "patch.json",
        &replace_body_patch("billing.credits", "sha256:stale").to_string(),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--apply", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    // Apply refusals are exit 1 — deliberately NOT the check's exit-4
    // stale-hash convention (ADR-0036).
    assert_eq!(output.status.code(), Some(1));
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(envelope["applied"], false);
    assert_eq!(envelope["written_files"].as_array().map(Vec::len), Some(0));
    assert_eq!(envelope["check"]["valid"], false);
    assert!(
        stdout(&output).contains("patch.base_hash_mismatch"),
        "envelope diagnostics name the stale hash"
    );
    let untouched =
        std::fs::read_to_string(workspace.root.join("docs/billing.adoc")).expect("source readable");
    assert_eq!(untouched, original, "refusal must write nothing");
}

#[test]
fn patch_apply_with_post_check_errors_exits_two_and_keeps_the_write() {
    let workspace = build_apply_workspace("patch-apply-postcheck");
    let base_hash = content_hash(&workspace, "billing.credits");
    let mut patch = replace_body_patch("billing.credits", &base_hash);
    patch["changes"]["body"] = serde_json::json!("Credits depend on [[no.such.object]] semantics.");
    workspace.write("patch.json", &patch.to_string());

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--apply", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(
        output.status.code(),
        Some(2),
        "applied-but-post-check-errors is exit 2\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(envelope["applied"], true, "never auto-revert");
    assert!(
        envelope["post_check"]["error_count"]
            .as_u64()
            .expect("error count")
            > 0
    );
    let written =
        std::fs::read_to_string(workspace.root.join("docs/billing.adoc")).expect("source readable");
    assert!(written.contains("no.such.object"), "the write stays on disk");
}

#[test]
fn patch_apply_reads_inline_patch_from_stdin_with_at_dash() {
    use std::io::Write as _;
    use std::process::Stdio;

    let workspace = build_apply_workspace("patch-apply-stdin");
    let base_hash = content_hash(&workspace, "billing.credits");
    let patch = replace_body_patch("billing.credits", &base_hash).to_string();

    let mut child = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--apply", "@-", "--format", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("adoc patch spawns");
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(patch.as_bytes())
        .expect("write patch to stdin");
    let output = child.wait_with_output().expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(0), "stderr:\n{}", stderr(&output));
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(envelope["applied"], true);
}

#[test]
fn patch_check_and_apply_flags_conflict() {
    let output = adoc_command()
        .args(["patch", "--check", "a.json", "--apply", "b.json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(1), "usage error");
    assert!(stderr(&output).contains("cannot be used with"));
}

#[test]
fn patch_apply_unparseable_patch_is_a_refusal_envelope_not_a_process_error() {
    let workspace = build_apply_workspace("patch-apply-unparseable");
    workspace.write("patch.json", "{\"schema_version\": \"adoc.patch.v0\"}");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["patch", "--apply", "patch.json", "--format", "json"])
        .output()
        .expect("adoc patch runs");

    assert_eq!(output.status.code(), Some(1));
    let envelope: serde_json::Value =
        serde_json::from_str(&stdout(&output)).expect("stdout is JSON");
    assert_eq!(envelope["schema_version"], "adoc.patch.apply.v0");
    assert_eq!(envelope["applied"], false);
}

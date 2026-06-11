use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use adoc_core::{
    GitRef, ObjectDiffEnvelope, ReviewEnvelope, ReviewInput, SnapshotSelector, diff_objects,
    load_review_from_git, load_review_with_changed_files_from_git, parse_patch_from_value,
    review_with_patch,
};
use adoc_mcp::{
    AdocPatchCheckParams, AdocReviewParams, AgentDocMcpServer, BuildParams, GraphParams,
    InitParams, PatchInput, ProjectStatusParams, SearchParams, StaleParams,
};
use serde_json::json;

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

fn source() -> &'static str {
    "# Billing @doc(team.billing)\n\n::claim billing.ready\nstatus: draft\n--\nBilling docs are ready.\n::\n"
}

fn schema(name: &str) -> serde_json::Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/agent/v0/schema")
        .join(name);
    serde_json::from_str(&fs::read_to_string(path).expect("schema is readable"))
        .expect("schema is json")
}

fn assert_valid(schema_name: &str, instance: &serde_json::Value) {
    let schema = schema(schema_name);
    let validator = jsonschema::validator_for(&schema).expect("schema compiles");
    let errors = validator
        .iter_errors(instance)
        .map(|error| error.to_string())
        .collect::<Vec<_>>();
    assert!(
        errors.is_empty(),
        "{schema_name} validation failed:\n{}\ninstance:\n{}",
        errors.join("\n"),
        serde_json::to_string_pretty(instance).expect("instance pretty prints")
    );
}

fn project_with_built_graph() -> (tempfile::TempDir, AgentDocMcpServer, String) {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());
    server
        .run_build(BuildParams {
            project_root: None,
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect("build succeeds");
    let graph: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("dist/docs.graph.json")).unwrap())
            .expect("graph json parses");
    let base_hash = graph["nodes"]
        .as_array()
        .expect("nodes")
        .iter()
        .find(|node| node["id"] == "billing.ready")
        .expect("target node")["content_hash"]
        .as_str()
        .expect("content hash")
        .to_string();
    (workspace, server, base_hash)
}

#[test]
fn validates_representative_serialized_agent_envelopes_against_contract_schemas() {
    let (_workspace, server, base_hash) = project_with_built_graph();

    let retrieval = server
        .run_search(SearchParams {
            project_root: None,
            query: "billing".to_string(),
            artifact: None,
            search_artifact: None,
            semantic: false,
            lexical: true,
            kind: None,
            status: None,
            owner: None,
            source_path: None,
            related_to: None,
            relation: None,
            direction: None,
            top: Some(5),
        })
        .expect("search succeeds");
    assert_valid("retrieval-envelope.json", &retrieval);

    let graph = server
        .run_graph(GraphParams {
            project_root: None,
            object_id: "billing.ready".to_string(),
            artifact: None,
            relation: None,
            direction: None,
        })
        .expect("graph succeeds");
    assert_valid("graph-traversal-envelope.json", &graph);

    for patch in [
        json!({
            "schema_version": "adoc.patch.v0",
            "op": "replace_body",
            "target": "billing.ready",
            "base_hash": base_hash.clone(),
            "changes": { "body": "Billing docs are ready after review." },
            "reason": "Update body."
        }),
        json!({
            "schema_version": "adoc.patch.v0",
            "op": "update_fields",
            "target": "billing.ready",
            "base_hash": base_hash.clone(),
            "changes": { "fields": { "owner": "team-billing" } },
            "reason": "Set owner."
        }),
        json!({
            "schema_version": "adoc.patch.v0",
            "op": "create_object",
            "target": "billing.created",
            "changes": {
                "kind": "claim",
                "status": "draft",
                "body": "Created claim.",
                "fields": {},
                "placement": { "page_id": "team.billing", "after": "billing.ready" }
            },
            "reason": "Create follow-up claim."
        }),
        json!({
            "schema_version": "adoc.patch.v0",
            "op": "supersede",
            "target": "billing.ready",
            "base_hash": base_hash.clone(),
            "changes": { "supersedes": ["billing.created"] },
            "reason": "Record supersession."
        }),
        json!({
            "schema_version": "adoc.patch.v0",
            "op": "revoke",
            "target": "billing.ready",
            "base_hash": base_hash.clone(),
            "changes": {},
            "reason": "Revoke stale claim."
        }),
    ] {
        assert_valid("patch-input.json", &patch);
    }

    let patch_check = server
        .run_patch_check(AdocPatchCheckParams {
            project_root: None,
            artifact: None,
            input: PatchInput::Inline {
                patch: json!({
                    "schema_version": "adoc.patch.v0",
                    "op": "replace_body",
                    "target": "billing.ready",
                    "base_hash": base_hash,
                    "changes": { "body": "Billing docs are ready after review." },
                    "reason": "Update body."
                }),
            },
        })
        .expect("patch check succeeds");
    assert_valid("patch-check.json", &patch_check);

    let project_status = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: Some("none".to_string()),
            no_embeddings: false,
        })
        .expect("project status succeeds");
    assert_valid("project-status.json", &project_status);

    // An empty-records stale envelope is also a contract case: the fixture
    // project has no expiry or review fields at all.
    let stale = server
        .run_stale(StaleParams {
            project_root: None,
            artifact: None,
            within_days: None,
        })
        .expect("stale succeeds");
    assert_valid("adoc.stale.v0.schema.json", &stale);
    assert_eq!(stale["records"], serde_json::json!([]));
}

/// V6.1: `adoc_stale` envelopes with all three record categories validate
/// against `adoc.stale.v0.schema.json`.
#[test]
fn validates_adoc_stale_v0_envelope_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(
        &root.join("docs/index.adoc"),
        concat!(
            "# Lifecycle @doc(team.lifecycle)\n",
            "\n",
            "::claim team.expired-verified\n",
            "status: verified\n",
            "owner: team-docs\n",
            "verified_at: 2020-01-01\n",
            "source: audit records 2020\n",
            "expires_at: 2024-01-01\n",
            "--\n",
            "Verified but expired claim.\n",
            "::\n",
            "\n",
            "::claim team.expired-draft\n",
            "status: draft\n",
            "owner: team-docs\n",
            "expires_at: 2026-01-15\n",
            "--\n",
            "Draft with a past expiry.\n",
            "::\n",
            "\n",
            "::policy team.review-policy\n",
            "status: active\n",
            "owner: team-docs\n",
            "approved_by: [team-docs]\n",
            "effective_at: 2020-01-01\n",
            "review_interval: 30d\n",
            "--\n",
            "Policy overdue for review.\n",
            "::\n",
            "\n",
            "::claim team.expiring\n",
            "status: verified\n",
            "owner: team-docs\n",
            "verified_at: 2026-01-01\n",
            "source: audit records 2026\n",
            "expires_at: 2120-01-01\n",
            "--\n",
            "Verified claim expiring far in the future.\n",
            "::\n",
        ),
    );
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());
    server
        .run_build(BuildParams {
            project_root: None,
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect("build succeeds");

    let stale = server
        .run_stale(StaleParams {
            project_root: None,
            artifact: None,
            within_days: None,
        })
        .expect("stale succeeds");
    assert_valid("adoc.stale.v0.schema.json", &stale);
    let records = stale["records"].as_array().expect("records array");
    assert_eq!(
        records.len(),
        3,
        "two stale + one review_overdue: {records:#?}"
    );

    let stale_within = server
        .run_stale(StaleParams {
            project_root: None,
            artifact: None,
            within_days: Some(36500),
        })
        .expect("stale with horizon succeeds");
    assert_valid("adoc.stale.v0.schema.json", &stale_within);
    let within_records = stale_within["records"].as_array().expect("records array");
    assert_eq!(
        within_records.len(),
        4,
        "plus one expiring_soon: {within_records:#?}"
    );
    let categories: Vec<&str> = within_records
        .iter()
        .filter_map(|record| record["category"].as_str())
        .collect();
    assert!(categories.contains(&"stale"));
    assert!(categories.contains(&"review_overdue"));
    assert!(categories.contains(&"expiring_soon"));
}

#[test]
fn validates_mcp_command_envelope_against_contract_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    let command = server
        .run_init(InitParams { project_root: None })
        .expect("init succeeds");

    assert_valid("mcp-command.json", &command);
}

#[test]
fn validates_adoc_diff_v0_envelope_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    build_two_commit_review_fixture(root);

    let envelope = run_review_diff(root);
    let value = serde_json::to_value(&envelope).expect("envelope serializes");

    assert_valid("adoc.diff.v0.schema.json", &value);
    assert_eq!(value["schema_version"], "adoc.diff.v0");
    assert!(
        value["created"]
            .as_array()
            .expect("created")
            .iter()
            .any(|node| node["id"] == "billing.holds")
    );
    assert!(
        value["deleted"]
            .as_array()
            .expect("deleted")
            .iter()
            .any(|node| node["id"] == "billing.legacy-credits")
    );
    assert!(
        value["changed"]
            .as_array()
            .expect("changed")
            .iter()
            .any(|entry| entry["id"] == "billing.credits")
    );
}

/// Build a 2-commit git fixture under `root` matching the V3.1 review
/// acceptance scenario. Mirrors the layout used by
/// `crates/adoc-cli/tests/diff_cli.rs::build_two_commit_fixture`.
fn build_two_commit_review_fixture(root: &Path) {
    let base = concat!(
        "# Billing @doc(team.billing)\n",
        "\n",
        "::claim billing.credits\n",
        "status: draft\n",
        "--\n",
        "Credits apply after payment.\n",
        "::\n",
        "\n",
        "::claim billing.legacy-credits\n",
        "status: draft\n",
        "--\n",
        "Legacy credits, slated for removal.\n",
        "::\n",
    );
    let head = concat!(
        "# Billing @doc(team.billing)\n",
        "\n",
        "::claim billing.credits\n",
        "status: draft\n",
        "--\n",
        "Credits apply after ledger commit.\n",
        "::\n",
        "\n",
        "::claim billing.holds\n",
        "status: draft\n",
        "--\n",
        "Holds delay disbursement.\n",
        "::\n",
    );

    write(&root.join("agentdoc.config.yaml"), config());
    run_git(root, &["init", "--initial-branch=main"]);
    run_git(root, &["config", "user.email", "test@adoc.dev"]);
    run_git(root, &["config", "user.name", "adoc tests"]);
    run_git(root, &["config", "commit.gpgsign", "false"]);

    write(&root.join("docs/billing.adoc"), base);
    run_git(root, &["add", "-A"]);
    run_git(root, &["commit", "-m", "base"]);

    run_git(root, &["checkout", "-b", "feature"]);
    write(&root.join("docs/billing.adoc"), head);
    run_git(root, &["add", "-A"]);
    run_git(root, &["commit", "-m", "head"]);
}

fn config() -> &'static str {
    "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n"
}

fn run_git(cwd: &Path, args: &[&str]) {
    let mut command = Command::new("git");
    command.arg("-C").arg(cwd).args(args);
    // Strip inherited GIT_* env vars so fixtures stay isolated from any
    // outer git repo whose context the test runner might have set (e.g.
    // pre-commit hooks via prek).
    for var in [
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        command.env_remove(var);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("spawn `git {args:?}`: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} failed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

fn run_review_diff(root: &Path) -> ObjectDiffEnvelope {
    let load = load_review_from_git(ReviewInput {
        project_root: root.to_path_buf(),
        base: SnapshotSelector::GitRef(GitRef::new("main")),
        head: SnapshotSelector::Workdir,
    })
    .expect("load review succeeds");
    let diff = diff_objects(&load.session);
    ObjectDiffEnvelope::from_diff(diff, load.diagnostics)
}

fn run_review(root: &Path) -> ReviewEnvelope {
    let load = load_review_with_changed_files_from_git(ReviewInput {
        project_root: root.to_path_buf(),
        base: SnapshotSelector::GitRef(GitRef::new("main")),
        head: SnapshotSelector::Workdir,
    })
    .expect("load review succeeds");
    ReviewEnvelope::from_session(&load.session, load.diagnostics)
}

#[test]
fn validates_adoc_review_v0_envelope_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    build_two_commit_review_fixture(root);

    let envelope = run_review(root);
    let value = serde_json::to_value(&envelope).expect("envelope serializes");

    assert_valid("adoc.review.v0.schema.json", &value);
    assert_eq!(value["schema_version"], "adoc.review.v0");
    assert_eq!(value["diff"]["schema_version"], "adoc.diff.v0");
    assert!(value["impact"].is_array());
    assert!(value["required_reviewers"].is_array());
    assert!(value["diagnostics"].is_array());

    // The embedded diff envelope must also stand on its own against its
    // schema — the two contracts are independently consumable.
    assert_valid("adoc.diff.v0.schema.json", &value["diff"]);

    // V3.7 — when no patch is supplied, patch_check is omitted from the
    // serialized envelope (not present as `null`).
    assert!(
        value.get("patch_check").is_none(),
        "patch_check must be omitted when no patch is supplied: {value:#}"
    );
}

#[test]
fn validates_adoc_review_v0_envelope_with_patch_check_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    build_two_commit_review_fixture(root);

    let load = load_review_with_changed_files_from_git(ReviewInput {
        project_root: root.to_path_buf(),
        base: SnapshotSelector::GitRef(GitRef::new("main")),
        head: SnapshotSelector::Workdir,
    })
    .expect("load review succeeds");

    // Pull the head content_hash for billing.credits so the patch validates
    // cleanly. Round-trip via the no-patch envelope so we don't reach into
    // adoc-core internals.
    let envelope_no_patch = ReviewEnvelope::from_session(&load.session, Vec::new());
    let value = serde_json::to_value(&envelope_no_patch).expect("envelope serializes");
    let base_hash = value["diff"]["changed"]
        .as_array()
        .expect("changed array")
        .iter()
        .find(|entry| entry["id"] == "billing.credits")
        .expect("billing.credits in changed")["head"]["content_hash"]
        .as_str()
        .expect("content_hash")
        .to_string();

    let patch = parse_patch_from_value(json!({
        "schema_version": "adoc.patch.v0",
        "op": "replace_body",
        "target": "billing.credits",
        "base_hash": base_hash,
        "changes": { "body": "Patched body." },
        "reason": "demo"
    }))
    .expect("patch parses");

    let envelope = review_with_patch(&load.session, load.diagnostics, Some(&patch));
    let value = serde_json::to_value(&envelope).expect("envelope serializes");

    assert_valid("adoc.review.v0.schema.json", &value);
    assert_eq!(value["patch_check"]["valid"], json!(true));
    assert_eq!(
        value["patch_check"]["schema_version"],
        "adoc.patch.check.v0"
    );
    assert_eq!(value["patch_check"]["target"], "billing.credits");
}

#[test]
fn adoc_review_mcp_tool_accepts_optional_patch_parameter() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    build_two_commit_review_fixture(root);

    let server = AgentDocMcpServer::new(root.to_path_buf());

    // Round-trip via the no-patch path first to learn the head content_hash.
    let base_envelope = server
        .run_review(AdocReviewParams {
            project_root: None,
            base_ref: "main".to_string(),
            head_ref: None,
            patch: None,
        })
        .expect("review without patch succeeds");
    let base_hash = base_envelope["diff"]["changed"]
        .as_array()
        .expect("changed array")
        .iter()
        .find(|entry| entry["id"] == "billing.credits")
        .expect("billing.credits in changed")["head"]["content_hash"]
        .as_str()
        .expect("content_hash")
        .to_string();

    let envelope = server
        .run_review(AdocReviewParams {
            project_root: None,
            base_ref: "main".to_string(),
            head_ref: None,
            patch: Some(PatchInput::Inline {
                patch: json!({
                    "schema_version": "adoc.patch.v0",
                    "op": "replace_body",
                    "target": "billing.credits",
                    "base_hash": base_hash,
                    "changes": { "body": "Patched body." },
                    "reason": "demo"
                }),
            }),
        })
        .expect("review with inline patch succeeds");

    assert_valid("adoc.review.v0.schema.json", &envelope);
    assert_eq!(envelope["patch_check"]["valid"], json!(true));
}

/// V3.6 contract: MCP must serve the V3 schema files verbatim (no transformation,
/// no drift between the bundled `include_str!` and the source-of-truth file).
#[test]
fn mcp_serves_v3_schema_resources_byte_equal_to_on_disk_files() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    for (uri, file) in [
        (
            "adoc://agent/v0/schema/adoc.diff.v0.schema.json",
            "adoc.diff.v0.schema.json",
        ),
        (
            "adoc://agent/v0/schema/adoc.review.v0.schema.json",
            "adoc.review.v0.schema.json",
        ),
    ] {
        let result = server
            .read_agent_resource(uri)
            .unwrap_or_else(|error| panic!("resource {uri} reads: {error}"));
        let served = match &result.contents[0] {
            rmcp::model::ResourceContents::TextResourceContents { text, .. } => text.clone(),
            other => panic!("expected text resource for {uri}, got {other:?}"),
        };
        let disk = fs::read_to_string(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../docs/agent/v0/schema")
                .join(file),
        )
        .unwrap_or_else(|error| panic!("disk schema {file} reads: {error}"));
        assert_eq!(
            served, disk,
            "MCP-served schema {uri} drifted from docs/agent/v0/schema/{file}"
        );
    }
}

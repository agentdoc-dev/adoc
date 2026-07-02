use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use adoc_core::{
    GitRef, ObjectDiffEnvelope, ReviewEnvelope, ReviewInput, SnapshotSelector, diff_objects,
    load_review_from_git, load_review_with_changed_files_from_git, parse_patch_from_value,
    review_with_patch,
};
use adoc_mcp::{
    AdocPatchCheckParams, AdocReviewParams, AgentDocMcpServer, BuildParams, ContradictionsParams,
    GraphParams, ImpactedByParams, InitParams, PatchInput, ProjectStatusParams, SearchParams,
    StaleParams,
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

    // Likewise the empty-lists contradictions envelope: the fixture project
    // has no contradiction objects at all.
    let contradictions = server
        .run_contradictions(ContradictionsParams {
            project_root: None,
            artifact: None,
            all: false,
        })
        .expect("contradictions succeeds");
    assert_valid("adoc.contradictions.v0.schema.json", &contradictions);
    assert_eq!(contradictions["contradictions"], serde_json::json!([]));
    assert_eq!(contradictions["contradicted_claims"], serde_json::json!([]));
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

/// V6.2: `adoc_contradictions` envelopes — populated default listing, the
/// `all: true` superset, and an orphaned authored-`contradicted` claim with an
/// empty `contradiction_ids` — validate against
/// `adoc.contradictions.v0.schema.json`.
#[test]
fn validates_adoc_contradictions_v0_envelope_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(
        &root.join("docs/index.adoc"),
        concat!(
            "# Conflicts @doc(team.conflicts)\n",
            "\n",
            "::claim team.storage-memory\n",
            "status: contradicted\n",
            "owner: team-docs\n",
            "--\n",
            "Tokens must be stored in memory only.\n",
            "::\n",
            "\n",
            "::claim team.storage-local\n",
            "status: accepted\n",
            "owner: team-docs\n",
            "--\n",
            "Tokens may be stored in localStorage.\n",
            "::\n",
            "\n",
            "::claim team.orphaned\n",
            "status: contradicted\n",
            "owner: team-docs\n",
            "--\n",
            "Authored contradicted with no unresolved contradiction left.\n",
            "::\n",
            "\n",
            "::claim team.settled-a\n",
            "status: accepted\n",
            "--\n",
            "First settled claim.\n",
            "::\n",
            "\n",
            "::claim team.settled-b\n",
            "status: accepted\n",
            "--\n",
            "Second settled claim.\n",
            "::\n",
            "\n",
            "::contradiction team.conflict-open\n",
            "severity: high\n",
            "status: unresolved\n",
            "claims: [team.storage-memory, team.storage-local]\n",
            "--\n",
            "Memory-only storage conflicts with the localStorage allowance.\n",
            "::\n",
            "\n",
            "::contradiction team.conflict-closed\n",
            "severity: critical\n",
            "status: resolved\n",
            "claims: [team.settled-a, team.settled-b]\n",
            "--\n",
            "Resolved conflict kept for history.\n",
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

    let envelope = server
        .run_contradictions(ContradictionsParams {
            project_root: None,
            artifact: None,
            all: false,
        })
        .expect("contradictions succeeds");
    assert_valid("adoc.contradictions.v0.schema.json", &envelope);
    assert!(
        envelope.get("evaluated_at").is_none(),
        "the contradictions envelope is clock-free"
    );
    let contradictions = envelope["contradictions"]
        .as_array()
        .expect("contradictions array");
    assert_eq!(
        contradictions.len(),
        1,
        "default listing is unresolved-only: {contradictions:#?}"
    );
    assert_eq!(contradictions[0]["id"], "team.conflict-open");
    let claims = envelope["contradicted_claims"]
        .as_array()
        .expect("contradicted_claims array");
    assert_eq!(
        claims.len(),
        3,
        "two implicated + one orphaned authored contradicted: {claims:#?}"
    );
    let orphan = claims
        .iter()
        .find(|claim| claim["id"] == "team.orphaned")
        .expect("orphaned claim listed");
    assert_eq!(
        orphan["contradiction_ids"],
        serde_json::json!([]),
        "orphaned authored status carries an empty contradiction_ids"
    );
    assert!(orphan.get("effective_reason").is_none());

    let all_envelope = server
        .run_contradictions(ContradictionsParams {
            project_root: None,
            artifact: None,
            all: true,
        })
        .expect("contradictions --all succeeds");
    assert_valid("adoc.contradictions.v0.schema.json", &all_envelope);
    let all_contradictions = all_envelope["contradictions"]
        .as_array()
        .expect("contradictions array");
    assert_eq!(
        all_contradictions.len(),
        2,
        "all: true adds the resolved record: {all_contradictions:#?}"
    );
    assert_eq!(
        all_contradictions[0]["id"], "team.conflict-closed",
        "critical sorts before high"
    );
    assert_eq!(all_contradictions[0]["status"], "resolved");
    assert_eq!(
        all_envelope["contradicted_claims"], envelope["contradicted_claims"],
        "--all never changes contradicted_claims"
    );
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
        (
            "adoc://agent/v0/schema/adoc.patch.apply.v0.schema.json",
            "adoc.patch.apply.v0.schema.json",
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

/// V6.3: `adoc_impacted_by` envelopes — a populated paths-shape query hitting
/// declared impacts, inline evidence, and evidence-ref resolution; the empty
/// no-match case; and the paths-XOR-ref argument rule — validate against
/// `adoc.impacted.v0.schema.json`.
#[test]
fn validates_adoc_impacted_v0_envelope_against_schema() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(
        &root.join("docs/index.adoc"),
        concat!(
            "# Impact @doc(team.impact)\n",
            "\n",
            "::claim team.refunds\n",
            "status: verified\n",
            "owner: team-billing\n",
            "verified_at: 2026-05-05\n",
            "source: crates/billing/src/refund.rs\n",
            "impacts: crates/billing/src/refund.rs\n",
            "--\n",
            "Refunds process within 24 hours.\n",
            "::\n",
            "\n",
            "::decision team.ledger-first\n",
            "status: accepted\n",
            "decided_by: architecture\n",
            "owner: team-billing\n",
            "evidence_ref: team.consume-source\n",
            "--\n",
            "Ledger-first credit consumption.\n",
            "::\n",
            "\n",
            "::source team.consume-source\n",
            "kind: source_code\n",
            "path: apps/backend/src/consume.ts\n",
            "owner: team-billing\n",
            "--\n",
            "Credit consumption implementation.\n",
            "::\n",
            "\n",
            "::claim team.draft-bystander\n",
            "status: draft\n",
            "impacts: crates/billing/src/refund.rs\n",
            "--\n",
            "Draft claim outside the verified-subject scope.\n",
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

    let impacted = server
        .run_impacted_by(ImpactedByParams {
            project_root: None,
            artifact: None,
            paths: Some(vec![
                "crates/billing/src/refund.rs".to_string(),
                "apps/backend/src/consume.ts".to_string(),
            ]),
            git_ref: None,
        })
        .expect("impacted-by succeeds");
    assert_valid("adoc.impacted.v0.schema.json", &impacted);
    assert_eq!(impacted["schema_version"], "adoc.impacted.v0");
    assert_eq!(
        impacted["changed_paths"],
        json!([
            "apps/backend/src/consume.ts",
            "crates/billing/src/refund.rs"
        ]),
        "changed_paths sorted ascending"
    );
    let records = impacted["impacted"].as_array().expect("impacted array");
    assert_eq!(
        records.len(),
        2,
        "verified claim + accepted decision, draft excluded: {records:#?}"
    );
    assert_eq!(records[0]["id"], "team.ledger-first");
    assert_eq!(records[0]["reasons"][0]["kind"], "evidence_path");
    assert_eq!(
        records[0]["reasons"][0]["via_source_object"],
        "team.consume-source"
    );
    assert_eq!(records[1]["id"], "team.refunds");
    let refund_reasons = records[1]["reasons"].as_array().expect("reasons");
    assert_eq!(
        refund_reasons.len(),
        2,
        "same path via impacts: and inline source evidence: {refund_reasons:#?}"
    );
    assert_eq!(refund_reasons[0]["kind"], "impacts_path");
    assert_eq!(refund_reasons[1]["kind"], "evidence_path");
    assert_eq!(
        impacted["proof_obligations"]
            .as_array()
            .expect("obligations")
            .len(),
        2
    );

    let empty = server
        .run_impacted_by(ImpactedByParams {
            project_root: None,
            artifact: None,
            paths: Some(vec!["unrelated/path.rs".to_string()]),
            git_ref: None,
        })
        .expect("impacted-by succeeds with no matches");
    assert_valid("adoc.impacted.v0.schema.json", &empty);
    assert_eq!(empty["impacted"], json!([]));
    assert_eq!(empty["proof_obligations"], json!([]));

    // Exactly one of `paths` / `ref` — both, neither, and empty `paths` are
    // argument errors. Empty `paths` mirrors the CLI, where clap treats an
    // empty Vec as "not present": an agent forwarding an empty diff must get
    // an argument error, not a silent empty envelope.
    for params in [
        ImpactedByParams {
            project_root: None,
            artifact: None,
            paths: Some(vec!["a.rs".to_string()]),
            git_ref: Some("main".to_string()),
        },
        ImpactedByParams {
            project_root: None,
            artifact: None,
            paths: None,
            git_ref: None,
        },
        ImpactedByParams {
            project_root: None,
            artifact: None,
            paths: Some(Vec::new()),
            git_ref: None,
        },
    ] {
        let error = server
            .run_impacted_by(params)
            .expect_err("paths XOR ref must be enforced");
        assert!(
            error.to_string().contains("paths"),
            "error must name the argument rule: {error}"
        );
    }
}

/// V6.4 TB4: `adoc_patch_apply` envelopes — an applied success, the
/// disabled-gate refusal, and a stale-base-hash refusal — validate against
/// `adoc.patch.apply.v0.schema.json`.
#[test]
fn validates_patch_apply_envelopes_against_contract_schema() {
    use adoc_mcp::AdocPatchApplyParams;

    fn inline_patch(base_hash: &str) -> PatchInput {
        PatchInput::Inline {
            patch: json!({
                "schema_version": "adoc.patch.v0",
                "op": "replace_body",
                "target": "billing.ready",
                "base_hash": base_hash,
                "changes": { "body": "Billing docs are ready and applied." },
                "reason": "Contract-test the apply envelope.",
                "proposer": { "type": "agent", "id": "contract-test" }
            }),
        }
    }

    // Disabled gate (the default project has no `mcp:` block).
    let (_workspace, server, base_hash) = project_with_built_graph();
    let refusal = server
        .run_patch_apply(AdocPatchApplyParams {
            project_root: None,
            artifact: None,
            input: inline_patch(&base_hash),
        })
        .expect("gate refusal is a normal envelope");
    assert_valid("adoc.patch.apply.v0.schema.json", &refusal);
    assert_eq!(refusal["applied"], false);
    assert_eq!(
        refusal["diagnostics"][0]["code"],
        "mcp.patch_apply_disabled"
    );

    // Enabled project: applied success, then a stale-base-hash refusal.
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\nmcp:\n  patch_apply: enabled\n",
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());
    server
        .run_build(BuildParams {
            project_root: None,
            path: None,
            out: None,
            no_embeddings: true,
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

    let applied = server
        .run_patch_apply(AdocPatchApplyParams {
            project_root: None,
            artifact: None,
            input: inline_patch(&base_hash),
        })
        .expect("apply runs");
    assert_valid("adoc.patch.apply.v0.schema.json", &applied);
    assert_eq!(applied["applied"], true);
    assert_eq!(applied["trace"]["interface"], "mcp");
    assert_eq!(applied["trace"]["proposer"]["kind"], "agent");

    let stale = server
        .run_patch_apply(AdocPatchApplyParams {
            project_root: None,
            artifact: None,
            input: inline_patch("sha256:stale"),
        })
        .expect("refusal is a normal envelope");
    assert_valid("adoc.patch.apply.v0.schema.json", &stale);
    assert_eq!(stale["applied"], false);
}

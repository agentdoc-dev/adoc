use std::fs;
use std::path::{Path, PathBuf};

use adoc_mcp::{
    AdocPatchCheckParams, AgentDocMcpServer, BuildParams, GraphParams, InitParams, PatchInput,
    ProjectStatusParams, SearchParams,
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

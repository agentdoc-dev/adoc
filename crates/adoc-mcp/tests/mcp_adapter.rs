use std::fs;
use std::path::Path;

use adoc_local::{PathPolicy, ProjectRootPathPolicy};
use adoc_mcp::{AdocPatchCheckParams, AgentDocMcpServer, BuildParams, InitParams, PatchInput};
use rmcp::ServerHandler;

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

fn source() -> &'static str {
    "# Billing @doc(team.billing)\n\n::claim billing.credits\nstatus: draft\n--\nCredits apply after payment.\n::\n"
}

#[test]
fn path_policy_rejects_parent_escape() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    let policy = ProjectRootPathPolicy::new(root).expect("policy");

    let error = policy
        .resolve_read_path(Path::new("../outside.adoc"))
        .expect_err("escape rejected");

    assert!(error.to_string().contains("path_outside_project"));
}

#[test]
fn init_tool_writes_inside_project_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    let value = server
        .run_init(InitParams { project_root: None })
        .expect("init succeeds");

    assert_eq!(value["schema_version"], "adoc.mcp.command.v0");
    assert_eq!(value["ok"], true);
    assert!(workspace.path().join("agentdoc.config.yaml").exists());
    assert!(workspace.path().join("docs/index.adoc").exists());
}

#[test]
fn build_tool_rejects_configured_outputs_outside_project_root() {
    let workspace = tempfile::tempdir().expect("workspace");
    let outside = tempfile::tempdir().expect("outside");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    write(
        &root.join("agentdoc.config.yaml"),
        &format!(
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  html: {}\n  graph: dist/docs.graph.json\nembeddings:\n  provider: none\n",
            outside.path().join("docs.html").display()
        ),
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());

    let error = server
        .run_build(BuildParams {
            project_root: None,
            path: None,
            out: None,
            no_embeddings: false,
        })
        .expect_err("outside configured output is rejected");

    assert!(error.to_string().contains("path_outside_project"));
    assert!(!outside.path().join("docs.html").exists());
}

#[test]
fn patch_check_accepts_inline_patch_json() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    let server = AgentDocMcpServer::new(root.to_path_buf());
    server
        .run_build(BuildParams {
            project_root: None,
            path: Some("docs".into()),
            out: Some("dist".into()),
            no_embeddings: true,
        })
        .expect("build succeeds");
    let graph: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(root.join("dist/docs.graph.json")).unwrap())
            .unwrap();
    let hash = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "billing.credits")
        .unwrap()["content_hash"]
        .as_str()
        .unwrap();

    let value = server
        .run_patch_check(AdocPatchCheckParams {
            project_root: None,
            artifact: Some("dist/docs.graph.json".into()),
            input: PatchInput::Inline {
                patch: serde_json::json!({
                    "schema_version": "adoc.patch.v0",
                    "op": "replace_body",
                    "target": "billing.credits",
                    "base_hash": hash,
                    "changes": { "body": "Credits apply after ledger commit." },
                    "reason": "Update billing behavior."
                }),
            },
        })
        .expect("inline patch check succeeds");

    assert_eq!(value["schema_version"], "adoc.patch.check.v0");
    assert_eq!(value["valid"], true);
}

#[test]
fn server_implements_rmcp_server_handler_with_tools_capability() {
    fn assert_handler<T: ServerHandler>(_server: &T) {}

    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    assert_handler(&server);
    assert!(server.get_info().capabilities.tools.is_some());
}

use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

mod support;

use adoc_local::{PathPolicy, ProjectRootPathPolicy};
use adoc_mcp::{
    AdocDiffParams, AdocPatchCheckParams, AdocReviewParams, AgentDocMcpServer, BuildParams,
    InitParams, PatchInput, ProjectStatusParams, SearchParams, WhyParams,
};
use rmcp::ServerHandler;
use rmcp::model::ResourceContents;

use support::build_v3_review_fixture;

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

fn source() -> &'static str {
    "# Billing @doc(team.billing)\n\n::claim billing.credits\nstatus: draft\n--\nCredits apply after payment.\n::\n"
}

fn copy_billing_pilot_fixture(root: &Path) {
    let fixture_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../examples/billing-pilot")
        .canonicalize()
        .expect("billing pilot fixture path");
    for file in [
        "agentdoc.config.yaml",
        "01-glossary.adoc",
        "02-claims.adoc",
        "03-decisions.adoc",
        "04-warnings.adoc",
    ] {
        fs::copy(fixture_root.join(file), root.join(file)).expect("fixture file copies");
    }
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
    assert!(server.get_info().capabilities.resources.is_some());
    assert!(server.get_info().capabilities.prompts.is_some());
}

#[test]
fn project_status_tool_is_read_only_by_default() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());

    let value = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: None,
            no_embeddings: false,
        })
        .expect("status succeeds");

    assert_eq!(value["schema_version"], "adoc.project.status.v0");
    assert_eq!(value["refresh"]["requested"], "none");
    assert_eq!(value["artifacts"]["graph"]["exists"], false);
    assert_eq!(value["readiness"]["retrieval"], false);
    assert!(
        !root.join("dist/docs.graph.json").exists(),
        "default status must not build artifacts"
    );
}

#[test]
fn project_status_tool_can_refresh_with_check_or_build() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    write(&root.join("docs/index.adoc"), source());
    write(
        &root.join("agentdoc.config.yaml"),
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
    );
    let server = AgentDocMcpServer::new(root.to_path_buf());

    let check = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: Some("check".to_string()),
            no_embeddings: true,
        })
        .expect("status check succeeds");
    assert_eq!(check["refresh"]["requested"], "check");
    assert_eq!(check["refresh"]["exit_code"], 0);
    assert_eq!(check["artifacts"]["graph"]["exists"], false);
    assert!(!root.join("dist/docs.graph.json").exists());

    let build = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: Some("build".to_string()),
            no_embeddings: false,
        })
        .expect("status build succeeds");
    assert_eq!(build["refresh"]["requested"], "build");
    assert_eq!(build["refresh"]["exit_code"], 0);
    assert_eq!(
        build["artifacts"]["graph"]["schema_version"],
        "adoc.graph.v3"
    );
    assert_eq!(build["artifacts"]["graph"]["object_count"], 1);
    assert_eq!(build["readiness"]["patch_validation"], true);
}

#[test]
fn project_status_rejects_configured_outputs_outside_project_root() {
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
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: Some("build".to_string()),
            no_embeddings: false,
        })
        .expect_err("outside configured output is rejected");

    assert!(error.to_string().contains("path_outside_project"));
    assert!(!outside.path().join("docs.html").exists());
}

#[test]
fn lists_and_reads_all_stable_agent_resources() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());
    let expected = [
        "adoc://agent/v0/usage-contract",
        "adoc://agent/v0/tool-guide",
        "adoc://agent/v0/answer-contract",
        "adoc://agent/v0/agent-instruction-guide",
        "adoc://agent/v0/patch-contract",
        "adoc://agent/v0/project-status-guide",
        "adoc://agent/v0/dogfood-billing-pilot",
        "adoc://agent/v0/review-workflow",
        "adoc://agent/v0/compat-guide",
        "adoc://agent/v0/schema/retrieval",
        "adoc://agent/v0/schema/graph-traversal",
        "adoc://agent/v0/schema/patch",
        "adoc://agent/v0/schema/project-status",
        "adoc://agent/v0/schema/mcp-command",
        "adoc://agent/v0/schema/diff",
        "adoc://agent/v0/schema/review",
        "adoc://agent/v0/schema/retrieval-envelope.json",
        "adoc://agent/v0/schema/graph-traversal-envelope.json",
        "adoc://agent/v0/schema/patch-input.json",
        "adoc://agent/v0/schema/patch-check.json",
        "adoc://agent/v0/schema/project-status.json",
        "adoc://agent/v0/schema/mcp-command.json",
        "adoc://agent/v0/schema/adoc.diff.v0.schema.json",
        "adoc://agent/v0/schema/adoc.review.v0.schema.json",
    ];

    let resources = server.list_agent_resources();
    let listed = resources
        .iter()
        .map(|resource| resource.raw.uri.as_str())
        .collect::<Vec<_>>();
    assert_eq!(listed, expected);

    for uri in expected {
        let resource = resources
            .iter()
            .find(|resource| resource.raw.uri == uri)
            .expect("resource listed");
        let result = server.read_agent_resource(uri).expect("resource reads");
        assert_eq!(result.contents.len(), 1);
        let ResourceContents::TextResourceContents {
            mime_type, text, ..
        } = &result.contents[0]
        else {
            panic!("agent resources should be text");
        };
        if uri.ends_with(".json") {
            assert_eq!(
                resource.raw.mime_type.as_deref(),
                Some("application/schema+json")
            );
            assert_eq!(mime_type.as_deref(), Some("application/schema+json"));
            let schema: serde_json::Value =
                serde_json::from_str(text).expect("json schema resource is valid JSON");
            assert_eq!(
                schema["$schema"],
                "https://json-schema.org/draft/2020-12/schema"
            );
        } else {
            assert_eq!(resource.raw.mime_type.as_deref(), Some("text/markdown"));
            assert_eq!(mime_type.as_deref(), Some("text/markdown"));
            assert!(text.starts_with("# "));
            assert!(text.contains("V2.2") || text.contains("adoc."));
        }
    }
}

#[test]
fn lists_versioned_prompts_and_pinned_aliases() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    let prompts = server.list_agent_prompts();
    let names = prompts
        .iter()
        .map(|prompt| prompt.name.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        names,
        [
            "adoc_answer_with_citations_v0",
            "adoc_answer_with_citations",
            "adoc_propose_patch_v0",
            "adoc_propose_patch",
            "adoc_inspect_project_status_v0",
            "adoc_inspect_project_status",
            "adoc_dogfood_billing_pilot_v0",
            "adoc_dogfood_billing_pilot",
            "adoc_review_pull_request_v0",
            "adoc_review_pull_request",
            "adoc_explain_what_changed_v0",
            "adoc_explain_what_changed",
        ]
    );

    let answer = prompts
        .iter()
        .find(|prompt| prompt.name == "adoc_answer_with_citations_v0")
        .expect("answer prompt listed");
    let args = answer.arguments.as_ref().expect("rich arguments");
    assert!(args.iter().any(|arg| {
        arg.name == "query"
            && arg.required == Some(true)
            && arg
                .description
                .as_deref()
                .is_some_and(|desc| desc.contains("question"))
    }));
    assert!(args.iter().any(|arg| {
        arg.name == "retrieval_mode"
            && arg.description.as_deref().is_some_and(|desc| {
                desc.contains("hybrid") && desc.contains("semantic") && desc.contains("lexical")
            })
    }));

    let versioned = server
        .get_agent_prompt("adoc_answer_with_citations_v0", None)
        .expect("versioned prompt");
    let alias = server
        .get_agent_prompt("adoc_answer_with_citations", None)
        .expect("alias prompt");
    assert_eq!(alias.messages, versioned.messages);

    let text = serde_json::to_string(&versioned).expect("prompt serializes");
    assert!(text.contains("adoc_search"));
    assert!(text.contains("Object ID"));
    assert!(text.contains("status"));
    assert!(text.contains("caveats"));
}

#[test]
fn dogfood_billing_pilot_flow_uses_status_search_why_and_patch_check() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    copy_billing_pilot_fixture(root);
    let server = AgentDocMcpServer::new(root.to_path_buf());

    let initial_status = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: None,
            no_embeddings: false,
        })
        .expect("initial status succeeds");
    assert_eq!(initial_status["schema_version"], "adoc.project.status.v0");

    let build_status = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: Some("build".to_string()),
            no_embeddings: true,
        })
        .expect("status build succeeds");
    assert_eq!(build_status["readiness"]["retrieval"], true);
    assert_eq!(build_status["readiness"]["patch_validation"], true);

    let search = server
        .run_search(SearchParams {
            project_root: None,
            query: "billing.credits".to_string(),
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
        .expect("lexical search succeeds");
    assert_eq!(search["schema_version"], "adoc.retrieval.v0");
    assert!(
        search["records"]
            .as_array()
            .expect("records array")
            .iter()
            .any(|record| record["id"] == "billing.credits")
    );

    let why = server
        .run_why(WhyParams {
            project_root: None,
            object_id: "billing.credits".to_string(),
            artifact: None,
        })
        .expect("why succeeds");
    let record = &why["records"][0];
    assert_eq!(record["id"], "billing.credits");
    assert_eq!(record["kind"], "glossary");
    assert_eq!(record["owner"], "team-billing");
    let base_hash = record["content_hash"]
        .as_str()
        .expect("record has content hash");

    let patch = server
        .run_patch_check(AdocPatchCheckParams {
            project_root: None,
            artifact: None,
            input: PatchInput::Inline {
                patch: serde_json::json!({
                    "schema_version": "adoc.patch.v0",
                    "op": "replace_body",
                    "target": "billing.credits",
                    "base_hash": base_hash,
                    "changes": {
                        "body": "Credits are account balance adjustments that reduce future invoices after reviewed ledger, refund, or support correction events."
                    },
                    "reason": "Dogfood a validated billing glossary patch."
                }),
            },
        })
        .expect("inline patch validates");

    assert_eq!(patch["schema_version"], "adoc.patch.check.v0");
    assert_eq!(patch["valid"], true);
}

#[test]
fn stdio_server_smoke_lists_agent_resources() {
    let workspace = tempfile::tempdir().expect("workspace");
    let mut child = Command::new(env!("CARGO_BIN_EXE_adoc-mcp"))
        .current_dir(workspace.path())
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("adoc-mcp binary spawns");
    let mut stdin = child.stdin.take().expect("stdin is piped");
    let stdout = child.stdout.take().expect("stdout is piped");
    let mut stdout = BufReader::new(stdout);

    write_json_line(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "initialize",
            "params": {
                "protocolVersion": "2025-06-18",
                "capabilities": {},
                "clientInfo": { "name": "adoc-mcp-test", "version": "0" }
            }
        }),
    );
    let init = read_json_line(&mut stdout);
    assert_eq!(init["id"], 1);
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["capabilities"]["resources"].is_object());
    assert!(init["result"]["capabilities"]["prompts"].is_object());

    write_json_line(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "method": "notifications/initialized"
        }),
    );
    write_json_line(
        &mut stdin,
        serde_json::json!({
            "jsonrpc": "2.0",
            "id": 2,
            "method": "resources/list"
        }),
    );
    let resources = read_json_line(&mut stdout);
    assert_eq!(resources["id"], 2);
    assert!(
        resources["result"]["resources"]
            .as_array()
            .expect("resources array")
            .iter()
            .any(|resource| resource["uri"] == "adoc://agent/v0/usage-contract")
    );

    drop(stdin);
    let _ = child.kill();
    let _ = child.wait();
}

#[test]
fn adoc_diff_returns_diff_envelope_for_two_commit_fixture() {
    let fixture = build_v3_review_fixture("diff-tool");
    let server = AgentDocMcpServer::new(fixture.root.clone());

    let value = server
        .run_diff(AdocDiffParams {
            project_root: None,
            base_ref: "main".to_string(),
            head_ref: None,
        })
        .expect("adoc_diff runs");

    assert_eq!(value["schema_version"], "adoc.diff.v0");
    let changed = value["changed"].as_array().expect("changed array");
    assert!(
        changed.iter().any(|entry| entry["id"] == "billing.refunds"),
        "expected billing.refunds in changed[]; got {value}"
    );
    let refunds = changed
        .iter()
        .find(|entry| entry["id"] == "billing.refunds")
        .expect("billing.refunds entry");
    assert!(refunds["base"]["content_hash"].is_string());
    assert!(refunds["head"]["content_hash"].is_string());
    assert_ne!(
        refunds["base"]["content_hash"],
        refunds["head"]["content_hash"]
    );
}

#[test]
fn adoc_diff_rejects_unresolvable_base_ref() {
    let fixture = build_v3_review_fixture("diff-bad-ref");
    let server = AgentDocMcpServer::new(fixture.root.clone());

    let error = server
        .run_diff(AdocDiffParams {
            project_root: None,
            base_ref: "definitely-not-a-real-ref".to_string(),
            head_ref: None,
        })
        .expect_err("unresolvable ref must error");

    assert!(
        error.to_string().contains("definitely-not-a-real-ref"),
        "error must surface the bad ref: {error}"
    );
}

#[test]
fn adoc_review_returns_review_envelope_with_obligations_and_impact() {
    let fixture = build_v3_review_fixture("review-tool");
    let server = AgentDocMcpServer::new(fixture.root.clone());

    let value = server
        .run_review(AdocReviewParams {
            project_root: None,
            base_ref: "main".to_string(),
            head_ref: None,
            patch: None,
        })
        .expect("adoc_review runs");

    assert_eq!(value["schema_version"], "adoc.review.v0");
    assert_eq!(value["diff"]["schema_version"], "adoc.diff.v0");

    let impact = value["impact"].as_array().expect("impact array");
    assert!(
        impact.iter().any(|entry| entry["id"] == "billing.refunds"),
        "billing.refunds should be impacted because crates/billing/src/refund.rs changed; got {value}"
    );

    let reviewers = value["required_reviewers"]
        .as_array()
        .expect("required_reviewers array");
    assert!(
        reviewers
            .iter()
            .any(|entry| entry["owner"] == "team-billing"),
        "team-billing must be a required reviewer: {value}"
    );

    let obligations = value["proof_obligations"]
        .as_array()
        .expect("proof_obligations array");
    assert!(
        !obligations.is_empty(),
        "verified-claim body change + impacted source path should produce proof obligations: {value}"
    );
}

#[test]
fn adoc_review_accepts_explicit_head_ref() {
    let fixture = build_v3_review_fixture("review-head-ref");
    let server = AgentDocMcpServer::new(fixture.root.clone());

    let value = server
        .run_review(AdocReviewParams {
            project_root: None,
            base_ref: "main".to_string(),
            head_ref: Some("feature".to_string()),
            patch: None,
        })
        .expect("adoc_review with explicit head_ref runs");

    assert_eq!(value["schema_version"], "adoc.review.v0");
    assert!(
        value["diff"]["changed"]
            .as_array()
            .expect("changed array")
            .iter()
            .any(|entry| entry["id"] == "billing.refunds")
    );
}

#[test]
fn project_status_readiness_review_is_true_in_git_repo_with_head() {
    let fixture = build_v3_review_fixture("readiness-review-ok");
    let server = AgentDocMcpServer::new(fixture.root.clone());

    let value = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: None,
            no_embeddings: false,
        })
        .expect("status succeeds");

    assert_eq!(value["readiness"]["review"], true);
}

#[test]
fn project_status_readiness_review_is_false_in_non_git_dir() {
    let workspace = tempfile::tempdir().expect("workspace");
    let server = AgentDocMcpServer::new(workspace.path().to_path_buf());

    let value = server
        .run_project_status(ProjectStatusParams {
            project_root: None,
            refresh: None,
            no_embeddings: false,
        })
        .expect("status succeeds");

    assert_eq!(value["readiness"]["review"], false);
}

fn write_json_line(stdin: &mut impl Write, value: serde_json::Value) {
    writeln!(stdin, "{value}").expect("json request can be written");
    stdin.flush().expect("json request can be flushed");
}

fn read_json_line(stdout: &mut impl BufRead) -> serde_json::Value {
    let mut line = String::new();
    stdout
        .read_line(&mut line)
        .expect("json response can be read");
    assert!(!line.is_empty(), "expected json response line");
    serde_json::from_str(&line).expect("response is valid JSON")
}

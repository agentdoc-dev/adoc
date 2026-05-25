mod support;

use std::collections::BTreeSet;
use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

use support::build_v3_review_fixture;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("mcp crate has workspace parent")
        .parent()
        .expect("workspace has repo root")
        .to_path_buf()
}

fn copy_billing_pilot_fixture(root: &Path) {
    let fixture_root = repo_root().join("examples/billing-pilot");
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

struct StdioServer {
    child: Child,
    stdin: ChildStdin,
    stdout: BufReader<ChildStdout>,
}

impl StdioServer {
    fn spawn(project_root: &Path) -> Self {
        let mut child = Command::new(env!("CARGO_BIN_EXE_adoc-mcp"))
            .current_dir(project_root)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("adoc-mcp binary spawns");
        let stdin = child.stdin.take().expect("stdin is piped");
        let stdout = child.stdout.take().expect("stdout is piped");
        Self {
            child,
            stdin,
            stdout: BufReader::new(stdout),
        }
    }

    fn send(&mut self, value: serde_json::Value) {
        writeln!(self.stdin, "{value}").expect("json request can be written");
        self.stdin.flush().expect("json request can be flushed");
    }

    fn receive(&mut self) -> serde_json::Value {
        let mut line = String::new();
        self.stdout
            .read_line(&mut line)
            .expect("json response can be read");
        assert!(!line.is_empty(), "expected json response line");
        serde_json::from_str(&line).expect("response is valid JSON")
    }
}

impl Drop for StdioServer {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn structured_content(response: &serde_json::Value) -> &serde_json::Value {
    &response["result"]["structuredContent"]
}

#[test]
fn stdio_server_runs_documented_mcp_agent_gateway_quickstart() {
    let docs = fs::read_to_string(repo_root().join("docs/mcp-agent-gateway.md"))
        .expect("MCP Agent Gateway docs should exist");
    assert!(docs.contains("adoc://agent/v0/usage-contract"));
    assert!(docs.contains("tools/call"));
    assert!(docs.contains("no_embeddings"));

    let workspace = tempfile::tempdir().expect("workspace");
    copy_billing_pilot_fixture(workspace.path());
    let mut server = StdioServer::spawn(workspace.path());

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "adoc-mcp-stdio-dogfood", "version": "0" }
        }
    }));
    let init = server.receive();
    assert_eq!(init["id"], 1);
    assert!(init["result"]["capabilities"]["tools"].is_object());
    assert!(init["result"]["capabilities"]["resources"].is_object());
    assert!(init["result"]["capabilities"]["prompts"].is_object());

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "resources/read",
        "params": { "uri": "adoc://agent/v0/usage-contract" }
    }));
    let usage_contract = server.receive();
    assert_eq!(usage_contract["id"], 2);
    assert!(
        usage_contract["result"]["contents"][0]["text"]
            .as_str()
            .expect("resource text")
            .contains("adoc_project_status")
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "prompts/get",
        "params": {
            "name": "adoc_answer_with_citations",
            "arguments": { "query": "How do billing credits work?" }
        }
    }));
    let prompt = server.receive();
    assert_eq!(prompt["id"], 3);
    let prompt_text = prompt["result"]["messages"][0]["content"]["text"]
        .as_str()
        .expect("prompt text");
    assert!(prompt_text.contains("adoc_search"));
    assert!(prompt_text.contains("Object ID"));

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "adoc_project_status",
            "arguments": {}
        }
    }));
    let initial_status = server.receive();
    assert_eq!(initial_status["id"], 4);
    assert_eq!(
        structured_content(&initial_status)["schema_version"],
        "adoc.project.status.v0"
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 5,
        "method": "tools/call",
        "params": {
            "name": "adoc_project_status",
            "arguments": { "refresh": "build", "no_embeddings": true }
        }
    }));
    let build_status = server.receive();
    assert_eq!(build_status["id"], 5);
    assert_eq!(
        structured_content(&build_status)["readiness"]["retrieval"],
        true
    );
    assert_eq!(
        structured_content(&build_status)["readiness"]["patch_validation"],
        true
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 6,
        "method": "tools/call",
        "params": {
            "name": "adoc_search",
            "arguments": {
                "query": "billing.credits",
                "lexical": true,
                "semantic": false,
                "top": 5
            }
        }
    }));
    let search = server.receive();
    assert_eq!(search["id"], 6);
    let search_content = structured_content(&search);
    assert_eq!(search_content["schema_version"], "adoc.retrieval.v0");
    assert!(
        search_content["records"]
            .as_array()
            .expect("records array")
            .iter()
            .any(|record| record["id"] == "billing.credits")
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 7,
        "method": "tools/call",
        "params": {
            "name": "adoc_why",
            "arguments": { "object_id": "billing.credits" }
        }
    }));
    let why = server.receive();
    assert_eq!(why["id"], 7);
    let why_content = structured_content(&why);
    let record = &why_content["records"][0];
    assert_eq!(record["id"], "billing.credits");
    assert_eq!(record["kind"], "glossary");
    assert_eq!(record["owner"], "team-billing");
    let base_hash = record["content_hash"].as_str().expect("content hash");

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 8,
        "method": "tools/call",
        "params": {
            "name": "adoc_patch_check",
            "arguments": {
                "source": "inline",
                "patch": {
                    "schema_version": "adoc.patch.v0",
                    "op": "replace_body",
                    "target": "billing.credits",
                    "base_hash": base_hash,
                    "changes": {
                        "body": "Credits are account balance adjustments that reduce future invoices after reviewed ledger, refund, or support correction events."
                    },
                    "reason": "Dogfood a validated billing glossary patch."
                }
            }
        }
    }));
    let patch = server.receive();
    assert_eq!(patch["id"], 8);
    assert!(patch.get("error").is_none(), "patch response: {patch:#?}");
    assert_eq!(
        structured_content(&patch)["schema_version"],
        "adoc.patch.check.v0"
    );
    assert_eq!(structured_content(&patch)["valid"], true);
}

/// V3.6 acceptance: the stdio server, given a 2-commit git fixture project,
/// returns valid `adoc.diff.v0` and `adoc.review.v0` envelopes via the
/// `adoc_diff` and `adoc_review` MCP tool calls. No file writes occur outside
/// the system tmp directory used by the worktree adapter.
#[test]
fn stdio_server_emits_diff_and_review_envelopes_for_v3_6_acceptance() {
    let fixture = build_v3_review_fixture("stdio-v3-6");
    let project_root = fixture.root.clone();
    let before = list_files(&project_root);

    let mut server = StdioServer::spawn(&project_root);

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "adoc-mcp-stdio-v3-6", "version": "0" }
        }
    }));
    let init = server.receive();
    assert_eq!(init["id"], 1);

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "adoc_project_status",
            "arguments": {}
        }
    }));
    let status = server.receive();
    assert_eq!(status["id"], 2);
    assert_eq!(
        structured_content(&status)["readiness"]["review"],
        true,
        "readiness.review must be true in a 2-commit git fixture"
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "adoc_diff",
            "arguments": { "base_ref": "main" }
        }
    }));
    let diff = server.receive();
    assert_eq!(diff["id"], 3);
    assert!(diff.get("error").is_none(), "adoc_diff error: {diff:#?}");
    let diff_content = structured_content(&diff);
    assert_eq!(diff_content["schema_version"], "adoc.diff.v0");
    assert!(
        diff_content["changed"]
            .as_array()
            .expect("changed array")
            .iter()
            .any(|entry| entry["id"] == "billing.refunds"),
        "adoc_diff should report billing.refunds as changed: {diff_content}"
    );

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "adoc_review",
            "arguments": { "base_ref": "main" }
        }
    }));
    let review = server.receive();
    assert_eq!(review["id"], 4);
    assert!(
        review.get("error").is_none(),
        "adoc_review error: {review:#?}"
    );
    let review_content = structured_content(&review);
    assert_eq!(review_content["schema_version"], "adoc.review.v0");
    assert_eq!(review_content["diff"]["schema_version"], "adoc.diff.v0");
    assert!(
        review_content["impact"]
            .as_array()
            .expect("impact array")
            .iter()
            .any(|entry| entry["id"] == "billing.refunds"),
        "billing.refunds should be impacted because refund.rs changed: {review_content}"
    );
    assert!(
        !review_content["proof_obligations"]
            .as_array()
            .expect("proof_obligations array")
            .is_empty(),
        "verified-claim body change must produce at least one proof obligation: {review_content}"
    );

    // V3.6 boundary acceptance: nothing under the project root may have been
    // written by adoc_diff / adoc_review. The git worktree adapter writes only
    // under `std::env::temp_dir()`, not the project root.
    let after = list_files(&project_root);
    let new_files: Vec<&PathBuf> = after.difference(&before).collect();
    assert!(
        new_files.is_empty(),
        "adoc_diff / adoc_review must not write under the project root; new files: {new_files:?}"
    );
}

/// V3.7 acceptance: the dogfood stdio server, given a 2-commit fixture
/// project, returns valid `adoc.review.v0` envelopes for `adoc_review` calls
/// with both the inline-patch and path-patch shapes of the optional `patch`
/// parameter, embedding `adoc.patch.check.v0` and unioning patch-driven
/// obligations into the top-level list. The patch is never applied.
#[test]
fn stdio_server_adoc_review_accepts_optional_patch_parameter_v3_7() {
    let fixture = build_v3_review_fixture("stdio-v3-7-patch");
    let project_root = fixture.root.clone();

    let mut server = StdioServer::spawn(&project_root);

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 1,
        "method": "initialize",
        "params": {
            "protocolVersion": "2025-06-18",
            "capabilities": {},
            "clientInfo": { "name": "adoc-mcp-stdio-v3-7", "version": "0" }
        }
    }));
    let init = server.receive();
    assert_eq!(init["id"], 1);

    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "method": "notifications/initialized"
    }));

    // Round-trip via the no-patch path first to learn the head content_hash
    // of billing.refunds so the test patch validates cleanly.
    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 2,
        "method": "tools/call",
        "params": {
            "name": "adoc_review",
            "arguments": { "base_ref": "main" }
        }
    }));
    let baseline = server.receive();
    assert_eq!(baseline["id"], 2);
    assert!(
        baseline.get("error").is_none(),
        "baseline adoc_review error: {baseline:#?}"
    );
    let baseline_content = structured_content(&baseline);
    assert!(
        baseline_content.get("patch_check").is_none(),
        "patch_check must be omitted when no patch parameter is supplied: {baseline_content}"
    );
    let head_hash = baseline_content["diff"]["changed"]
        .as_array()
        .expect("changed array")
        .iter()
        .find(|entry| entry["id"] == "billing.refunds")
        .expect("billing.refunds in changed")["head"]["content_hash"]
        .as_str()
        .expect("content_hash")
        .to_string();

    // Inline patch variant.
    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 3,
        "method": "tools/call",
        "params": {
            "name": "adoc_review",
            "arguments": {
                "base_ref": "main",
                "patch": {
                    "source": "inline",
                    "patch": {
                        "schema_version": "adoc.patch.v0",
                        "op": "replace_body",
                        "target": "billing.refunds",
                        "base_hash": head_hash,
                        "changes": { "body": "Refunds process within 6 hours." },
                        "reason": "V3.7 dogfood (inline)"
                    }
                }
            }
        }
    }));
    let inline = server.receive();
    assert_eq!(inline["id"], 3);
    assert!(
        inline.get("error").is_none(),
        "inline-patch adoc_review error: {inline:#?}"
    );
    let inline_content = structured_content(&inline);
    assert_eq!(inline_content["schema_version"], "adoc.review.v0");
    let patch_check = &inline_content["patch_check"];
    assert!(
        patch_check.is_object(),
        "patch_check must be present when patch parameter supplied: {inline_content}"
    );
    assert_eq!(patch_check["schema_version"], "adoc.patch.check.v0");
    assert_eq!(patch_check["valid"], true);
    assert_eq!(patch_check["target"], "billing.refunds");

    // Path patch variant. Path-policy resolves under the project root, so
    // the patch file must live there.
    fixture.write(
        "tests-tmp/patch.json",
        &format!(
            concat!(
                "{{\n",
                "  \"schema_version\": \"adoc.patch.v0\",\n",
                "  \"op\": \"replace_body\",\n",
                "  \"target\": \"billing.refunds\",\n",
                "  \"base_hash\": \"{}\",\n",
                "  \"changes\": {{ \"body\": \"Refunds process within 4 hours.\" }},\n",
                "  \"reason\": \"V3.7 dogfood (path)\"\n",
                "}}\n",
            ),
            head_hash,
        ),
    );
    server.send(serde_json::json!({
        "jsonrpc": "2.0",
        "id": 4,
        "method": "tools/call",
        "params": {
            "name": "adoc_review",
            "arguments": {
                "base_ref": "main",
                "patch": {
                    "source": "path",
                    "patch_path": "tests-tmp/patch.json"
                }
            }
        }
    }));
    let path = server.receive();
    assert_eq!(path["id"], 4);
    assert!(
        path.get("error").is_none(),
        "path-patch adoc_review error: {path:#?}"
    );
    let path_content = structured_content(&path);
    assert_eq!(path_content["patch_check"]["valid"], true);
    assert_eq!(path_content["patch_check"]["target"], "billing.refunds");
}

fn list_files(root: &Path) -> BTreeSet<PathBuf> {
    let mut out = BTreeSet::new();
    walk(root, &mut out);
    out
}

fn walk(dir: &Path, out: &mut BTreeSet<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            walk(&path, out);
        } else {
            out.insert(path);
        }
    }
}

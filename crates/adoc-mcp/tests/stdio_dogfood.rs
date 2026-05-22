use std::fs;
use std::io::{BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process::{Child, ChildStdin, ChildStdout, Command, Stdio};

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

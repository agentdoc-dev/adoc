mod support;

use adoc_core::RetrievalPolicy;
use adoc_mcp::AgentDocMcpServer;
use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command};

const CONFIG: &str = "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n";
const SOURCE: &str = "\
# Billing @doc(team.billing)

Billing policy explains refunds and hold processing.

::claim billing.internal
status: verified
owner: team.billing
verified_at: 1999-01-01
source: test
expires_at: 2000-01-01
visibility: internal
depends_on: billing.public
supersedes: billing.public
related_to: billing.public
impacts: [src/billing.rs]
--
Billing internal refunds need review before release.
::

::claim billing.public
status: draft
--
Billing public refunds follow the published policy.
::

::claim billing.incoming
status: draft
depends_on: billing.internal
supersedes: billing.internal
related_to: billing.internal
--
Billing incoming refunds depend on internal guidance.
::

::claim billing.excluded
status: verified
owner: team.billing
verified_at: 1999-01-01
source: test
expires_at: 2000-01-01
related_to: billing.internal
impacts: [src/billing.rs]
--
EXCLUDED_GATEWAY_CANARY billing refunds must never be disclosed.
::

::contradiction billing.conflict
visibility: internal
severity: high
status: unresolved
claims: [billing.internal, billing.public]
--
Billing internal and public refund guidance conflict.
::
";

fn policy(internal: bool) -> RetrievalPolicy {
    RetrievalPolicy {
        audience: if internal { "internal" } else { "public" }.into(),
        allowed_visibilities: if internal {
            ["public".into(), "internal".into()].into()
        } else {
            ["public".into()].into()
        },
        excluded_object_ids: ["billing.excluded".into()].into(),
    }
}

fn write_policy(workspace: &TestWorkspace, policy: &RetrievalPolicy) {
    workspace.write(
        "agentdoc.config.yaml",
        &format!("{CONFIG}retrieval_policy: {}\n", json!(policy)),
    );
}

fn fixture() -> TestWorkspace {
    let workspace = TestWorkspace::new("gateway-parity");
    workspace.write("agentdoc.config.yaml", CONFIG);
    workspace.write("docs/billing.adoc", SOURCE);
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(workspace.root.join("dist/docs.search.json").is_file());
    workspace
}

fn queries() -> Vec<(Vec<&'static str>, Value)> {
    let mut queries = Vec::new();
    for mode in ["lexical", "hybrid", "semantic"] {
        let mut args = vec!["search", "billing", "--top", "20"];
        if mode == "lexical" {
            args.push("--lexical");
        } else if mode == "semantic" {
            args.push("--semantic");
        }
        queries.push((
            args,
            json!({"query": "billing", "top": 20, "lexical": mode == "lexical", "semantic": mode == "semantic"}),
        ));
    }
    for direction in ["incoming", "outgoing", "both"] {
        for relation in ["depends_on", "supersedes", "related_to"] {
            queries.push((
                vec!["graph", "billing.internal", "--direction", direction, "--relation", relation],
                json!({"object_id": "billing.internal", "direction": direction, "relation": relation}),
            ));
        }
    }
    queries.extend([
        (
            vec!["why", "billing.internal"],
            json!({"object_id": "billing.internal"}),
        ),
        (vec!["stale"], json!({})),
        (vec!["contradictions", "--all"], json!({"all": true})),
        (
            vec!["impacted-by", "src/billing.rs"],
            json!({"paths": ["src/billing.rs"]}),
        ),
    ]);
    queries
}

fn cli(workspace: &TestWorkspace, args: &[&str], exit_code: i32) -> Value {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(args)
        .args(["--format", "json", "--color", "never"])
        .output()
        .unwrap();
    assert_eq!(
        output.status.code(),
        Some(exit_code),
        "{args:?}: {output:?}"
    );
    assert!(output.stderr.is_empty(), "{args:?}: {output:?}");
    serde_json::from_slice(&output.stdout).expect("CLI emits its complete retrieval envelope")
}

fn mcp(
    server: &AgentDocMcpServer,
    workspace: &TestWorkspace,
    command: &str,
    mut params: Value,
) -> Value {
    params["project_root"] = json!(workspace.root);
    match command {
        "search" => server.run_search(serde_json::from_value(params).unwrap()),
        "why" => server.run_why(serde_json::from_value(params).unwrap()),
        "graph" => server.run_graph(serde_json::from_value(params).unwrap()),
        "stale" => server.run_stale(serde_json::from_value(params).unwrap()),
        "contradictions" => server.run_contradictions(serde_json::from_value(params).unwrap()),
        "impacted-by" => server.run_impacted_by(serde_json::from_value(params).unwrap()),
        _ => unreachable!(),
    }
    .unwrap()
}

fn assert_bytes(expected: &Value, actual: &Value, args: &[&str]) {
    // CLI whitespace and MCP framing are different transports. Serialize both
    // complete payloads identically; remove no fields, scores, diagnostics or dates.
    assert_eq!(
        serde_json::to_vec(expected).unwrap(),
        serde_json::to_vec(actual).unwrap(),
        "{args:?}"
    );
}

#[test]
fn trusted_gateway_matches_cli_envelopes_for_search_graph_and_signals() {
    let workspace = fixture();
    let gateway = TestWorkspace::new("gateway-binding");
    let trusted = policy(true);
    let server =
        AgentDocMcpServer::new(gateway.root.clone()).with_retrieval_policy(trusted.clone());
    let mut project_policy = policy(false);
    project_policy.excluded_object_ids.clear();

    for (mut args, mut params) in queries() {
        // Explicit graph paths must not bypass the binding. Search keeps config
        // artifact/provider selection, exercising that separate shared loader.
        if args[0] != "search" {
            args.extend(["--artifact", "dist/docs.graph.json"]);
            params["artifact"] = json!("dist/docs.graph.json");
        }
        write_policy(&workspace, &trusted);
        let expected = cli(&workspace, &args, 0);
        let encoded = expected.to_string();
        assert!(
            encoded.contains("billing.internal"),
            "nonempty permitted carrier: {args:?}: {expected}"
        );
        assert!(
            !encoded.contains("billing.excluded"),
            "{args:?}: {expected}"
        );
        assert!(!encoded.contains("EXCLUDED_GATEWAY_CANARY"));
        if args[0] == "search" && !args.contains(&"--lexical") {
            assert!(
                expected["records"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|record| record["match"]["vector_rank"].is_number()),
                "must exercise vectors: {expected}"
            );
        }
        if args[0] == "graph" {
            assert!(
                !expected["edges"].as_array().unwrap().is_empty(),
                "must exercise each direction/relation: {args:?}"
            );
        }
        write_policy(&workspace, &project_policy);
        assert_bytes(&expected, &mcp(&server, &workspace, args[0], params), &args);
    }
}

#[test]
fn malformed_gateway_or_project_policy_fails_closed_with_cli_envelope_parity() {
    let workspace = fixture();
    let gateway = TestWorkspace::new("gateway-invalid-binding");
    let valid = policy(true);
    let mut invalid = valid.clone();
    invalid.audience = "INVALID_GATEWAY_CANARY".into();
    let invalid_server =
        AgentDocMcpServer::new(gateway.root.clone()).with_retrieval_policy(invalid.clone());
    let valid_server =
        AgentDocMcpServer::new(gateway.root.clone()).with_retrieval_policy(valid.clone());
    for (args, params) in queries() {
        write_policy(&workspace, &invalid);
        let expected = cli(&workspace, &args, 2);
        assert_eq!(expected["diagnostics"].as_array().unwrap().len(), 1);
        assert_eq!(
            expected["diagnostics"][0]["code"],
            "retrieval.audience_unresolved"
        );
        assert_eq!(expected["diagnostics"][0]["severity"], "error");
        for key in [
            "records",
            "nodes",
            "edges",
            "contradictions",
            "contradicted_claims",
            "impacted",
        ] {
            if let Some(entries) = expected[key].as_array() {
                assert!(entries.is_empty(), "{args:?}: {expected}");
            }
        }
        assert!(!expected.to_string().contains("CANARY"));
        // A trusted binding cannot bypass an invalid project configuration.
        assert_bytes(
            &expected,
            &mcp(&valid_server, &workspace, args[0], params.clone()),
            &args,
        );
        write_policy(&workspace, &valid);
        // Conversely, a valid selected project cannot rescue an invalid binding.
        assert_bytes(
            &expected,
            &mcp(&invalid_server, &workspace, args[0], params),
            &args,
        );
    }
}

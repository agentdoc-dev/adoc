mod support;

use std::{collections::BTreeSet, fs};

use adoc_core::RetrievalPolicy;
use adoc_mcp::{AgentDocMcpServer, McpAdapterError};
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
    // This read-policy fixture deliberately starts with an authorized wide index.
    write_policy(&workspace, &embedding_policy("restricted", false));
    workspace.write("docs/billing.adoc", SOURCE);
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist"])
        .output()
        .unwrap();
    assert!(output.status.success(), "{output:?}");
    assert!(workspace.root.join("dist/docs.search.json").is_file());
    workspace.write("agentdoc.config.yaml", CONFIG);
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

fn mcp_result(
    server: &AgentDocMcpServer,
    workspace: &TestWorkspace,
    command: &str,
    mut params: Value,
) -> Result<Value, McpAdapterError> {
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
    .map(|result| result.structured_content.unwrap())
}

fn mcp(
    server: &AgentDocMcpServer,
    workspace: &TestWorkspace,
    command: &str,
    params: Value,
) -> Value {
    mcp_result(server, workspace, command, params).unwrap()
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
fn trusted_gateway_requires_recording_for_sensitive_cli_exempt_output() {
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
        assert!(
            matches!(
                mcp_result(&server, &workspace, args[0], params),
                Err(McpAdapterError::AuditSinkUnavailable)
            ),
            "sensitive {args:?} requires recording"
        );
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

#[test]
fn ordinary_gateway_preserves_complete_cli_envelope_parity() {
    let workspace = fixture();
    let trusted = policy(false);
    let server =
        AgentDocMcpServer::new(workspace.root.clone()).with_retrieval_policy(trusted.clone());
    write_policy(&workspace, &trusted);
    for (mut args, mut params) in queries() {
        if matches!(args[0], "graph" | "why") {
            args[1] = "billing.public";
            params["object_id"] = json!("billing.public");
        }
        let expected = cli(&workspace, &args, 0);
        assert_bytes(&expected, &mcp(&server, &workspace, args[0], params), &args);
    }
}

const EMBEDDING_SOURCE: &str = "# Embedding boundaries @doc(embedding.page)

This ordinary orientation has enough words for embedding.

This protected orientation refers to [[embedding.internal]] for details.

::claim embedding.internal
status: draft
visibility: internal
--
INTERNAL_PASSAGE_CANARY is private context.
::

::claim embedding.excluded
status: draft
visibility: public
--
EXCLUDED_PASSAGE_CANARY is explicitly denied.
::

::claim embedding.partial
status: draft
visibility: public
owner: HIDDEN_OWNER_CANARY
field_visibility: owner=internal, body=restricted
--
HIDDEN_BODY_CANARY must never become public model input.
::

::claim embedding.safe
status: draft
owner: public-team
--
PUBLIC_SIBLING_CANARY remains useful and searchable.
::
";

fn embedding_policy(audience: &str, exclude: bool) -> RetrievalPolicy {
    RetrievalPolicy {
        audience: audience.into(),
        allowed_visibilities: ["public".into(), "internal".into(), "restricted".into()].into(),
        excluded_object_ids: if exclude {
            ["embedding.excluded".into()].into()
        } else {
            Default::default()
        },
    }
}

fn build(workspace: &TestWorkspace, args: &[&str]) -> String {
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(args)
        .output()
        .unwrap();
    assert!(output.status.success(), "{args:?}: {output:?}");
    format!(
        "{}{}",
        String::from_utf8(output.stdout).unwrap(),
        String::from_utf8(output.stderr).unwrap()
    )
}

fn artifact(workspace: &TestWorkspace, path: &str) -> Value {
    serde_json::from_slice(&fs::read(workspace.root.join(path)).unwrap()).unwrap()
}

fn embedding<'a>(search: &'a Value, id: &str) -> &'a Value {
    search["embeddings"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["id"] == id)
        .unwrap()
}

fn assert_embedding_ids(search: &Value, graph: &Value, internal: bool, excluded: bool) {
    let mut expected: BTreeSet<String> =
        ["embedding.partial".into(), "embedding.safe".into()].into();
    if internal {
        expected.insert("embedding.internal".into());
    }
    if excluded {
        expected.insert("embedding.excluded".into());
    }
    for node in graph["nodes"].as_array().unwrap() {
        let text = node["text"].as_str().unwrap_or_default();
        if text.starts_with("This ordinary orientation")
            || (internal && text.starts_with("This protected orientation"))
        {
            expected.insert(node["id"].as_str().unwrap().into());
        }
    }
    assert!(
        expected.len() >= 3,
        "must include the real safe prose carrier"
    );
    let actual = search["embeddings"]
        .as_array()
        .unwrap()
        .iter()
        .map(|entry| entry["id"].as_str().unwrap().to_owned())
        .collect();
    assert_eq!(expected, actual);
    assert_eq!(search["schema_version"], "adoc.search.v2");
}

#[test]
fn trusted_public_build_excludes_protected_embeddings_despite_broad_project_policy() {
    let workspace = TestWorkspace::new("gateway-embedding-policy");
    workspace.write("docs/embedding.adoc", EMBEDDING_SOURCE);
    write_policy(&workspace, &embedding_policy("restricted", false));
    let public = embedding_policy("public", true);
    let server =
        AgentDocMcpServer::new(workspace.root.clone()).with_retrieval_policy(public.clone());
    let built = server.run_build(Default::default()).unwrap();
    assert_eq!(built["exit_code"], 0, "{built}");
    let graph = artifact(&workspace, "dist/docs.graph.json");
    let narrow = artifact(&workspace, "dist/docs.search.json");
    assert_embedding_ids(&narrow, &graph, false, false);
    assert!(graph.to_string().contains("HIDDEN_BODY_CANARY"));
    assert!(graph.to_string().contains("HIDDEN_OWNER_CANARY"));

    // Same public authority gives complete CLI/MCP parity, including vector ranks.
    write_policy(&workspace, &public);
    let args = [
        "search",
        "PUBLIC_SIBLING_CANARY",
        "--semantic",
        "--top",
        "20",
    ];
    let expected = cli(&workspace, &args, 0);
    assert!(!expected["records"].as_array().unwrap().is_empty());
    assert!(
        expected["records"]
            .as_array()
            .unwrap()
            .iter()
            .all(|r| r["match"]["vector_rank"].is_number())
    );
    assert_bytes(
        &expected,
        &mcp(
            &server,
            &workspace,
            "search",
            json!({"query":"PUBLIC_SIBLING_CANARY", "semantic":true, "top":20}),
        ),
        &args,
    );

    // Absent policy is public and still projects authored fields, but has no exclusion list.
    workspace.write("agentdoc.config.yaml", CONFIG);
    build(&workspace, &["build", "docs", "--out", "default"]);
    let default = artifact(&workspace, "default/docs.search.json");
    assert_embedding_ids(&default, &graph, false, true);
    assert_eq!(
        embedding(&default, "embedding.partial"),
        embedding(&narrow, "embedding.partial")
    );
    assert_eq!(
        fs::read(workspace.root.join("dist/docs.graph.json")).unwrap(),
        fs::read(workspace.root.join("default/docs.graph.json")).unwrap()
    );
    assert_eq!(
        default["graph_artifact_hash"],
        narrow["graph_artifact_hash"]
    );
    build(
        &workspace,
        &["build", "docs", "--out", "skipped", "--no-embeddings"],
    );
    assert!(workspace.root.join("skipped/docs.graph.json").is_file());
    assert!(!workspace.root.join("skipped/docs.search.json").exists());
}

#[test]
fn cli_narrow_rebuild_rejects_wide_cached_embeddings_and_retains_authority_controls() {
    let workspace = TestWorkspace::new("gateway-embedding-cache");
    workspace.write("docs/embedding.adoc", EMBEDDING_SOURCE);
    write_policy(&workspace, &embedding_policy("restricted", false));
    build(&workspace, &["build", "docs", "--out", "dist"]);
    let graph = artifact(&workspace, "dist/docs.graph.json");
    let graph_bytes = fs::read(workspace.root.join("dist/docs.graph.json")).unwrap();
    let broad = artifact(&workspace, "dist/docs.search.json");
    assert_embedding_ids(&broad, &graph, true, true);
    let authorized = cli(
        &workspace,
        &[
            "search",
            "INTERNAL_PASSAGE_CANARY",
            "--semantic",
            "--top",
            "20",
        ],
        0,
    );
    assert!(
        authorized["records"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == "embedding.internal" && r["match"]["vector_rank"].is_number())
    );
    let authorized_server = AgentDocMcpServer::new(workspace.root.clone())
        .with_retrieval_policy(embedding_policy("restricted", false));
    assert!(matches!(
        mcp_result(
            &authorized_server,
            &workspace,
            "search",
            json!({"query":"INTERNAL_PASSAGE_CANARY", "semantic":true, "top":20})
        ),
        Err(McpAdapterError::AuditSinkUnavailable)
    ));

    write_policy(&workspace, &embedding_policy("public", true));
    let rebuilt = build(&workspace, &["build", "docs", "--out", "dist"]);
    let narrow = artifact(&workspace, "dist/docs.search.json");
    assert_embedding_ids(&narrow, &graph, false, false);
    assert!(
        rebuilt.contains("embeddings: cached 2, computed 1"),
        "{rebuilt}"
    );
    assert_eq!(
        embedding(&narrow, "embedding.safe"),
        embedding(&broad, "embedding.safe")
    );
    assert_ne!(
        embedding(&narrow, "embedding.partial"),
        embedding(&broad, "embedding.partial")
    );
    assert_eq!(
        graph_bytes,
        fs::read(workspace.root.join("dist/docs.graph.json")).unwrap()
    );
    assert_eq!(narrow["graph_artifact_hash"], broad["graph_artifact_hash"]);
    build(&workspace, &["build", "docs", "--out", "clean"]);
    assert_eq!(
        fs::read(workspace.root.join("dist/docs.search.json")).unwrap(),
        fs::read(workspace.root.join("clean/docs.search.json")).unwrap()
    );

    // CLI audience override retains the configured exclusion and allowed set.
    build(
        &workspace,
        &[
            "build",
            "docs",
            "--out",
            "internal",
            "--audience",
            "internal",
        ],
    );
    let internal = artifact(&workspace, "internal/docs.search.json");
    assert_embedding_ids(&internal, &graph, true, false);
    assert_ne!(
        embedding(&internal, "embedding.partial"),
        embedding(&narrow, "embedding.partial")
    );
    assert_ne!(
        embedding(&internal, "embedding.partial"),
        embedding(&broad, "embedding.partial")
    );
    let mut ceiling = embedding_policy("public", true);
    ceiling.allowed_visibilities = ["public".into()].into();
    write_policy(&workspace, &ceiling);
    build(
        &workspace,
        &[
            "build",
            "docs",
            "--out",
            "ceiling",
            "--audience",
            "internal",
        ],
    );
    assert_embedding_ids(
        &artifact(&workspace, "ceiling/docs.search.json"),
        &graph,
        false,
        false,
    );
}

use std::collections::BTreeMap;
use std::path::PathBuf;

use adoc_core::{
    AgentJsonDocument, AgentJsonObject, AgentJsonRelations, AgentJsonSourceSpan,
    BuildEmbeddingMode, BuildInput, DiagnosticCode, GraphArtifactDocument, GraphDirection,
    GraphEdge, GraphInput, GraphNode, GraphRelationKind, GraphTraversalQuery, load_graph_session,
    traverse_graph,
};
use sha2::{Digest, Sha256};

mod support;

use support::TestWorkspace;

fn write_temp_artifact(name: &str, suffix: &str, contents: &str) -> tempfile::NamedTempFile {
    let artifact = tempfile::Builder::new()
        .prefix(&format!("adoc-graph-{name}-"))
        .suffix(suffix)
        .tempfile()
        .expect("temp artifact can be created");
    std::fs::write(artifact.path(), contents).expect("temp artifact can be written");
    artifact
}

fn sha256_prefixed(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

fn object(id: &str, relations: AgentJsonRelations) -> AgentJsonObject {
    AgentJsonObject {
        id: id.to_string(),
        kind: "claim".to_string(),
        status: Some("draft".to_string()),
        body: format!("{id} body."),
        page_id: "team.graph".to_string(),
        source_span: AgentJsonSourceSpan {
            path: "docs/graph.adoc".to_string(),
            line: 1,
            column: 1,
        },
        fields: BTreeMap::new(),
        relations,
    }
}

fn agent_document(objects: Vec<AgentJsonObject>) -> AgentJsonDocument {
    AgentJsonDocument {
        schema_version: "adoc.agent.v0".to_string(),
        pages: Vec::new(),
        objects,
        diagnostics: Vec::new(),
    }
}

fn graph_document(agent_json: &str, nodes: Vec<GraphNode>, edges: Vec<GraphEdge>) -> String {
    GraphArtifactDocument {
        schema_version: "adoc.graph.v0".to_string(),
        agent_artifact_hash: sha256_prefixed(agent_json.as_bytes()),
        nodes,
        edges,
    }
    .to_pretty_json()
    .expect("graph artifact serializes")
}

fn load_session(agent: AgentJsonDocument, graph: GraphArtifactDocument) -> adoc_core::GraphSession {
    let agent_json = agent.to_pretty_json().expect("agent serializes");
    let agent_artifact = write_temp_artifact("agent", ".agent.json", &agent_json);
    let graph_artifact = write_temp_artifact(
        "graph",
        ".graph.json",
        &GraphArtifactDocument {
            agent_artifact_hash: sha256_prefixed(agent_json.as_bytes()),
            ..graph
        }
        .to_pretty_json()
        .expect("graph serializes"),
    );

    let result = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: graph_artifact.path().to_path_buf(),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean graph load, got {:?}",
        result.diagnostics
    );
    result.session.expect("graph session loads")
}

#[test]
fn graph_artifact_serializes_with_v0_shape() {
    let artifact = GraphArtifactDocument {
        schema_version: "adoc.graph.v0".to_string(),
        agent_artifact_hash: "sha256:agent".to_string(),
        nodes: vec![GraphNode {
            id: "billing.credits".to_string(),
            kind: "claim".to_string(),
            status: Some("verified".to_string()),
            page_id: "team.billing".to_string(),
        }],
        edges: vec![GraphEdge {
            source: "billing.refunds".to_string(),
            target: "billing.credits".to_string(),
            relation: GraphRelationKind::DependsOn,
        }],
    };

    let value = serde_json::to_value(&artifact).expect("graph artifact serializes");

    assert_eq!(value["schema_version"], "adoc.graph.v0");
    assert_eq!(value["agent_artifact_hash"], "sha256:agent");
    assert_eq!(value["nodes"][0]["id"], "billing.credits");
    assert_eq!(value["nodes"][0]["kind"], "claim");
    assert_eq!(value["nodes"][0]["status"], "verified");
    assert_eq!(value["nodes"][0]["page_id"], "team.billing");
    assert_eq!(value["edges"][0]["source"], "billing.refunds");
    assert_eq!(value["edges"][0]["target"], "billing.credits");
    assert_eq!(value["edges"][0]["relation"], "depends_on");
}

#[test]
fn build_workspace_emits_graph_artifact_with_deterministic_order_when_embeddings_are_skipped() {
    let workspace = TestWorkspace::new("graph-build-artifact");
    let source = workspace.write(
        "graph.adoc",
        concat!(
            "# Graph @doc(team.graph)\n",
            "\n",
            "::claim billing.beta\n",
            "status: draft\n",
            "depends_on: billing.alpha\n",
            "related_to: billing.gamma\n",
            "--\n",
            "Beta depends on alpha.\n",
            "::\n",
            "\n",
            "::claim billing.alpha\n",
            "status: draft\n",
            "--\n",
            "Alpha.\n",
            "::\n",
            "\n",
            "::claim billing.gamma\n",
            "status: draft\n",
            "supersedes: billing.alpha\n",
            "--\n",
            "Gamma.\n",
            "::\n",
        ),
    );

    let result = adoc_core::build_workspace(BuildInput {
        root: source,
        embeddings: BuildEmbeddingMode::Skipped,
        prior_search_artifact_path: None,
    });

    assert!(
        !result.has_errors(),
        "build should pass: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts are produced");
    let graph = artifacts.graph_json;
    assert_eq!(
        graph
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["billing.alpha", "billing.beta", "billing.gamma"]
    );
    assert_eq!(
        graph
            .edges
            .iter()
            .map(|edge| (edge.source.as_str(), edge.relation, edge.target.as_str()))
            .collect::<Vec<_>>(),
        [
            (
                "billing.beta",
                GraphRelationKind::DependsOn,
                "billing.alpha"
            ),
            (
                "billing.beta",
                GraphRelationKind::RelatedTo,
                "billing.gamma"
            ),
            (
                "billing.gamma",
                GraphRelationKind::Supersedes,
                "billing.alpha"
            ),
        ]
    );
    assert!(
        graph.agent_artifact_hash.starts_with("sha256:"),
        "graph artifact should carry agent hash"
    );
    assert!(
        artifacts.search_json.is_none(),
        "graph artifact is emitted even when embeddings are skipped"
    );
}

#[test]
fn load_graph_session_rejects_schema_malformed_missing_artifacts_and_warns_on_hash_drift() {
    let agent = agent_document(vec![object("billing.root", AgentJsonRelations::default())]);
    let agent_json = agent.to_pretty_json().expect("agent serializes");
    let agent_artifact = write_temp_artifact("agent", ".agent.json", &agent_json);

    let missing = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: PathBuf::from("/tmp/adoc-missing-docs.graph.json"),
    });
    assert!(missing.session.is_none());
    assert_eq!(
        missing.diagnostics[0].code,
        DiagnosticCode::IoArtifactMissing
    );

    let malformed_artifact = write_temp_artifact("malformed", ".graph.json", "{");
    let malformed = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: malformed_artifact.path().to_path_buf(),
    });
    assert!(malformed.session.is_none());
    assert_eq!(
        malformed.diagnostics[0].code,
        DiagnosticCode::IoArtifactMalformed
    );

    let unsupported_artifact = write_temp_artifact(
        "unsupported",
        ".graph.json",
        r#"{"schema_version":"adoc.graph.v99","agent_artifact_hash":"sha256:agent","nodes":[],"edges":[]}"#,
    );
    let unsupported = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: unsupported_artifact.path().to_path_buf(),
    });
    assert!(unsupported.session.is_none());
    assert_eq!(
        unsupported.diagnostics[0].code,
        DiagnosticCode::SchemaUnsupportedVersion
    );

    let drift_graph = GraphArtifactDocument {
        schema_version: "adoc.graph.v0".to_string(),
        agent_artifact_hash: "sha256:stale".to_string(),
        nodes: vec![GraphNode {
            id: "billing.root".to_string(),
            kind: "claim".to_string(),
            status: Some("draft".to_string()),
            page_id: "team.graph".to_string(),
        }],
        edges: Vec::new(),
    };
    let drift_artifact = write_temp_artifact(
        "drift",
        ".graph.json",
        &drift_graph.to_pretty_json().expect("graph serializes"),
    );
    let drift = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: drift_artifact.path().to_path_buf(),
    });
    assert!(
        drift.session.is_some(),
        "hash drift is a warning, not fatal"
    );
    assert_eq!(drift.diagnostics[0].code, DiagnosticCode::GraphHashDrift);
}

#[test]
fn graph_traversal_is_full_reachable_and_marks_cycle_edges_without_revisiting_nodes() {
    let agent = agent_document(vec![
        object(
            "billing.a",
            AgentJsonRelations {
                depends_on: vec!["billing.b".to_string()],
                supersedes: Vec::new(),
                related_to: Vec::new(),
            },
        ),
        object(
            "billing.b",
            AgentJsonRelations {
                depends_on: vec!["billing.c".to_string()],
                supersedes: Vec::new(),
                related_to: Vec::new(),
            },
        ),
        object(
            "billing.c",
            AgentJsonRelations {
                depends_on: vec!["billing.a".to_string()],
                supersedes: Vec::new(),
                related_to: Vec::new(),
            },
        ),
        object(
            "billing.d",
            AgentJsonRelations {
                depends_on: Vec::new(),
                supersedes: Vec::new(),
                related_to: vec!["billing.a".to_string()],
            },
        ),
    ]);
    let graph = GraphArtifactDocument {
        schema_version: "adoc.graph.v0".to_string(),
        agent_artifact_hash: "sha256:filled-by-helper".to_string(),
        nodes: vec![
            GraphNode {
                id: "billing.a".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
            GraphNode {
                id: "billing.b".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
            GraphNode {
                id: "billing.c".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
            GraphNode {
                id: "billing.d".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
        ],
        edges: vec![
            GraphEdge {
                source: "billing.a".to_string(),
                target: "billing.b".to_string(),
                relation: GraphRelationKind::DependsOn,
            },
            GraphEdge {
                source: "billing.b".to_string(),
                target: "billing.c".to_string(),
                relation: GraphRelationKind::DependsOn,
            },
            GraphEdge {
                source: "billing.c".to_string(),
                target: "billing.a".to_string(),
                relation: GraphRelationKind::DependsOn,
            },
            GraphEdge {
                source: "billing.d".to_string(),
                target: "billing.a".to_string(),
                relation: GraphRelationKind::RelatedTo,
            },
        ],
    };
    let session = load_session(agent, graph);

    let traversal = traverse_graph(
        &session,
        GraphTraversalQuery {
            root_id: "billing.a".to_string(),
            direction: GraphDirection::Outgoing,
            relations: vec![GraphRelationKind::DependsOn],
        },
    );

    assert!(traversal.diagnostics.is_empty());
    assert_eq!(
        traversal
            .nodes
            .iter()
            .map(|node| (node.id.as_str(), node.distance))
            .collect::<Vec<_>>(),
        [("billing.a", 0), ("billing.b", 1), ("billing.c", 2)]
    );
    assert_eq!(traversal.edges.len(), 3);
    assert!(
        !traversal.edges[0].revisit,
        "first tree edge should not be a revisit"
    );
    assert!(
        traversal.edges[2].revisit,
        "cycle edge back to the root should be marked as a revisit"
    );
    assert_eq!(traversal.edges[2].source, "billing.c");
    assert_eq!(traversal.edges[2].target, "billing.a");
}

#[test]
fn graph_traversal_applies_direction_and_relation_filters() {
    let agent = agent_document(vec![
        object("billing.a", AgentJsonRelations::default()),
        object(
            "billing.b",
            AgentJsonRelations {
                depends_on: vec!["billing.a".to_string()],
                supersedes: Vec::new(),
                related_to: Vec::new(),
            },
        ),
        object(
            "billing.c",
            AgentJsonRelations {
                depends_on: Vec::new(),
                supersedes: Vec::new(),
                related_to: vec!["billing.a".to_string()],
            },
        ),
    ]);
    let agent_json = agent.to_pretty_json().expect("agent serializes");
    let graph_json = graph_document(
        &agent_json,
        vec![
            GraphNode {
                id: "billing.a".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
            GraphNode {
                id: "billing.b".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
            GraphNode {
                id: "billing.c".to_string(),
                kind: "claim".to_string(),
                status: Some("draft".to_string()),
                page_id: "team.graph".to_string(),
            },
        ],
        vec![
            GraphEdge {
                source: "billing.b".to_string(),
                target: "billing.a".to_string(),
                relation: GraphRelationKind::DependsOn,
            },
            GraphEdge {
                source: "billing.c".to_string(),
                target: "billing.a".to_string(),
                relation: GraphRelationKind::RelatedTo,
            },
        ],
    );
    let agent_artifact = write_temp_artifact("agent", ".agent.json", &agent_json);
    let graph_artifact = write_temp_artifact("graph", ".graph.json", &graph_json);
    let result = load_graph_session(GraphInput {
        agent_artifact_path: agent_artifact.path().to_path_buf(),
        graph_artifact_path: graph_artifact.path().to_path_buf(),
    });
    let session = result.session.expect("graph session loads");

    let traversal = traverse_graph(
        &session,
        GraphTraversalQuery {
            root_id: "billing.a".to_string(),
            direction: GraphDirection::Incoming,
            relations: vec![GraphRelationKind::RelatedTo],
        },
    );

    assert!(traversal.diagnostics.is_empty());
    assert_eq!(
        traversal
            .nodes
            .iter()
            .map(|node| node.id.as_str())
            .collect::<Vec<_>>(),
        ["billing.a", "billing.c"]
    );
    assert_eq!(traversal.edges[0].source, "billing.c");
    assert_eq!(traversal.edges[0].target, "billing.a");
    assert_eq!(traversal.edges[0].relation, GraphRelationKind::RelatedTo);
}

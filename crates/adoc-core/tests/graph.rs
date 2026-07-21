use std::path::PathBuf;

use adoc_core::{
    BuildEmbeddingMode, BuildInput, DiagnosticCode, GraphDirection, GraphInput, GraphRelationKind,
    GraphTraversalQuery, LocalProjectContext, build_project_workspace, load_graph_session,
    traverse_graph,
};
use serde_json::{Value, json};

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

fn empty_relations() -> Value {
    json!({
        "depends_on": [],
        "supersedes": [],
        "related_to": []
    })
}

fn graph_node(id: &str) -> Value {
    json!({
        "type": "knowledge_object",
        "id": id,
        "kind": "claim",
        "content_hash": format!("sha256:{id}"),
        "status": "draft",
        "body": format!("{id} body."),
        "page_id": "team.graph",
        "source_span": {
            "path": "docs/graph.adoc",
            "line": 1,
            "column": 1
        },
        "fields": {},
        "relations": empty_relations()
    })
}

fn relation_edge(source: &str, relation: GraphRelationKind, target: &str) -> Value {
    json!({
        "kind": "relation",
        "source": source,
        "target": target,
        "relation": relation.as_str()
    })
}

fn graph_document(nodes: Vec<Value>, edges: Vec<Value>) -> String {
    serde_json::to_string_pretty(&json!({
          "schema_version": "adoc.graph.v5",
    "repository_identity": null,
          "nodes": nodes,
          "edges": edges,
          "diagnostics": []
      }))
    .expect("graph fixture serializes")
}

fn load_session(graph_json: String) -> adoc_core::GraphSession {
    let graph_artifact = write_temp_artifact("graph", ".graph.json", &graph_json);

    let result = load_graph_session(GraphInput {
        graph_artifact_path: graph_artifact.path().to_path_buf(),
    });

    assert!(
        result.diagnostics.is_empty(),
        "expected clean graph load, got {:?}",
        result.diagnostics
    );
    result.session.expect("graph session loads")
}

fn build_graph_value(source: &str) -> Value {
    let workspace = TestWorkspace::new("graph-hash");
    let source_path = workspace.write("graph.adoc", source);
    let result = adoc_core::build_workspace(BuildInput {
        root: source_path,
        embeddings: BuildEmbeddingMode::Skipped,
        prior_search_artifact_path: None,
    });
    assert!(
        !result.has_errors(),
        "build should pass: {:?}",
        result.diagnostics
    );
    serde_json::from_str(&result.artifacts.expect("artifacts are produced").graph_json)
        .expect("graph artifact is JSON")
}

fn object_hash(graph: &Value, id: &str) -> String {
    graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["type"] == "knowledge_object" && node["id"] == id)
        .and_then(|node| node["content_hash"].as_str())
        .expect("knowledge object content_hash")
        .to_string()
}

fn hash_source(body: &str, page_id: &str, owner: &str, relation: &str, prefix: &str) -> String {
    format!(
        concat!(
            "# Graph @doc({page_id})\n",
            "\n",
            "{prefix}",
            "::claim billing.credits\n",
            "status: draft\n",
            "owner: {owner}\n",
            "depends_on: {relation}\n",
            "--\n",
            "{body}\n",
            "::\n",
            "\n",
            "::claim {relation}\n",
            "status: draft\n",
            "--\n",
            "Related body.\n",
            "::\n",
        ),
        page_id = page_id,
        prefix = prefix,
        owner = owner,
        relation = relation,
        body = body,
    )
}

#[test]
fn graph_artifact_serializes_with_v2_shape() {
    let artifact = graph_document(
        vec![json!({
            "type": "knowledge_object",
            "id": "billing.credits",
            "kind": "claim",
            "content_hash": "sha256:billing.credits",
            "status": "verified",
            "body": "billing.credits body.",
            "page_id": "team.billing",
            "source_span": {
                "path": "docs/graph.adoc",
                "line": 1,
                "column": 1
            },
            "fields": {},
            "relations": empty_relations()
        })],
        vec![relation_edge(
            "billing.refunds",
            GraphRelationKind::DependsOn,
            "billing.credits",
        )],
    );

    let value: Value = serde_json::from_str(&artifact).expect("graph artifact serializes");

    assert_eq!(value["schema_version"], "adoc.graph.v5");
    assert_eq!(value.get("graph_artifact_hash"), None);
    assert!(
        !artifact.contains("\"html\""),
        "graph artifact must be presentation-free: {artifact}"
    );
    assert_eq!(value["nodes"][0]["type"], "knowledge_object");
    assert_eq!(value["nodes"][0]["id"], "billing.credits");
    assert_eq!(value["nodes"][0]["kind"], "claim");
    assert_eq!(value["nodes"][0]["content_hash"], "sha256:billing.credits");
    assert_eq!(value["nodes"][0]["status"], "verified");
    assert_eq!(value["nodes"][0]["page_id"], "team.billing");
    assert_eq!(value["edges"][0]["kind"], "relation");
    assert_eq!(value["edges"][0]["source"], "billing.refunds");
    assert_eq!(value["edges"][0]["target"], "billing.credits");
    assert_eq!(value["edges"][0]["relation"], "depends_on");
}

#[test]
fn graph_content_hash_is_stable_for_same_source() {
    let source = hash_source(
        "Credits apply after successful payment.",
        "team.graph",
        "team-billing",
        "billing.ledger",
        "",
    );
    let workspace = TestWorkspace::new("graph-hash-stable");
    let source_path = workspace.write("graph.adoc", &source);
    let build = || {
        let result = adoc_core::build_workspace(BuildInput {
            root: source_path.clone(),
            embeddings: BuildEmbeddingMode::Skipped,
            prior_search_artifact_path: None,
        });
        assert!(
            !result.has_errors(),
            "build should pass: {:?}",
            result.diagnostics
        );
        serde_json::from_str(&result.artifacts.expect("artifacts are produced").graph_json)
            .expect("graph artifact is JSON")
    };

    let first = object_hash(&build(), "billing.credits");
    let second = object_hash(&build(), "billing.credits");

    assert!(first.starts_with("sha256:"));
    assert_eq!(first, second);
}

#[test]
fn standalone_build_emits_graph_v5_without_repository_identity() {
    let graph = build_graph_value(
        "# Graph @doc(team.graph)\n\n::claim billing.credits\nstatus: draft\n--\nCredits.\n::\n",
    );

    assert_eq!(graph["schema_version"], "adoc.graph.v5");
    assert!(graph.get("repository_identity").is_some());
    assert!(graph["repository_identity"].is_null());
    assert_eq!(graph["nodes"][0]["source_path"], "graph.adoc");
}

#[test]
fn project_build_emits_repository_identity_and_project_relative_source_paths() {
    let workspace = TestWorkspace::new("graph-project-identity");
    let docs_root = workspace.root().join("knowledge");
    std::fs::create_dir_all(&docs_root).expect("docs directory");
    let source_path = docs_root.join("graph.adoc");
    std::fs::write(
        &source_path,
        "# Graph @doc(team.graph)\n\n::claim billing.credits\nstatus: draft\n--\nCredits.\n::\n",
    )
    .expect("source");

    let result = build_project_workspace(
        BuildInput {
            root: docs_root.clone(),
            embeddings: BuildEmbeddingMode::Skipped,
            prior_search_artifact_path: None,
        },
        LocalProjectContext {
            project_root: workspace.root().to_path_buf(),
            docs_root,
        },
    );
    let graph: Value =
        serde_json::from_str(&result.artifacts.expect("artifacts").graph_json).expect("graph JSON");

    assert_eq!(
        graph["repository_identity"],
        json!({"kind": "local_project", "config_path": "agentdoc.config.yaml"})
    );
    assert_eq!(graph["nodes"][0]["source_path"], "knowledge/graph.adoc");
}

#[test]
fn project_graph_objects_are_portable_across_checkout_locations() {
    fn build_clone(workspace: &TestWorkspace) -> Value {
        let docs_root = workspace.root().join("knowledge");
        let source_path = docs_root.join("billing/credits.adoc");
        std::fs::create_dir_all(source_path.parent().expect("source parent"))
            .expect("docs directory");
        std::fs::write(
            &source_path,
            "# Credits\n\n::claim billing.credits\nstatus: draft\n--\nCredits.\n::\n",
        )
        .expect("source");
        let result = build_project_workspace(
            BuildInput {
                root: docs_root.clone(),
                embeddings: BuildEmbeddingMode::Skipped,
                prior_search_artifact_path: None,
            },
            LocalProjectContext {
                project_root: workspace.root().to_path_buf(),
                docs_root,
            },
        );
        serde_json::from_str(&result.artifacts.expect("artifacts").graph_json).expect("graph JSON")
    }

    let first = build_clone(&TestWorkspace::new("portable-clone-a"));
    let second = build_clone(&TestWorkspace::new("portable-clone-b"));

    assert_eq!(first["nodes"], second["nodes"]);
    assert_eq!(first["repository_identity"], second["repository_identity"]);
    assert_eq!(
        object_hash(&first, "billing.credits"),
        object_hash(&second, "billing.credits")
    );
    assert_eq!(first["nodes"][0]["id"], "billing.credits");
}

#[test]
fn graph_v5_reader_requires_repository_identity_member() {
    let graph_json =
        graph_document(Vec::new(), Vec::new()).replace("  \"repository_identity\": null,\n", "");
    let artifact = write_temp_artifact("missing-repository-identity", ".graph.json", &graph_json);

    let result = load_graph_session(GraphInput {
        graph_artifact_path: artifact.path().to_path_buf(),
    });

    assert!(result.session.is_none());
    assert_eq!(
        result.diagnostics[0].code,
        DiagnosticCode::IoArtifactMalformed
    );
}

#[test]
fn graph_content_hash_changes_when_node_semantics_change() {
    let base = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after successful payment.",
            "team.graph",
            "team-billing",
            "billing.ledger",
            "",
        )),
        "billing.credits",
    );

    let changed_body = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after ledger commit.",
            "team.graph",
            "team-billing",
            "billing.ledger",
            "",
        )),
        "billing.credits",
    );
    let changed_page = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after successful payment.",
            "team.changed",
            "team-billing",
            "billing.ledger",
            "",
        )),
        "billing.credits",
    );
    let changed_source_span = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after successful payment.",
            "team.graph",
            "team-billing",
            "billing.ledger",
            "Intro paragraph.\n\n",
        )),
        "billing.credits",
    );
    let changed_fields = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after successful payment.",
            "team.graph",
            "team-risk",
            "billing.ledger",
            "",
        )),
        "billing.credits",
    );
    let changed_relations = object_hash(
        &build_graph_value(&hash_source(
            "Credits apply after successful payment.",
            "team.graph",
            "team-billing",
            "billing.source",
            "",
        )),
        "billing.credits",
    );

    assert_ne!(base, changed_body);
    assert_ne!(base, changed_page);
    assert_ne!(base, changed_source_span);
    assert_ne!(base, changed_fields);
    assert_ne!(base, changed_relations);
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
    let graph: Value = serde_json::from_str(&artifacts.graph_json).expect("graph artifact is JSON");
    assert_eq!(graph["schema_version"], "adoc.graph.v5");
    assert!(
        !artifacts.graph_json.contains("\"html\""),
        "graph artifact must not serialize HTML fragments: {}",
        artifacts.graph_json
    );
    assert_eq!(
        graph
            .get("nodes")
            .and_then(Value::as_array)
            .expect("nodes is an array")
            .iter()
            .filter(|node| node["type"] == "knowledge_object")
            .map(|node| node["id"].as_str().expect("node id"))
            .collect::<Vec<_>>(),
        ["billing.alpha", "billing.beta", "billing.gamma"]
    );
    assert_eq!(
        graph
            .get("edges")
            .and_then(Value::as_array)
            .expect("edges is an array")
            .iter()
            .filter(|edge| edge["kind"] == "relation")
            .map(|edge| (
                edge["source"].as_str().expect("source"),
                edge["relation"].as_str(),
                edge["target"].as_str().expect("target")
            ))
            .collect::<Vec<_>>(),
        [
            ("billing.beta", Some("depends_on"), "billing.alpha"),
            ("billing.beta", Some("related_to"), "billing.gamma"),
            ("billing.gamma", Some("supersedes"), "billing.alpha"),
        ]
    );
    assert!(
        graph
            .get("edges")
            .and_then(Value::as_array)
            .expect("edges is an array")
            .iter()
            .any(|edge| edge["kind"] == "contains"),
        "graph should include content containment edges"
    );
    assert!(
        artifacts.search_json.is_none(),
        "graph artifact is emitted even when embeddings are skipped"
    );
}

#[test]
fn load_graph_session_rejects_missing_malformed_and_unsupported_artifacts() {
    let missing = load_graph_session(GraphInput {
        graph_artifact_path: PathBuf::from("/tmp/adoc-missing-docs.graph.json"),
    });
    assert!(missing.session.is_none());
    assert_eq!(
        missing.diagnostics[0].code,
        DiagnosticCode::IoArtifactMissing
    );

    let malformed_artifact = write_temp_artifact("malformed", ".graph.json", "{");
    let malformed = load_graph_session(GraphInput {
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
        r#"{"schema_version":"adoc.graph.v99","nodes":[],"edges":[],"diagnostics":[]}"#,
    );
    let unsupported = load_graph_session(GraphInput {
        graph_artifact_path: unsupported_artifact.path().to_path_buf(),
    });
    assert!(unsupported.session.is_none());
    assert_eq!(
        unsupported.diagnostics[0].code,
        DiagnosticCode::SchemaUnsupportedVersion
    );
}

#[test]
fn graph_traversal_is_full_reachable_and_marks_cycle_edges_without_revisiting_nodes() {
    let graph = graph_document(
        vec![
            graph_node("billing.a"),
            graph_node("billing.b"),
            graph_node("billing.c"),
            graph_node("billing.d"),
        ],
        vec![
            relation_edge("billing.a", GraphRelationKind::DependsOn, "billing.b"),
            relation_edge("billing.b", GraphRelationKind::DependsOn, "billing.c"),
            relation_edge("billing.c", GraphRelationKind::DependsOn, "billing.a"),
            relation_edge("billing.d", GraphRelationKind::RelatedTo, "billing.a"),
        ],
    );
    let session = load_session(graph);

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
    let graph = graph_document(
        vec![
            graph_node("billing.a"),
            graph_node("billing.b"),
            graph_node("billing.c"),
        ],
        vec![
            relation_edge("billing.b", GraphRelationKind::DependsOn, "billing.a"),
            relation_edge("billing.c", GraphRelationKind::RelatedTo, "billing.a"),
        ],
    );
    let session = load_session(graph);

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

// ── V6.5.1: v4 golden api node ───────────────────────────────────────────────

/// Pins the `adoc.graph.v5` api node shape: lifecycle-only `status`, method
/// and path in the hashed `fields` map, and no `severity`/`trust` carriers —
/// api is born under the ADR-0039 lifecycle-only rule.
#[test]
fn built_api_node_is_lifecycle_only_with_method_and_path_fields() {
    let graph = build_graph_value(
        "# Billing API @doc(team.billing-api)\n\
         \n\
         ::api billing.consume-credit\n\
         method: POST\n\
         path: /api/billing/credits/consume\n\
         status: verified\n\
         source: openapi/billing.yaml#/paths/~1credits~1consume\n\
         owner: backend-platform\n\
         verified_at: 2026-04-30\n\
         --\n\
         Consumes one or more credits for a completed generation job.\n\
         ::\n",
    );

    assert_eq!(graph["schema_version"], "adoc.graph.v5");

    let api = graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["kind"] == "api")
        .expect("graph contains the api node");

    assert_eq!(api["id"], "billing.consume-credit");
    assert_eq!(api["status"], "verified");
    assert_eq!(api["fields"]["method"], "POST");
    assert_eq!(api["fields"]["path"], "/api/billing/credits/consume");
    assert_eq!(api["fields"]["owner"], "backend-platform");
    assert_eq!(api["fields"]["verified_at"], "2026-04-30");
    assert!(
        api.get("severity").is_none() && api.get("trust").is_none(),
        "api nodes carry no severity/trust: {api}"
    );
    // Inline `source:` evidence lands in the typed evidence array.
    assert_eq!(api["evidence"][0]["kind"], "source_code");
    assert!(
        api["content_hash"]
            .as_str()
            .expect("content_hash")
            .starts_with("sha256:")
    );
}

/// V6.5.2: the PRD §13.9 observation example emits a v4 node with the
/// `observed` lifecycle status, sample_size/observed_at in the hashed
/// `fields` map, inline `source:` as typed evidence, and no `severity`/`trust`
/// carriers — observation is born under the ADR-0039 lifecycle-only rule.
#[test]
fn built_observation_node_is_lifecycle_only_with_sample_size_and_observed_at_fields() {
    let graph = build_graph_value(
        "# Onboarding findings @doc(team.onboarding-findings)\n\
         \n\
         ::observation onboarding.credit-confusion\n\
         status: observed\n\
         source: support_tickets\n\
         sample_size: 37\n\
         observed_at: 2026-04-30\n\
         --\n\
         Users often misunderstand credit usage before their first generation.\n\
         ::\n",
    );

    assert_eq!(graph["schema_version"], "adoc.graph.v5");

    let observation = graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["kind"] == "observation")
        .expect("graph contains the observation node");

    assert_eq!(observation["id"], "onboarding.credit-confusion");
    assert_eq!(observation["status"], "observed");
    assert_eq!(observation["fields"]["sample_size"], "37");
    assert_eq!(observation["fields"]["observed_at"], "2026-04-30");
    assert!(
        observation.get("severity").is_none() && observation.get("trust").is_none(),
        "observation nodes carry no severity/trust: {observation}"
    );
    // Inline `source:` evidence lands in the typed evidence array.
    assert_eq!(observation["evidence"][0]["kind"], "source_code");
    assert_eq!(observation["evidence"][0]["value"], "support_tickets");
    assert!(
        observation["content_hash"]
            .as_str()
            .expect("content_hash")
            .starts_with("sha256:")
    );
}

// ── V6.5.3: question → answer edge ───────────────────────────────────────────

/// Pins the derived `resolved_by` edge: an answered question emits a
/// question → answering-claim edge (the evidence-edge precedent) so
/// traversal can walk from the question to the knowledge that answered it.
#[test]
fn built_answered_question_emits_resolved_by_edge_to_answering_claim() {
    let graph = build_graph_value(
        "# Billing Questions @doc(team.billing-questions)\n\
         \n\
         ::claim billing.trial-credit-decision\n\
         status: draft\n\
         --\n\
         Unused trial credits expire after 30 days.\n\
         ::\n\
         \n\
         ::question billing.trial-credit-expiration\n\
         owner: product-growth\n\
         status: answered\n\
         resolved_by: billing.trial-credit-decision\n\
         --\n\
         Should unused trial credits expire after 30 days or remain available indefinitely?\n\
         ::\n",
    );

    let question = graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["kind"] == "question")
        .expect("graph contains the question node");
    assert_eq!(question["id"], "billing.trial-credit-expiration");
    assert_eq!(question["status"], "answered");
    assert_eq!(
        question["fields"]["resolved_by"],
        "billing.trial-credit-decision"
    );

    let edge = graph["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .find(|edge| edge["kind"] == "resolved_by")
        .expect("graph contains the resolved_by edge");
    assert_eq!(edge["source"], "billing.trial-credit-expiration");
    assert_eq!(edge["target"], "billing.trial-credit-decision");
}

// ── V6.5.4: v4 golden task node ──────────────────────────────────────────────

/// Pins the `adoc.graph.v5` task node shape: lifecycle-only `status`, owner
/// and due in the hashed `fields` map, and no `severity`/`trust` carriers —
/// task is born under the ADR-0039 lifecycle-only rule. The PRD §13.11
/// `depends_on` relation emits a graph edge.
#[test]
fn built_task_node_is_lifecycle_only_with_owner_and_due_fields() {
    let graph = build_graph_value(
        "# Billing Tasks @doc(team.billing-tasks)\n\
         \n\
         ::claim billing.credits.refund-on-failed-persistence\n\
         status: plain\n\
         --\n\
         Credits are refunded when persistence fails after generation.\n\
         ::\n\
         \n\
         ::task billing.update-support-runbook\n\
         owner: support-ops\n\
         status: open\n\
         due: 2026-05-20\n\
         depends_on: billing.credits.refund-on-failed-persistence\n\
         --\n\
         Update the support runbook to mention refund behavior after persistence failure.\n\
         ::\n",
    );

    assert_eq!(graph["schema_version"], "adoc.graph.v5");

    let task = graph["nodes"]
        .as_array()
        .expect("nodes array")
        .iter()
        .find(|node| node["kind"] == "task")
        .expect("graph contains the task node");

    assert_eq!(task["id"], "billing.update-support-runbook");
    assert_eq!(task["status"], "open");
    assert_eq!(task["fields"]["owner"], "support-ops");
    assert_eq!(task["fields"]["due"], "2026-05-20");
    assert!(
        task.get("severity").is_none() && task.get("trust").is_none(),
        "task nodes carry no severity/trust: {task}"
    );
    assert!(
        task["content_hash"]
            .as_str()
            .expect("content_hash")
            .starts_with("sha256:")
    );

    let has_depends_on_edge = graph["edges"]
        .as_array()
        .expect("edges array")
        .iter()
        .any(|edge| {
            edge["relation"] == "depends_on"
                && edge["source"] == "billing.update-support-runbook"
                && edge["target"] == "billing.credits.refund-on-failed-persistence"
        });
    assert!(has_depends_on_edge, "expected the task depends_on edge");
}

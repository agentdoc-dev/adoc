use std::path::Path;

use adoc_local::{
    ContradictionsInput, GraphInput, ImpactedChangedSet, ImpactedInput, LocalContext, StaleInput,
    UnrestrictedPathPolicy,
};
use serde_json::{Value, json};

fn responses(root: &Path, artifact: &Path) -> Value {
    let context = LocalContext::new(root.to_path_buf(), UnrestrictedPathPolicy);
    json!({
        "graph": context.graph(GraphInput {
            object_id: "billing.hidden".into(), artifact: Some(artifact.into()),
            relation: None, direction: None,
        }).unwrap(),
        "stale": context.stale(StaleInput {
            artifact: Some(artifact.into()), within_days: None,
        }).unwrap(),
        "contradictions": context.contradictions(ContradictionsInput {
            artifact: Some(artifact.into()), all: true,
        }).unwrap(),
        "impacted": context.impacted(ImpactedInput {
            artifact: Some(artifact.into()),
            changed: ImpactedChangedSet::Paths(vec!["src/billing.rs".into()]),
        }).unwrap(),
    })
}

#[test]
fn graph_and_signals_honor_policy_with_explicit_artifacts() {
    let workspace = tempfile::tempdir().unwrap();
    let root = workspace.path();
    let artifact = root.join("graph.json");
    let config = root.join("agentdoc.config.yaml");
    std::fs::write(&config, "version: 1\nmode: strict\ndocs_path: docs\nretrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n  excluded_object_ids: [billing.hidden, billing.impacted]\n").unwrap();
    let hidden = json!({
        "type": "knowledge_object", "id": "billing.hidden", "kind": "contradiction",
        "content_hash": "sha256:hidden", "status": "unresolved", "severity": "high",
        "body": "Hidden source evidence", "page_id": "billing.page",
        "source_span": {"path": "docs/billing.adoc", "line": 1, "column": 1},
        "fields": {"expires_at": "2000-01-01"},
        "contradiction_claims": ["billing.visible"],
        "relations": {"depends_on": [], "supersedes": [], "related_to": []}
    });
    let mut impacted = hidden.clone();
    impacted["id"] = json!("billing.impacted");
    impacted["kind"] = json!("claim");
    impacted["status"] = json!("verified");
    impacted
        .as_object_mut()
        .unwrap()
        .remove("contradiction_claims");
    impacted["impacts"] = json!(["src/billing.rs"]);
    let document = |nodes| {
        json!({
            "schema_version": "adoc.graph.v6", "repository_identity": null,
            "nodes": nodes, "edges": [], "diagnostics": []
        })
    };
    std::fs::write(&artifact, document(vec![hidden, impacted]).to_string()).unwrap();
    let present = responses(root, &artifact);
    std::fs::write(&config, "version: 1\nmode: strict\ndocs_path: docs\n").unwrap();
    let unrestricted = responses(root, &artifact);
    assert_eq!(unrestricted["graph"]["exit_code"], 0);
    assert_eq!(
        unrestricted["stale"]["envelope"]["records"]
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        unrestricted["contradictions"]["envelope"]["contradictions"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_eq!(
        unrestricted["impacted"]["envelope"]["impacted"]
            .as_array()
            .unwrap()
            .len(),
        1
    );
    assert_ne!(
        present, unrestricted,
        "policy removal must restore the carriers"
    );
    std::fs::write(&artifact, document(vec![]).to_string()).unwrap();
    let absent = responses(root, &artifact);
    assert_eq!(present, absent);
}

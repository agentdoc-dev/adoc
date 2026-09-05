use std::path::PathBuf;

use adoc_core::{
    DiagnosticCode, RetrievalInput, RetrievalLoadResult, Severity, load_retrieval_session,
};
use serde_json::{Value, json};

const SENTINEL: &str = "raw-private-decode-payload";

fn load_bad_artifact(search: bool, unsupported: bool) -> (PathBuf, RetrievalLoadResult) {
    let workspace = tempfile::tempdir().unwrap();
    let graph_path = workspace.path().join("caller-selected.graph.json");
    let search_path = workspace.path().join("caller-selected.search.json");
    let graph = json!({
        "schema_version": "adoc.graph.v6", "repository_identity": null,
        "nodes": [], "edges": [], "diagnostics": []
    });
    std::fs::write(&graph_path, graph.to_string()).unwrap();
    let mut document = if search {
        json!({
            "schema_version": "adoc.search.v2",
            "model": {"id": "fixture", "provider": "fixture", "dim": 1},
            "graph_artifact_hash": "sha256:fixture", "embeddings": []
        })
    } else {
        graph
    };
    if unsupported {
        document["schema_version"] = json!(SENTINEL);
    } else {
        document[if search { "model" } else { "nodes" }] = json!(SENTINEL);
    }
    let selected_path = if search { &search_path } else { &graph_path };
    std::fs::write(selected_path, document.to_string()).unwrap();
    let loaded = load_retrieval_session(RetrievalInput {
        artifact_path: graph_path.clone(),
        search_artifact_path: search.then_some(search_path.clone()),
        policy: None,
    });
    if search {
        assert!(!loaded.session.as_ref().unwrap().has_semantic_index());
    } else {
        assert!(loaded.session.is_none());
    }
    (selected_path.clone(), loaded)
}

fn assert_safe_diagnostic(search: bool, unsupported: bool, expected_help: &str) {
    let (path, loaded) = load_bad_artifact(search, unsupported);
    let [diagnostic] = loaded.diagnostics.as_slice() else {
        panic!("expected one reader diagnostic: {:?}", loaded.diagnostics);
    };
    assert_eq!(diagnostic.severity, Severity::Error);
    assert_eq!(
        diagnostic.code,
        if unsupported {
            DiagnosticCode::SchemaUnsupportedVersion
        } else {
            DiagnosticCode::IoArtifactMalformed
        },
    );
    let encoded: Value = serde_json::to_value(diagnostic).unwrap();
    assert!(!encoded.to_string().contains(SENTINEL), "{encoded}");
    assert_eq!(
        (
            diagnostic.message.contains(path.to_str().unwrap()),
            diagnostic.help.as_deref()
        ),
        (true, Some(expected_help)),
        "retain the caller-selected path and trusted reader remediation: {encoded}",
    );
}

#[test]
fn malformed_graph_names_selected_path_and_preserves_graph_rebuild_help() {
    assert_safe_diagnostic(
        false,
        false,
        "Rebuild docs.graph.json from the source workspace.",
    );
}

#[test]
fn malformed_search_names_selected_path_and_preserves_search_rebuild_help() {
    assert_safe_diagnostic(
        true,
        false,
        "Rebuild docs.search.json from the source workspace.",
    );
}

#[test]
fn unsupported_graph_names_selected_path_and_preserves_safe_version_guidance() {
    assert_safe_diagnostic(
        false,
        true,
        &format!(
            "Expected schema_version 'adoc.graph.v6'. {}",
            DiagnosticCode::SchemaUnsupportedVersion.default_help(),
        ),
    );
}

#[test]
fn unsupported_search_names_selected_path_and_preserves_safe_version_guidance() {
    assert_safe_diagnostic(
        true,
        true,
        "Expected schema_version 'adoc.search.v2'. Rebuild the artifact with `adoc build`.",
    );
}

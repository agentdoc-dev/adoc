use std::fs;
use std::path::Path;

use adoc_core::{
    ArtifactLoadStatus, BuildEmbeddingMode, BuildInput, DiagnosticCode, EmbeddingProviderSelection,
    GraphArtifactInspectionInput, SearchArtifactInspectionInput, Severity,
    build_workspace_with_embedding_provider, inspect_graph_artifact, inspect_search_artifact,
};
use serde_json::json;

fn write(path: &Path, contents: &str) {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).expect("parent directory can be created");
    }
    fs::write(path, contents).expect("file can be written");
}

fn valid_source() -> &'static str {
    "# Billing @doc(team.billing)\n\n::claim billing.ready\nstatus: draft\n--\nBilling docs are ready.\n::\n"
}

fn valid_graph_json() -> String {
    serde_json::to_string_pretty(&json!({
        "schema_version": "adoc.graph.v3",
        "nodes": [
            {
                "type": "page",
                "id": "team.billing",
                "order": 0,
                "source_path": "docs/billing.adoc"
            },
            {
                "type": "knowledge_object",
                "id": "billing.ready",
                "kind": "claim",
                "content_hash": "sha256:billing.ready",
                "status": "draft",
                "body": "Billing docs are ready.",
                "page_id": "team.billing",
                "source_span": {
                    "path": "docs/billing.adoc",
                    "line": 3,
                    "column": 1
                },
                "fields": {},
                "relations": {
                    "depends_on": [],
                    "supersedes": [],
                    "related_to": []
                }
            }
        ],
        "edges": [],
        "diagnostics": []
    }))
    .expect("graph json serializes")
}

fn build_valid_artifacts(root: &Path) -> (std::path::PathBuf, std::path::PathBuf) {
    write(&root.join("docs/billing.adoc"), valid_source());
    let result = build_workspace_with_embedding_provider(
        BuildInput {
            root: root.join("docs"),
            embeddings: BuildEmbeddingMode::Enabled,
            prior_search_artifact_path: None,
        },
        EmbeddingProviderSelection::Deterministic,
    );
    assert!(
        !result.has_errors(),
        "fixture build should pass: {:?}",
        result.diagnostics
    );
    let artifacts = result.artifacts.expect("artifacts");
    let graph_path = root.join("dist/docs.graph.json");
    let search_path = root.join("dist/docs.search.json");
    write(&graph_path, &artifacts.graph_json);
    write(
        &search_path,
        artifacts.search_json.as_deref().expect("search artifact"),
    );
    (graph_path, search_path)
}

#[test]
fn graph_artifact_inspector_reports_missing_malformed_and_unsupported() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();

    let missing = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: root.join("missing.graph.json"),
    });
    assert_eq!(missing.load_status, ArtifactLoadStatus::Missing);
    assert!(!missing.exists);

    let malformed_path = root.join("malformed.graph.json");
    write(&malformed_path, "{");
    let malformed = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: malformed_path,
    });
    assert_eq!(malformed.load_status, ArtifactLoadStatus::Malformed);
    assert!(
        malformed
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::IoArtifactMalformed)
    );

    let unsupported_path = root.join("unsupported.graph.json");
    let unsupported_json = valid_graph_json().replace("adoc.graph.v3", "adoc.graph.v99");
    write(&unsupported_path, &unsupported_json);
    let unsupported = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: unsupported_path,
    });
    assert_eq!(
        unsupported.load_status,
        ArtifactLoadStatus::UnsupportedVersion
    );
    assert_eq!(
        unsupported.schema_version.as_deref(),
        Some("adoc.graph.v99")
    );
}

#[test]
fn graph_artifact_inspector_rejects_invalid_object_ids_and_counts_valid_objects() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    let valid_path = root.join("valid.graph.json");
    write(&valid_path, &valid_graph_json());

    let valid = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: valid_path,
    });
    assert_eq!(valid.load_status, ArtifactLoadStatus::Readable);
    assert_eq!(valid.schema_version.as_deref(), Some("adoc.graph.v3"));
    assert_eq!(valid.object_count, Some(1));

    let invalid_path = root.join("invalid.graph.json");
    write(
        &invalid_path,
        &valid_graph_json().replace("billing.ready", "Billing.Ready"),
    );
    let invalid = inspect_graph_artifact(GraphArtifactInspectionInput {
        graph_artifact_path: invalid_path,
    });
    assert_eq!(invalid.load_status, ArtifactLoadStatus::Malformed);
    assert!(
        invalid
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::IdInvalid)
    );
}

#[test]
fn search_artifact_inspector_reports_missing_malformed_and_unsupported() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    let (graph_path, _) = build_valid_artifacts(root);

    let missing = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path.clone(),
        search_artifact_path: Some(root.join("missing.search.json")),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(missing.load_status, ArtifactLoadStatus::Missing);

    let malformed_path = root.join("malformed.search.json");
    write(&malformed_path, "{");
    let malformed = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path.clone(),
        search_artifact_path: Some(malformed_path),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(malformed.load_status, ArtifactLoadStatus::Malformed);

    let unsupported_path = root.join("unsupported.search.json");
    write(
        &unsupported_path,
        &json!({
            "schema_version": "adoc.search.v99",
            "model": { "id": "hash-v1", "provider": "deterministic", "dim": 384 },
            "graph_artifact_hash": "sha256:graph",
            "embeddings": []
        })
        .to_string(),
    );
    let unsupported = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path,
        search_artifact_path: Some(unsupported_path),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(
        unsupported.load_status,
        ArtifactLoadStatus::UnsupportedVersion
    );
    assert_eq!(
        unsupported.schema_version.as_deref(),
        Some("adoc.search.v99")
    );
}

#[test]
fn search_artifact_inspector_validates_model_hash_and_deterministic_quality() {
    let workspace = tempfile::tempdir().expect("workspace");
    let root = workspace.path();
    let (graph_path, search_path) = build_valid_artifacts(root);

    let valid = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path.clone(),
        search_artifact_path: Some(search_path.clone()),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(valid.load_status, ArtifactLoadStatus::Readable);
    assert_eq!(valid.schema_version.as_deref(), Some("adoc.search.v0"));
    assert_eq!(valid.object_count, Some(1));
    assert!(
        valid.diagnostics.iter().any(|diagnostic| {
            diagnostic.code == DiagnosticCode::SearchDeterministicQuality
                && diagnostic.severity == Severity::Warning
        }),
        "deterministic readiness should warn about quality: {:?}",
        valid.diagnostics
    );

    let mut mismatch_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&search_path).expect("search readable"))
            .expect("search json parses");
    mismatch_json["model"] =
        json!({ "id": "bge-small-en-v1.5", "provider": "fastembed", "dim": 384 });
    let mismatch_path = root.join("dist/model-mismatch.search.json");
    write(
        &mismatch_path,
        &serde_json::to_string_pretty(&mismatch_json).expect("json serializes"),
    );
    let mismatch = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path.clone(),
        search_artifact_path: Some(mismatch_path),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(mismatch.load_status, ArtifactLoadStatus::Unreadable);
    assert!(
        mismatch
            .diagnostics
            .iter()
            .any(|diagnostic| diagnostic.code == DiagnosticCode::SearchModelMismatch)
    );

    let mut drift_json: serde_json::Value =
        serde_json::from_str(&fs::read_to_string(&search_path).expect("search readable"))
            .expect("search json parses");
    drift_json["graph_artifact_hash"] = json!("sha256:stale");
    let drift_path = root.join("dist/hash-drift.search.json");
    write(
        &drift_path,
        &serde_json::to_string_pretty(&drift_json).expect("json serializes"),
    );
    let drift = inspect_search_artifact(SearchArtifactInspectionInput {
        graph_artifact_path: graph_path,
        search_artifact_path: Some(drift_path),
        embedding_provider: Some(EmbeddingProviderSelection::Deterministic),
    });
    assert_eq!(drift.load_status, ArtifactLoadStatus::Readable);
    assert!(drift.diagnostics.iter().any(|diagnostic| {
        diagnostic.code == DiagnosticCode::SearchHashDrift
            && diagnostic.severity == Severity::Warning
    }));
}

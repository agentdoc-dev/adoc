use std::path::PathBuf;

use adoc_core::{DiagnosticCode, RetrievalInput, Severity, load_retrieval_session};
use serial_test::serial;

fn fixture(name: &str) -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/v1_4_semantic")
        .join(name)
}

fn force_in_memory_provider() {
    // Serialized via #[serial(env_provider)] on each test below; safe
    // because no other test reads/writes ADOC_TEST_EMBEDDING_PROVIDER
    // under the same lock.
    unsafe { std::env::set_var("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory") };
}

#[test]
#[serial(env_provider)]
fn missing_search_artifact_warns_and_disables_semantic() {
    force_in_memory_provider();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture("hash_drift.agent.json"),
        search_artifact_path: Some(fixture("not_present.search.json")),
        graph_artifact_path: None,
    });
    let session = result.session.expect("session loads without semantic");
    assert!(!session.has_semantic_index());
    assert!(
        result
            .diagnostics
            .iter()
            .any(|d| d.code == DiagnosticCode::SearchArtifactMissing),
        "expected SearchArtifactMissing; got {:?}",
        result.diagnostics
    );
}

#[test]
#[serial(env_provider)]
fn mismatched_model_emits_error_and_disables_semantic() {
    force_in_memory_provider();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture("hash_drift.agent.json"),
        search_artifact_path: Some(fixture("model_mismatch.search.json")),
        graph_artifact_path: None,
    });
    let session = result.session.expect("session still loads for lexical");
    assert!(
        !session.has_semantic_index(),
        "mismatched model must disable semantic"
    );
    let mismatch = result
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::SearchModelMismatch)
        .expect("SearchModelMismatch must be emitted");
    assert_eq!(mismatch.severity, Severity::Error);
}

#[test]
#[serial(env_provider)]
fn hash_drift_warns_but_keeps_semantic_index_loaded() {
    force_in_memory_provider();
    let result = load_retrieval_session(RetrievalInput {
        artifact_path: fixture("hash_drift.agent.json"),
        search_artifact_path: Some(fixture("hash_drift.search.json")),
        graph_artifact_path: None,
    });
    let session = result.session.expect("session loads");
    assert!(
        session.has_semantic_index(),
        "drift must NOT disable semantic"
    );
    let drift = result
        .diagnostics
        .iter()
        .find(|d| d.code == DiagnosticCode::SearchHashDrift)
        .expect("SearchHashDrift must be emitted");
    assert_eq!(drift.severity, Severity::Warning);
}

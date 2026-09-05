mod support;

use std::fs;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use support::v1_4::build_v1_4_pilot;

#[test]
fn semantic_search_warns_on_partial_staleness_and_refuses_fully_stale_vectors() {
    let pilot = build_v1_4_pilot();
    let run = |artifact: &Path, semantic| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
        command
            .current_dir(&pilot._workspace.root)
            .args(["search", "credits", "--format", "json", "--artifact"])
            .arg(artifact)
            .arg("--search-artifact")
            .arg(&pilot.search_path)
            .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
        if semantic {
            command.arg("--semantic");
        }
        command.output().expect("semantic search runs")
    };
    let fresh = run(&pilot.artifact_path, true);
    assert!(fresh.status.success(), "{fresh:?}");
    let fresh: Value = serde_json::from_slice(&fresh.stdout).expect("retrieval JSON");
    assert!(!fresh["records"].as_array().unwrap().is_empty());

    // Rebuild graphs from changed source while retaining the real old search
    // artifact. All three objects remain public throughout.
    let source_path = pilot._workspace.root.join("billing.adoc");
    let source = fs::read_to_string(&source_path).expect("source readable");
    assert_eq!(source.matches("--\n").count(), 3);
    let rebuild = |contents: &str, directory: &str| {
        fs::write(&source_path, contents).expect("update object bodies");
        let changed_dir = pilot._workspace.root.join(directory);
        let build = Command::new(env!("CARGO_BIN_EXE_adoc"))
            .current_dir(&pilot._workspace.root)
            .arg("build")
            .arg(&source_path)
            .arg("--out")
            .arg(&changed_dir)
            .arg("--no-embeddings")
            .output()
            .expect("graph rebuild runs");
        assert!(build.status.success(), "{build:?}");
        changed_dir.join("docs.graph.json")
    };

    let partial_graph = rebuild(
        &source.replacen("--\n", "--\nUpdated policy. ", 1),
        "partial-dist",
    );
    let partial = run(&partial_graph, true);
    assert!(partial.status.success(), "{partial:?}");
    let partial: Value = serde_json::from_slice(&partial.stdout).expect("retrieval JSON");
    let records = partial["records"].as_array().unwrap();
    assert!(!records.is_empty(), "{partial}");
    let changed_id = "billing.credits.ledger-source";
    assert!(
        records.iter().all(|record| record["id"] != changed_id),
        "{partial}"
    );
    let hybrid = run(&partial_graph, false);
    assert!(hybrid.status.success(), "{hybrid:?}");
    let hybrid: Value = serde_json::from_slice(&hybrid.stdout).expect("retrieval JSON");
    assert!(
        hybrid["records"].as_array().unwrap().iter().any(|record| {
            record["id"] == changed_id
                && record["match"]["lexical_rank"].is_number()
                && record["match"]["vector_rank"].is_null()
        }),
        "{hybrid}"
    );
    let warning = partial["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .find(|diagnostic| diagnostic["code"] == "search.hash_drift")
        .expect("stale vectors produce a drift warning");
    assert_eq!(warning["severity"], "warning");
    assert!(
        warning["message"]
            .as_str()
            .unwrap()
            .to_lowercase()
            .contains("semantic results may be incomplete"),
        "{warning}"
    );

    let changed_graph = rebuild(
        &source.replace("--\n", "--\nUpdated policy. "),
        "changed-dist",
    );
    let hybrid = run(&changed_graph, false);
    assert!(hybrid.status.success(), "{hybrid:?}");
    let hybrid: Value = serde_json::from_slice(&hybrid.stdout).expect("retrieval JSON");
    let records = hybrid["records"].as_array().unwrap();
    assert!(!records.is_empty(), "{hybrid}");
    assert!(
        records
            .iter()
            .all(|record| record["match"]["mode"] == "lexical"),
        "{hybrid}"
    );
    assert!(
        hybrid["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .all(|diagnostic| diagnostic["severity"] != "error"),
        "{hybrid}"
    );

    let stale = run(&changed_graph, true);
    assert_eq!(stale.status.code(), Some(2), "{stale:?}");
    let stale: Value = serde_json::from_slice(&stale.stdout).expect("retrieval JSON");
    assert!(stale["records"].as_array().unwrap().is_empty(), "{stale}");
    assert!(
        stale["diagnostics"]
            .as_array()
            .unwrap()
            .iter()
            .any(|diagnostic| {
                diagnostic["code"] == "search.artifact_missing" && diagnostic["severity"] == "error"
            }),
        "{stale}"
    );
}

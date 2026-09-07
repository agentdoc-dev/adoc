mod support;

use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command};

#[test]
fn managed_input_is_explicit_and_ignores_ambient_project_authority() {
    let workspace = TestWorkspace::new("managed-retrieval");
    workspace.write("agentdoc.config.yaml", "INVALID AMBIENT CONFIG");
    workspace.write("input.json", &json!({
        "schema_version": "adoc.managed_retrieval_input.v0",
        "workspace_id": "workspace-test",
        "policy": {"audience": "public", "allowed_visibilities": ["public"], "excluded_object_ids": []},
        "receipts": [], "objects": []
    }).to_string());
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "managed-retrieve",
            "--require-sensitive-classification",
            "--require-field-projection",
            "--input",
            "input.json",
            "--manifest-out",
            "contributors.json",
            "search",
            "billing",
            "--mode",
            "lexical",
        ])
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let body: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(
        body,
        json!({"schema_version":"adoc.retrieval.v1", "records":[], "diagnostics":[]})
    );
    assert_eq!(
        std::fs::read_to_string(workspace.root.join("contributors.json")).unwrap(),
        "[]"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            std::fs::metadata(workspace.root.join("contributors.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    let original = std::fs::read(workspace.root.join("input.json")).unwrap();
    let collision = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "managed-retrieve",
            "--input",
            "input.json",
            "--manifest-out",
            "input.json",
            "search",
            "billing",
        ])
        .output()
        .unwrap();
    assert_eq!(collision.status.code(), Some(2));
    assert_eq!(
        std::fs::read(workspace.root.join("input.json")).unwrap(),
        original
    );
    let refusal: Value = serde_json::from_slice(&collision.stdout).unwrap();
    assert_eq!(refusal["records"], json!([]));
}

#[test]
fn unavailable_or_malformed_managed_input_never_echoes_private_data() {
    let workspace = TestWorkspace::new("managed-retrieval-refusal");
    workspace.write("private-input.json", "SECRET_MANAGED_INPUT_NOT_JSON");
    for path in ["private-input.json", "MISSING_PRIVATE_PATH"] {
        let output = adoc_command()
            .current_dir(&workspace.root)
            .args(["managed-retrieve", "--input", path, "why", "billing.credit"])
            .output()
            .unwrap();
        assert_eq!(output.status.code(), Some(2));
        let body: Value = serde_json::from_slice(&output.stdout).unwrap();
        assert_eq!(body["records"], json!([]));
        assert_eq!(
            body["diagnostics"][0]["code"],
            "retrieval.visibility_unavailable"
        );
        let observable = format!(
            "{}{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(!observable.contains(path));
        assert!(!observable.contains("SECRET_MANAGED_INPUT"));
    }
}

#[test]
fn managed_cli_projects_real_compiler_fields_and_writes_exact_access_manifest() {
    use adoc_core::{
        BuildEmbeddingMode, BuildInput, ManagedRetrievalQuery, SearchMode, build_workspace,
        run_managed_retrieval, semantic_context_content_digest,
    };
    use std::num::NonZeroUsize;
    let workspace = TestWorkspace::new("managed-field-projection");
    workspace.write("source.adoc", "# Policy @doc(team.policy)\n\n::claim policy.example\nstatus: draft\nowner: OWNERPHOTONCANARY\n--\nRetained billing knowledge.\n::\n");
    let built = build_workspace(BuildInput {
        root: workspace.root.join("source.adoc"),
        embeddings: BuildEmbeddingMode::Skipped,
        prior_search_artifact_path: None,
    });
    assert!(!built.has_errors(), "{:?}", built.diagnostics);
    let graph: Value = serde_json::from_str(&built.artifacts.unwrap().graph_json).unwrap();
    let node = graph["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["id"] == "policy.example")
        .unwrap();
    // Both exact strings use the shared deterministic JSON digest helper.
    let content = serde_json::to_string(node).unwrap();
    let digest = semantic_context_content_digest(node);
    let input = json!({
        "schema_version":"adoc.managed_retrieval_input.v0","workspace_id":"00000000-0000-4000-8000-000000000001",
        "policy":{"audience":"restricted","allowed_visibilities":["public","internal","restricted"],"excluded_object_ids":[]},
        "receipts":[{"id":"receipt","workspace_id":"00000000-0000-4000-8000-000000000001","graph_bytes":serde_json::to_string(&graph).unwrap(),"graph_digest":semantic_context_content_digest(&graph)}],
        "objects":[{"canonical":{"workspace_id":"00000000-0000-4000-8000-000000000001","canonical_id":"00000000-0000-4000-8000-000000000002"},"version_id":"00000000-0000-4000-8000-000000000003","object_id":"policy.example","receipt_id":"receipt","content_bytes":content,"content_digest":digest,
          "field_projection":{"workspace_id":"00000000-0000-4000-8000-000000000001","canonical_id":"00000000-0000-4000-8000-000000000002","version_id":"00000000-0000-4000-8000-000000000003","content_digest":digest,"fields":[{"selector":"/body","classification":"internal"},{"selector":"/fields/owner","classification":null}]}}]
    });
    workspace.write("input.json", &input.to_string());
    for (index, (args, query)) in [
        (
            vec!["search", "billing", "--mode", "lexical"],
            ManagedRetrievalQuery::Search {
                text: "billing".into(),
                mode: SearchMode::Lexical,
                top: NonZeroUsize::new(20).unwrap(),
            },
        ),
        (
            vec!["why", "policy.example"],
            ManagedRetrievalQuery::Why {
                object_id: "policy.example".into(),
            },
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let manifest = format!("manifest-{index}.json");
        let output = adoc_command()
            .current_dir(&workspace.root)
            .args([
                "managed-retrieve",
                "--require-field-projection",
                "--require-sensitive-classification",
                "--input",
                "input.json",
                "--manifest-out",
                &manifest,
            ])
            .args(args)
            .output()
            .unwrap();
        assert!(output.status.success(), "{:?}", output);
        let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
        let expected = run_managed_retrieval(input.to_string().as_bytes(), query);
        assert_eq!(envelope, serde_json::to_value(&expected.envelope).unwrap());
        assert!(envelope["records"][0].get("owner").is_none());
        assert_eq!(envelope["records"][0]["classification"], "internal");
        assert_eq!(envelope["records"][0]["content_hash"], node["content_hash"]);
        assert_eq!(
            std::fs::read(workspace.root.join(manifest)).unwrap(),
            serde_json::to_vec(&expected.contributing_bindings).unwrap()
        );
    }
}

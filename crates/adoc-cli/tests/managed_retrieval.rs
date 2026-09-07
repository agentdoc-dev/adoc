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

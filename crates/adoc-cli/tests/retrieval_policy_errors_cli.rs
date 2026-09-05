mod support;

use std::{fs, process::Command};

use serde_json::Value;
use support::{TestWorkspace, fixture_path};

const CONFIG: &str =
    "version: 1\nmode: strict\ndocs_path: .\noutputs:\n  graph: dist/docs.graph.json\n";

fn assert_refusal(workspace: &TestWorkspace, code: &str) {
    for (command, mode) in [
        ("search", Some("--lexical")),
        ("search", None),
        ("search", Some("--semantic")),
        ("why", None),
    ] {
        for explicit in [false, true] {
            for format in ["json", "plain"] {
                let mut process = Command::new(env!("CARGO_BIN_EXE_adoc"));
                process.current_dir(&workspace.root).args([
                    command,
                    "billing.refunds.issue-credit",
                    "--format",
                    format,
                ]);
                if let Some(mode) = mode {
                    process.arg(mode);
                }
                if explicit {
                    process.args(["--artifact", "dist/docs.graph.json"]);
                }
                let output = process.output().expect("CLI runs");
                assert_eq!(output.status.code(), Some(2), "{code}: {output:?}");
                for bytes in [&output.stdout, &output.stderr] {
                    let text = String::from_utf8_lossy(bytes);
                    assert!(!text.contains("private-sentinel"));
                    // Artifact diagnostics retain their resolved input path,
                    // whether supplied explicitly or through valid config.
                    if code != "retrieval.visibility_unavailable" {
                        let directory = workspace.root.file_name().unwrap().to_str().unwrap();
                        assert!(
                            !text.contains(directory),
                            "config diagnostic disclosed {text}"
                        );
                    }
                }
                if format == "json" {
                    let envelope: Value = serde_json::from_slice(&output.stdout).unwrap();
                    assert!(envelope["records"].as_array().unwrap().is_empty());
                    let diagnostics = envelope["diagnostics"].as_array().unwrap();
                    assert_eq!(diagnostics.len(), 1, "{envelope}");
                    assert_eq!(diagnostics[0]["code"], code, "{envelope}");
                    assert_eq!(diagnostics[0]["severity"], "error");
                } else {
                    assert!(output.stdout.is_empty());
                    assert!(
                        String::from_utf8_lossy(&output.stderr).contains(&format!("error[{code}]"))
                    );
                }
            }
        }
    }
}

#[test]
fn malformed_policy_refuses_every_retrieval_mode_with_typed_safe_output() {
    let workspace = TestWorkspace::new("retrieval-policy-errors");
    workspace.write(
        "dist/docs.graph.json",
        &fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json")).unwrap(),
    );
    for (policy, code) in [
        (
            "retrieval_policy: [private-sentinel\n",
            "retrieval.policy_invalid",
        ),
        ("retrieval_policy: null\n", "retrieval.policy_invalid"),
        (
            "retrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n  private-sentinel: true\n",
            "retrieval.policy_invalid",
        ),
        (
            "retrieval_policy:\n  allowed_visibilities: [public]\n",
            "retrieval.audience_unresolved",
        ),
        (
            "retrieval_policy:\n  audience: private-sentinel\n  allowed_visibilities: [public]\n",
            "retrieval.audience_unresolved",
        ),
    ] {
        workspace.write("agentdoc.config.yaml", &format!("{CONFIG}{policy}"));
        assert_refusal(&workspace, code);
    }
}

#[test]
fn invalid_utf8_policy_refuses_before_artifact_loading() {
    let workspace = TestWorkspace::new("retrieval-invalid-utf8");
    fs::write(workspace.root.join("agentdoc.config.yaml"), [0xff, 0xfe]).unwrap();
    assert_refusal(&workspace, "retrieval.policy_invalid");
}

#[test]
fn unreadable_policy_source_returns_a_typed_refusal() {
    let workspace = TestWorkspace::new("retrieval-unreadable-policy");
    fs::create_dir(workspace.root.join("agentdoc.config.yaml")).unwrap();
    assert_refusal(&workspace, "retrieval.policy_invalid");
}

#[test]
#[cfg(unix)]
fn dangling_policy_symlink_returns_a_typed_refusal() {
    let workspace = TestWorkspace::new("retrieval-dangling-policy");
    std::os::unix::fs::symlink(
        "missing-policy.yaml",
        workspace.root.join("agentdoc.config.yaml"),
    )
    .unwrap();
    assert_refusal(&workspace, "retrieval.policy_invalid");
}

#[test]
fn unclassifiable_artifacts_refuse_every_retrieval_mode_without_payload_text() {
    let workspace = TestWorkspace::new("retrieval-visibility-errors");
    workspace.write("agentdoc.config.yaml", CONFIG);
    let fixture = fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json")).unwrap();
    for (field, value) in [
        ("visibility", serde_json::json!(null)),
        ("visibility", serde_json::json!("private-sentinel")),
        (
            "field_visibility",
            serde_json::json!({"body": "private-sentinel"}),
        ),
    ] {
        let mut graph: Value = serde_json::from_str(&fixture).unwrap();
        let object = graph["nodes"]
            .as_array_mut()
            .unwrap()
            .iter_mut()
            .find(|node| node["type"] == "knowledge_object")
            .unwrap();
        object[field] = value;
        workspace.write("dist/docs.graph.json", &graph.to_string());
        assert_refusal(&workspace, "retrieval.visibility_unavailable");
    }
}

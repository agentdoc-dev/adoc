mod support;

use std::io::Write;
use std::process::Stdio;

use serde_json::Value;
use support::{TestWorkspace, adoc_command};

const INPUT: &str = include_str!("fixtures/portable_projection/input.json");

fn invoke(bytes: &[u8]) -> std::process::Output {
    let mut child = adoc_command()
        .arg("portable-project")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(bytes).unwrap();
    child.wait_with_output().unwrap()
}

#[test]
fn actual_cli_projects_exact_retained_fixture_and_strictly_round_trips() {
    let output = invoke(INPUT.as_bytes());
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let projection: Value = serde_json::from_slice(&output.stdout).unwrap();
    assert_eq!(projection["outcome"], "complete", "{projection}");
    assert_eq!(
        projection["objects"][0]["lifecycle_projection"]["status"],
        "draft"
    );
    let workspace = TestWorkspace::new("portable-projection-roundtrip");
    for document in projection["documents"].as_array().unwrap() {
        workspace.write(
            document["path"].as_str().unwrap(),
            document["adoc_source"].as_str().unwrap(),
        );
    }
    let checked = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "projections", "--as-of", "2026-09-08"])
        .output()
        .unwrap();
    assert!(
        checked.status.success(),
        "{}",
        String::from_utf8_lossy(&checked.stderr)
    );
    let mut partial: Value = serde_json::from_str(INPUT).unwrap();
    let mut bad = partial["objects"][0].clone();
    bad["node_digest"] = Value::String("sha256:bad".into());
    partial["objects"].as_array_mut().unwrap().push(bad);
    let output = invoke(&serde_json::to_vec(&partial).unwrap());
    assert!(output.status.success());
    assert_eq!(
        serde_json::from_slice::<Value>(&output.stdout).unwrap()["outcome"],
        "partial"
    );
}

#[test]
fn actual_cli_rejects_malformed_transport_without_echoing_input() {
    let output = invoke(b"private-source-text");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    assert!(!String::from_utf8_lossy(&output.stderr).contains("private-source-text"));
    let mut input: Value = serde_json::from_str(INPUT).unwrap();
    input["projection_version"] = Value::String("unregistered".into());
    let output = invoke(&serde_json::to_vec(&input).unwrap());
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
}

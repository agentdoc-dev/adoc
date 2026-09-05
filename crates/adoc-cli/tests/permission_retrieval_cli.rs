mod support;

use std::fs;
use std::process::Command;

use support::{TestWorkspace, fixture_path};

#[test]
fn carried_source_errors_refuse_search_and_why_without_raw_diagnostics() {
    use serde_json::{Value, json};

    let workspace = TestWorkspace::new("permission-carried-errors");
    let object = |id, visibility| {
        json!({
            "type": "knowledge_object", "id": id, "kind": "claim", "status": "draft",
            "content_hash": "sha256:0000000000000000000000000000000000000000000000000000000000000000",
            "body": "Shared credits.", "page_id": "billing",
            "source_span": {"path": "docs/billing.adoc", "line": 1, "column": 1},
            "visibility": visibility, "fields": {},
            "relations": {"depends_on": [], "supersedes": [], "related_to": []}
        })
    };
    workspace.write("dist/docs.graph.json", &json!({
        "schema_version": "adoc.graph.v6", "repository_identity": null,
        "nodes": [object("billing.public", "public"), object("billing.internal", "internal")],
        "edges": [], "diagnostics": [
            {"code": "schema.unknown_field", "severity": "error", "object_id": "billing.public",
             "message": "private-schema-sentinel billing.internal", "help": "private-schema-sentinel"},
            {"code": "schema.unknown_field", "severity": "error", "object_id": "billing.internal",
             "message": "private-schema-sentinel"}
        ]
    }).to_string());

    let run = |command, format| {
        let mut process = Command::new(env!("CARGO_BIN_EXE_adoc"));
        process.current_dir(&workspace.root).args([
            command,
            "billing.public",
            "--artifact",
            "dist/docs.graph.json",
            "--format",
            format,
        ]);
        if command == "search" {
            process.arg("--lexical");
        }
        process.output().expect("CLI runs")
    };
    for command in ["search", "why"] {
        let output = run(command, "json");
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let envelope: Value = serde_json::from_slice(&output.stdout).expect("retrieval JSON");
        let records = envelope["records"].as_array().expect("records");
        assert!(records.is_empty(), "{command} returned records: {envelope}");
        let diagnostics = envelope["diagnostics"].as_array().expect("diagnostics");
        assert_eq!(diagnostics.len(), 1, "{envelope}");
        assert_eq!(diagnostics[0]["code"], "retrieval.visibility_unavailable");
        assert_eq!(diagnostics[0]["severity"], "error");
        let help = diagnostics[0]["help"].as_str().expect("safe help");
        assert!(
            help.contains("adoc check") && help.contains("adoc build"),
            "{help}"
        );
        for text in [&output.stdout, &output.stderr] {
            let text = String::from_utf8_lossy(text);
            assert!(!text.contains("private-schema-sentinel"), "{text}");
            assert!(!text.contains("billing.internal"), "{text}");
        }
        let plain = run(command, "plain");
        assert_eq!(plain.status.code(), Some(2));
        let stdout = String::from_utf8_lossy(&plain.stdout);
        let stderr = String::from_utf8_lossy(&plain.stderr);
        assert!(
            stdout.is_empty(),
            "{command} returned plain records: {plain:?}"
        );
        assert_eq!(
            stderr
                .matches("error[retrieval.visibility_unavailable]")
                .count(),
            1
        );
        assert!(stderr.contains("adoc check") && stderr.contains("adoc build"));
        for text in [&stdout, &stderr] {
            assert!(
                !text.contains("billing.internal") && !text.contains("private-schema-sentinel")
            );
        }
    }

    // The same carried errors refuse an all-public corpus without policy too.
    let path = workspace.root.join("dist/docs.graph.json");
    let mut graph: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
    graph["nodes"][1]["visibility"] = json!("public");
    fs::write(path, graph.to_string()).unwrap();
    for command in ["search", "why"] {
        let output = run(command, "json");
        assert_eq!(output.status.code(), Some(2), "{output:?}");
        let envelope: Value = serde_json::from_slice(&output.stdout).expect("retrieval JSON");
        assert!(
            envelope["records"].as_array().unwrap().is_empty(),
            "{envelope}"
        );
        let diagnostics = envelope["diagnostics"].as_array().unwrap();
        assert_eq!(diagnostics.len(), 2, "{envelope}");
        assert!(
            diagnostics.iter().all(|diagnostic| {
                diagnostic["code"] == "schema.unknown_field" && diagnostic["severity"] == "error"
            }),
            "{envelope}"
        );
        let plain = run(command, "plain");
        assert_eq!(plain.status.code(), Some(2));
        assert!(plain.stdout.is_empty(), "{plain:?}");
    }
}

#[test]
fn project_policy_excludes_search_and_why_even_with_explicit_artifact() {
    let workspace = TestWorkspace::new("permission-retrieval");
    workspace.write(
        "dist/docs.graph.json",
        &fs::read_to_string(fixture_path("v1_1_why/valid_artifact.graph.json"))
            .expect("fixture readable"),
    );
    let config = "version: 1\nmode: strict\ndocs_path: .\nretrieval_policy:\n  audience: public\n  allowed_visibilities: [public]\n  excluded_object_ids: [billing.refunds.issue-credit]\n";
    workspace.write("agentdoc.config.yaml", config);

    // Transitional T1 behavior: graph still reads the unfiltered artifact.
    // Replace this control with policy exclusion when E6.1.T3 closes graph reads.
    let graph = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .args([
            "graph",
            "billing.refunds.issue-credit",
            "--artifact",
            "dist/docs.graph.json",
            "--format",
            "json",
        ])
        .output()
        .expect("graph CLI runs");
    assert!(graph.status.success(), "{graph:?}");
    let graph: serde_json::Value = serde_json::from_slice(&graph.stdout).expect("graph JSON");
    assert_eq!(graph["root"], "billing.refunds.issue-credit");
    assert!(
        graph["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|node| node["id"] == "billing.refunds.issue-credit"),
        "{graph}"
    );

    for command in ["search", "why"] {
        let run = |object_id| {
            let mut process = Command::new(env!("CARGO_BIN_EXE_adoc"));
            process.current_dir(&workspace.root).args([
                command,
                object_id,
                "--artifact",
                "dist/docs.graph.json",
                "--format",
                "json",
            ]);
            if command == "search" {
                process.arg("--lexical");
            }
            process.output().expect("CLI runs")
        };
        let denied = run("billing.refunds.issue-credit");
        assert_eq!(
            denied.status.code(),
            Some(if command == "why" { 3 } else { 0 })
        );
        let denied: serde_json::Value = serde_json::from_slice(&denied.stdout)
            .expect("policy refusal remains a structured retrieval response");
        assert!(
            denied["records"]
                .as_array()
                .expect("records")
                .iter()
                .all(|record| record["id"] != "billing.refunds.issue-credit")
        );
        let sibling = run("billing.refunds.fraud-window");
        assert!(sibling.status.success(), "{sibling:?}");
        let sibling: serde_json::Value = serde_json::from_slice(&sibling.stdout).expect("JSON");
        assert!(
            sibling["records"]
                .as_array()
                .unwrap()
                .iter()
                .any(|record| record["id"] == "billing.refunds.fraud-window")
        );
        fs::remove_file(workspace.root.join("agentdoc.config.yaml")).expect("remove policy");
        let allowed = run("billing.refunds.issue-credit");
        assert!(allowed.status.success(), "{:?}", allowed);
        let allowed: serde_json::Value = serde_json::from_slice(&allowed.stdout).expect("JSON");
        assert!(
            allowed["records"]
                .as_array()
                .expect("records")
                .iter()
                .any(|record| record["id"] == "billing.refunds.issue-credit")
        );
        workspace.write("agentdoc.config.yaml", config);
    }
}

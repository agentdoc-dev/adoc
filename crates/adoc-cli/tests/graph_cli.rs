mod support;

use std::process::Command;

use support::{TestWorkspace, adoc_command, stderr, stdout};

fn write_graph_source(workspace: &TestWorkspace) {
    workspace.write(
        "docs/graph.adoc",
        concat!(
            "# Graph @doc(team.graph)\n",
            "\n",
            "::claim billing.root\n",
            "status: draft\n",
            "depends_on: [billing.alpha, billing.gamma]\n",
            "--\n",
            "Root target.\n",
            "::\n",
            "\n",
            "::claim billing.alpha\n",
            "status: draft\n",
            "--\n",
            "Alpha target target.\n",
            "::\n",
            "\n",
            "::claim billing.beta\n",
            "status: draft\n",
            "--\n",
            "Beta target target target.\n",
            "::\n",
            "\n",
            "::claim billing.gamma\n",
            "status: draft\n",
            "related_to: billing.root\n",
            "--\n",
            "Gamma target.\n",
            "::\n",
        ),
    );
}

fn build_graph_workspace(workspace_name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(workspace_name);
    write_graph_source(&workspace);
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    workspace
}

#[test]
fn build_writes_graph_json_even_when_embeddings_are_skipped() {
    let workspace = build_graph_workspace("graph-build-output");

    assert!(workspace.root.join("dist/docs.html").is_file());
    assert!(workspace.root.join("dist/docs.graph.json").is_file());
    assert!(!workspace.root.join("dist/docs.agent.json").exists());
    assert!(!workspace.root.join("dist/docs.search.json").exists());

    let graph_text = std::fs::read_to_string(workspace.root.join("dist/docs.graph.json"))
        .expect("graph artifact is readable");
    let graph_json: serde_json::Value =
        serde_json::from_str(&graph_text).expect("graph artifact is JSON");
    assert_eq!(graph_json["schema_version"], "adoc.graph.v5");
    assert_eq!(
        graph_json["nodes"]
            .as_array()
            .expect("nodes array")
            .iter()
            .filter(|node| node["type"] == "knowledge_object")
            .count(),
        4
    );
    assert!(
        graph_json["edges"]
            .as_array()
            .expect("edges array")
            .iter()
            .any(|edge| edge["kind"] == "contains")
    );
    assert!(
        graph_json["edges"]
            .as_array()
            .expect("edges array")
            .iter()
            .any(|edge| edge["kind"] == "relation" && edge["relation"] == "depends_on")
    );
}

#[test]
fn graph_cli_renders_plain_traversal_from_compiled_artifacts() {
    let workspace = build_graph_workspace("graph-plain");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "graph",
            "billing.root",
            "--format",
            "plain",
            "--direction",
            "outgoing",
            "--relation",
            "depends_on",
        ])
        .output()
        .expect("adoc graph runs");

    assert!(
        output.status.success(),
        "expected graph command to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stderr(&output).is_empty());
    let stdout = stdout(&output);
    assert!(stdout.contains("Root: billing.root"));
    assert!(stdout.contains("Nodes:"));
    assert!(stdout.contains("- billing.root (distance 0, claim, draft)"));
    assert!(stdout.contains("- billing.alpha (distance 1, claim, draft)"));
    assert!(stdout.contains("- billing.gamma (distance 1, claim, draft)"));
    assert!(stdout.contains("Edges:"));
    assert!(stdout.contains("- billing.root --depends_on--> billing.alpha"));
    assert!(stdout.contains("- billing.root --depends_on--> billing.gamma"));
    assert!(!stdout.contains("billing.beta"));
}

#[test]
fn graph_cli_json_output_uses_graph_traversal_envelope() {
    let workspace = build_graph_workspace("graph-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["graph", "billing.root", "--format", "json"])
        .output()
        .expect("adoc graph runs");

    assert!(
        output.status.success(),
        "expected graph JSON to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let value: serde_json::Value = serde_json::from_slice(&output.stdout).expect("stdout is JSON");
    assert_eq!(value["schema_version"], "adoc.graph.traversal.v0");
    assert_eq!(value["root"], "billing.root");
    assert_eq!(value["diagnostics"], serde_json::json!([]));
    assert_eq!(value["nodes"][0]["id"], "billing.root");
    assert!(value["edges"][0]["relation"].is_string());
}

#[test]
fn graph_cli_exit_codes_distinguish_invalid_missing_and_unknown_roots() {
    let workspace = build_graph_workspace("graph-exit-codes");

    let invalid = adoc_command()
        .current_dir(&workspace.root)
        .args(["graph", "bad"])
        .output()
        .expect("adoc graph invalid runs");
    assert_eq!(invalid.status.code(), Some(1));
    assert!(stderr(&invalid).contains("error[id.invalid]"));

    let missing = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "graph",
            "billing.root",
            "--artifact",
            "dist/missing.graph.json",
        ])
        .output()
        .expect("adoc graph missing runs");
    assert_eq!(missing.status.code(), Some(2));
    assert!(stderr(&missing).contains("error[io.artifact_missing]"));

    let unknown = adoc_command()
        .current_dir(&workspace.root)
        .args(["graph", "billing.missing"])
        .output()
        .expect("adoc graph unknown runs");
    assert_eq!(unknown.status.code(), Some(3));
    assert!(stderr(&unknown).contains("error[graph.object_not_found]"));
}

#[test]
fn search_cli_related_to_filters_lexical_candidates_through_graph_artifact() {
    let workspace = build_graph_workspace("graph-search-filter");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "search",
            "target",
            "--lexical",
            "--related-to",
            "billing.root",
            "--relation",
            "depends_on",
            "--direction",
            "outgoing",
        ])
        .output()
        .expect("adoc search runs");

    assert!(
        output.status.success(),
        "expected graph-filtered search to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("Object: billing.alpha"));
    assert!(stdout.contains("Object: billing.gamma"));
    assert!(!stdout.contains("Object: billing.beta"));
}

#[test]
fn graph_cli_styled_color_always_emits_ansi_codes() {
    let workspace = build_graph_workspace("graph-styled");

    let output = Command::new(env!("CARGO_BIN_EXE_adoc"))
        .current_dir(&workspace.root)
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic")
        .env_remove("NO_COLOR")
        .args([
            "graph",
            "billing.root",
            "--format",
            "styled",
            "--color",
            "always",
        ])
        .output()
        .expect("adoc graph runs");

    assert!(
        output.status.success(),
        "expected styled graph to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains('\x1b'));
    assert!(stdout(&output).contains("billing.root"));
}

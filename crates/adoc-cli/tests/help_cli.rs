mod support;

use support::{adoc_command, fixture_path, stderr, stdout, workspace_fixture_path};

#[test]
fn long_top_level_help_lists_command_descriptions_and_examples() {
    let output = adoc_command()
        .arg("--help")
        .output()
        .expect("adoc --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "help should render to stdout, stderr was:\n{}",
        stderr(&output)
    );
    let stdout = stdout(&output);

    assert!(stdout.contains("AgentDoc Local CLI"));
    assert!(stdout.contains("Create AgentDoc config and starter docs"));
    assert!(stdout.contains("Check AgentDoc Source for strict-mode diagnostics"));
    assert!(stdout.contains("Build HTML, graph, and search artifacts"));
    assert!(stdout.contains("Explain one Knowledge Object from a compiled artifact"));
    assert!(stdout.contains("Traverse Knowledge Object relations from graph artifacts"));
    assert!(stdout.contains("Validate one AgentDoc patch document against graph artifacts"));
    assert!(stdout.contains("Search compiled Knowledge Objects"));
    assert!(stdout.contains("List stale, review-overdue, and expiring Knowledge Objects"));
    assert!(stdout.contains("Examples:"));
    assert!(stdout.contains("adoc init"));
    assert!(stdout.contains("adoc check docs"));
    assert!(stdout.contains("adoc build docs --out dist"));
    assert!(stdout.contains("adoc why billing.refunds.issue-credit"));
    assert!(stdout.contains("adoc graph billing.refunds.issue-credit"));
    assert!(stdout.contains("adoc patch --check patch.json"));
    assert!(stdout.contains("adoc search \"refund policy\""));
    assert!(stdout.contains("adoc stale --within 30d"));
}

#[test]
fn short_top_level_help_stays_compact() {
    let output = adoc_command().arg("-h").output().expect("adoc -h runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "help should render to stdout, stderr was:\n{}",
        stderr(&output)
    );
    let stdout = stdout(&output);

    assert!(stdout.contains("Usage: adoc"));
    assert!(stdout.contains("Create AgentDoc config and starter docs"));
    assert!(!stdout.contains("Examples:"));
    assert!(!stdout.contains("adoc build docs --out dist"));
}

#[test]
fn contextual_why_help_forms_render_the_same_command_help() {
    let outputs = [
        adoc_command()
            .args(["why", "--help"])
            .output()
            .expect("adoc why --help runs"),
        adoc_command()
            .args(["help", "why"])
            .output()
            .expect("adoc help why runs"),
    ];

    let first = stdout(&outputs[0]);
    for output in outputs {
        assert_eq!(output.status.code(), Some(0));
        assert!(
            output.stderr.is_empty(),
            "why help should render to stdout, stderr was:\n{}",
            stderr(&output)
        );
        let stdout = stdout(&output);
        assert!(stdout.contains("Usage: adoc why [OPTIONS] <OBJECT_ID>"));
        assert!(stdout.contains("Object ID to explain"));
        assert!(stdout.contains("--artifact <ARTIFACT>"));
        assert!(stdout.contains("dist/docs.graph.json"));
        assert!(stdout.contains("Examples:"));
        assert!(stdout.contains("adoc why billing.refunds.issue-credit"));
        assert_eq!(stdout, first);
    }
}

#[test]
fn contextual_graph_help_lists_graph_artifact_and_relation_filters() {
    let output = adoc_command()
        .args(["graph", "--help"])
        .output()
        .expect("adoc graph --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "graph help should render to stdout, stderr was:\n{}",
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("Usage: adoc graph [OPTIONS] <OBJECT_ID>"));
    assert!(stdout.contains("Graph JSON artifact path"));
    assert!(stdout.contains("dist/docs.graph.json"));
    assert!(!stdout.contains("--agent-artifact"));
    assert!(stdout.contains("--relation <RELATION>"));
    assert!(stdout.contains("--direction <DIRECTION>"));
    assert!(stdout.contains("depends_on"));
    assert!(stdout.contains("outgoing"));
}

#[test]
fn contextual_search_help_lists_graph_relation_filters() {
    let output = adoc_command()
        .args(["search", "--help"])
        .output()
        .expect("adoc search --help runs");

    assert_eq!(output.status.code(), Some(0));
    assert!(
        output.stderr.is_empty(),
        "search help should render to stdout, stderr was:\n{}",
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("--related-to <RELATED_TO>"));
    assert!(!stdout.contains("--graph-artifact"));
    assert!(stdout.contains("--relation <RELATION>"));
    assert!(stdout.contains("--direction <DIRECTION>"));
}

#[test]
fn why_treats_trailing_help_as_object_id_when_options_precede_it() {
    let artifact = fixture_path("v1_1_why/valid_artifact.graph.json");
    let output = adoc_command()
        .arg("why")
        .arg("--artifact")
        .arg(artifact)
        .arg("help")
        .output()
        .expect("adoc why --artifact <path> help runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected invalid Object ID exit\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).is_empty(),
        "why errors should render to stderr, stdout was:\n{}",
        stdout(&output)
    );
    let stderr = stderr(&output);
    assert!(stderr.contains("error[id.invalid] Object ID `help` is invalid."));
    assert!(!stderr.contains("Usage: adoc why"));
}

#[test]
fn search_treats_trailing_help_as_query_when_options_precede_it() {
    let artifact = workspace_fixture_path("v1_2_search/pilot_subset.graph.json");
    let output = adoc_command()
        .arg("search")
        .arg("--artifact")
        .arg(artifact)
        .args(["--lexical", "help"])
        .output()
        .expect("adoc search --artifact <path> --lexical help runs");

    assert_eq!(
        output.status.code(),
        Some(0),
        "expected literal help query to run\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(!stdout.contains("Usage: adoc search"));
    assert!(stdout.contains("(no matches)") || stdout.contains("Object: "));
    assert!(
        stderr(&output).is_empty(),
        "literal help query should not render stderr, got:\n{}",
        stderr(&output)
    );
}

#[test]
fn check_treats_trailing_help_as_path() {
    let output = adoc_command()
        .args(["check", "help"])
        .output()
        .expect("adoc check help runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "expected missing path error\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("error[io.unreadable_file]"));
    assert!(!stdout.contains("Usage: adoc check"));
    assert!(stderr(&output).is_empty());
}

#[test]
fn double_dash_keeps_help_as_a_literal_positional_value() {
    let check = adoc_command()
        .args(["check", "--", "help"])
        .output()
        .expect("adoc check -- help runs");
    assert!(
        !stdout(&check).contains("Usage: adoc check"),
        "escaped literal should not render check help"
    );

    let search = adoc_command()
        .args(["search", "--", "help"])
        .output()
        .expect("adoc search -- help runs");
    assert!(
        !stdout(&search).contains("Usage: adoc search"),
        "escaped literal should not render search help"
    );
}

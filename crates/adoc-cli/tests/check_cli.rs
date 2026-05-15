mod support;

use std::fs;
#[cfg(unix)]
use std::os::unix::fs::PermissionsExt;
use std::path::Path;
use std::process::Command;

use serde_json::Value;

use support::{TestWorkspace, fixture_path};

fn adoc_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    command.env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory");
    command
}

fn write_fixture_to_workspace(
    workspace: &TestWorkspace,
    fixture_relative: &str,
    source_file: &str,
) {
    let fixture_contents =
        fs::read_to_string(fixture_path(fixture_relative)).expect("fixture is readable");
    workspace.write(source_file, &fixture_contents);
}

fn assert_fixture_build_matches_golden(
    workspace_name: &str,
    fixture_relative: &str,
    source_file: &str,
    artifact_file: &str,
    golden_relative: &str,
) -> String {
    let workspace = TestWorkspace::new(workspace_name);
    write_fixture_to_workspace(&workspace, fixture_relative, source_file);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", source_file, "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join(artifact_file))
        .expect("artifact is written");
    let golden =
        fs::read_to_string(fixture_path(golden_relative)).expect("golden fixture is readable");

    assert_eq!(
        actual, golden,
        "{artifact_file} diverged from {golden_relative}"
    );
    actual
}

fn copy_fixture_directory_to_workspace(
    workspace: &TestWorkspace,
    fixture_relative: &str,
    target_dir: &str,
) {
    let fixture_root = fixture_path(fixture_relative);
    let target_root = workspace.root.join(target_dir);
    copy_directory(&fixture_root, &target_root);
}

fn copy_directory(source: &Path, target: &Path) {
    fs::create_dir_all(target).expect("target directory can be created");
    for entry in fs::read_dir(source).expect("fixture directory is readable") {
        let entry = entry.expect("fixture entry is readable");
        let source_path = entry.path();
        let target_path = target.join(entry.file_name());
        if source_path.is_dir() {
            copy_directory(&source_path, &target_path);
        } else {
            fs::copy(&source_path, &target_path).expect("fixture file can be copied");
        }
    }
}

fn assert_fixture_directory_build_matches_golden(
    workspace_name: &str,
    fixture_relative: &str,
    source_dir: &str,
    artifact_file: &str,
    golden_relative: &str,
) -> String {
    let workspace = TestWorkspace::new(workspace_name);
    copy_fixture_directory_to_workspace(&workspace, fixture_relative, source_dir);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", source_dir, "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join(artifact_file))
        .expect("artifact is written");
    let golden =
        fs::read_to_string(fixture_path(golden_relative)).expect("golden fixture is readable");

    assert_eq!(
        actual, golden,
        "{artifact_file} diverged from {golden_relative}"
    );
    actual
}

fn assert_fixture_builds_graph(
    workspace_name: &str,
    fixture_relative: &str,
    source_file: &str,
) -> Value {
    let workspace = TestWorkspace::new(workspace_name);
    write_fixture_to_workspace(&workspace, fixture_relative, source_file);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", source_file, "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"))
}

fn assert_fixture_directory_builds_graph(
    workspace_name: &str,
    fixture_relative: &str,
    source_dir: &str,
) -> String {
    let workspace = TestWorkspace::new(workspace_name);
    copy_fixture_directory_to_workspace(&workspace, fixture_relative, source_dir);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", source_dir, "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    assert_graph_artifact(&actual);
    actual
}

fn read_graph_artifact(path: &Path) -> Value {
    let text = fs::read_to_string(path).expect("graph artifact is written");
    assert_graph_artifact(&text)
}

fn assert_graph_artifact(text: &str) -> Value {
    let graph: Value = serde_json::from_str(text).expect("graph artifact is valid JSON");
    assert_eq!(graph["schema_version"], "adoc.graph.v1");
    assert!(graph["nodes"].as_array().is_some());
    assert!(graph["edges"].as_array().is_some());
    assert_eq!(
        graph["diagnostics"]
            .as_array()
            .expect("diagnostics is an array")
            .len(),
        0
    );
    graph
}

fn assert_graph_has_node(graph: &Value, expected_id: &str) {
    let has_node = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .any(|node| node["id"] == expected_id);
    assert!(
        has_node,
        "expected graph node `{expected_id}` in:\n{graph:#}"
    );
}

fn assert_graph_lacks_node(graph: &Value, forbidden_id: &str) {
    let has_node = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .any(|node| node["id"] == forbidden_id);
    assert!(
        !has_node,
        "did not expect graph node `{forbidden_id}` in:\n{graph:#}"
    );
}

fn graph_node<'a>(graph: &'a Value, expected_id: &str) -> &'a Value {
    graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["id"] == expected_id)
        .unwrap_or_else(|| panic!("expected graph node `{expected_id}` in:\n{graph:#}"))
}

#[test]
fn check_accepts_v0_1_prose_fixture_with_all_inline_syntax() {
    let fixture = fixture_path("v0_1/prose_page.adoc");

    let output = adoc_command()
        .args(["check", fixture.to_str().expect("fixture path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.1 prose fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors"),
        "expected zero errors in summary, got:\n{stdout}"
    );
}

#[test]
fn check_unclosed_fence_diagnostic_surfaces_all_six_fields() {
    let workspace = TestWorkspace::new("check-unclosed-fence-shape");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/unclosed_fence.adoc"))
        .expect("unclosed_fence fixture is readable");
    let source = workspace.write("unclosed_fence.adoc", &fixture_contents);

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unclosed fence to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);

    // Issue #3 acceptance: the diagnostic must carry file, line, column,
    // severity, code, and a fix-oriented message.
    let prefix = format!("{}:5:1:", source.to_str().expect("source path is utf-8"));
    assert!(
        stdout.contains(&prefix),
        "expected diagnostic to start with `path:line:column:` prefix `{prefix}`, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.unclosed_fence]"),
        "expected severity + code `error[parse.unclosed_fence]` in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("Fenced code block is missing a closing"),
        "expected fix-oriented message about the missing closing fence:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_unsafe_link_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-unsafe-link");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/unsafe_link.adoc"))
        .expect("unsafe_link fixture is readable");
    let source = workspace.write("unsafe_link.adoc", &fixture_contents);

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unsafe link to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unsafe_link.adoc:3:10"),
        "expected diagnostic at line 3 column 10 (where the link starts), got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.unsafe_link]"),
        "expected parse.unsafe_link code in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains("javascript:alert"),
        "expected diagnostic message to quote the rejected URL:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn build_renders_v0_1_prose_fixture_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-prose-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_1/prose_page.adoc"))
        .expect("prose fixture is readable");
    workspace.write("prose_page.adoc", &fixture_contents);

    // Run with cwd=workspace so the recorded source_path is "prose_page.adoc"
    // rather than a host-specific absolute path.
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "prose_page.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    assert_graph_has_node(&graph, "v0-1.prose");
    assert!(graph["nodes"].as_array().expect("nodes array").len() > 1);
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn build_renders_v0_1_prose_fixture_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-prose-golden-html");
    let fixture = fixture_path("v0_1/prose_page.adoc");
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            fixture.to_str().expect("fixture path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual =
        fs::read_to_string(output_directory.join("docs.html")).expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_1/prose_page.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from prose_page.golden.html; \
         re-run `adoc build` against prose_page.adoc and review before updating the snapshot"
    );
}

#[test]
fn check_accepts_minimal_prose_page() {
    let workspace = TestWorkspace::new("check-accepts-minimal-prose-page");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(
        String::from_utf8_lossy(&output.stdout).contains("0 errors"),
        "stdout should summarize successful diagnostics"
    );
}

#[test]
fn build_creates_missing_output_directory_and_writes_artifacts() {
    let workspace = TestWorkspace::new("build-writes-artifacts");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Getting Started @doc(docs.getting-started)\n",
            "\n",
            "AgentDoc keeps knowledge readable.\n",
            "\n",
            "::claim docs.search-ready\n",
            "status: draft\n",
            "--\n",
            "Default build writes a search artifact.\n",
            "::\n",
        ),
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("<h1>Getting Started</h1>"));
    assert!(html.contains("<p>AgentDoc keeps knowledge readable.</p>"));

    let graph = read_graph_artifact(&output_directory.join("docs.graph.json"));
    assert_graph_has_node(&graph, "docs.search-ready");
    assert!(!output_directory.join("docs.agent.json").exists());

    let search_json_text = fs::read_to_string(output_directory.join("docs.search.json"))
        .expect("search JSON is written");
    let search_json: serde_json::Value =
        serde_json::from_str(&search_json_text).expect("search JSON is valid");
    assert_eq!(search_json["schema_version"], "adoc.search.v0");
    assert_eq!(search_json["model"]["id"], "in-memory");
    assert_eq!(search_json["model"]["provider"], "test");
    assert_eq!(search_json["model"]["dim"], 384);
    assert_eq!(search_json["embeddings"][0]["id"], "docs.search-ready");
}

#[test]
fn build_no_embeddings_emits_info_and_leaves_prior_search_artifact_untouched() {
    let workspace = TestWorkspace::new("build-no-embeddings");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits apply after successful payment.\n",
            "::\n",
        ),
    );
    let output_directory = workspace.root.join("dist");
    fs::create_dir_all(&output_directory).expect("output directory can be created");
    let search_artifact_path = output_directory.join("docs.search.json");
    fs::write(&search_artifact_path, "existing search artifact")
        .expect("prior search artifact can be written");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory.to_str().expect("output path is utf-8"),
            "--no-embeddings",
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("info[build.embeddings_skipped]"),
        "expected skipped embedding info diagnostic in stdout:\n{stdout}"
    );
    assert!(workspace.root.join("dist/docs.html").is_file());
    assert!(workspace.root.join("dist/docs.graph.json").is_file());
    assert!(!workspace.root.join("dist/docs.agent.json").exists());
    assert_eq!(
        fs::read_to_string(search_artifact_path).expect("prior search artifact remains readable"),
        "existing search artifact",
        "--no-embeddings must leave prior docs.search.json untouched"
    );
}

#[test]
fn build_reuses_search_artifact_cache_for_unchanged_source() {
    let workspace = TestWorkspace::new("build-search-cache");
    let source = workspace.write(
        "guide.adoc",
        concat!(
            "# Guide @doc(team.guide)\n",
            "\n",
            "::claim billing.credits\n",
            "status: draft\n",
            "--\n",
            "Credits apply after successful payment.\n",
            "::\n",
        ),
    );
    let output_directory = workspace.root.join("dist");

    for _ in 0..2 {
        let output = adoc_command()
            .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
            .args([
                "build",
                source.to_str().expect("source path is utf-8"),
                "--out",
                output_directory.to_str().expect("output path is utf-8"),
            ])
            .output()
            .expect("adoc build runs");

        assert!(
            output.status.success(),
            "expected build to pass\nstdout:\n{}\nstderr:\n{}",
            String::from_utf8_lossy(&output.stdout),
            String::from_utf8_lossy(&output.stderr)
        );
    }

    let first_search: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output_directory.join("docs.search.json"))
            .expect("search artifact is readable"),
    )
    .expect("search artifact is valid");
    let first_vector = first_search["embeddings"][0]["vector"].clone();

    let output = adoc_command()
        .env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory")
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory.to_str().expect("output path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");
    assert!(output.status.success());

    let second_search: serde_json::Value = serde_json::from_str(
        &fs::read_to_string(output_directory.join("docs.search.json"))
            .expect("search artifact is readable"),
    )
    .expect("search artifact is valid");

    assert_eq!(second_search["embeddings"][0]["id"], "billing.credits");
    assert_eq!(second_search["embeddings"][0]["vector"], first_vector);
}

#[test]
fn build_missing_out_without_config_exits_1_with_config_error() {
    let workspace = TestWorkspace::new("build-missing-out-without-config");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );

    let output = adoc_command()
        .args(["build", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc build runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "config errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("error[config.missing]"),
        "expected config-missing error, got:\n{stderr}"
    );
}

#[test]
fn build_unknown_out_flag_exits_1_with_parse_error() {
    let workspace = TestWorkspace::new("build-invalid-usage-wrong-out-flag");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--output",
            "dist",
        ])
        .output()
        .expect("adoc build runs");

    assert_eq!(output.status.code(), Some(1));
    assert!(
        output.stdout.is_empty(),
        "parse errors should render to stderr, stdout was:\n{}",
        String::from_utf8_lossy(&output.stdout)
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(
        stderr.contains("unexpected argument '--output'"),
        "expected unknown-flag parse error, got:\n{stderr}"
    );
    assert!(
        stderr.contains("similar argument exists: '--out'"),
        "expected --out suggestion, got:\n{stderr}"
    );
}

#[test]
fn build_groups_contiguous_list_items_by_list_kind() {
    let workspace = TestWorkspace::new("build-groups-contiguous-lists");
    let source = workspace.write(
        "guide.adoc",
        "# Lists @doc(docs.lists)\n\n- Write source\n- Run check\n\n1. Build artifacts\n2. Inspect output\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("<ul>\n<li>Write source</li>\n<li>Run check</li>\n</ul>"));
    assert!(html.contains("<ol>\n<li>Build artifacts</li>\n<li>Inspect output</li>\n</ol>"));
    assert_eq!(html.matches("<ul>").count(), 1);
    assert_eq!(html.matches("<ol>").count(), 1);
}

#[test]
fn build_derives_distinct_page_ids_from_directory_relative_paths() {
    let workspace = TestWorkspace::new("build-derives-distinct-page-ids");
    workspace.write("a/guide.adoc", "# Alpha Guide\n\nAlpha content.\n");
    workspace.write("b/guide.adoc", "# Beta Guide\n\nBeta content.\n");
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            workspace.root.to_str().expect("workspace path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"a.guide\""));
    assert!(html.contains("data-page-id=\"b.guide\""));

    let graph = read_graph_artifact(&output_directory.join("docs.graph.json"));
    assert_graph_has_node(&graph, "a.guide");
    assert_graph_has_node(&graph, "b.guide");
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn build_keeps_page_identity_from_first_heading_annotation() {
    let workspace = TestWorkspace::new("build-keeps-first-heading-page-id");
    let source = workspace.write(
        "guide.adoc",
        "# Guide @doc(docs.primary-guide)\n\n## Details @doc(docs.details-section)\n\nMore context.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"docs.primary-guide\""));
    assert!(!html.contains("data-page-id=\"docs.details-section\""));

    let graph = read_graph_artifact(&output_directory.join("docs.graph.json"));
    assert_graph_has_node(&graph, "docs.primary-guide");
    assert_graph_lacks_node(&graph, "docs.details-section");
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn build_uses_first_top_level_heading_annotation_for_page_identity() {
    let workspace = TestWorkspace::new("build-uses-top-level-page-heading-id");
    let source = workspace.write(
        "guide.adoc",
        "## Draft Notes @doc(draft.notes)\n\n# Guide @doc(product.area)\n\nMore context.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(html.contains("data-page-id=\"product.area\""));
    assert!(!html.contains("data-page-id=\"draft.notes\""));

    let graph = read_graph_artifact(&output_directory.join("docs.graph.json"));
    assert_graph_has_node(&graph, "product.area");
    assert_graph_lacks_node(&graph, "draft.notes");
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn build_fails_clearly_when_output_path_is_a_file() {
    let workspace = TestWorkspace::new("build-output-path-is-file");
    let source = workspace.write(
        "guide.adoc",
        "# Getting Started @doc(docs.getting-started)\n\nAgentDoc keeps knowledge readable.\n",
    );
    let output_path = workspace.write("dist", "not a directory");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_path.to_str().expect("output path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        !output.status.success(),
        "expected build to fail when --out is a file"
    );
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("io.output_not_directory"));
    assert!(stderr.contains("exists as a file"));
    assert!(!output_path.join("docs.html").exists());
    assert!(!output_path.join("docs.graph.json").exists());
    assert!(!output_path.join("docs.agent.json").exists());
}

#[test]
fn check_rejects_raw_html_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-raw-html");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<div>raw html</div>\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(!output.status.success(), "expected raw HTML to fail check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("Raw HTML is not allowed in strict mode"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_unknown_raw_html_tag() {
    let workspace = TestWorkspace::new("check-rejects-unknown-raw-html-tag");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<foo>bar</foo>\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unknown raw HTML tag to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_custom_element_tag() {
    let workspace = TestWorkspace::new("check-rejects-custom-element-tag");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\n<my-component>x</my-component>\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected custom element tag to fail check in strict mode"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.raw_html]"));
}

#[test]
fn check_does_not_flag_angle_brackets_in_prose() {
    let workspace = TestWorkspace::new("check-does-not-flag-angle-brackets-in-prose");
    let source = workspace.write(
        "guide.adoc",
        "# Technical Prose @doc(docs.technical-prose)\n\nUse Vec<String> for a list.\n\nSet x < 5 here.\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected angle-bracket prose to pass check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 errors"));
}

#[test]
fn check_rejects_adjacent_inline_raw_html_tag() {
    let workspace = TestWorkspace::new("check-rejects-adjacent-inline-raw-html");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\nKeep<span>raw html</span> out.\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(!output.status.success(), "expected raw HTML to fail check");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:5"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("Raw HTML is not allowed in strict mode"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_single_file_with_non_adoc_extension() {
    let workspace = TestWorkspace::new("check-rejects-single-md-file");
    let source = workspace.write(
        "guide.md",
        "# Guide\n\n<div>This must not compile as AgentDoc Source.</div>\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unsupported extension to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[io.unsupported_source_extension]"),
        "expected unsupported source extension diagnostic in stdout:\n{stdout}"
    );
    assert!(
        stdout.contains(".adoc"),
        "expected message to name the supported extension:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn build_rejects_inline_raw_html_and_writes_no_artifacts() {
    let workspace = TestWorkspace::new("build-rejects-inline-raw-html");
    let source = workspace.write(
        "guide.adoc",
        "# Unsafe Input @doc(docs.unsafe-input)\n\nKeep <span>raw html</span> out.\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(!output.status.success(), "expected raw HTML to fail build");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:6"));
    assert!(stdout.contains("error[parse.raw_html]"));
    assert!(stdout.contains("Raw HTML is not allowed in strict mode"));
    assert!(stdout.contains("1 errors"));
    assert!(!output_directory.join("docs.html").exists());
    assert!(!output_directory.join("docs.graph.json").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn duplicate_claim_ids_fail_check_and_block_build_artifacts() {
    let workspace = TestWorkspace::new("duplicate-claim-ids");
    workspace.write(
        "01-billing.adoc",
        "# Billing Credits @doc(billing.credits-page)\n\n::claim billing.credits.foo\nstatus: draft\n--\nCredits are granted after payment succeeds.\n::\n",
    );
    workspace.write(
        "02-billing-extra.adoc",
        "# Billing Extra @doc(billing.extra-page)\n\n::claim billing.credits.foo\nstatus: draft\n--\nCredits are also described here.\n::\n",
    );

    let check_output = adoc_command()
        .args([
            "check",
            workspace.root.to_str().expect("root path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    assert!(
        !check_output.status.success(),
        "expected duplicate claim ids to fail check"
    );
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        check_stdout.contains("error[id.duplicate]"),
        "expected id.duplicate diagnostic in stdout:\n{check_stdout}"
    );
    assert!(check_stdout.contains("1 errors"));

    let output_directory = workspace.root.join("dist");
    let build_output = adoc_command()
        .args([
            "build",
            workspace.root.to_str().expect("root path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        !build_output.status.success(),
        "expected duplicate claim ids to fail build"
    );
    let build_stdout = String::from_utf8_lossy(&build_output.stdout);
    assert!(
        build_stdout.contains("error[id.duplicate]"),
        "expected id.duplicate diagnostic in build stdout:\n{build_stdout}"
    );
    assert!(!output_directory.join("docs.html").exists());
    assert!(!output_directory.join("docs.graph.json").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn broken_prose_reference_fails_check_and_blocks_build_artifacts() {
    let workspace = TestWorkspace::new("broken-prose-reference");
    workspace.write(
        "guide.adoc",
        "# Guide @doc(team.guide)\n\nSee [[missing.object]] for details.\n",
    );

    let check_output = adoc_command()
        .args([
            "check",
            workspace.root.to_str().expect("root path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    assert!(
        !check_output.status.success(),
        "expected broken reference to fail check"
    );
    let check_stdout = String::from_utf8_lossy(&check_output.stdout);
    assert!(
        check_stdout.contains("error[ref.broken]"),
        "expected ref.broken diagnostic in stdout:\n{check_stdout}"
    );

    let output_directory = workspace.root.join("dist");
    let build_output = adoc_command()
        .args([
            "build",
            workspace.root.to_str().expect("root path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        !build_output.status.success(),
        "expected broken reference to fail build"
    );
    assert!(!output_directory.join("docs.html").exists());
    assert!(!output_directory.join("docs.graph.json").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn check_allows_raw_html_inside_closed_fenced_code_block() {
    let workspace = TestWorkspace::new("check-allows-raw-html-in-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Fenced HTML Sample @doc(docs.fenced-html)\n\n```html\n<div>example</div>\n<script>alert(1)</script>\n```\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected fenced HTML sample to pass check\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        !stdout.contains("parse.raw_html"),
        "expected no parse.raw_html diagnostic for HTML inside a fenced code block:\n{stdout}"
    );
    assert!(stdout.contains("0 errors"));
}

#[test]
fn build_writes_artifacts_for_raw_html_inside_fenced_code_block() {
    let workspace = TestWorkspace::new("build-allows-raw-html-in-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Fenced HTML Sample @doc(docs.fenced-html)\n\n```html\n<div>example</div>\n```\n",
    );
    let output_directory = workspace.root.join("dist");

    let output = adoc_command()
        .args([
            "build",
            source.to_str().expect("source path is utf-8"),
            "--out",
            output_directory
                .to_str()
                .expect("output directory path is utf-8"),
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to succeed when HTML is inside a fenced block\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let html = fs::read_to_string(output_directory.join("docs.html")).expect("HTML is written");
    assert!(
        html.contains("&lt;div&gt;example&lt;/div&gt;"),
        "fenced HTML sample must be HTML-escaped inside <pre><code>:\n{html}"
    );
    assert!(output_directory.join("docs.graph.json").exists());
    assert!(!output_directory.join("docs.agent.json").exists());
}

#[test]
fn check_rejects_unclosed_fenced_code_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-unclosed-fence");
    let source = workspace.write(
        "guide.adoc",
        "# Broken Code @doc(docs.broken-code)\n\n```rust\nfn main() {}\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unclosed fenced code to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:3:1"));
    assert!(stdout.contains("error[parse.unclosed_fence]"));
    assert!(stdout.contains("Fenced code block is missing a closing"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_malformed_page_annotation_with_source_location() {
    let workspace = TestWorkspace::new("check-rejects-malformed-page-annotation");
    let source = workspace.write(
        "guide.adoc",
        "# Broken Annotation @doc(broken-page\n\nContent.\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected malformed page annotation to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("guide.adoc:1:21"));
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
    assert!(stdout.contains("Page annotation must use @doc(id)"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_reports_malformed_annotation_with_indented_heading() {
    let workspace = TestWorkspace::new("check-reports-malformed-annotation-indented");
    let source = workspace.write("guide.adoc", "  # Broken @doc(\n\nContent.\n");

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected indented malformed page annotation to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("guide.adoc:1:12"),
        "expected diagnostic at column 12 (the `@`), got:\n{stdout}"
    );
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_reports_trailing_content_malformed_with_indent() {
    let workspace = TestWorkspace::new("check-reports-trailing-content-indent");
    let source = workspace.write(
        "guide.adoc",
        "   # Notes (per @doc(thing) sidebar)\n\nContent.\n",
    );

    let output = adoc_command()
        .args(["check", source.to_str().expect("source path is utf-8")])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected trailing-content annotation with indent to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("guide.adoc:1:17"),
        "expected diagnostic at column 17 (the `@`), got:\n{stdout}"
    );
    assert!(stdout.contains("error[parse.malformed_page_annotation]"));
}

#[test]
fn check_accepts_at_doc_without_parentheses_as_heading_text() {
    let workspace = TestWorkspace::new("check-accepts-at-doc-without-parentheses");
    workspace.write(
        "team/guide.adoc",
        "# Broken Annotation @doc product.area\n\nContent.\n",
    );

    let output = adoc_command()
        .args([
            "check",
            workspace.root.to_str().expect("workspace path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected @doc without parentheses to parse as heading text\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("0 errors"));
}

#[test]
fn check_accepts_v0_2_claim_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-2-claim");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "claim_basic.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.2 claim fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors"),
        "expected zero errors in summary, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_claim_with_missing_status() {
    let workspace = TestWorkspace::new("check-rejects-claim-missing-status");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_missing_status.adoc"))
        .expect("claim_missing_status fixture is readable");
    workspace.write("claim_missing_status.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "claim_missing_status.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-status claim to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("claim_missing_status.adoc:3:1"),
        "expected diagnostic at line 3 column 1 (open-fence line), got:\n{stdout}"
    );
    assert!(
        stdout.contains("status"),
        "expected message to mention `status`, got:\n{stdout}"
    );
    assert!(
        stdout.contains("  object_id: billing.credits"),
        "expected diagnostic object_id metadata, got:\n{stdout}"
    );
    assert!(
        stdout.contains("  help: Claims require non-empty `status`."),
        "expected diagnostic help metadata, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_2_claim_fixture_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-claim-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "claim_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_2/claim_basic.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from claim_basic.golden.html; \
         re-run `adoc build` against claim_basic.adoc and review before updating the snapshot"
    );
}

#[test]
fn build_renders_v0_2_claim_fixture_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-claim-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_2/claim_basic.adoc"))
        .expect("claim_basic fixture is readable");
    workspace.write("claim_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "claim_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    let node = graph_node(&graph, "billing.credits");
    assert_eq!(node["type"], "knowledge_object");
    assert_eq!(node["kind"], "claim");
    assert_eq!(node["status"], "draft");
    assert_eq!(node["fields"]["owner"], "team-billing");
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn check_accepts_v0_3_verified_claims_pilot_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-3-verified-pilot");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "verified_claims_pilot.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.3 verified pilot fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_verified_claim_without_evidence() {
    let workspace = TestWorkspace::new("check-rejects-verified-missing-evidence");
    let fixture_contents =
        fs::read_to_string(fixture_path("v0_3/verified_claim_missing_evidence.adoc"))
            .expect("verified missing evidence fixture is readable");
    workspace.write("verified_claim_missing_evidence.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "verified_claim_missing_evidence.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-evidence verified claim to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[claim.verified_missing_evidence]"),
        "expected claim.verified_missing_evidence diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("verified_claim_missing_evidence.adoc:3:1"),
        "expected diagnostic at the ::claim open-fence, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_3_verified_claims_pilot_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-verified-pilot-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "verified_claims_pilot.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from verified_claims_pilot.golden.html"
    );
}

#[test]
fn build_renders_v0_3_verified_claims_pilot_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-verified-pilot-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_3/verified_claims_pilot.adoc"))
        .expect("verified pilot fixture is readable");
    workspace.write("verified_claims_pilot.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "verified_claims_pilot.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    for id in [
        "billing.credits.automatic",
        "billing.credits.tested",
        "billing.credits.reviewed",
        "billing.credits.review-only",
    ] {
        let node = graph_node(&graph, id);
        assert_eq!(node["type"], "knowledge_object");
        assert_eq!(node["kind"], "claim");
    }
    assert_eq!(
        graph_node(&graph, "billing.credits.automatic")["status"],
        "verified"
    );
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn check_accepts_v0_4_proposed_decision_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-4-proposed-decision");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_proposed.adoc"))
        .expect("decision_proposed fixture is readable");
    workspace.write("decision_proposed.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "decision_proposed.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.4 proposed decision fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_4_proposed_decision_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-proposed-decision-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_proposed.adoc"))
        .expect("decision_proposed fixture is readable");
    workspace.write("decision_proposed.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "decision_proposed.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_4/decision_proposed.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from decision_proposed.golden.html"
    );
}

#[test]
fn build_renders_v0_4_proposed_decision_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-proposed-decision-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_proposed.adoc"))
        .expect("decision_proposed fixture is readable");
    workspace.write("decision_proposed.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "decision_proposed.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    let node = graph_node(&graph, "billing.policy");
    assert_eq!(node["type"], "knowledge_object");
    assert_eq!(node["kind"], "decision");
    assert_eq!(node["status"], "proposed");
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn build_renders_v0_4_accepted_decision_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-accepted-decision-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_accepted.adoc"))
        .expect("decision_accepted fixture is readable");
    workspace.write("decision_accepted.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "decision_accepted.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_4/decision_accepted.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from decision_accepted.golden.html"
    );
    assert!(
        actual.contains("<section class=\"decision decision--accepted\" id=\"billing.policy\">"),
        "accepted decision modifier missing from HTML"
    );
    assert!(
        actual.contains("<div class=\"decision__verdict\"><dl><div class=\"decision__verdict-item\"><dt>decided_by</dt><dd>architecture</dd></div></dl></div>"),
        "accepted decision verdict block missing from HTML"
    );
}

#[test]
fn build_renders_v0_4_accepted_decision_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-accepted-decision-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_accepted.adoc"))
        .expect("decision_accepted fixture is readable");
    workspace.write("decision_accepted.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "decision_accepted.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    let node = graph_node(&graph, "billing.policy");
    assert_eq!(node["type"], "knowledge_object");
    assert_eq!(node["kind"], "decision");
    assert_eq!(node["status"], "accepted");
    assert_eq!(node["fields"]["decided_by"], "architecture");
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn check_accepts_v0_4_warning_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-4-warning");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_basic.adoc"))
        .expect("warning_basic fixture is readable");
    workspace.write("warning_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "warning_basic.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.4 warning fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_4_warning_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-warning-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_basic.adoc"))
        .expect("warning_basic fixture is readable");
    workspace.write("warning_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "warning_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_4/warning_basic.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from warning_basic.golden.html"
    );
    assert!(
        actual.contains("<section class=\"warning warning--high\" id=\"auth.session.clock-skew\">"),
        "warning severity modifier missing from HTML"
    );
}

#[test]
fn build_renders_v0_4_warning_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-warning-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_basic.adoc"))
        .expect("warning_basic fixture is readable");
    workspace.write("warning_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "warning_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    let node = graph_node(&graph, "auth.session.clock-skew");
    assert_eq!(node["type"], "knowledge_object");
    assert_eq!(node["kind"], "warning");
    assert_eq!(node["status"], "high");
    assert!(node["fields"].get("severity").is_none());
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn check_accepts_v0_4_glossary_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-4-glossary");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/glossary_basic.adoc"))
        .expect("glossary_basic fixture is readable");
    workspace.write("glossary_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "glossary_basic.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.4 glossary fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_4_glossary_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-glossary-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/glossary_basic.adoc"))
        .expect("glossary_basic fixture is readable");
    workspace.write("glossary_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "glossary_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_4/glossary_basic.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from glossary_basic.golden.html"
    );
    assert!(
        actual.contains("<section class=\"glossary\" id=\"billing.credits\">"),
        "glossary section missing from HTML"
    );
}

#[test]
fn build_renders_v0_4_glossary_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-glossary-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/glossary_basic.adoc"))
        .expect("glossary_basic fixture is readable");
    workspace.write("glossary_basic.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "glossary_basic.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    let node = graph_node(&graph, "billing.credits");
    assert_eq!(node["type"], "knowledge_object");
    assert_eq!(node["kind"], "glossary");
    assert!(node.get("status").is_none());
    assert_eq!(node["fields"]["status"], "draft");
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn check_accepts_v0_4_core_object_set_fixture() {
    let workspace = TestWorkspace::new("check-accepts-v0-4-core-object-set");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/core_object_set.adoc"))
        .expect("core_object_set fixture is readable");
    workspace.write("core_object_set.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "core_object_set.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected v0.4 core object set fixture to check cleanly\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("0 errors, 0 warnings"),
        "expected clean summary, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_4_core_object_set_to_golden_html() {
    let workspace = TestWorkspace::new("build-renders-core-object-set-golden-html");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/core_object_set.adoc"))
        .expect("core_object_set fixture is readable");
    workspace.write("core_object_set.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "core_object_set.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let actual = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("docs.html is written");
    let golden = fs::read_to_string(fixture_path("v0_4/core_object_set.golden.html"))
        .expect("golden HTML fixture is readable");

    assert_eq!(
        actual, golden,
        "rendered HTML diverged from core_object_set.golden.html"
    );
    assert!(
        actual
            .contains("<section class=\"claim claim--verified\" id=\"billing.credits.lifecycle\">"),
        "verified claim class family missing from HTML"
    );
    assert!(
        actual.contains("<section class=\"decision decision--accepted\" id=\"billing.policy\">"),
        "accepted decision class family missing from HTML"
    );
    assert!(
        actual.contains("<section class=\"warning warning--high\" id=\"auth.session.clock-skew\">"),
        "warning severity class family missing from HTML"
    );
    assert!(
        actual.contains("<section class=\"glossary\" id=\"billing.credits\">"),
        "glossary class family missing from HTML"
    );
}

#[test]
fn build_renders_v0_4_core_object_set_to_graph_json() {
    let workspace = TestWorkspace::new("build-renders-core-object-set-golden-graph");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/core_object_set.adoc"))
        .expect("core_object_set fixture is readable");
    workspace.write("core_object_set.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "core_object_set.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );

    let graph = read_graph_artifact(&workspace.root.join("dist").join("docs.graph.json"));
    assert_eq!(
        graph_node(&graph, "billing.credits.lifecycle")["status"],
        "verified"
    );
    assert_eq!(graph_node(&graph, "billing.policy")["status"], "accepted");
    assert_eq!(
        graph_node(&graph, "auth.session.clock-skew")["status"],
        "high"
    );
    assert!(
        graph_node(&graph, "billing.credits")
            .get("status")
            .is_none()
    );
    assert!(!workspace.root.join("dist").join("docs.agent.json").exists());
}

#[test]
fn build_renders_v0_5_scalar_relation_to_golden_html() {
    assert_fixture_build_matches_golden(
        "build-renders-v0-5-scalar-relation-html",
        "v0_5/relation_scalar.adoc",
        "relation_scalar.adoc",
        "docs.html",
        "v0_5/relation_scalar.golden.html",
    );
}

#[test]
fn build_renders_v0_5_scalar_relation_to_graph_json() {
    let graph = assert_fixture_builds_graph(
        "build-renders-v0-5-scalar-relation-json",
        "v0_5/relation_scalar.adoc",
        "relation_scalar.adoc",
    );
    assert_graph_has_node(&graph, "billing.credits");
    assert_graph_has_node(&graph, "billing.refunds");
}

#[test]
fn build_renders_v0_5_bracket_array_relation_to_golden_html() {
    assert_fixture_build_matches_golden(
        "build-renders-v0-5-bracket-array-relation-html",
        "v0_5/relation_bracket_array.adoc",
        "relation_bracket_array.adoc",
        "docs.html",
        "v0_5/relation_bracket_array.golden.html",
    );
}

#[test]
fn build_renders_v0_5_bracket_array_relation_to_graph_json() {
    let graph = assert_fixture_builds_graph(
        "build-renders-v0-5-bracket-array-relation-json",
        "v0_5/relation_bracket_array.adoc",
        "relation_bracket_array.adoc",
    );
    assert_graph_has_node(&graph, "billing.credits");
    assert_graph_has_node(&graph, "billing.refunds");
}

#[test]
fn build_renders_v0_5_decision_supersedes_to_golden_html() {
    assert_fixture_build_matches_golden(
        "build-renders-v0-5-decision-supersedes-html",
        "v0_5/decision_supersedes.adoc",
        "decision_supersedes.adoc",
        "docs.html",
        "v0_5/decision_supersedes.golden.html",
    );
}

#[test]
fn build_renders_v0_5_decision_supersedes_to_graph_json() {
    let graph = assert_fixture_builds_graph(
        "build-renders-v0-5-decision-supersedes-json",
        "v0_5/decision_supersedes.adoc",
        "decision_supersedes.adoc",
    );
    assert_graph_has_node(&graph, "billing.legacy-refunds");
    assert_graph_has_node(&graph, "billing.refund-policy");
}

#[test]
fn check_rejects_v0_5_broken_relation_fixture() {
    let workspace = TestWorkspace::new("check-rejects-v0-5-broken-relation");
    write_fixture_to_workspace(
        &workspace,
        "v0_5/relation_broken.adoc",
        "relation_broken.adoc",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "relation_broken.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected broken relation fixture to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("relation_broken.adoc:7:13"),
        "expected diagnostic on relation target, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[ref.broken]"),
        "expected ref.broken diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("depends_on target `missing.object`"),
        "expected diagnostic to name missing relation target, got:\n{stdout}"
    );
}

#[test]
fn build_renders_v0_6_multi_file_project_to_golden_html() {
    let actual = assert_fixture_directory_build_matches_golden(
        "build-renders-v0-6-multi-file-html",
        "v0_6/project",
        "project",
        "docs.html",
        "v0_6/project.golden.html",
    );

    assert!(
        actual.contains("data-page-id=\"billing.glossary\"")
            && actual.contains("data-page-id=\"billing.policy\"")
            && actual.contains("data-page-id=\"billing.ledger-page\""),
        "expected every .adoc page to render into consolidated HTML"
    );
    assert!(
        !actual.contains("ignored.markdown"),
        "non-.adoc fixture files must be ignored"
    );
}

#[test]
fn build_renders_v0_6_multi_file_project_to_graph_json() {
    let actual = assert_fixture_directory_builds_graph(
        "build-renders-v0-6-multi-file-json",
        "v0_6/project",
        "project",
    );

    assert!(
        actual.contains("\"source_path\": \"project/01-glossary.adoc\"")
            && actual.contains("\"source_path\": \"project/02-policy.adoc\"")
            && actual.contains("\"source_path\": \"project/nested/03-ledger.adoc\""),
        "expected deterministic recursive .adoc source paths"
    );
    assert!(
        !actual.contains("ignored.markdown"),
        "non-.adoc fixture files must be ignored"
    );
}

#[test]
fn check_rejects_v0_6_duplicate_id_across_files() {
    let workspace = TestWorkspace::new("check-rejects-v0-6-duplicate-id");
    copy_fixture_directory_to_workspace(&workspace, "v0_6/duplicate_id", "duplicate_id");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "duplicate_id"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected duplicate id fixture to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("duplicate_id/02-refunds.adoc:3:1"),
        "expected duplicate diagnostic on second file, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[id.duplicate]"),
        "expected id.duplicate diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("previously defined as claim at duplicate_id/01-refunds.adoc:3:1"),
        "expected first definition location, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_unknown_typed_block_kind() {
    let workspace = TestWorkspace::new("check-rejects-unknown-kind");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/unknown_kind.adoc"))
        .expect("unknown_kind fixture is readable");
    workspace.write("unknown_kind.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "unknown_kind.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unknown kind to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unknown_kind.adoc:5:3"),
        "expected diagnostic on the unknown kind word, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[schema.unknown_kind]"),
        "expected schema.unknown_kind diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("`fact`"),
        "expected message to mention rejected kind, got:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_unknown_typed_block_kind_without_field_cascade() {
    let workspace = TestWorkspace::new("check-rejects-unknown-kind-freeform");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/unknown_kind_freeform.adoc"))
        .expect("unknown_kind_freeform fixture is readable");
    workspace.write("unknown_kind_freeform.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "unknown_kind_freeform.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected unknown kind to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("unknown_kind_freeform.adoc:5:3"),
        "expected diagnostic on the unknown kind word, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[schema.unknown_kind]"),
        "expected schema.unknown_kind diagnostic, got:\n{stdout}"
    );
    assert!(
        !stdout.contains("error[parse.malformed_field]"),
        "unknown kind must not cascade into field-shape diagnostics, got:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_nested_typed_block_in_fields() {
    let workspace = TestWorkspace::new("check-rejects-nested-fields");
    let fixture_contents =
        fs::read_to_string(fixture_path("v0_4/nested_typed_block_in_fields.adoc"))
            .expect("nested fields fixture is readable");
    workspace.write("nested_typed_block_in_fields.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "nested_typed_block_in_fields.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected nested typed block in fields to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("nested_typed_block_in_fields.adoc:7:1"),
        "expected diagnostic on the nested opener line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.nested_typed_block]"),
        "expected parse.nested_typed_block diagnostic, got:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_nested_typed_block_in_body() {
    let workspace = TestWorkspace::new("check-rejects-nested-body");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/nested_typed_block_in_body.adoc"))
        .expect("nested body fixture is readable");
    workspace.write("nested_typed_block_in_body.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "nested_typed_block_in_body.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected nested typed block in body to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("nested_typed_block_in_body.adoc:9:1"),
        "expected diagnostic on the nested opener line, got:\n{stdout}"
    );
    assert!(
        stdout.contains("error[parse.nested_typed_block]"),
        "expected parse.nested_typed_block diagnostic, got:\n{stdout}"
    );
    assert!(stdout.contains("1 errors"));
}

#[test]
fn check_rejects_glossary_with_missing_body() {
    let workspace = TestWorkspace::new("check-rejects-glossary-missing-body");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/glossary_missing_body.adoc"))
        .expect("glossary_missing_body fixture is readable");
    workspace.write("glossary_missing_body.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "glossary_missing_body.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-body glossary to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("body"),
        "expected message to mention `body`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_warning_with_missing_severity() {
    let workspace = TestWorkspace::new("check-rejects-warning-missing-severity");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_missing_severity.adoc"))
        .expect("warning_missing_severity fixture is readable");
    workspace.write("warning_missing_severity.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "warning_missing_severity.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-severity warning to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("severity"),
        "expected message to mention `severity`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_warning_with_missing_body() {
    let workspace = TestWorkspace::new("check-rejects-warning-missing-body");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_missing_body.adoc"))
        .expect("warning_missing_body fixture is readable");
    workspace.write("warning_missing_body.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "warning_missing_body.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-body warning to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("body"),
        "expected message to mention `body`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_warning_with_invalid_severity() {
    let workspace = TestWorkspace::new("check-rejects-warning-invalid-severity");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_invalid_severity.adoc"))
        .expect("warning_invalid_severity fixture is readable");
    workspace.write("warning_invalid_severity.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "warning_invalid_severity.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected invalid-severity warning to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.invalid_status]"),
        "expected schema.invalid_status diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("panic"),
        "expected message to mention rejected severity, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_warning_with_severity_casing() {
    let workspace = TestWorkspace::new("check-rejects-warning-severity-casing");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/warning_severity_casing.adoc"))
        .expect("warning_severity_casing fixture is readable");
    workspace.write("warning_severity_casing.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "warning_severity_casing.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected casing-variant severity warning to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.invalid_status]"),
        "expected schema.invalid_status diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Critical"),
        "expected message to mention rejected severity, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_decision_with_missing_status() {
    let workspace = TestWorkspace::new("check-rejects-decision-missing-status");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_missing_status.adoc"))
        .expect("decision_missing_status fixture is readable");
    workspace.write("decision_missing_status.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "decision_missing_status.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected missing-status decision to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("decision_missing_status.adoc:3:1"),
        "expected diagnostic at line 3 column 1, got:\n{stdout}"
    );
    assert!(
        stdout.contains("status"),
        "expected message to mention `status`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_accepted_decision_with_missing_decided_by() {
    let workspace = TestWorkspace::new("check-rejects-accepted-decision-missing-decided-by");
    let fixture_contents = fs::read_to_string(fixture_path(
        "v0_4/decision_accepted_missing_decided_by.adoc",
    ))
    .expect("decision_accepted_missing_decided_by fixture is readable");
    workspace.write(
        "decision_accepted_missing_decided_by.adoc",
        &fixture_contents,
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "decision_accepted_missing_decided_by.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected accepted decision without decided_by to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("decided_by"),
        "expected message to mention `decided_by`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_accepted_decision_with_empty_decided_by() {
    let workspace = TestWorkspace::new("check-rejects-accepted-decision-empty-decided-by");
    let fixture_contents =
        fs::read_to_string(fixture_path("v0_4/decision_accepted_empty_decided_by.adoc"))
            .expect("decision_accepted_empty_decided_by fixture is readable");
    workspace.write("decision_accepted_empty_decided_by.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "decision_accepted_empty_decided_by.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected accepted decision with empty decided_by to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.missing_field]"),
        "expected schema.missing_field diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("decided_by"),
        "expected message to mention `decided_by`, got:\n{stdout}"
    );
}

#[test]
fn check_rejects_decision_with_invalid_status() {
    let workspace = TestWorkspace::new("check-rejects-decision-invalid-status");
    let fixture_contents = fs::read_to_string(fixture_path("v0_4/decision_invalid_status.adoc"))
        .expect("decision_invalid_status fixture is readable");
    workspace.write("decision_invalid_status.adoc", &fixture_contents);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "decision_invalid_status.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected invalid-status decision to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(
        stdout.contains("error[schema.invalid_status]"),
        "expected schema.invalid_status diagnostic, got:\n{stdout}"
    );
    assert!(
        stdout.contains("Accepted"),
        "expected message to mention rejected status, got:\n{stdout}"
    );
}

#[cfg(unix)]
#[test]
fn check_reports_unreadable_source_path() {
    let workspace = TestWorkspace::new("check-reports-unreadable-source-path");
    let source = workspace.write("private/guide.adoc", "# Private Guide\n\nHidden.\n");
    let mut permissions = fs::metadata(&source)
        .expect("source metadata can be read")
        .permissions();
    permissions.set_mode(0o000);
    fs::set_permissions(&source, permissions).expect("source can be made unreadable");

    let output = adoc_command()
        .args([
            "check",
            workspace.root.to_str().expect("root path is utf-8"),
        ])
        .output()
        .expect("adoc check runs");

    let mut permissions = fs::metadata(&source)
        .expect("source metadata can be read")
        .permissions();
    permissions.set_mode(0o644);
    fs::set_permissions(&source, permissions).expect("source permissions can be restored");

    assert!(
        !output.status.success(),
        "expected unreadable source to fail check"
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("error[io.unreadable_file]"));
    assert!(stdout.contains(source.to_str().expect("source path is utf-8")));
    assert!(stdout.contains("1 errors"));
}

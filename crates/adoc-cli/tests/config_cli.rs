mod support;

use std::fs;
use std::process::Command;

use chrono::{Local, Months};
use support::TestWorkspace;

fn adoc_command() -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_adoc"));
    command.env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory");
    command
}

fn write_valid_source(workspace: &TestWorkspace, relative_path: &str) {
    let verified_at = Local::now().date_naive();
    let expires_at = verified_at
        .checked_add_months(Months::new(12))
        .expect("verified_at plus 12 months is valid");

    workspace.write(
        relative_path,
        &format!(
            "# Billing Guide @doc(billing.guide)\n\n::claim billing.ready\nstatus: verified\nowner: team-docs\nverified_at: {verified_at}\nsource: test\nexpires_at: {expires_at}\n--\nBilling docs are ready.\n::\n"
        ),
    );
}

fn copy_valid_artifact(workspace: &TestWorkspace, relative_path: &str) {
    workspace.write(
        relative_path,
        r#"{
  "schema_version": "adoc.graph.v2",
  "nodes": [
    {
      "type": "page",
      "id": "billing.refunds",
      "order": 0,
      "title": "Refunds",
      "source_path": "docs/refunds.adoc"
    },
    {
      "type": "knowledge_object",
      "id": "billing.refunds.issue-credit",
      "kind": "claim",
      "content_hash": "sha256:billing.refunds.issue-credit",
      "status": "verified",
      "body": "Refund credits are issued from the ledger after approval.",
      "page_id": "billing.refunds",
      "source_span": { "path": "docs/refunds.adoc", "line": 12, "column": 3 },
      "fields": {
        "owner": "team-billing",
        "reviewed_by": "risk-team",
        "source": "ledger-export",
        "test": "cargo test refunds",
        "verified_at": "2026-05-06"
      },
      "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
    },
    {
      "type": "knowledge_object",
      "id": "billing.refunds.fraud-window",
      "kind": "warning",
      "content_hash": "sha256:billing.refunds.fraud-window",
      "status": "high",
      "body": "Refund attempts above the risk threshold require manual review.",
      "page_id": "billing.refunds",
      "source_span": { "path": "docs/refunds.adoc", "line": 28, "column": 1 },
      "fields": { "source": "risk-runbook" },
      "relations": { "depends_on": [], "supersedes": [], "related_to": [] }
    }
  ],
  "edges": [],
  "diagnostics": []
}
"#,
    );
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

#[test]
fn config_check_without_path_uses_nearest_docs_path_resolved_from_config_dir() {
    let workspace = TestWorkspace::new("config-check-docs-path");
    write_valid_source(&workspace, "docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
    );
    fs::create_dir_all(workspace.root.join("nested/deeper")).expect("nested cwd can be created");

    let output = adoc_command()
        .current_dir(workspace.root.join("nested/deeper"))
        .args(["check"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected config-backed check to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains("0 errors"));
}

#[test]
fn config_discovery_stops_at_git_boundary_before_parent_config() {
    for git_boundary in ["directory", "file"] {
        let workspace = TestWorkspace::new(&format!("config-git-boundary-{git_boundary}"));
        write_valid_source(&workspace, "parent-docs/index.adoc");
        workspace.write(
            "agentdoc.config.yaml",
            "version: 1\nmode: strict\ndocs_path: parent-docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
        );

        match git_boundary {
            "directory" => fs::create_dir_all(workspace.root.join("nested/repo/.git"))
                .expect(".git directory can be created"),
            "file" => {
                workspace.write("nested/repo/.git", "gitdir: ../.git/worktrees/repo\n");
            }
            _ => unreachable!("covered git boundary cases"),
        }
        fs::create_dir_all(workspace.root.join("nested/repo/deeper"))
            .expect("nested repo cwd can be created");

        let output = adoc_command()
            .current_dir(workspace.root.join("nested/repo/deeper"))
            .args(["check"])
            .output()
            .expect("adoc check runs");

        assert_eq!(
            output.status.code(),
            Some(1),
            "check should not use parent config across .git {git_boundary}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        );
        assert!(
            stderr(&output).contains("error[config.missing]"),
            "expected config.missing when no config exists inside repo boundary, got:\n{}",
            stderr(&output)
        );
    }
}

#[test]
fn config_discovery_stops_at_home_boundary_before_parent_config() {
    let workspace = TestWorkspace::new("config-home-boundary");
    let home = workspace.root.join("home");
    write_valid_source(&workspace, "parent-docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: parent-docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
    );
    fs::create_dir_all(home.join("nested/deeper")).expect("nested cwd can be created");

    let output = adoc_command()
        .current_dir(home.join("nested/deeper"))
        .env("HOME", &home)
        .args(["check"])
        .output()
        .expect("adoc check runs");

    assert_eq!(
        output.status.code(),
        Some(1),
        "check should not use config above HOME\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stderr(&output).contains("error[config.missing]"),
        "expected config.missing when no config exists before HOME boundary, got:\n{}",
        stderr(&output)
    );
}

#[test]
fn config_discovery_allows_config_at_home_boundary() {
    let workspace = TestWorkspace::new("config-home-boundary-config");
    let home = workspace.root.join("home");
    write_valid_source(&workspace, "home/docs/index.adoc");
    workspace.write(
        "home/agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
    );
    fs::create_dir_all(home.join("nested/deeper")).expect("nested cwd can be created");

    let output = adoc_command()
        .current_dir(home.join("nested/deeper"))
        .env("HOME", &home)
        .args(["check"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected config at HOME boundary to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(stdout(&output).contains("0 errors"));
}

#[test]
fn config_build_uses_exact_output_paths_and_dir_fills_omitted_paths() {
    let workspace = TestWorkspace::new("config-build-output-paths");
    write_valid_source(&workspace, "docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: bundled\n  html: public/site.html\n  graph: artifacts/graph.json\nembeddings:\n  provider: local\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected config-backed build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(workspace.root.join("public/site.html").is_file());
    assert!(workspace.root.join("artifacts/graph.json").is_file());
    assert!(workspace.root.join("bundled/docs.search.json").is_file());
    assert!(!workspace.root.join("bundled/docs.html").exists());
    assert!(!workspace.root.join("bundled/docs.graph.json").exists());
}

#[test]
fn config_build_provider_none_allows_exact_html_and_graph_without_search() {
    let workspace = TestWorkspace::new("config-build-provider-none-no-search");
    write_valid_source(&workspace, "docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  html: public/site.html\n  graph: artifacts/graph.json\nembeddings:\n  provider: none\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected config-backed skipped embedding build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("info[build.embeddings_skipped]"),
        "expected skipped embedding info diagnostic in stdout:\n{}",
        stdout(&output)
    );
    assert!(workspace.root.join("public/site.html").is_file());
    assert!(workspace.root.join("artifacts/graph.json").is_file());
    assert!(!workspace.root.join("docs.search.json").exists());
}

#[test]
fn config_build_no_embeddings_allows_exact_html_and_graph_without_search() {
    let workspace = TestWorkspace::new("config-build-no-embeddings-no-search");
    write_valid_source(&workspace, "docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  html: public/site.html\n  graph: artifacts/graph.json\nembeddings:\n  provider: local\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "--no-embeddings"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected config-backed --no-embeddings build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        stdout(&output).contains("info[build.embeddings_skipped]"),
        "expected skipped embedding info diagnostic in stdout:\n{}",
        stdout(&output)
    );
    assert!(workspace.root.join("public/site.html").is_file());
    assert!(workspace.root.join("artifacts/graph.json").is_file());
    assert!(!workspace.root.join("docs.search.json").exists());
}

#[test]
fn config_build_enabled_embeddings_requires_search_output_path() {
    let workspace = TestWorkspace::new("config-build-enabled-missing-search");
    write_valid_source(&workspace, "docs/index.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  html: public/site.html\n  graph: artifacts/graph.json\nembeddings:\n  provider: local\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build"])
        .output()
        .expect("adoc build runs");

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("error[config.missing]"),
        "expected config.missing, got:\n{stderr}"
    );
    assert!(
        stderr.contains("html, graph, and search outputs"),
        "expected missing graph/search guidance, got:\n{stderr}"
    );
    assert!(!workspace.root.join("public/site.html").exists());
    assert!(!workspace.root.join("artifacts/graph.json").exists());
}

#[test]
fn config_build_explicit_path_and_out_ignore_config_outputs() {
    let workspace = TestWorkspace::new("config-build-explicit-wins");
    write_valid_source(&workspace, "configured/index.adoc");
    write_valid_source(&workspace, "explicit/source.adoc");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: configured\noutputs:\n  dir: configured-dist\n  html: configured-html/custom.html\nembeddings:\n  provider: none\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "explicit/source.adoc", "--out", "explicit-dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected explicit build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(workspace.root.join("explicit-dist/docs.html").is_file());
    assert!(
        workspace
            .root
            .join("explicit-dist/docs.graph.json")
            .is_file()
    );
    assert!(!workspace.root.join("configured-html/custom.html").exists());
    assert!(!workspace.root.join("configured-dist/docs.html").exists());
}

#[test]
fn config_build_fully_explicit_no_embeddings_ignores_malformed_config() {
    let workspace = TestWorkspace::new("config-build-explicit-malformed-ignored");
    write_valid_source(&workspace, "explicit/source.adoc");
    workspace.write("agentdoc.config.yaml", "version: [\n");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "build",
            "explicit/source.adoc",
            "--out",
            "explicit-dist",
            "--no-embeddings",
        ])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected fully explicit build to ignore malformed config\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(workspace.root.join("explicit-dist/docs.html").is_file());
    assert!(
        workspace
            .root
            .join("explicit-dist/docs.graph.json")
            .is_file()
    );
    assert!(
        !workspace
            .root
            .join("explicit-dist/docs.search.json")
            .exists()
    );
}

#[test]
fn config_build_missing_outputs_error_names_loaded_config_path() {
    let workspace = TestWorkspace::new("config-build-missing-outputs-path");
    write_valid_source(&workspace, "docs/index.adoc");
    let config_path = workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\nembeddings:\n  provider: local\n",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build"])
        .output()
        .expect("adoc build runs");

    assert_eq!(output.status.code(), Some(1));
    let stderr = stderr(&output);
    assert!(
        stderr.contains("error[config.missing]"),
        "expected config.missing, got:\n{stderr}"
    );
    assert!(
        stderr.contains("outputs.dir"),
        "expected missing outputs guidance, got:\n{stderr}"
    );
    assert!(
        stderr.contains(&config_path.display().to_string()),
        "expected loaded config path in error, got:\n{stderr}"
    );
}

#[test]
fn config_why_and_search_use_configured_artifacts_unless_args_are_explicit() {
    let workspace = TestWorkspace::new("config-retrieval-artifacts");
    copy_valid_artifact(&workspace, "configured/docs.graph.json");
    copy_valid_artifact(&workspace, "explicit/docs.graph.json");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\n  graph: configured/docs.graph.json\n  search: configured/docs.search.json\nembeddings:\n  provider: local\n",
    );

    let why = adoc_command()
        .current_dir(&workspace.root)
        .args(["why", "billing.refunds.issue-credit", "--format", "plain"])
        .output()
        .expect("adoc why runs");
    assert!(
        why.status.success(),
        "expected config artifact why to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&why),
        stderr(&why)
    );
    assert!(stdout(&why).contains("Object: billing.refunds.issue-credit"));

    let explicit_why = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "why",
            "billing.refunds.fraud-window",
            "--artifact",
            "explicit/docs.graph.json",
            "--format",
            "plain",
        ])
        .output()
        .expect("adoc why runs");
    assert!(
        explicit_why.status.success(),
        "expected explicit why to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&explicit_why),
        stderr(&explicit_why)
    );
    assert!(stdout(&explicit_why).contains("Object: billing.refunds.fraud-window"));

    let search = adoc_command()
        .current_dir(&workspace.root)
        .args(["search", "ledger", "--lexical"])
        .output()
        .expect("adoc search runs");
    assert!(
        search.status.success(),
        "expected config artifact lexical search to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&search),
        stderr(&search)
    );
    assert!(stdout(&search).contains("Object: billing.refunds.issue-credit"));
}

#[test]
fn config_invalid_mode_provider_and_version_exit_with_config_errors() {
    let cases = [
        (
            "unsupported-mode",
            "version: 1\nmode: loose\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
            "unsupported mode",
        ),
        (
            "unsupported-provider",
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: hosted\n",
            "unsupported embeddings provider",
        ),
        (
            "unsupported-version",
            "version: 2\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: local\n",
            "unsupported version",
        ),
    ];

    for (name, config, expected) in cases {
        let workspace = TestWorkspace::new(&format!("config-{name}"));
        write_valid_source(&workspace, "docs/index.adoc");
        workspace.write("agentdoc.config.yaml", config);

        let output = adoc_command()
            .current_dir(&workspace.root)
            .args(["check"])
            .output()
            .expect("adoc check runs");

        assert!(
            !output.status.success(),
            "expected invalid config to fail for {name}"
        );
        let stderr = stderr(&output);
        assert!(
            stderr.contains("error[config.invalid]"),
            "expected config error for {name}, got:\n{stderr}"
        );
        assert!(
            stderr.contains(expected),
            "expected {expected:?} for {name}, got:\n{stderr}"
        );
    }
}

#[test]
fn config_rejects_unknown_config_fields() {
    let cases = [
        (
            "top-level-typo",
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembedings:\n  provider: none\n",
            ["check"].as_slice(),
            "embedings",
        ),
        (
            "outputs-typo",
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dyr: dist\nembeddings:\n  provider: none\n",
            ["build"].as_slice(),
            "dyr",
        ),
        (
            "embeddings-extra",
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n  mode: local\n",
            ["check"].as_slice(),
            "mode",
        ),
    ];

    for (name, config, args, expected_field) in cases {
        let workspace = TestWorkspace::new(&format!("config-unknown-{name}"));
        write_valid_source(&workspace, "docs/index.adoc");
        workspace.write("agentdoc.config.yaml", config);

        let output = adoc_command()
            .current_dir(&workspace.root)
            .args(args)
            .output()
            .expect("adoc command runs");

        assert!(
            !output.status.success(),
            "expected unknown config field to fail for {name}"
        );
        let stderr = stderr(&output);
        assert!(
            stderr.contains("error[config.parse]"),
            "expected parse error for {name}, got:\n{stderr}"
        );
        assert!(
            stderr.contains(expected_field),
            "expected parse error to name {expected_field:?} for {name}, got:\n{stderr}"
        );
    }
}

mod support;

use std::fs;

use assert_cmd::Command;
use chrono::{Local, Months, NaiveDate};
use predicates::prelude::*;
use serde::Deserialize;
use support::TestWorkspace;

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct InitConfig {
    version: u8,
    mode: String,
    docs_path: String,
    outputs: InitOutputs,
    embeddings: InitEmbeddings,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct InitOutputs {
    dir: String,
    html: String,
    agent_json: String,
    search: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct InitEmbeddings {
    provider: String,
}

fn adoc() -> Command {
    let mut cmd = Command::cargo_bin("adoc").expect("adoc binary is available");
    cmd.env("ADOC_TEST_EMBEDDING_PROVIDER", "in-memory");
    cmd
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn field_value<'a>(text: &'a str, field: &str) -> &'a str {
    text.lines()
        .find_map(|line| line.strip_prefix(field))
        .unwrap_or_else(|| panic!("expected {field} in generated docs"))
        .trim()
}

#[test]
fn init_creates_config_and_example_docs_in_current_directory() {
    let workspace = TestWorkspace::new("init-creates-project");
    let earliest_today = Local::now().date_naive();

    let init = adoc()
        .current_dir(&workspace.root)
        .arg("init")
        .output()
        .expect("adoc init runs");
    assert!(
        init.status.success(),
        "expected init to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&init),
        stderr(&init)
    );
    let latest_today = Local::now().date_naive();
    assert!(stdout(&init).contains("Created agentdoc.config.yaml and docs/index.adoc"));
    assert!(stdout(&init).contains("Next: adoc check"));

    let config_text = fs::read_to_string(workspace.root.join("agentdoc.config.yaml"))
        .expect("config file is written");
    assert!(config_text.contains("version: 1"));
    assert!(config_text.contains("mode: strict"));
    assert!(config_text.contains("docs_path: docs"));
    assert!(config_text.contains("outputs:"));
    assert!(config_text.contains("  dir: dist"));
    assert!(config_text.contains("  html: dist/docs.html"));
    assert!(config_text.contains("  agent_json: dist/docs.agent.json"));
    assert!(config_text.contains("  search: dist/docs.search.json"));
    assert!(config_text.contains("embeddings:"));
    assert!(config_text.contains("  provider: local"));

    let config: InitConfig =
        serde_saphyr::from_str(&config_text).expect("generated config is valid YAML");
    assert_eq!(
        config,
        InitConfig {
            version: 1,
            mode: "strict".to_string(),
            docs_path: "docs".to_string(),
            outputs: InitOutputs {
                dir: "dist".to_string(),
                html: "dist/docs.html".to_string(),
                agent_json: "dist/docs.agent.json".to_string(),
                search: "dist/docs.search.json".to_string(),
            },
            embeddings: InitEmbeddings {
                provider: "local".to_string(),
            },
        }
    );

    let docs_text =
        fs::read_to_string(workspace.root.join("docs/index.adoc")).expect("example doc is written");
    assert!(docs_text.contains("# AgentDoc Project"));
    assert!(docs_text.contains("::claim project.initialized"));
    assert!(docs_text.contains("status: verified"));
    assert!(docs_text.contains("owner: team-docs"));
    let verified_at =
        NaiveDate::parse_from_str(field_value(&docs_text, "verified_at:"), "%Y-%m-%d")
            .expect("verified_at is an ISO date");
    assert!(
        (earliest_today..=latest_today).contains(&verified_at),
        "verified_at should be today's date, got {verified_at}"
    );
    assert!(docs_text.contains("source: adoc init template"));
    let expires_at = NaiveDate::parse_from_str(field_value(&docs_text, "expires_at:"), "%Y-%m-%d")
        .expect("expires_at is an ISO date");
    assert_eq!(
        expires_at,
        verified_at
            .checked_add_months(Months::new(12))
            .expect("verified_at plus 12 months is valid")
    );

    adoc()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist"])
        .assert()
        .success();

    adoc()
        .current_dir(&workspace.root)
        .args(["explain", "project.initialized"])
        .assert()
        .success()
        .stdout(predicate::str::contains("Object: project.initialized"))
        .stdout(predicate::str::contains("Kind: claim"));
}

#[cfg(unix)]
#[test]
fn init_cleans_up_config_when_index_write_fails() {
    use std::os::unix::fs::PermissionsExt;

    let workspace = TestWorkspace::new("init-index-write-fails");
    let docs_dir = workspace.root.join("docs");
    fs::create_dir_all(&docs_dir).expect("docs dir can be created");
    fs::set_permissions(&docs_dir, fs::Permissions::from_mode(0o555))
        .expect("docs dir can be made read-only");

    let init = adoc()
        .current_dir(&workspace.root)
        .arg("init")
        .output()
        .expect("adoc init runs");

    fs::set_permissions(&docs_dir, fs::Permissions::from_mode(0o755))
        .expect("docs dir permissions can be restored");

    assert_eq!(init.status.code(), Some(1));
    assert!(
        !workspace.root.join("agentdoc.config.yaml").exists(),
        "failed init must remove config so rerun is not blocked"
    );
    assert!(
        !workspace.root.join("docs/index.adoc").exists(),
        "failed init must not leave index behind"
    );
}

#[test]
fn init_refuses_to_overwrite_existing_config() {
    let workspace = TestWorkspace::new("init-existing-config");
    workspace.write("agentdoc.config.yaml", "version: 1\n");

    adoc()
        .current_dir(&workspace.root)
        .arg("init")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("agentdoc.config.yaml"));

    assert!(
        !workspace.root.join("docs/index.adoc").exists(),
        "init must not partially create docs when config exists"
    );
}

#[test]
fn init_refuses_to_overwrite_existing_index() {
    let workspace = TestWorkspace::new("init-existing-index");
    workspace.write("docs/index.adoc", "# Existing\n");

    adoc()
        .current_dir(&workspace.root)
        .arg("init")
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("already exists"))
        .stderr(predicate::str::contains("docs/index.adoc"));

    assert!(
        !workspace.root.join("agentdoc.config.yaml").exists(),
        "init must not partially create config when index exists"
    );
}

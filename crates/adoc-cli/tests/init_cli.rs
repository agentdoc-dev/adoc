mod support;

use std::fs;

use assert_cmd::Command;
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
#[serde(deny_unknown_fields)]
struct InitOutputs {
    dir: String,
}

#[derive(Debug, Deserialize, PartialEq, Eq)]
struct InitEmbeddings {
    provider: String,
}

fn adoc() -> Command {
    let mut cmd = Command::cargo_bin("adoc").expect("adoc binary is available");
    cmd.env("ADOC_TEST_EMBEDDING_PROVIDER", "deterministic");
    cmd
}

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

#[test]
fn init_creates_config_and_example_docs_in_current_directory() {
    let workspace = TestWorkspace::new("init-creates-project");

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
    assert!(stdout(&init).contains("Created agentdoc.config.yaml and docs/index.adoc"));
    assert!(stdout(&init).contains("Next: adoc check"));

    let config_text = fs::read_to_string(workspace.root.join("agentdoc.config.yaml"))
        .expect("config file is written");
    assert!(config_text.contains("version: 1"));
    assert!(config_text.contains("mode: strict"));
    assert!(config_text.contains("docs_path: docs"));
    assert!(config_text.contains("outputs:"));
    assert!(config_text.contains("  dir: dist"));
    assert!(!config_text.contains("  html:"));
    assert!(!config_text.contains("  agent_json:"));
    assert!(!config_text.contains("  search:"));
    assert!(config_text.contains("embeddings:"));
    assert!(config_text.contains("  provider: local"));
    // V6.4 (ADR-0037): init never writes the MCP patch-apply gate — opting
    // in is always a deliberate human edit.
    assert!(!config_text.contains("mcp"));

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
    assert!(docs_text.contains("status: draft"));
    assert!(!docs_text.contains("owner:"));
    assert!(!docs_text.contains("verified_at:"));
    assert!(!docs_text.contains("source:"));
    assert!(!docs_text.contains("expires_at:"));

    adoc()
        .current_dir(&workspace.root)
        .args(["build", "docs", "--out", "dist"])
        .assert()
        .success();

    adoc()
        .current_dir(&workspace.root)
        .args(["why", "project.initialized"])
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

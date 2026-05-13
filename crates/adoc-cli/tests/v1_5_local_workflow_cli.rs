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

fn stdout(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &std::process::Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn assert_success(command: &str, output: &std::process::Output) {
    assert!(
        output.status.success(),
        "expected `{command}` to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(output),
        stderr(output)
    );
}

fn write_valid_source(workspace: &TestWorkspace, relative_path: &str, object_id: &str) {
    let verified_at = Local::now().date_naive();
    let expires_at = verified_at
        .checked_add_months(Months::new(12))
        .expect("verified_at plus 12 months is valid");

    workspace.write(
        relative_path,
        &format!(
            "# Billing Guide @doc({object_id}.doc)\n\n::claim {object_id}\nstatus: verified\nowner: team-docs\nverified_at: {verified_at}\nsource: test\nexpires_at: {expires_at}\n--\nBilling docs are ready for local workflow validation.\n::\n"
        ),
    );
}

#[test]
fn v1_5_init_check_build_why_and_search_use_config_defaults_end_to_end() {
    let workspace = TestWorkspace::new("v1-5-local-workflow");

    let init = adoc_command()
        .current_dir(&workspace.root)
        .arg("init")
        .output()
        .expect("adoc init runs");
    assert_success("adoc init", &init);

    let generated_source = fs::read_to_string(workspace.root.join("docs/index.adoc"))
        .expect("init writes generated example source");
    assert!(
        generated_source.contains("::claim project.initialized"),
        "generated example should contain the object used by retrieval commands"
    );

    let check = adoc_command()
        .current_dir(&workspace.root)
        .arg("check")
        .output()
        .expect("adoc check runs");
    assert_success("adoc check", &check);
    assert!(stdout(&check).contains("0 errors, 0 warnings"));

    let build = adoc_command()
        .current_dir(&workspace.root)
        .arg("build")
        .output()
        .expect("adoc build runs");
    assert_success("adoc build", &build);

    assert!(workspace.root.join("dist/docs.html").is_file());
    assert!(workspace.root.join("dist/docs.agent.json").is_file());
    assert!(workspace.root.join("dist/docs.search.json").is_file());

    let why = adoc_command()
        .current_dir(&workspace.root)
        .args(["why", "project.initialized"])
        .output()
        .expect("adoc why runs");
    assert_success("adoc why project.initialized", &why);
    assert!(stdout(&why).contains("Object: project.initialized"));

    let search = adoc_command()
        .current_dir(&workspace.root)
        .args(["search", "initialized"])
        .output()
        .expect("adoc search runs");
    assert_success("adoc search initialized", &search);
    assert!(stdout(&search).contains("Object: project.initialized"));
}

#[test]
fn v1_5_explicit_check_path_and_build_out_ignore_config_defaults() {
    let workspace = TestWorkspace::new("v1-5-explicit-workflow");
    write_valid_source(&workspace, "configured/index.adoc", "configured.ready");
    write_valid_source(&workspace, "explicit/index.adoc", "explicit.ready");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: configured\noutputs:\n  dir: configured-dist\n  html: configured-html/custom.html\n  agent_json: configured-artifacts/docs.agent.json\n  search: configured-artifacts/docs.search.json\nembeddings:\n  provider: local\n",
    );

    let check = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "explicit"])
        .output()
        .expect("adoc check explicit path runs");
    assert_success("adoc check explicit", &check);
    assert!(stdout(&check).contains("0 errors, 0 warnings"));

    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "explicit", "--out", "explicit-dist"])
        .output()
        .expect("adoc build explicit path runs");
    assert_success("adoc build explicit --out explicit-dist", &build);

    assert!(workspace.root.join("explicit-dist/docs.html").is_file());
    assert!(
        workspace
            .root
            .join("explicit-dist/docs.agent.json")
            .is_file()
    );
    assert!(
        workspace
            .root
            .join("explicit-dist/docs.search.json")
            .is_file()
    );
    assert!(!workspace.root.join("configured-html/custom.html").exists());
    assert!(!workspace.root.join("configured-dist/docs.html").exists());
    assert!(
        !workspace
            .root
            .join("configured-artifacts/docs.agent.json")
            .exists()
    );
    assert!(
        !workspace
            .root
            .join("configured-artifacts/docs.search.json")
            .exists()
    );
}

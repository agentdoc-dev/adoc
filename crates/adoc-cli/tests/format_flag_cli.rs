mod support;

use std::fs;

use assert_cmd::Command;
use predicates::prelude::*;
use support::{TestWorkspace, fixture_path};

fn copy_valid_artifact(workspace: &TestWorkspace, relative_path: &str) {
    let artifact = fs::read_to_string(fixture_path("v1_1_why/valid_artifact.agent.json"))
        .expect("fixture artifact is readable");
    workspace.write(relative_path, &artifact);
}

fn adoc() -> Command {
    let mut cmd = Command::cargo_bin("adoc").expect("adoc binary is available");
    cmd.env_remove("NO_COLOR")
        .env_remove("CLICOLOR")
        .env_remove("CLICOLOR_FORCE");
    cmd
}

// ----------------------------------------------------------------- piped → plain

/// When stdout is not a TTY (piped), `--format auto` (the default) must
/// produce plain text output identical to explicit `--format plain`.
#[test]
fn piped_default_produces_plain_output() {
    let workspace = TestWorkspace::new("format-flag-piped");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = adoc()
        .current_dir(&workspace.root)
        .args(["why", "billing.refunds.issue-credit"])
        .output()
        .expect("adoc runs");

    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    // Plain output contains the key header fields.
    assert!(stdout.contains("Object: billing.refunds.issue-credit"));
    assert!(stdout.contains("Kind: claim"));
    // No ANSI escape codes in plain output.
    assert!(
        !stdout.contains('\x1b'),
        "plain output must not contain ANSI escape codes"
    );
}

// -------------------------------------------------------------- --format=json

#[test]
fn format_json_flag_emits_json_envelope() {
    let workspace = TestWorkspace::new("format-flag-json");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    adoc()
        .current_dir(&workspace.root)
        .args(["why", "billing.refunds.issue-credit", "--format", "json"])
        .assert()
        .success()
        .stdout(predicate::str::starts_with("{"))
        .stdout(predicate::str::contains("schema_version"))
        .stdout(predicate::str::contains("adoc.retrieval.v0"))
        .stdout(predicate::str::contains("billing.refunds.issue-credit"));
}

// ----------------------------------------------------------- NO_COLOR=1 → plain

#[test]
fn no_color_env_forces_plain_output() {
    let workspace = TestWorkspace::new("format-flag-no-color");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    let output = adoc()
        .current_dir(&workspace.root)
        .env("NO_COLOR", "1")
        .args(["why", "billing.refunds.issue-credit"])
        .output()
        .expect("adoc runs");

    assert!(
        output.status.success(),
        "expected success\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("Object: billing.refunds.issue-credit"));
    assert!(
        !stdout.contains('\x1b'),
        "NO_COLOR output must not contain ANSI escape codes"
    );
}

// ---------------------------------------------------------- --format=plain

#[test]
fn explicit_plain_flag_produces_plain_output() {
    let workspace = TestWorkspace::new("format-flag-explicit-plain");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    adoc()
        .current_dir(&workspace.root)
        .args(["why", "billing.refunds.issue-credit", "--format", "plain"])
        .assert()
        .success()
        .stdout(predicate::str::contains(
            "Object: billing.refunds.issue-credit",
        ))
        .stdout(predicate::str::contains("Kind: claim"))
        .stdout(predicate::function(|s: &str| !s.contains('\x1b')));
}

// --------------------------------------------------------- --format=styled

/// `--format styled` with `--color=always` emits ANSI codes.  The output
/// contains the same record fields as plain but with escape sequences for
/// faint labels and a status chip.
#[test]
fn styled_flag_with_color_always_emits_ansi_codes() {
    let workspace = TestWorkspace::new("format-flag-styled");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    adoc()
        .current_dir(&workspace.root)
        .args([
            "why",
            "billing.refunds.issue-credit",
            "--format",
            "styled",
            "--color",
            "always",
        ])
        .assert()
        .success()
        // Must contain ANSI escape codes.
        .stdout(predicate::str::contains("\x1b["))
        // Must still contain the object id in the visible text.
        .stdout(predicate::str::contains("billing.refunds.issue-credit"))
        // Status chip text has brackets.
        .stdout(predicate::str::contains("[verified]"));
}

// ----------------------------------------- --color=never overrides --format

/// `--format styled --color never` must emit no ANSI escape codes.
/// This pins the cargo/git/ripgrep convention: `--color=never` wins over
/// any explicit format choice (except JSON).
#[test]
fn format_styled_with_color_never_emits_no_ansi_codes() {
    let workspace = TestWorkspace::new("format-flag-styled-color-never");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    adoc()
        .current_dir(&workspace.root)
        .args([
            "why",
            "billing.refunds.issue-credit",
            "--format",
            "styled",
            "--color",
            "never",
        ])
        .assert()
        .success()
        // No ANSI escapes despite --format=styled.
        .stdout(predicate::str::contains("\x1b[").not())
        // Visible content must still be present.
        .stdout(predicate::str::contains("billing.refunds.issue-credit"));
}

/// `--format plain --color always` must emit ANSI escape codes.
/// This pins the same convention from the other direction: `--color=always`
/// wins over an explicit `--format=plain`.
#[test]
fn format_plain_with_color_always_emits_ansi_codes() {
    let workspace = TestWorkspace::new("format-flag-plain-color-always");
    copy_valid_artifact(&workspace, "dist/docs.agent.json");

    adoc()
        .current_dir(&workspace.root)
        .args([
            "why",
            "billing.refunds.issue-credit",
            "--format",
            "plain",
            "--color",
            "always",
        ])
        .assert()
        .success()
        // Must contain ANSI escape codes despite --format=plain.
        .stdout(predicate::str::contains("\x1b["))
        // Visible content must still be present.
        .stdout(predicate::str::contains("billing.refunds.issue-credit"));
}

// ------------------------------------------------- invalid --format value

#[test]
fn invalid_format_value_exits_1_with_error() {
    adoc()
        .args(["why", "some.id", "--format", "yaml"])
        .assert()
        .failure()
        .code(1)
        .stderr(predicate::str::contains("invalid value 'yaml'"));
}

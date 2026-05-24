mod support;

use std::process::Command;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const BASE_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.refunds\n",
    "status: verified\n",
    "owner: team-billing\n",
    "verified_at: 2026-05-05\n",
    "source: ledger\n",
    "impacts: crates/billing/src/refund.rs\n",
    "--\n",
    "Refunds process within 24 hours.\n",
    "::\n",
    "\n",
    "::claim billing.credits\n",
    "status: draft\n",
    "--\n",
    "Credits apply after payment.\n",
    "::\n",
);

const HEAD_BILLING_ADOC: &str = concat!(
    "# Billing @doc(team.billing)\n",
    "\n",
    "::claim billing.refunds\n",
    "status: verified\n",
    "owner: team-billing\n",
    "verified_at: 2026-05-05\n",
    "source: ledger\n",
    "impacts: crates/billing/src/refund.rs\n",
    "--\n",
    "Refunds process within 12 hours.\n",
    "::\n",
    "\n",
    "::claim billing.credits\n",
    "status: draft\n",
    "--\n",
    "Credits apply after payment.\n",
    "::\n",
);

const BASE_REFUND_SRC: &str = "// initial stub\n";
const HEAD_REFUND_SRC: &str = "// updated implementation\n";

fn run_git(workspace: &TestWorkspace, args: &[&str]) {
    let mut command = Command::new("git");
    command.args(args).current_dir(&workspace.root);
    for var in [
        "GIT_DIR",
        "GIT_INDEX_FILE",
        "GIT_WORK_TREE",
        "GIT_NAMESPACE",
        "GIT_OBJECT_DIRECTORY",
        "GIT_COMMON_DIR",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
        "GIT_PREFIX",
    ] {
        command.env_remove(var);
    }
    let output = command
        .output()
        .unwrap_or_else(|error| panic!("git {args:?} should spawn: {error}"));
    assert!(
        output.status.success(),
        "git {args:?} should succeed\nstdout:\n{}\nstderr:\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

/// Build a 2-commit billing-pilot fixture for V3.3 acceptance:
/// - base on `main`: `docs/billing.adoc` declares a verified
///   `billing.refunds` claim with `impacts: crates/billing/src/refund.rs`
///   and an evidence stub at that file.
/// - head on `feature`: edits `crates/billing/src/refund.rs` and updates the
///   claim's body so its `content_hash` flips.
fn build_billing_pilot_with_impacts(name: &str) -> TestWorkspace {
    let workspace = TestWorkspace::new(name);
    run_git(&workspace, &["init", "--initial-branch=main"]);
    run_git(&workspace, &["config", "user.email", "test@adoc.dev"]);
    run_git(&workspace, &["config", "user.name", "adoc tests"]);
    run_git(&workspace, &["config", "commit.gpgsign", "false"]);

    workspace.write("docs/billing.adoc", BASE_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", BASE_REFUND_SRC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "base"]);

    run_git(&workspace, &["checkout", "-b", "feature"]);
    workspace.write("docs/billing.adoc", HEAD_BILLING_ADOC);
    workspace.write("crates/billing/src/refund.rs", HEAD_REFUND_SRC);
    run_git(&workspace, &["add", "-A"]);
    run_git(&workspace, &["commit", "-m", "head"]);

    workspace
}

#[test]
fn review_main_json_envelope_flags_billing_refunds() {
    let workspace = build_billing_pilot_with_impacts("review-json");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "json"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let value: serde_json::Value = serde_json::from_slice(&output.stdout)
        .expect("adoc review --format json must emit a JSON envelope on stdout");

    assert_eq!(value["schema_version"], "adoc.review.v0");
    assert_eq!(value["diff"]["schema_version"], "adoc.diff.v0");

    let impact = value["impact"].as_array().expect("impact array");
    assert_eq!(
        impact.len(),
        1,
        "expected exactly one impacted object, got: {impact:#?}"
    );
    assert_eq!(impact[0]["id"], "billing.refunds");
    assert_eq!(impact[0]["paths"][0], "crates/billing/src/refund.rs");

    let reviewers = value["required_reviewers"]
        .as_array()
        .expect("required_reviewers array");
    assert_eq!(
        reviewers.len(),
        1,
        "expected one required reviewer, got: {reviewers:#?}"
    );
    assert_eq!(reviewers[0]["owner"], "team-billing");
    let object_ids = reviewers[0]["object_ids"]
        .as_array()
        .expect("object_ids array");
    assert!(
        object_ids.iter().any(|id| id == "billing.refunds"),
        "team-billing must own billing.refunds; got: {object_ids:#?}"
    );
}

#[test]
fn review_main_plain_lists_impact_and_reviewer_sections() {
    let workspace = build_billing_pilot_with_impacts("review-plain");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "main", "--format", "plain"])
        .output()
        .expect("adoc review runs");

    assert!(
        output.status.success(),
        "expected adoc review to exit zero\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let stdout = stdout(&output);
    assert!(stdout.contains("Impact:"));
    assert!(stdout.contains("billing.refunds (crates/billing/src/refund.rs)"));
    assert!(stdout.contains("Required reviewers:"));
    assert!(stdout.contains("team-billing: billing.refunds"));
}

#[test]
fn review_unresolvable_ref_exits_nonzero_with_actionable_stderr() {
    let workspace = build_billing_pilot_with_impacts("review-bad-ref");

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["review", "definitely-not-a-real-ref"])
        .output()
        .expect("adoc review runs");

    assert!(
        !output.status.success(),
        "expected unresolvable ref to exit non-zero\nstdout:\n{}",
        stdout(&output)
    );
    let stderr = stderr(&output);
    assert!(stderr.contains("definitely-not-a-real-ref"));
}

#[test]
fn check_rejects_impacts_parent_segment_path() {
    let workspace = TestWorkspace::new("review-impacts-parent-segment");
    workspace.write(
        "agentdoc.config.yaml",
        "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: deterministic\n",
    );
    workspace.write(
        "docs/billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n",
            "\n",
            "::claim billing.refunds\n",
            "status: draft\n",
            "impacts: ..\n",
            "--\n",
            "Refunds.\n",
            "::\n",
        ),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected adoc check to exit non-zero for `impacts: ..`\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let combined = format!("{}\n{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("schema.impacts_invalid_path"),
        "expected schema.impacts_invalid_path in output; got:\n{combined}"
    );
}

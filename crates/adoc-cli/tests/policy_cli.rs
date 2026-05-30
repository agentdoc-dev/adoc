mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const VALID_POLICY: &str = "\
# Security Policies @doc(team.security)

Org-wide security policies.

::policy security.data-retention
status: active
owner: security-lead
approved_by: security-lead
effective_at: 2026-04-01
review_interval: 90d
--
Customer data is retained for no more than 365 days.
::
";

const FUTURE_EFFECTIVE_AT_POLICY: &str = "\
# Security Policies @doc(team.security)

Org-wide security policies.

::policy security.data-retention
status: active
owner: security-lead
approved_by: security-lead
effective_at: 2999-01-01
review_interval: 90d
--
Customer data is retained for no more than 365 days.
::
";

const MISSING_APPROVED_BY_POLICY: &str = "\
# Security Policies @doc(team.security)

Org-wide security policies.

::policy security.data-retention
status: active
owner: security-lead
effective_at: 2026-04-01
review_interval: 90d
--
Customer data is retained for no more than 365 days.
::
";

const LIST_APPROVED_BY_POLICY: &str = "\
# Security Policies @doc(team.security)

Org-wide security policies.

::policy security.data-retention
status: active
owner: security-lead
approved_by: [security-lead, platform-lead]
effective_at: 2026-04-01
review_interval: 90d
--
Customer data is retained for no more than 365 days.
::
";

#[test]
fn check_accepts_valid_policy() {
    let workspace = TestWorkspace::new("policy-check-ok");
    workspace.write("policy.adoc", VALID_POLICY);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "policy.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_policy_missing_approved_by() {
    let workspace = TestWorkspace::new("policy-missing-approved-by");
    workspace.write("policy.adoc", MISSING_APPROVED_BY_POLICY);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "policy.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a policy missing approved_by"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.policy_missing_approved_by]"),
        "expected policy missing-approved-by diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_accepts_list_form_approved_by() {
    let workspace = TestWorkspace::new("policy-list-approved-by");
    workspace.write("policy.adoc", LIST_APPROVED_BY_POLICY);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "policy.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass for list-form approved_by\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn build_renders_approval_block_and_emits_policy_into_graph_v3() {
    let workspace = TestWorkspace::new("policy-build");
    workspace.write("policy.adoc", VALID_POLICY);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "policy.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // HTML: approval block contains effective_at and at least one approver.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("security-lead"),
        "html must contain approver value\n{html}"
    );
    assert!(
        html.contains("2026-04-01"),
        "html must contain effective_at value\n{html}"
    );

    // Graph: the policy node is emitted with the correct fields.
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v3");

    let policy = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "policy")
        .expect("graph contains a policy node");

    assert_eq!(policy["id"], "security.data-retention");
    assert_eq!(policy["status"], "active");
    assert!(
        policy["approved_by"]
            .as_array()
            .expect("approved_by is an array")
            .iter()
            .any(|v| v == "security-lead"),
        "approved_by must contain security-lead"
    );
    assert_eq!(policy["fields"]["effective_at"], "2026-04-01");
}

#[test]
fn check_rejects_active_policy_with_future_effective_at() {
    let workspace = TestWorkspace::new("policy-future-effective-at");
    workspace.write("policy.adoc", FUTURE_EFFECTIVE_AT_POLICY);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "policy.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an active policy with a future effective_at"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.policy_future_effective_at]"),
        "expected policy future-effective-at diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

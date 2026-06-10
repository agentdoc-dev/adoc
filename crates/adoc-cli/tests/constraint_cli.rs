mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const VALID_CONSTRAINT: &str = "\
# Auth Constraints @doc(team.auth)

Security rules for authentication.

::constraint auth.session.no-local-storage
severity: critical
owner: platform-security
impacts: [crates/auth/src/session.rs]
--
Session tokens must not be stored in localStorage.
::
";

const INVALID_SEVERITY_CONSTRAINT: &str = "\
# Auth Constraints @doc(team.auth)

::constraint auth.session.no-local-storage
severity: catastrophic
--
Session tokens must not be stored in localStorage.
::
";

#[test]
fn check_accepts_valid_constraint() {
    let workspace = TestWorkspace::new("constraint-check-ok");
    workspace.write("constraint.adoc", VALID_CONSTRAINT);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "constraint.adoc"])
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
fn build_emits_constraint_into_graph_v3() {
    let workspace = TestWorkspace::new("constraint-build");
    workspace.write("constraint.adoc", VALID_CONSTRAINT);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "constraint.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v3");

    let constraint = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "constraint")
        .expect("graph contains a constraint node");

    assert_eq!(constraint["id"], "auth.session.no-local-storage");
    // `status` keeps carrying the severity discriminant within adoc.graph.v3;
    // the dedicated `severity` field is the ADR-0035 dual-emit.
    assert_eq!(constraint["status"], "critical");
    assert_eq!(constraint["severity"], "critical");
    assert_eq!(
        constraint["body"],
        "Session tokens must not be stored in localStorage."
    );
    assert_eq!(constraint["impacts"][0], "crates/auth/src/session.rs");
}

#[test]
fn check_rejects_invalid_constraint_severity() {
    let workspace = TestWorkspace::new("constraint-check-invalid");
    workspace.write("constraint.adoc", INVALID_SEVERITY_CONSTRAINT);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "constraint.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an invalid constraint severity"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.constraint_invalid_severity]"),
        "expected constraint invalid-severity diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

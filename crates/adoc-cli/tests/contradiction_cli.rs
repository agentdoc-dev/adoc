mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// A page with two claims and a valid contradiction referencing both.
const TWO_CLAIMS_AND_CONTRADICTION: &str = "\
# Auth Contradictions @doc(auth.contradictions)

Two conflicting claims about session storage.

::claim auth.a
status: plain
--
Session tokens must be stored in memory only.
::

::claim auth.b
status: plain
--
Session tokens may be stored in localStorage for convenience.
::

::contradiction auth.session.conflict
severity: high
status: unresolved
claims: [auth.a, auth.b]
--
Claim auth.a requires memory-only storage while auth.b permits localStorage.
This creates an unresolved conflict in session storage guidance.
::
";

/// A contradiction with only one claim (too few).
const ONE_CLAIM_CONTRADICTION: &str = "\
# Auth Contradictions @doc(auth.contradictions)

::claim auth.a
status: plain
--
Session tokens must be stored in memory only.
::

::contradiction auth.session.conflict
severity: high
status: unresolved
claims: [auth.a]
--
This has too few claims.
::
";

/// A contradiction referencing a nonexistent claim.
const NONEXISTENT_CLAIM_CONTRADICTION: &str = "\
# Auth Contradictions @doc(auth.contradictions)

::claim auth.a
status: plain
--
Session tokens must be stored in memory only.
::

::contradiction auth.session.conflict
severity: high
status: unresolved
claims: [auth.a, auth.does-not-exist]
--
References a claim that does not exist.
::
";

#[test]
fn check_accepts_contradiction_with_two_valid_claims() {
    let workspace = TestWorkspace::new("contradiction-check-ok");
    workspace.write("contradiction.adoc", TWO_CLAIMS_AND_CONTRADICTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "contradiction.adoc"])
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
fn check_rejects_contradiction_with_one_claim() {
    let workspace = TestWorkspace::new("contradiction-one-claim");
    workspace.write("contradiction.adoc", ONE_CLAIM_CONTRADICTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "contradiction.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for too-few claims"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.contradiction_claims_too_few]"),
        "expected claims_too_few diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_contradiction_with_nonexistent_claim() {
    let workspace = TestWorkspace::new("contradiction-nonexistent-claim");
    workspace.write("contradiction.adoc", NONEXISTENT_CLAIM_CONTRADICTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "contradiction.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for nonexistent claim"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.contradiction_claim_not_found]"),
        "expected claim_not_found diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        combined.contains("auth.does-not-exist"),
        "diagnostic must mention the missing claim id, got:\n{combined}"
    );
}

#[test]
fn build_emits_contradiction_graph_node_with_expected_fields() {
    let workspace = TestWorkspace::new("contradiction-build");
    workspace.write("contradiction.adoc", TWO_CLAIMS_AND_CONTRADICTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "contradiction.adoc", "--out", "dist"])
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

    let node = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "contradiction")
        .expect("graph contains a contradiction node");

    assert_eq!(node["id"], "auth.session.conflict");
    // status carries the contradiction lifecycle status (discriminant slot).
    assert_eq!(node["status"], "unresolved");
    // severity is a typed metadata field.
    assert_eq!(node["fields"]["severity"], "high");
    // contradiction_claims list contains both claim ids (sorted).
    let claims = node["contradiction_claims"]
        .as_array()
        .expect("contradiction_claims is an array");
    assert!(
        claims.iter().any(|v| v == "auth.a"),
        "contradiction_claims must contain auth.a: {node}"
    );
    assert!(
        claims.iter().any(|v| v == "auth.b"),
        "contradiction_claims must contain auth.b: {node}"
    );
}

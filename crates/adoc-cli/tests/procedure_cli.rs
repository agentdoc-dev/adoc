mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const VERIFIED_PROCEDURE: &str = "\
# Auth Runbooks @doc(team.auth)

How to operate authentication safely.

::procedure auth.key.rotate
status: verified
owner: platform-security
verified_at: 2026-05-06
human_review: ran end-to-end in staging
impacts: [crates/auth/src/key.rs]
--
1. Open the secrets console.
2. Rotate the signing key.
3. Redeploy the auth service.
4. Verify the health endpoint.
::
";

const MISSING_STATUS_PROCEDURE: &str = "\
# Auth Runbooks @doc(team.auth)

::procedure auth.key.rotate
owner: platform-security
--
1. Open the secrets console.
2. Rotate the signing key.
::
";

const PROSE_BODY_PROCEDURE: &str = "\
# Auth Runbooks @doc(team.auth)

::procedure auth.key.rotate
status: draft
--
First open the console, then rotate the key.
::
";

#[test]
fn check_accepts_valid_verified_procedure() {
    let workspace = TestWorkspace::new("procedure-check-ok");
    workspace.write("procedure.adoc", VERIFIED_PROCEDURE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "procedure.adoc"])
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
fn build_renders_ordered_steps_and_emits_procedure_into_graph_v3() {
    let workspace = TestWorkspace::new("procedure-build");
    workspace.write("procedure.adoc", VERIFIED_PROCEDURE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "procedure.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // HTML: four steps render as an <ol> with four <li> items in source order.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    let ol_start = html.find("<ol>").expect("rendered <ol>");
    let ol_end = html[ol_start..].find("</ol>").expect("closing </ol>") + ol_start;
    let item_count = html[ol_start..ol_end].matches("<li>").count();
    assert_eq!(item_count, 4, "expected four <li> steps\n{html}");
    assert!(
        html.contains("<li>Open the secrets console.</li>"),
        "first step text with marker stripped\n{html}"
    );

    // Graph: the procedure node is emitted into adoc.graph.v3 with verified metadata.
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v3");

    let procedure = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "procedure")
        .expect("graph contains a procedure node");

    assert_eq!(procedure["id"], "auth.key.rotate");
    assert_eq!(procedure["status"], "verified");
    assert_eq!(
        procedure["body"],
        "1. Open the secrets console.\n2. Rotate the signing key.\n3. Redeploy the auth service.\n4. Verify the health endpoint."
    );
    assert_eq!(procedure["impacts"][0], "crates/auth/src/key.rs");
    // Verified metadata is recorded on the node fields.
    assert_eq!(procedure["fields"]["owner"], "platform-security");
    assert_eq!(procedure["fields"]["verified_at"], "2026-05-06");
    assert_eq!(
        procedure["fields"]["human_review"],
        "ran end-to-end in staging"
    );
}

#[test]
fn check_rejects_procedure_missing_status() {
    let workspace = TestWorkspace::new("procedure-missing-status");
    workspace.write("procedure.adoc", MISSING_STATUS_PROCEDURE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "procedure.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a procedure missing status"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.procedure_missing_status]"),
        "expected procedure missing-status diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_procedure_body_without_ordered_list() {
    let workspace = TestWorkspace::new("procedure-prose-body");
    workspace.write("procedure.adoc", PROSE_BODY_PROCEDURE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "procedure.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a procedure body that is not an ordered list"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.procedure_body_must_start_with_ordered_list]"),
        "expected ordered-list body diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

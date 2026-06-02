mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// A valid source block: `kind: source_code` + `path`.
const SOURCE_CODE_WITH_PATH: &str = "\
# Sources @doc(billing.sources)

Evidence pointers for the billing feature.

::source billing.consume-use-case
kind: source_code
path: apps/backend/src/features/credits/consume.use-case.ts
owner: backend-platform
--
Implementation of credit consumption.
::
";

/// A source with both `path` and `url` (conflicting).
const SOURCE_CONFLICTING_PATH_AND_URL: &str = "\
# Sources @doc(billing.sources)

::source billing.conflict
kind: source_code
path: src/main.rs
url: https://example.com/main
--
This source has both path and url.
::
";

/// A source where `kind: external_url` is paired with a `path` (mismatch).
const SOURCE_KIND_TARGET_MISMATCH: &str = "\
# Sources @doc(billing.sources)

::source billing.mismatch
kind: external_url
path: src/main.rs
--
external_url must not use a path target.
::
";

/// A source with neither `path` nor `url`.
const SOURCE_MISSING_TARGET: &str = "\
# Sources @doc(billing.sources)

::source billing.no-target
kind: source_code
--
No path or url provided.
::
";

/// A valid source block: `kind: external_url` + `url`.
const SOURCE_EXTERNAL_URL_WITH_URL: &str = "\
# Sources @doc(billing.sources)

::source billing.external-ref
kind: external_url
url: https://example.com/credits-api
owner: backend-platform
--
External documentation for the credits API.
::
";

#[test]
fn check_accepts_source_code_with_path() {
    let workspace = TestWorkspace::new("source-check-ok");
    workspace.write("sources.adoc", SOURCE_CODE_WITH_PATH);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "sources.adoc"])
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
fn build_emits_source_graph_node_with_expected_fields() {
    let workspace = TestWorkspace::new("source-build");
    workspace.write("sources.adoc", SOURCE_CODE_WITH_PATH);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "sources.adoc", "--out", "dist"])
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
        .find(|node| node["kind"] == "source")
        .expect("graph contains a source node");

    assert_eq!(node["id"], "billing.consume-use-case");
    // evidence kind is projected into fields["kind"]
    assert_eq!(node["fields"]["kind"], "source_code");
    // repo-relative path is projected into fields["path"]
    assert_eq!(
        node["fields"]["path"],
        "apps/backend/src/features/credits/consume.use-case.ts"
    );
    // source nodes have no status discriminant
    assert!(node["status"].is_null(), "source must have no status");
    // body carries the prose description
    assert!(
        node["body"]
            .as_str()
            .unwrap_or("")
            .contains("Implementation of credit consumption"),
        "body must contain the description: {node}"
    );
}

#[test]
fn check_rejects_source_with_conflicting_path_and_url() {
    let workspace = TestWorkspace::new("source-conflict");
    workspace.write("sources.adoc", SOURCE_CONFLICTING_PATH_AND_URL);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "sources.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for conflicting path and url"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.source_conflicting_path_and_url]"),
        "expected source_conflicting_path_and_url diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_source_with_kind_target_mismatch() {
    let workspace = TestWorkspace::new("source-mismatch");
    workspace.write("sources.adoc", SOURCE_KIND_TARGET_MISMATCH);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "sources.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for kind/target mismatch"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.source_kind_target_mismatch]"),
        "expected source_kind_target_mismatch diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_source_with_missing_target() {
    let workspace = TestWorkspace::new("source-missing-target");
    workspace.write("sources.adoc", SOURCE_MISSING_TARGET);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "sources.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for missing path or url"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.source_missing_path_or_url]"),
        "expected source_missing_path_or_url diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_accepts_external_url_with_url() {
    let workspace = TestWorkspace::new("source-external-url-ok");
    workspace.write("sources.adoc", SOURCE_EXTERNAL_URL_WITH_URL);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "sources.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass for external_url + url\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

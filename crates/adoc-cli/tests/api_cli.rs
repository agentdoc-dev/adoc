//! V6.5.1 `api` Knowledge Object CLI acceptance (PRD §13.7, ADR-0039).

mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// The PRD §13.7 example, verbatim.
const PRD_EXAMPLE_API: &str = "\
# Billing API @doc(team.billing-api)

Billing service API contracts.

::api billing.consume-credit
method: POST
path: /api/billing/credits/consume
status: verified
source: openapi/billing.yaml#/paths/~1credits~1consume
owner: backend-platform
verified_at: 2026-04-30
--
Consumes one or more credits for a completed generation job.
::
";

const MISSING_METHOD_API: &str = "\
# Billing API @doc(team.billing-api)

Billing service API contracts.

::api billing.consume-credit
path: /api/billing/credits/consume
--
Consumes one or more credits for a completed generation job.
::
";

const VERIFIED_REVIEWED_BY_ONLY_API: &str = "\
# Billing API @doc(team.billing-api)

Billing service API contracts.

::api billing.consume-credit
method: POST
path: /api/billing/credits/consume
status: verified
owner: backend-platform
verified_at: 2026-04-30
reviewed_by: api-guild
--
Consumes one or more credits for a completed generation job.
::
";

const IMPACTS_API: &str = "\
# Billing API @doc(team.billing-api)

Billing service API contracts.

::api billing.consume-credit
method: POST
path: /api/billing/credits/consume
status: verified
source: openapi/billing.yaml#/paths/~1credits~1consume
owner: backend-platform
verified_at: 2026-04-30
impacts: [openapi/billing.yaml]
--
Consumes one or more credits for a completed generation job.
::
";

#[test]
fn build_accepts_prd_example_and_emits_api_into_graph_v4() {
    let workspace = TestWorkspace::new("api-build");
    workspace.write("api.adoc", PRD_EXAMPLE_API);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "api.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected the PRD §13.7 example to build\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // Graph: kind api with method and path preserved in the hashed fields map.
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v4");

    let api = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "api")
        .expect("graph contains an api node");

    assert_eq!(api["id"], "billing.consume-credit");
    assert_eq!(api["status"], "verified");
    assert_eq!(api["fields"]["method"], "POST");
    assert_eq!(api["fields"]["path"], "/api/billing/credits/consume");
    // ADR-0039: api is born lifecycle-only — no severity/trust carriers.
    assert!(api.get("severity").is_none());
    assert!(api.get("trust").is_none());

    // HTML: endpoint signature above the body — method badge + code-style path.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("<span class=\"api__method\">POST</span>"),
        "html must contain the method badge\n{html}"
    );
    assert!(
        html.contains("<code class=\"api__path\">/api/billing/credits/consume</code>"),
        "html must contain the code-style path\n{html}"
    );
}

#[test]
fn check_rejects_api_without_method_or_interface_type() {
    let workspace = TestWorkspace::new("api-missing-method");
    workspace.write("api.adoc", MISSING_METHOD_API);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "api.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an api with neither method nor interface_type"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.api_missing_method_or_interface_type]"),
        "expected missing method/interface_type diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_verified_api_with_only_reviewed_by_evidence() {
    let workspace = TestWorkspace::new("api-reviewed-by-only");
    workspace.write("api.adoc", VERIFIED_REVIEWED_BY_ONLY_API);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "api.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a verified api whose only evidence is reviewed_by"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[api.verified_missing_schema_evidence]"),
        "expected schema-evidence diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn impacted_by_covers_verified_api_impacts_declaration() {
    let workspace = TestWorkspace::new("api-impacted-by");
    workspace.write("api.adoc", IMPACTS_API);

    let build = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "api.adoc", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("adoc build runs");
    assert!(
        build.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&build),
        stderr(&build)
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["impacted-by", "openapi/billing.yaml", "--format", "json"])
        .output()
        .expect("adoc impacted-by runs");
    assert_eq!(output.status.code(), Some(0));

    let envelope: Value = serde_json::from_str(&stdout(&output)).unwrap_or_else(|error| {
        panic!(
            "impacted-by stdout is JSON: {error}\nstdout:\n{}\nstderr:\n{}",
            stdout(&output),
            stderr(&output)
        )
    });
    let impacted_ids: Vec<&str> = envelope["impacted"]
        .as_array()
        .expect("impacted array")
        .iter()
        .map(|record| record["id"].as_str().expect("id"))
        .collect();
    assert!(
        impacted_ids.contains(&"billing.consume-credit"),
        "impacted-by must flag the verified api for its declared path: {impacted_ids:?}"
    );
}

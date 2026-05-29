mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const VERIFIED_EXAMPLE: &str = "\
# Billing Examples @doc(team.billing)

Usage examples for the credits API.

::example billing.credits.limit-rejection
lang: ts
status: verified
checks: npm run test -- credits
sandbox: node-test
owner: team-billing
--
expect(result.error).toBe(\"credits.limitExceeded\");
::
";

const NON_EXECUTABLE_EXAMPLE: &str = "\
# Billing Examples @doc(team.billing)

::example billing.credits.snippet
lang: ts
--
const remaining = credits.balance - amount;
::
";

const VERIFIED_MISSING_SANDBOX: &str = "\
# Billing Examples @doc(team.billing)

::example billing.credits.no-sandbox
lang: ts
status: verified
checks: npm run test -- credits
--
expect(result.error).toBe(\"credits.limitExceeded\");
::
";

const VERIFIED_MISSING_CHECKS: &str = "\
# Billing Examples @doc(team.billing)

::example billing.credits.no-checks
lang: ts
status: verified
sandbox: node-test
--
expect(result.error).toBe(\"credits.limitExceeded\");
::
";

const MISSING_LANG_AND_FORMAT: &str = "\
# Billing Examples @doc(team.billing)

::example billing.credits.no-lang
--
expect(result.error).toBe(\"credits.limitExceeded\");
::
";

#[test]
fn check_accepts_valid_verified_example() {
    let workspace = TestWorkspace::new("example-check-verified");
    workspace.write("example.adoc", VERIFIED_EXAMPLE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "example.adoc"])
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
fn check_accepts_non_executable_example() {
    let workspace = TestWorkspace::new("example-check-non-executable");
    workspace.write("example.adoc", NON_EXECUTABLE_EXAMPLE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "example.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected check to pass for a lang-only example\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn build_emits_example_into_graph_v3() {
    let workspace = TestWorkspace::new("example-build");
    workspace.write("example.adoc", VERIFIED_EXAMPLE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "example.adoc", "--out", "dist"])
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

    let example = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "example")
        .expect("graph contains an example node");

    assert_eq!(example["id"], "billing.credits.limit-rejection");
    assert_eq!(example["status"], "verified");
    assert_eq!(
        example["body"],
        "expect(result.error).toBe(\"credits.limitExceeded\");"
    );
    // Typed example fields are recorded on the graph node `fields` map.
    assert_eq!(example["fields"]["lang"], "ts");
    assert_eq!(example["fields"]["checks"], "npm run test -- credits");
    assert_eq!(example["fields"]["sandbox"], "node-test");
}

#[test]
fn check_rejects_verified_example_missing_sandbox() {
    let workspace = TestWorkspace::new("example-check-no-sandbox");
    workspace.write("example.adoc", VERIFIED_MISSING_SANDBOX);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "example.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a verified example missing sandbox"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.example_verified_requires_sandbox]"),
        "expected verified-requires-sandbox diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_verified_example_missing_checks() {
    let workspace = TestWorkspace::new("example-check-no-checks");
    workspace.write("example.adoc", VERIFIED_MISSING_CHECKS);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "example.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a verified example missing checks"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.example_verified_requires_checks]"),
        "expected verified-requires-checks diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_example_missing_lang_and_format() {
    let workspace = TestWorkspace::new("example-check-no-lang");
    workspace.write("example.adoc", MISSING_LANG_AND_FORMAT);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "example.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an example without lang or format"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.example_missing_lang]"),
        "expected missing-lang diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

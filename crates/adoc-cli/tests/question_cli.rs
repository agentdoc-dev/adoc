//! V6.5.3 `question` Knowledge Object CLI acceptance (PRD §13.10).

mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// The PRD §13.10 example, verbatim.
const PRD_EXAMPLE_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::question billing.trial-credit-expiration
owner: product-growth
status: open
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

const MISSING_STATUS_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::question billing.trial-credit-expiration
owner: product-growth
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

const ANSWERED_WITHOUT_RESOLVED_BY_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::question billing.trial-credit-expiration
owner: product-growth
status: answered
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

const OPEN_WITH_RESOLVED_BY_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::question billing.trial-credit-expiration
owner: product-growth
status: open
resolved_by: billing.credits-expire
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

const RESOLVED_BY_NOT_FOUND_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::question billing.trial-credit-expiration
owner: product-growth
status: answered
resolved_by: billing.no-such-object
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

const RESOLVED_BY_GLOSSARY_QUESTION: &str = "\
# Billing Questions @doc(team.billing-questions)

Open questions tracked by the billing team.

::glossary billing.trial-credits
status: draft
--
Trial credits are promotional balance granted at signup.
::

::question billing.trial-credit-expiration
owner: product-growth
status: answered
resolved_by: billing.trial-credits
--
Should unused trial credits expire after 30 days or remain available indefinitely?
::
";

#[test]
fn build_accepts_prd_example_and_emits_question_into_graph() {
    let workspace = TestWorkspace::new("question-build");
    workspace.write("questions.adoc", PRD_EXAMPLE_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "questions.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected the PRD §13.10 example to build\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // Graph: kind question with lifecycle status and owner in fields.
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    let question = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "question")
        .expect("graph contains a question node");

    assert_eq!(question["id"], "billing.trial-credit-expiration");
    assert_eq!(question["status"], "open");
    assert_eq!(question["fields"]["owner"], "product-growth");

    // HTML: open questions carry a prominent Open badge.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("<div class=\"question__open-badge\">Open</div>"),
        "html must contain the open-question badge\n{html}"
    );
}

#[test]
fn check_rejects_question_without_status() {
    let workspace = TestWorkspace::new("question-missing-status");
    workspace.write("questions.adoc", MISSING_STATUS_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "questions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a question without status"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.question_missing_status]"),
        "expected missing-status diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_answered_question_without_resolved_by() {
    let workspace = TestWorkspace::new("question-answered-missing-resolved-by");
    workspace.write("questions.adoc", ANSWERED_WITHOUT_RESOLVED_BY_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "questions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an answered question without resolved_by"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.question_answered_missing_resolved_by]"),
        "expected answered-missing-resolved_by diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_resolved_by_on_open_question() {
    let workspace = TestWorkspace::new("question-open-with-resolved-by");
    workspace.write("questions.adoc", OPEN_WITH_RESOLVED_BY_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "questions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for an open question carrying resolved_by"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.question_unexpected_resolved_by]"),
        "expected unexpected-resolved_by diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_resolved_by_naming_unknown_object() {
    let workspace = TestWorkspace::new("question-resolved-by-not-found");
    workspace.write("questions.adoc", RESOLVED_BY_NOT_FOUND_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "questions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a resolved_by naming a missing object"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.question_resolved_by_not_found]"),
        "expected resolved_by-not-found diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_resolved_by_naming_glossary_object() {
    let workspace = TestWorkspace::new("question-resolved-by-wrong-kind");
    workspace.write("questions.adoc", RESOLVED_BY_GLOSSARY_QUESTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "questions.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a resolved_by naming a glossary object"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.question_resolved_by_wrong_kind]"),
        "expected resolved_by-wrong-kind diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

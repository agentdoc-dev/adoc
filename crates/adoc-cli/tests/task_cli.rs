//! V6.5.4 `task` Knowledge Object CLI acceptance (PRD §13.11).
//!
//! `task.overdue` is clock-dependent by design and the CLI compile path runs
//! on the real clock, so every fixture uses fixed wide-margin dates: past
//! dates fire the warning deterministically, 2120-style far-future dates stay
//! quiet. Never dates relative to now.

mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

/// The PRD §13.11 example, verbatim, plus the claim its `depends_on` names —
/// relation targets must resolve in the workspace.
const PRD_EXAMPLE_TASK: &str = "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::claim billing.credits.refund-on-failed-persistence
status: plain
--
Credits are refunded when persistence fails after generation.
::

::task billing.update-support-runbook
owner: support-ops
status: open
due: 2026-05-20
depends_on: billing.credits.refund-on-failed-persistence
--
Update the support runbook to mention refund behavior after persistence failure.
::
";

/// Far-future `due` (the pilot's 2120/2125 `expires_at` precedent): quiet on
/// any clock.
const FAR_FUTURE_DUE_TASK: &str = "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::task billing.update-support-runbook
owner: support-ops
status: open
due: 2126-05-20
--
Update the support runbook to mention refund behavior after persistence failure.
::
";

const MISSING_OWNER_TASK: &str = "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::task billing.update-support-runbook
status: open
due: 2126-05-20
--
Update the support runbook to mention refund behavior after persistence failure.
::
";

/// Wide-margin past `due` (2020–2024 range): overdue on any clock.
const WIDE_MARGIN_PAST_DUE_TASK: &str = "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::task billing.update-support-runbook
owner: support-ops
status: open
due: 2021-03-15
--
Update the support runbook to mention refund behavior after persistence failure.
::
";

#[test]
fn check_accepts_prd_example_with_exactly_one_overdue_warning() {
    let workspace = TestWorkspace::new("task-prd-check");
    workspace.write("tasks.adoc", PRD_EXAMPLE_TASK);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    // The example's fixed `due: 2026-05-20` is already past, so exactly one
    // `task.overdue` warning fires — warnings never fail the build.
    assert!(
        output.status.success(),
        "expected the PRD §13.11 example to pass check despite the warning\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert_eq!(
        combined.matches("warning[task.overdue]").count(),
        1,
        "expected exactly one task.overdue warning, got:\n{combined}"
    );
}

#[test]
fn build_emits_task_node_and_depends_on_edge_into_graph() {
    let workspace = TestWorkspace::new("task-prd-build");
    workspace.write("tasks.adoc", PRD_EXAMPLE_TASK);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "tasks.adoc", "--out", "dist", "--no-embeddings"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected the PRD §13.11 example to build\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    // Node: kind task, lifecycle-only status, owner and due in the fields map.
    let task = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "task")
        .expect("graph contains a task node");
    assert_eq!(task["id"], "billing.update-support-runbook");
    assert_eq!(task["status"], "open");
    assert_eq!(task["fields"]["owner"], "support-ops");
    assert_eq!(task["fields"]["due"], "2026-05-20");
    assert!(task.get("severity").is_none());
    assert!(task.get("trust").is_none());

    // Edge: the PRD example's depends_on relation appears in graph JSON.
    let has_depends_on_edge = graph["edges"]
        .as_array()
        .expect("edges is an array")
        .iter()
        .any(|edge| {
            edge["kind"] == "relation"
                && edge["relation"] == "depends_on"
                && edge["source"] == "billing.update-support-runbook"
                && edge["target"] == "billing.credits.refund-on-failed-persistence"
        });
    assert!(
        has_depends_on_edge,
        "expected the task depends_on edge in graph JSON:\n{graph_text}"
    );

    // HTML: the task card carries owner, due date, and open/done state.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("<section class=\"task task--open\""),
        "html must carry the open state on the task card\n{html}"
    );
    assert!(
        html.contains("<div class=\"task__field-item\"><dt>owner</dt><dd>support-ops</dd></div>"),
        "html must contain the owner field\n{html}"
    );
    assert!(
        html.contains("<div class=\"task__field-item\"><dt>due</dt><dd>2026-05-20</dd></div>"),
        "html must contain the due date\n{html}"
    );
}

#[test]
fn check_accepts_far_future_due_task_warning_free() {
    let workspace = TestWorkspace::new("task-far-future");
    workspace.write("tasks.adoc", FAR_FUTURE_DUE_TASK);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "expected a far-future due task to pass check\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        !combined.contains("task.overdue"),
        "a far-future due must stay quiet on any clock, got:\n{combined}"
    );
}

#[test]
fn check_rejects_task_without_owner() {
    let workspace = TestWorkspace::new("task-missing-owner");
    workspace.write("tasks.adoc", MISSING_OWNER_TASK);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a task without owner"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.task_missing_owner]"),
        "expected missing-owner diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
}

#[test]
fn check_rejects_task_without_status() {
    let workspace = TestWorkspace::new("task-missing-status");
    workspace.write(
        "tasks.adoc",
        "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::task billing.update-support-runbook
owner: support-ops
--
Update the support runbook to mention refund behavior after persistence failure.
::
",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a task without status"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.task_missing_status]"),
        "expected missing-status diagnostic, got:\n{combined}"
    );
}

#[test]
fn check_rejects_task_with_status_outside_closed_set() {
    let workspace = TestWorkspace::new("task-invalid-status");
    workspace.write(
        "tasks.adoc",
        "\
# Billing Tasks @doc(team.billing-tasks)

Billing documentation action items.

::task billing.update-support-runbook
owner: support-ops
status: in-progress
--
Update the support runbook to mention refund behavior after persistence failure.
::
",
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for a status outside open|done"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.task_invalid_status]"),
        "expected invalid-status diagnostic, got:\n{combined}"
    );
}

#[test]
fn check_emits_exactly_one_overdue_warning_for_wide_margin_past_due() {
    let workspace = TestWorkspace::new("task-past-due");
    workspace.write("tasks.adoc", WIDE_MARGIN_PAST_DUE_TASK);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "tasks.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        output.status.success(),
        "task.overdue is a warning; warnings never fail the build\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert_eq!(
        combined.matches("warning[task.overdue]").count(),
        1,
        "expected exactly one task.overdue warning, got:\n{combined}"
    );
}

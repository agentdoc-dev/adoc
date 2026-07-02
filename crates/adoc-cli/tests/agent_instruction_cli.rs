mod support;

use std::fs;

use serde_json::Value;

use support::{TestWorkspace, adoc_command, stderr, stdout};

const VALID_AGENT_INSTRUCTION: &str = "\
# Agent Instructions @doc(team.auth)

Instructions for agents answering auth questions.

::agent_instruction auth.docs-answering-policy
scope: docs/auth/*
trust: team
owner: ai-platform
allowed_actions: [summarize, cite, suggest_edits]
forbidden_actions: [execute_shell, access_secrets, modify_auth_code]
--
Prefer verified claims over draft notes when answering auth questions.
::
";

const OVERLAPPING_ACTIONS: &str = "\
# Agent Instructions @doc(team.auth)

Instructions for agents answering auth questions.

::agent_instruction auth.docs-answering-policy
scope: docs/auth/*
trust: team
allowed_actions: [cite]
forbidden_actions: [cite]
--
Prefer verified claims over draft notes when answering auth questions.
::
";

const MISSING_SCOPE: &str = "\
# Agent Instructions @doc(team.auth)

Instructions for agents answering auth questions.

::agent_instruction auth.docs-answering-policy
trust: team
allowed_actions: [summarize]
forbidden_actions: [execute_shell]
--
Prefer verified claims over draft notes when answering auth questions.
::
";

const INVALID_TRUST: &str = "\
# Agent Instructions @doc(team.auth)

Instructions for agents answering auth questions.

::agent_instruction auth.docs-answering-policy
scope: docs/auth/*
trust: internal
allowed_actions: [summarize]
forbidden_actions: [execute_shell]
--
Prefer verified claims over draft notes when answering auth questions.
::
";

#[test]
fn check_accepts_valid_agent_instruction() {
    let workspace = TestWorkspace::new("agent-instruction-check-ok");
    workspace.write("agent.adoc", VALID_AGENT_INSTRUCTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "agent.adoc"])
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
fn check_rejects_overlapping_action_sets_naming_the_overlap() {
    let workspace = TestWorkspace::new("agent-instruction-overlap");
    workspace.write("agent.adoc", OVERLAPPING_ACTIONS);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "agent.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(
        !output.status.success(),
        "expected check to fail for overlapping action sets"
    );
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.agent_instruction_actions_not_disjoint]"),
        "expected disjointness diagnostic, got:\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );
    assert!(
        combined.contains("cite"),
        "diagnostic must name the overlapping action `cite`, got:\n{combined}"
    );
}

#[test]
fn check_rejects_agent_instruction_missing_scope() {
    let workspace = TestWorkspace::new("agent-instruction-missing-scope");
    workspace.write("agent.adoc", MISSING_SCOPE);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "agent.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(!output.status.success(), "expected check to fail");
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.agent_instruction_missing_scope]"),
        "expected missing-scope diagnostic, got:\n{combined}"
    );
}

#[test]
fn check_rejects_agent_instruction_invalid_trust() {
    let workspace = TestWorkspace::new("agent-instruction-invalid-trust");
    workspace.write("agent.adoc", INVALID_TRUST);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["check", "agent.adoc"])
        .output()
        .expect("adoc check runs");

    assert!(!output.status.success(), "expected check to fail");
    let combined = format!("{}{}", stdout(&output), stderr(&output));
    assert!(
        combined.contains("error[schema.agent_instruction_invalid_trust]"),
        "expected invalid-trust diagnostic, got:\n{combined}"
    );
}

#[test]
fn build_emits_agent_instruction_into_graph_and_renders_banner() {
    let workspace = TestWorkspace::new("agent-instruction-build");
    workspace.write("agent.adoc", VALID_AGENT_INSTRUCTION);

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args(["build", "agent.adoc", "--out", "dist"])
        .output()
        .expect("adoc build runs");

    assert!(
        output.status.success(),
        "expected build to pass\nstdout:\n{}\nstderr:\n{}",
        stdout(&output),
        stderr(&output)
    );

    // HTML: the mandatory "NOT runtime ACL" banner and the guide link.
    let html = fs::read_to_string(workspace.root.join("dist").join("docs.html"))
        .expect("html artifact is written");
    assert!(
        html.contains("NOT runtime ACL"),
        "html must contain the runtime-not-enforced banner\n{html}"
    );
    assert!(
        html.contains("adoc://agent/v0/agent-instruction-guide"),
        "banner must link to the agent-instruction guide\n{html}"
    );

    // Graph: the agent_instruction node carries kind, trust, scope, and both
    // action sets (schema stays adoc.graph.v4 — additive).
    let graph_text = fs::read_to_string(workspace.root.join("dist").join("docs.graph.json"))
        .expect("graph artifact is written");
    let graph: Value = serde_json::from_str(&graph_text).expect("graph json parses");

    assert_eq!(graph["schema_version"], "adoc.graph.v4");

    let node = graph["nodes"]
        .as_array()
        .expect("nodes is an array")
        .iter()
        .find(|node| node["kind"] == "agent_instruction")
        .expect("graph contains an agent_instruction node");

    assert_eq!(node["id"], "auth.docs-answering-policy");
    // ADR-0039: agent_instruction has no lifecycle status; `trust` is the
    // sole, authored, hashed carrier.
    assert_eq!(node["status"], Value::Null);
    assert_eq!(node["trust"], "team");
    assert_eq!(node["fields"]["scope"], "docs/auth/*");
    assert!(
        node["allowed_actions"]
            .as_array()
            .expect("allowed_actions is an array")
            .iter()
            .any(|v| v == "summarize"),
        "allowed_actions must contain summarize: {node}"
    );
    assert!(
        node["forbidden_actions"]
            .as_array()
            .expect("forbidden_actions is an array")
            .iter()
            .any(|v| v == "execute_shell"),
        "forbidden_actions must contain execute_shell: {node}"
    );
}

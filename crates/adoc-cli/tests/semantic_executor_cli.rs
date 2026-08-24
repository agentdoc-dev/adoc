mod support;

use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command, stderr, stdout};

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";

fn context_input() -> Value {
    json!({
        "schema_version": "adoc.semantic_context_input.v0",
        "evaluation_date": "2026-08-24",
        "subject_revision": {"system": "git", "value": "head-sha"},
        "source_revision": {"system": "git", "value": "head-sha"},
        "base_revision": {"system": "git", "value": "base-sha"},
        "head_revision": {"system": "git", "value": "head-sha"},
        "basis": {
            "assessment_digest": A,
            "knowledge_basis": {"kind": "graph_artifact", "digest": B}
        },
        "selection": {
            "algorithm": "action-bounded-lexical",
            "version": "1",
            "authorized_scope": ["repo:agentdoc/test"]
        },
        "capability_policy": {
            "version": "semantic-context-policy-v1",
            "rules": [
                {"reason": "permission", "outcome": "insufficient"},
                {"reason": "retention", "outcome": "insufficient"},
                {"reason": "source_outage", "outcome": "failed"},
                {"reason": "truncation", "outcome": "insufficient"},
                {"reason": "resource_limit", "outcome": "insufficient"}
            ]
        },
        "context_classes": [{
            "class_id": "changed_knowledge", "requirement": "required", "byte_budget": 4096
        }],
        "items": [
            {
                "handle_id": "hunk-a", "class_id": "changed_knowledge",
                "scope_ref": "repo:agentdoc/test",
                "handle": {"kind": "diff_hunk", "changed_source_id": "src/billing.rs", "hunk_digest": A},
                "content": {"diff": "+ durable billing behavior"}, "truncated": false
            },
            {
                "handle_id": "object-a", "class_id": "changed_knowledge",
                "scope_ref": "repo:agentdoc/test",
                "handle": {"kind": "knowledge_object", "object_id": "billing.policy", "semantic_hash": B},
                "content": {"body": "Current billing policy."}, "truncated": false
            }
        ],
        "unavailability": []
    })
}

fn assessment(context_digest: &str) -> Value {
    json!({
        "schema_version": "adoc.semantic_assessment.v0",
        "context_digest": context_digest,
        "base_revision": {"system": "git", "value": "base-sha"},
        "head_revision": {"system": "git", "value": "head-sha"},
        "identity": {"provider": "codex", "model": "gpt-5.6-codex"},
        "materiality_policy_version": "adoc.materiality.v0",
        "scope": {"handle_ids": ["hunk-a", "object-a"]},
        "findings": [{
            "finding_id": "finding-001",
            "classification": "extends_existing_knowledge",
            "affected_objects": [{"object_id": "billing.policy", "content_hash": B}],
            "citations": ["hunk-a", "object-a"],
            "materiality": "material",
            "proposed_disposition": "update_existing",
            "candidate_updates": [{"object_id": "billing.policy", "body": "Updated.", "fields": {}}],
            "unresolved_questions": [],
            "explanation": "The change extends billing behavior."
        }]
    })
}

#[test]
fn semantic_executor_cli_builds_context_and_records_completed_or_failed_validation() {
    let workspace = TestWorkspace::new("semantic-executor-cli");
    workspace.write(
        "context-input.json",
        &serde_json::to_string_pretty(&context_input()).expect("serializes"),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "semantic-context",
            "--input",
            "context-input.json",
            "--out",
            "context.json",
        ])
        .output()
        .expect("context command runs");
    assert!(output.status.success(), "{}", stderr(&output));
    let context: Value = serde_json::from_str(
        &std::fs::read_to_string(workspace.root.join("context.json")).expect("context exists"),
    )
    .expect("context JSON");

    let request = json!({
        "schema_version": "adoc.semantic_executor_request.v0",
        "request_id": "request-1",
        "capability": "code_change_assessment",
        "adapter": {
            "kind": "codex", "provider": "codex", "model": "gpt-5.6-codex",
            "endpoint_class": "public_provider", "endpoint_id": "openai",
            "executor_digest": A, "model_digest": B, "config_digest": A
        },
        "task_digest": B,
        "prompt": {"contract_version": "semantic-assessment-task-v1", "digest": A,
            "instructions": "Return structured JSON."},
        "timeout_seconds": 600,
        "context": context
    });
    workspace.write(
        "request.json",
        &serde_json::to_string_pretty(&request).expect("serializes"),
    );
    workspace.write(
        "assessment.json",
        &serde_json::to_string_pretty(&assessment(
            request["context"]["context_digest"]
                .as_str()
                .expect("digest"),
        ))
        .expect("serializes"),
    );

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "semantic-executor",
            "--request",
            "request.json",
            "--assessment",
            "assessment.json",
            "--receipt",
            "receipt.json",
            "--validated-assessment",
            "validated.json",
        ])
        .output()
        .expect("executor command runs");
    assert!(output.status.success(), "{}", stderr(&output));
    assert!(stdout(&output).contains("\"outcome\": \"completed\""));
    assert!(workspace.root.join("validated.json").is_file());

    let mut invalid = assessment(
        request["context"]["context_digest"]
            .as_str()
            .expect("digest"),
    );
    invalid["findings"][0]["citations"] = json!(["fabricated"]);
    invalid["scope"]["handle_ids"] = json!(["fabricated"]);
    workspace.write(
        "assessment.json",
        &serde_json::to_string_pretty(&invalid).expect("serializes"),
    );
    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "semantic-executor",
            "--request",
            "request.json",
            "--assessment",
            "assessment.json",
            "--receipt",
            "failed.json",
            "--validated-assessment",
            "never-written.json",
        ])
        .output()
        .expect("executor command runs");
    assert_eq!(output.status.code(), Some(2));
    let failed =
        std::fs::read_to_string(workspace.root.join("failed.json")).expect("failure receipt");
    assert!(failed.contains("\"outcome\": \"failed\""));
    assert!(failed.contains("assessment.semantic_citation_invalid"));
    assert!(!workspace.root.join("never-written.json").exists());

    let output = adoc_command()
        .current_dir(&workspace.root)
        .args([
            "semantic-executor",
            "--request",
            "request.json",
            "--assessment",
            "assessment.json",
            "--failure-code",
            "provider_timeout",
            "--receipt",
            "timeout.json",
            "--validated-assessment",
            "never-timeout.json",
        ])
        .output()
        .expect("executor failure command runs");
    assert_eq!(output.status.code(), Some(2));
    let timeout =
        std::fs::read_to_string(workspace.root.join("timeout.json")).expect("timeout receipt");
    assert!(timeout.contains("\"failure_code\": \"provider_timeout\""));
    assert!(!workspace.root.join("never-timeout.json").exists());
}

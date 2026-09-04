//! E5.5.T1 - internal/synthetic producer-side tracer through existing contracts.

use std::{fmt::Write as _, process::Command};

use adoc_core::{
    AssessmentCompleteness, AssessmentOutcome, ChangeAssessmentInput, ExactRevision,
    ExecutorAuthority, ExecutorConfiguration, ExecutorQualificationExpectedBindings,
    ModelConfiguration, ProposalBindings, ProposalChangeRequest, ProposalPatchInput,
    SEMANTIC_EXECUTOR_RECEIPT_SCHEMA_VERSION, assess_changes_from_git, build_proposal_record,
    build_semantic_context_from_document, complete_semantic_execution, semantic_prompt_digest,
    validate_executor_qualification, validate_semantic_assessment,
    validate_semantic_executor_request,
};
use chrono::NaiveDate;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

const GRAPH: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const EXECUTOR: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const MODEL: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const CONFIG: &str = "sha256:eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const TASK: &str = "sha256:ffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffffff";
const POLICY: &str = "sha256:9999999999999999999999999999999999999999999999999999999999999999";

struct Repo {
    root: TempDir,
}

impl Repo {
    fn new() -> Self {
        let repo = Self {
            root: TempDir::new().expect("tempdir"),
        };
        repo.git(&["init", "--initial-branch=main"]);
        repo.git(&["config", "user.email", "test@agentdoc.dev"]);
        repo.git(&["config", "user.name", "AgentDoc Test"]);
        repo.git(&["config", "commit.gpgsign", "false"]);
        repo.write(
            "agentdoc.config.yaml",
            "version: 1\nmode: strict\ndocs_path: docs\noutputs:\n  dir: dist\nembeddings:\n  provider: none\n",
        );
        repo.write(
            "docs/billing.adoc",
            concat!(
                "# Billing @doc(team.billing)\n\n",
                "::claim billing.policy\n",
                "status: verified\n",
                "owner: billing-platform\n",
                "verified_at: 2026-09-01\n",
                "source: src/billing.rs\n",
                "impacts: [src/billing.rs]\n",
                "--\nCredits settle after payment.\n::\n",
            ),
        );
        repo.write("src/billing.rs", "pub fn settle() {}\n");
        repo.git(&["add", "-A"]);
        repo.git(&["commit", "-m", "base"]);
        repo
    }

    fn root_path(&self) -> std::path::PathBuf {
        self.root.path().to_path_buf()
    }

    fn write(&self, relative_path: &str, contents: &str) {
        let path = self.root.path().join(relative_path);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).expect("parent dir");
        }
        std::fs::write(path, contents).expect("write fixture");
    }

    fn git(&self, args: &[&str]) -> String {
        let output = Command::new("git")
            .current_dir(self.root.path())
            .args(args)
            .output()
            .expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        String::from_utf8(output.stdout)
            .expect("UTF-8 git output")
            .trim()
            .to_string()
    }
}

fn digest(bytes: &[u8]) -> String {
    let mut output = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        write!(&mut output, "{byte:02x}").expect("write digest");
    }
    output
}

fn canonical_bytes(value: &impl serde::Serialize) -> Vec<u8> {
    let mut json = serde_json::to_vec_pretty(value).expect("serializes");
    json.push(b'\n');
    json
}

fn revision(value: &str) -> ExactRevision {
    ExactRevision {
        system: "git".to_string(),
        value: value.to_string(),
    }
}

fn context_input(
    assessment_digest: &str,
    graph_digest: &str,
    content_hash: &str,
    base: &str,
    head: &str,
) -> Value {
    json!({
        "schema_version": "adoc.semantic_context_input.v0",
        "evaluation_date": "2026-09-30",
        "subject_revision": {"system": "git", "value": head},
        "source_revision": {"system": "git", "value": head},
        "base_revision": {"system": "git", "value": base},
        "head_revision": {"system": "git", "value": head},
        "basis": {
            "assessment_digest": assessment_digest,
            "knowledge_basis": {"kind": "graph_artifact", "digest": graph_digest}
        },
        "selection": {
            "algorithm": "internal-synthetic-tracer",
            "version": "1",
            "authorized_scope": ["repo:agentdoc/internal-synthetic"]
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
            "class_id": "changed_knowledge",
            "requirement": "required",
            "byte_budget": 4096
        }],
        "items": [{
            "handle_id": "hunk-a",
            "class_id": "changed_knowledge",
            "scope_ref": "repo:agentdoc/internal-synthetic",
            "handle": {"kind": "diff_hunk", "changed_source_id": "src/billing.rs", "hunk_digest": assessment_digest},
            "content": {"diff": "+ durable billing behavior"},
            "truncated": false
        }, {
            "handle_id": "object-a",
            "class_id": "changed_knowledge",
            "scope_ref": "repo:agentdoc/internal-synthetic",
            "handle": {"kind": "knowledge_object", "object_id": "billing.policy", "semantic_hash": content_hash},
            "content": {"body": "Current billing policy."},
            "truncated": false
        }],
        "unavailability": []
    })
}

fn semantic_assessment(context_digest: &str, content_hash: &str, base: &str, head: &str) -> Value {
    json!({
        "schema_version": "adoc.semantic_assessment.v0",
        "context_digest": context_digest,
        "base_revision": {"system": "git", "value": base},
        "head_revision": {"system": "git", "value": head},
        "identity": {"provider": "codex", "model": "gpt-5.6-codex"},
        "materiality_policy_version": "adoc.materiality.v0",
        "scope": {"handle_ids": ["hunk-a", "object-a"]},
        "findings": [{
            "finding_id": "finding-001",
            "classification": "extends_existing_knowledge",
            "affected_objects": [{"object_id": "billing.policy", "content_hash": content_hash}],
            "citations": ["hunk-a", "object-a"],
            "materiality": "material",
            "proposed_disposition": "update_existing",
            "candidate_updates": [{"object_id": "billing.policy", "body": "Updated billing policy.", "fields": {}}],
            "unresolved_questions": [],
            "explanation": "The synthetic change extends durable billing behavior."
        }]
    })
}

fn qualified_executor() -> (Value, ExecutorConfiguration) {
    (
        json!({
            "schema_version": "adoc.executor_qualification.v0",
            "qualification_id": "internal-synthetic-codex",
            "capability": {"name": "code_change_assessment", "version": "1"},
            "subject": {
                "kind": "model",
                "provider": "codex",
                "executor_digest": EXECUTOR,
                "model_digest": MODEL,
                "config_digest": CONFIG,
                "configuration": {
                    "model_revision_digest": MODEL,
                    "quantization_digest": GRAPH,
                    "system_prompt_task_digest": TASK,
                    "context_strategy_digest": GRAPH,
                    "output_constraints_digest": TASK,
                    "toolset_digest": GRAPH,
                    "inference_parameters_digest": TASK,
                    "safety_configuration_digest": POLICY,
                    "adapter_implementation_digest": EXECUTOR
                }
            },
            "protocol": {"valid": true, "version": "semantic-executor-v1"},
            "agentdoc_evaluation": {
                "kind": "capability",
                "qualified": true,
                "evidence_ref": "internal-synthetic-tracer"
            },
            "organization_approval": {
                "approved": true,
                "scope": ["repo:agentdoc/internal-synthetic"],
                "risk": ["high"],
                "deployment": ["customer_worker"],
                "policy_digest": POLICY
            },
            "runtime_policy": {
                "eligible": true,
                "operation_digest": TASK,
                "policy_digest": POLICY
            }
        }),
        ExecutorConfiguration::Model {
            provider: "codex".to_string(),
            executor_digest: EXECUTOR.to_string(),
            model_digest: MODEL.to_string(),
            config_digest: CONFIG.to_string(),
            configuration: Box::new(ModelConfiguration {
                model_revision_digest: MODEL.to_string(),
                quantization_digest: GRAPH.to_string(),
                system_prompt_task_digest: TASK.to_string(),
                context_strategy_digest: GRAPH.to_string(),
                output_constraints_digest: TASK.to_string(),
                toolset_digest: GRAPH.to_string(),
                inference_parameters_digest: TASK.to_string(),
                safety_configuration_digest: POLICY.to_string(),
                adapter_implementation_digest: EXECUTOR.to_string(),
            }),
        },
    )
}

#[test]
fn internal_synthetic_tracer_links_existing_producer_contracts_by_real_digests() {
    let repo = Repo::new();
    let base = repo.git(&["rev-parse", "HEAD"]);
    repo.write("src/billing.rs", "pub fn settle() { charge(); }\n");
    repo.git(&["add", "src/billing.rs"]);
    repo.git(&["commit", "-m", "change"]);
    let head = repo.git(&["rev-parse", "HEAD"]);

    let assessment = assess_changes_from_git(ChangeAssessmentInput {
        project_root: Some(repo.root_path()),
        base_ref: base.clone(),
        head_ref: Some(head.clone()),
        evaluation_date: NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    });
    assert_eq!(assessment.completeness, AssessmentCompleteness::Complete);
    assert_eq!(assessment.outcome, AssessmentOutcome::ReviewRequired);
    assert_eq!(
        assessment
            .snapshots
            .requested_base
            .resolved_commit
            .as_deref(),
        Some(base.as_str())
    );
    assert_eq!(
        assessment.snapshots.head.resolved_commit.as_deref(),
        Some(head.as_str())
    );
    let assessment_digest = digest(&canonical_bytes(&assessment));
    let graph_digest = assessment
        .knowledge_snapshot
        .graph_sha256
        .as_deref()
        .expect("assessment graph digest");
    let content_hash = assessment
        .objects
        .value
        .as_ref()
        .expect("assessment objects")[0]
        .content_hash
        .clone();

    let context = build_semantic_context_from_document(
        &serde_json::to_vec(&context_input(
            &assessment_digest,
            graph_digest,
            &content_hash,
            &base,
            &head,
        ))
        .expect("context input serializes"),
    )
    .expect("context builds");
    let context_json: Value =
        serde_json::from_str(&context.to_canonical_json().expect("context serializes"))
            .expect("context JSON");
    assert_eq!(
        context_json["basis"]["assessment_digest"],
        assessment_digest
    );
    assert_eq!(
        context_json["basis"]["knowledge_basis"]["digest"],
        graph_digest
    );
    let context_digest = context.context_digest().to_string();

    let instructions = "Return one structured semantic assessment.";
    let prompt_digest =
        semantic_prompt_digest("semantic-assessment-task-v1", instructions).expect("prompt");
    let (qualification_json, current_executor) = qualified_executor();
    let qualification_bytes =
        serde_json::to_vec(&qualification_json).expect("qualification serializes");
    let qualification =
        validate_executor_qualification(&qualification_bytes).expect("qualification validates");
    let eligibility = qualification.evaluate(
        &current_executor,
        TASK,
        &ExecutorQualificationExpectedBindings {
            qualification_id: "internal-synthetic-codex".to_string(),
            record_digest: digest(&qualification_bytes),
            capability_name: "code_change_assessment".to_string(),
            capability_version: "1".to_string(),
            protocol_version: "semantic-executor-v1".to_string(),
            requested_scope: "repo:agentdoc/internal-synthetic".to_string(),
            requested_risk: "high".to_string(),
            requested_deployment: "customer_worker".to_string(),
            organization_policy_digest: POLICY.to_string(),
            runtime_policy_digest: POLICY.to_string(),
        },
    );
    assert_eq!(
        eligibility.authority(),
        ExecutorAuthority::GateAuthoritative
    );

    let request = validate_semantic_executor_request(
        &serde_json::to_vec(&json!({
            "schema_version": "adoc.semantic_executor_request.v0",
            "request_id": "internal-synthetic-request",
            "capability": "code_change_assessment",
            "adapter": {
                "kind": "codex",
                "provider": "codex",
                "model": "gpt-5.6-codex",
                "endpoint_class": "public_provider",
                "endpoint_id": "openai",
                "executor_digest": EXECUTOR,
                "model_digest": MODEL,
                "config_digest": CONFIG
            },
            "task_digest": TASK,
            "prompt": {
                "contract_version": "semantic-assessment-task-v1",
                "digest": prompt_digest,
                "instructions": instructions
            },
            "timeout_seconds": 600,
            "context": context_json
        }))
        .expect("request serializes"),
    )
    .expect("request validates");
    let semantic = validate_semantic_assessment(
        &serde_json::to_vec(&semantic_assessment(
            &context_digest,
            &content_hash,
            &base,
            &head,
        ))
        .expect("semantic assessment serializes"),
        request.context(),
    )
    .expect("semantic assessment validates");
    let semantic_assessment_digest = digest(
        semantic
            .to_canonical_json()
            .expect("semantic serializes")
            .as_bytes(),
    );
    let receipt = complete_semantic_execution(&request, &semantic, None).expect("receipt builds");
    assert_eq!(
        serde_json::to_value(&receipt).expect("receipt JSON")["schema_version"],
        SEMANTIC_EXECUTOR_RECEIPT_SCHEMA_VERSION
    );
    assert_eq!(
        receipt.assessment_digest(),
        Some(semantic_assessment_digest.as_str())
    );
    assert_eq!(receipt.context_digest(), context_digest);

    let patch = json!({
        "schema_version": "adoc.patch.v0",
        "op": "update_fields",
        "target": "billing.policy",
        "base_hash": content_hash,
        "changes": {"fields": {"owner": "billing-platform", "status": "draft"}},
        "reason": format!("AgentDoc assessment {assessment_digest} finding finding-001."),
        "proposer": {"type": "agent", "id": "agentdoc-action/codex/internal-synthetic"}
    });
    let mut patch_bytes = serde_json::to_vec(&patch).expect("patch serializes");
    patch_bytes.push(b'\n');
    let proposal = build_proposal_record(
        ProposalBindings {
            base_revision: revision(&base),
            head_revision: revision(&head),
            change_request: ProposalChangeRequest {
                system: "github_pull_request".to_string(),
                id: "42".to_string(),
            },
            assessment_digest: assessment_digest.clone(),
            semantic_context_digest: context_digest.clone(),
            semantic_assessment_digest: semantic_assessment_digest.clone(),
        },
        vec![ProposalPatchInput {
            finding_id: "finding-001".to_string(),
            placement_path: "docs/billing.adoc".to_string(),
            page_id: "billing.kb".to_string(),
            patch_bytes,
        }],
        None,
    )
    .expect("proposal builds");
    assert_eq!(proposal.bindings().assessment_digest, assessment_digest);
    assert_eq!(proposal.bindings().semantic_context_digest, context_digest);
    assert_eq!(
        proposal.bindings().semantic_assessment_digest,
        semantic_assessment_digest
    );
}

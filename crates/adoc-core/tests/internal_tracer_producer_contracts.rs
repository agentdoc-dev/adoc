//! E5.5.T1-T2 - internal/synthetic producer-side evidence for the cross-repo tracer.
//! This test is not the full E5.5 acceptance tracer; Cloud owns the governed hops.

use std::{fmt::Write as _, process::Command};

use adoc_core::{
    AssessmentCompleteness, AssessmentOutcome, BuildEmbeddingMode, BuildInput, CapabilityPolicy,
    CapabilityPolicyRule, ChangeAssessmentInput, CitationContentProjection, CitationHandle,
    CompileInput, ContextClass, ContextRequirement, DiffHunkCitation, ExactRevision,
    ExecutorAuthority, ExecutorConfiguration, ExecutorQualificationExpectedBindings,
    GraphCitationObject, LocalProjectContext, ModelConfiguration, PatchApplyInput,
    ProposalBindings, ProposalChangeRequest, ProposalPatchInput,
    SEMANTIC_EXECUTOR_RECEIPT_SCHEMA_VERSION, SemanticContextOutcome,
    SemanticContextValidationBasis, UnavailabilityOutcome, UnavailabilityReason,
    apply_patch_for_date, assess_changes_from_git, build_project_workspace, build_proposal_record,
    build_semantic_context_from_document, compile_project_workspace_with_anchor_root_for_date,
    complete_semantic_execution, parse_patch_from_value, semantic_context_content_digest,
    semantic_prompt_digest, validate_executor_qualification, validate_semantic_assessment,
    validate_semantic_context, validate_semantic_executor_request,
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
                "status: draft\n",
                "owner: billing-platform\n",
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

    fn compile_graph(&self) -> String {
        compile_project_workspace_with_anchor_root_for_date(
            CompileInput {
                root: self.root_path().join("docs"),
            },
            LocalProjectContext {
                project_root: self.root_path(),
                docs_root: self.root_path().join("docs"),
            },
            self.root_path(),
            NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
        )
        .artifacts
        .expect("fixture graph compiles")
        .graph_json
    }

    fn git(&self, args: &[&str]) -> String {
        String::from_utf8(self.git_stdout(args))
            .expect("UTF-8 git output")
            .trim()
            .to_string()
    }

    fn git_stdout(&self, args: &[&str]) -> Vec<u8> {
        let mut command = Command::new("git");
        command.current_dir(self.root.path()).args(args);
        // Keep fixtures isolated when a pre-commit hook exports an outer Git context.
        for var in [
            "GIT_DIR",
            "GIT_INDEX_FILE",
            "GIT_WORK_TREE",
            "GIT_NAMESPACE",
            "GIT_OBJECT_DIRECTORY",
            "GIT_COMMON_DIR",
            "GIT_ALTERNATE_OBJECT_DIRECTORIES",
            "GIT_PREFIX",
        ] {
            command.env_remove(var);
        }
        let output = command.output().expect("git runs");
        assert!(
            output.status.success(),
            "git {args:?} failed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        output.stdout
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
    // Match the exact pretty JSON plus newline emitted by the CLI.
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
    hunk_digest: &str,
    hunk_content: &str,
    content_hash: &str,
    object_body: &str,
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
            "handle": {"kind": "diff_hunk", "changed_source_id": "src/settlement.rs", "hunk_digest": hunk_digest},
            "content": {"diff": hunk_content},
            "truncated": false
        }, {
            "handle_id": "object-a",
            "class_id": "changed_knowledge",
            "scope_ref": "repo:agentdoc/internal-synthetic",
            "handle": {"kind": "knowledge_object", "object_id": "billing.policy", "semantic_hash": content_hash},
            "content": {"body": object_body},
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
fn internal_synthetic_producer_contracts_are_linked_by_real_digests() {
    let repo = Repo::new();
    let base = repo.git(&["rev-parse", "HEAD"]);
    let baseline_assessment = assess_changes_from_git(ChangeAssessmentInput {
        project_root: Some(repo.root_path()),
        base_ref: base.clone(),
        head_ref: Some(base.clone()),
        evaluation_date: NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    });
    let build_graph = || {
        let docs_root = repo.root_path().join("docs");
        let result = build_project_workspace(
            BuildInput {
                root: docs_root.clone(),
                embeddings: BuildEmbeddingMode::Skipped,
                prior_search_artifact_path: None,
            },
            LocalProjectContext {
                project_root: repo.root_path(),
                docs_root,
            },
        );
        assert!(
            !result.has_errors(),
            "graph build: {:?}",
            result.diagnostics
        );
        serde_json::from_str::<Value>(&result.artifacts.expect("graph artifacts").graph_json)
            .expect("graph JSON")
    };
    let baseline_source_digest = digest(
        &std::fs::read(repo.root.path().join("docs/billing.adoc")).expect("baseline source"),
    );
    let baseline_graph_document = build_graph();
    repo.git(&["mv", "docs/billing.adoc", "docs/renamed-billing.adoc"]);
    repo.git(&["commit", "-m", "move billing docs"]);
    let moved = repo.git(&["rev-parse", "HEAD"]);
    let placement_assessment = assess_changes_from_git(ChangeAssessmentInput {
        project_root: Some(repo.root_path()),
        base_ref: base.clone(),
        head_ref: Some(moved.clone()),
        evaluation_date: NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    });
    let moved_graph_document = build_graph();
    repo.write(
        "docs/renamed-billing.adoc",
        concat!(
            "# Billing @doc(team.billing)\n\n",
            "::claim billing.policy\n",
            "status: verified\n",
            "owner: billing-platform\n",
            "verified_at: 2026-09-01\n",
            "source: src/settlement.rs\n",
            "impacts: [src/settlement.rs]\n",
            "--\nCredits settle after payment.\n::\n",
        ),
    );
    repo.write("src/settlement.rs", "pub fn settle() { charge(); }\n");
    repo.git(&["add", "docs/renamed-billing.adoc", "src/settlement.rs"]);
    repo.git(&["commit", "-m", "promote billing policy"]);
    let head = repo.git(&["rev-parse", "HEAD"]);
    let hunk_content = String::from_utf8(repo.git_stdout(&[
        "diff",
        "--unified=0",
        "--no-ext-diff",
        "--no-color",
        &moved,
        &head,
        "--",
        "src/settlement.rs",
    ]))
    .expect("UTF-8 git diff output");
    assert!(hunk_content.contains("+pub fn settle() { charge(); }"));
    assert!(
        hunk_content.ends_with('\n'),
        "exact git diff stdout retains its terminal LF"
    );
    let hunk_digest = digest(hunk_content.as_bytes());

    let assessment = assess_changes_from_git(ChangeAssessmentInput {
        project_root: Some(repo.root_path()),
        base_ref: moved.clone(),
        head_ref: Some(head.clone()),
        evaluation_date: NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    });
    assert_eq!(assessment.completeness, AssessmentCompleteness::Complete);
    assert_eq!(assessment.outcome, AssessmentOutcome::ReviewRequired);
    let baseline_object_set = baseline_assessment
        .knowledge_snapshot
        .object_set_sha256
        .as_deref()
        .expect("baseline object-set digest");
    let placement_object_set = placement_assessment
        .knowledge_snapshot
        .object_set_sha256
        .as_deref()
        .expect("placement object-set digest");
    assert_eq!(
        placement_object_set, baseline_object_set,
        "placement-only changes preserve semantic object-set identity"
    );
    let baseline_graph_digest = baseline_assessment
        .knowledge_snapshot
        .graph_sha256
        .as_deref()
        .expect("baseline graph digest");
    let placement_graph_digest = placement_assessment
        .knowledge_snapshot
        .graph_sha256
        .as_deref()
        .expect("placement graph digest");
    assert_ne!(
        placement_graph_digest, baseline_graph_digest,
        "placement changes remain visible in the exact graph artifact"
    );
    let baseline_graph_object = baseline_graph_document["nodes"]
        .as_array()
        .expect("baseline graph nodes")
        .iter()
        .find(|node| node["id"] == "billing.policy")
        .expect("baseline billing.policy graph node");
    let moved_graph_object = moved_graph_document["nodes"]
        .as_array()
        .expect("moved graph nodes")
        .iter()
        .find(|node| node["id"] == "billing.policy")
        .expect("moved billing.policy graph node");
    let baseline_binding = &baseline_graph_object["source_binding"];
    let moved_binding = &moved_graph_object["source_binding"];
    assert_eq!(
        baseline_binding["path"].as_str().expect("baseline path"),
        "docs/billing.adoc"
    );
    let moved_binding_path = moved_binding["path"]
        .as_str()
        .expect("moved path")
        .to_string();
    assert_eq!(moved_binding_path, "docs/renamed-billing.adoc");
    let baseline_content_hash = baseline_graph_object["content_hash"]
        .as_str()
        .expect("baseline content hash");
    let moved_content_hash = moved_graph_object["content_hash"]
        .as_str()
        .expect("moved content hash");
    assert_eq!(baseline_content_hash, moved_content_hash);
    assert_eq!(
        baseline_binding["source_revision_digest"]
            .as_str()
            .expect("baseline source revision digest"),
        baseline_source_digest
    );
    assert_eq!(
        moved_binding["source_revision_digest"]
            .as_str()
            .expect("moved source revision digest"),
        baseline_source_digest
    );
    let placement_changes = placement_assessment
        .knowledge_changes
        .value
        .as_ref()
        .expect("placement knowledge changes");
    assert!(placement_changes.created.is_empty());
    assert!(placement_changes.changed.is_empty());
    assert!(placement_changes.deleted.is_empty());
    assert_eq!(
        assessment
            .snapshots
            .requested_base
            .resolved_commit
            .as_deref(),
        Some(moved.as_str())
    );
    assert_eq!(
        assessment.snapshots.head.resolved_commit.as_deref(),
        Some(head.as_str())
    );
    let assessment_digest = digest(&canonical_bytes(&assessment));
    let assessed_graph_digest = assessment
        .knowledge_snapshot
        .graph_sha256
        .as_deref()
        .expect("assessment graph digest")
        .to_string();
    let assessed_object = assessment
        .objects
        .value
        .as_ref()
        .expect("assessment objects")
        .iter()
        .find(|object| object.id == "billing.policy")
        .expect("billing.policy assessed");
    assert_eq!(
        assessed_object.source.path, moved_binding_path,
        "assessment placement and graph Source Binding agree on the moved path"
    );
    let assessed_content_hash = assessed_object.content_hash.clone();
    let promotion_change = assessment
        .knowledge_changes
        .value
        .as_ref()
        .expect("promotion knowledge changes")
        .changed
        .first()
        .expect("billing.policy semantic change");
    assert_eq!(promotion_change.id, "billing.policy");
    assert_eq!(
        promotion_change.base_content_hash.as_deref(),
        Some(moved_content_hash)
    );
    assert_eq!(
        promotion_change.head_content_hash.as_deref(),
        Some(assessed_content_hash.as_str())
    );
    let promoted_object_set = assessment
        .knowledge_snapshot
        .object_set_sha256
        .as_deref()
        .expect("promoted object-set digest");
    assert_ne!(promoted_object_set, placement_object_set);
    let head_graph = compile_project_workspace_with_anchor_root_for_date(
        CompileInput {
            root: repo.root_path().join("docs"),
        },
        LocalProjectContext {
            project_root: repo.root_path(),
            docs_root: repo.root_path().join("docs"),
        },
        repo.root_path(),
        NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    )
    .artifacts
    .expect("exact-head graph compiles")
    .graph_json;
    let head_graph_json: Value = serde_json::from_str(&head_graph).expect("graph JSON");
    let graph_object = head_graph_json["nodes"]
        .as_array()
        .expect("graph nodes")
        .iter()
        .find(|node| node["id"] == "billing.policy")
        .expect("billing.policy graph object");
    let graph_digest = digest(head_graph.as_bytes());
    assert_eq!(graph_digest, assessed_graph_digest);
    let content_hash = graph_object["content_hash"]
        .as_str()
        .expect("graph content hash")
        .to_string();
    assert_eq!(content_hash, assessed_content_hash);
    let object_body = graph_object["body"]
        .as_str()
        .expect("graph object body")
        .to_string();
    let has_source_binding = graph_object["source_binding"].is_object();
    let evidence_count = graph_object["evidence"]
        .as_array()
        .expect("graph evidence")
        .len();
    assert_eq!(
        serde_json::to_value(&assessment.authority_promotions)
            .expect("authority promotions serialize"),
        json!({
            "status": "available",
            "value": [{
                "id": "billing.policy",
                "content_hash": content_hash,
                "kind": "claim",
                "before_kind": "claim",
                "before_status": "draft",
                "after_status": "verified"
            }]
        })
    );

    let context = build_semantic_context_from_document(
        &serde_json::to_vec(&context_input(
            &assessment_digest,
            &graph_digest,
            &hunk_digest,
            &hunk_content,
            &content_hash,
            &object_body,
            &moved,
            &head,
        ))
        .expect("context input serializes"),
    )
    .expect("context builds");
    let context_canonical = context.to_canonical_json().expect("context serializes");
    let context_json: Value = serde_json::from_str(&context_canonical).expect("context JSON");
    assert_eq!(
        context_json["items"][1]["content"]["body"],
        graph_object["body"]
    );
    assert_eq!(
        context_json["basis"]["assessment_digest"],
        assessment_digest
    );
    assert_eq!(
        context_json["basis"]["knowledge_basis"]["digest"],
        graph_digest
    );
    let capability_policy = CapabilityPolicy {
        version: "semantic-context-policy-v1".to_string(),
        rules: [
            (
                UnavailabilityReason::Permission,
                UnavailabilityOutcome::Insufficient,
            ),
            (
                UnavailabilityReason::Retention,
                UnavailabilityOutcome::Insufficient,
            ),
            (
                UnavailabilityReason::SourceOutage,
                UnavailabilityOutcome::Failed,
            ),
            (
                UnavailabilityReason::Truncation,
                UnavailabilityOutcome::Insufficient,
            ),
            (
                UnavailabilityReason::ResourceLimit,
                UnavailabilityOutcome::Insufficient,
            ),
        ]
        .into_iter()
        .map(|(reason, outcome)| CapabilityPolicyRule { reason, outcome })
        .collect(),
    };
    let context_class = ContextClass {
        class_id: "changed_knowledge".to_string(),
        requirement: ContextRequirement::Required,
        byte_budget: 4096,
    };
    let scope = "repo:agentdoc/internal-synthetic".to_string();
    let validated_context = validate_semantic_context(
        context_canonical.as_bytes(),
        &SemanticContextValidationBasis {
            evaluation_date: NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
            subject_revision: revision(&head),
            source_revision: revision(&head),
            base_revision: revision(&moved),
            head_revision: revision(&head),
            assessment_digest: assessment_digest.clone(),
            selection_algorithm: "internal-synthetic-tracer".to_string(),
            selection_version: "1".to_string(),
            context_classes: vec![context_class],
            authorized_scope: vec![scope.clone()],
            capability_policy,
            graph_artifact_digest: Some(graph_digest),
            managed_revision_digest: None,
            graph_objects: vec![GraphCitationObject {
                object_id: "billing.policy".to_string(),
                semantic_hash: content_hash.clone(),
                has_source_binding,
                evidence_count,
            }],
            diff_hunks: vec![DiffHunkCitation {
                changed_source_id: "src/settlement.rs".to_string(),
                hunk_digest: hunk_digest.clone(),
            }],
            source_assertions: Vec::new(),
            citation_contents: vec![
                CitationContentProjection {
                    handle: CitationHandle::DiffHunk {
                        changed_source_id: "src/settlement.rs".to_string(),
                        hunk_digest,
                    },
                    class_id: "changed_knowledge".to_string(),
                    scope_ref: scope.clone(),
                    content_digest: semantic_context_content_digest(&json!({
                        "diff": hunk_content
                    })),
                    truncated_content_digests: Vec::new(),
                },
                CitationContentProjection {
                    handle: CitationHandle::KnowledgeObject {
                        object_id: "billing.policy".to_string(),
                        semantic_hash: content_hash.clone(),
                    },
                    class_id: "changed_knowledge".to_string(),
                    scope_ref: scope,
                    content_digest: semantic_context_content_digest(&json!({
                        "body": object_body
                    })),
                    truncated_content_digests: Vec::new(),
                },
            ],
        },
    )
    .expect("context validates against exact Git and graph evidence");
    assert_eq!(validated_context.outcome(), SemanticContextOutcome::Ready);
    let context_digest = validated_context.context_digest().to_string();

    let instructions = "Return one structured semantic assessment.";
    let prompt_digest =
        semantic_prompt_digest("semantic-assessment-task-v1", instructions).expect("prompt");
    let (qualification_json, current_executor) = qualified_executor();
    let qualification_bytes =
        serde_json::to_vec(&qualification_json).expect("qualification serializes");
    let qualification =
        validate_executor_qualification(&qualification_bytes).expect("qualification validates");
    let evaluated_task_digest = TASK;
    let eligibility = qualification.evaluate(
        &current_executor,
        evaluated_task_digest,
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
    let ExecutorConfiguration::Model {
        executor_digest,
        model_digest,
        config_digest,
        ..
    } = &current_executor
    else {
        panic!("internal synthetic executor is a model")
    };

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
                "executor_digest": executor_digest,
                "model_digest": model_digest,
                "config_digest": config_digest
            },
            "task_digest": evaluated_task_digest,
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
    assert_eq!(request.adapter().executor_digest, *executor_digest);
    assert_eq!(request.adapter().model_digest, *model_digest);
    assert_eq!(request.adapter().config_digest, *config_digest);
    assert_eq!(request.task_digest(), evaluated_task_digest);
    let semantic = validate_semantic_assessment(
        &serde_json::to_vec(&semantic_assessment(
            &context_digest,
            &content_hash,
            &moved,
            &head,
        ))
        .expect("semantic assessment serializes"),
        request.context(),
    )
    .expect("semantic assessment validates");
    let semantic_json: Value = serde_json::from_str(
        &semantic
            .to_canonical_json()
            .expect("semantic serializes for candidate translation"),
    )
    .expect("semantic JSON");
    let assessment_finding_id = semantic_json["findings"][0]["finding_id"]
        .as_str()
        .expect("assessment finding ID");
    let candidate_body = semantic_json["findings"][0]["candidate_updates"][0]["body"]
        .as_str()
        .expect("candidate body");
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

    let update_patch = json!({
        "schema_version": "adoc.patch.v0",
        "op": "update_fields",
        "target": "billing.policy",
        "base_hash": content_hash,
        "changes": {"fields": {"status": "draft"}},
        "reason": format!("AgentDoc assessment {assessment_digest} finding finding-001."),
        "proposer": {"type": "agent", "id": "agentdoc-action/codex/internal-synthetic"}
    });
    repo.write("dist/docs.graph.json", &head_graph);
    let applied_update = apply_patch_for_date(
        PatchApplyInput {
            graph_artifact_path: repo.root_path().join("dist/docs.graph.json"),
            docs_root: repo.root_path().join("docs"),
            project_root: repo.root_path(),
            interface: "internal-synthetic-tracer".to_string(),
        },
        parse_patch_from_value(update_patch.clone()).expect("update patch parses"),
        NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    );
    assert!(applied_update.applied, "status-floor patch applies");
    let body_base_hash = applied_update
        .object
        .after_content_hash
        .expect("status-floor patch re-derives the body patch base hash");
    let replace_body_patch = json!({
        "schema_version": "adoc.patch.v0",
        "op": "replace_body",
        "target": "billing.policy",
        "base_hash": body_base_hash,
        "changes": {"body": candidate_body},
        "reason": format!("AgentDoc assessment {assessment_digest} finding finding-001."),
        "proposer": {"type": "agent", "id": "agentdoc-action/codex/internal-synthetic"}
    });
    let post_status_graph = repo.compile_graph();
    let post_status_graph_json: Value =
        serde_json::from_str(&post_status_graph).expect("post-status graph JSON");
    let post_status_object = post_status_graph_json["nodes"]
        .as_array()
        .expect("post-status graph nodes")
        .iter()
        .find(|node| node["id"] == "billing.policy")
        .expect("post-status billing.policy graph object");
    assert_eq!(post_status_object["status"], "draft");
    assert_eq!(post_status_object["content_hash"], body_base_hash);
    repo.write("dist/docs.graph.json", &post_status_graph);
    let applied_body = apply_patch_for_date(
        PatchApplyInput {
            graph_artifact_path: repo.root_path().join("dist/docs.graph.json"),
            docs_root: repo.root_path().join("docs"),
            project_root: repo.root_path(),
            interface: "internal-synthetic-tracer".to_string(),
        },
        parse_patch_from_value(replace_body_patch.clone()).expect("body patch parses"),
        NaiveDate::from_ymd_opt(2026, 9, 30).expect("date"),
    );
    assert!(applied_body.applied, "body patch applies sequentially");
    let applied_body_hash = applied_body
        .object
        .after_content_hash
        .expect("body patch re-derives the final content hash");
    let final_graph = repo.compile_graph();
    let final_graph_json: Value = serde_json::from_str(&final_graph).expect("final graph JSON");
    let final_object = final_graph_json["nodes"]
        .as_array()
        .expect("final graph nodes")
        .iter()
        .find(|node| node["id"] == "billing.policy")
        .expect("final billing.policy graph object");
    assert_eq!(final_object["body"], candidate_body);
    assert_eq!(final_object["content_hash"], applied_body_hash);
    let patch_input = |patch: &Value| {
        let mut patch_bytes = serde_json::to_vec(patch).expect("patch serializes");
        patch_bytes.push(b'\n');
        ProposalPatchInput {
            finding_id: assessment_finding_id.to_string(),
            placement_path: moved_binding_path.clone(),
            page_id: "team.billing".to_string(),
            patch_bytes,
        }
    };
    let proposal = build_proposal_record(
        ProposalBindings {
            base_revision: revision(&moved),
            head_revision: revision(&head),
            change_request: ProposalChangeRequest {
                system: "github_pull_request".to_string(),
                id: "42".to_string(),
            },
            assessment_digest: assessment_digest.clone(),
            semantic_context_digest: context_digest.clone(),
            semantic_assessment_digest: semantic_assessment_digest.clone(),
        },
        vec![patch_input(&update_patch), patch_input(&replace_body_patch)],
        None,
    )
    .expect("proposal builds");
    assert_eq!(proposal.bindings().assessment_digest, assessment_digest);
    assert_eq!(proposal.bindings().semantic_context_digest, context_digest);
    assert_eq!(
        proposal.bindings().semantic_assessment_digest,
        semantic_assessment_digest
    );
    let proposal_json: Value =
        serde_json::from_str(&proposal.to_canonical_json().expect("proposal serializes"))
            .expect("proposal JSON");
    let proposal_patches = proposal_json["patches"]
        .as_array()
        .expect("proposal patches");
    assert_eq!(proposal_patches[0]["placement_path"], moved_binding_path);
    assert!(
        proposal_patches
            .iter()
            .all(|patch| patch["finding_id"] == assessment_finding_id),
        "every proposal patch binds the assessment finding ID"
    );
    assert_eq!(proposal_patches[0]["page_id"], "team.billing");
    let body_patch = proposal_patches
        .iter()
        .find(|patch| patch["operation"] == "replace_body")
        .expect("candidate body is translated to a replace_body patch");
    assert_eq!(body_patch["patch"]["changes"]["body"], candidate_body);
}

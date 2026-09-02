//! E5.1 — canonical proposal record (`adoc.proposal.v0`).

use std::fs;
use std::path::PathBuf;

use adoc_core::{
    ExactRevision, PROPOSAL_SCHEMA_VERSION, ProposalBindings, ProposalChangeRequest,
    ProposalPatchInput, ProposalRecord, ProposalRecordError, build_proposal_record,
    validate_proposal_record,
};
use serde_json::{Value, json};

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn bindings() -> ProposalBindings {
    ProposalBindings {
        base_revision: ExactRevision {
            system: "git".to_string(),
            value: "1111111111111111111111111111111111111111".to_string(),
        },
        head_revision: ExactRevision {
            system: "git".to_string(),
            value: "2222222222222222222222222222222222222222".to_string(),
        },
        change_request: ProposalChangeRequest {
            system: "github_pull_request".to_string(),
            id: "42".to_string(),
        },
        assessment_digest: A.to_string(),
        semantic_context_digest: B.to_string(),
        semantic_assessment_digest: C.to_string(),
    }
}

/// Exact patch bytes per ADR-0053 §8: sorted compact JSON plus one newline.
fn patch_bytes(patch: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(patch).expect("patch serializes");
    bytes.push(b'\n');
    bytes
}

fn create_patch(target: &str) -> Value {
    json!({
        "schema_version": "adoc.patch.v0",
        "op": "create_object",
        "target": target,
        "changes": {
            "kind": "claim",
            "status": "draft",
            "body": "A proposed claim.",
            "placement": {"page_id": "billing.kb"}
        },
        "reason": format!("AgentDoc assessment {A} finding finding-001."),
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code@2.1.215/claude-sonnet-5"}
    })
}

fn update_patch(target: &str, base_hash: &str, owner: &str) -> Value {
    json!({
        "schema_version": "adoc.patch.v0",
        "op": "update_fields",
        "target": target,
        "base_hash": base_hash,
        "changes": {"fields": {"owner": owner}},
        "reason": format!("AgentDoc assessment {A} finding finding-002."),
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code@2.1.215/claude-sonnet-5"}
    })
}

fn patch_input(finding_id: &str, path: &str, page_id: &str, patch: &Value) -> ProposalPatchInput {
    ProposalPatchInput {
        finding_id: finding_id.to_string(),
        placement_path: path.to_string(),
        page_id: page_id.to_string(),
        patch_bytes: patch_bytes(patch),
    }
}

fn record() -> ProposalRecord {
    build_proposal_record(
        bindings(),
        vec![
            patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", D, "billing"),
            ),
            patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &create_patch("billing.proposed"),
            ),
        ],
        None,
    )
    .expect("record builds")
}

fn sha256(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    let mut hex = String::from("sha256:");
    for byte in Sha256::digest(bytes) {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

fn schema() -> Value {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/agent/v0/schema/adoc.proposal.v0.schema.json");
    serde_json::from_str(&fs::read_to_string(path).expect("schema is readable"))
        .expect("schema is JSON")
}

#[test]
fn proposal_set_digest_hashes_the_ordered_patch_digests_exactly() {
    let record = record();
    let value: Value = serde_json::from_str(&record.to_canonical_json().expect("serializes"))
        .expect("record is JSON");

    assert_eq!(value["schema_version"], PROPOSAL_SCHEMA_VERSION);
    // Ordering is (placement_path, page_id, target, patch_digest): the update
    // of `billing.credits` sorts before the create of `billing.proposed`
    // regardless of input order.
    let targets: Vec<_> = value["patches"]
        .as_array()
        .expect("patches")
        .iter()
        .map(|patch| patch["target"].as_str().expect("target").to_string())
        .collect();
    assert_eq!(targets, ["billing.credits", "billing.proposed"]);

    let expected_digests = [
        sha256(&patch_bytes(&update_patch("billing.credits", D, "billing"))),
        sha256(&patch_bytes(&create_patch("billing.proposed"))),
    ];
    let digests: Vec<_> = value["patches"]
        .as_array()
        .expect("patches")
        .iter()
        .map(|patch| patch["patch_digest"].as_str().expect("digest").to_string())
        .collect();
    assert_eq!(digests, expected_digests);

    // ADR-0053 §8: compact JSON array of the ordered digests plus one newline.
    let mut set_bytes = serde_json::to_vec(&expected_digests).expect("array serializes");
    set_bytes.push(b'\n');
    assert_eq!(record.proposal_set_digest(), sha256(&set_bytes));
    assert_eq!(value["proposal_set_digest"], record.proposal_set_digest());
    assert_eq!(value["supersedes"], Value::Null);
    assert_eq!(
        value["content_bindings"],
        json!([{"object_id": "billing.credits", "content_hash": D}])
    );
    assert!(!value.to_string().contains("timestamp"));
}

#[test]
fn record_bytes_are_deterministic_and_match_the_published_schema() {
    let first = record().to_canonical_json().expect("serializes");
    let second = record().to_canonical_json().expect("serializes");
    assert_eq!(first, second);
    assert!(first.ends_with('\n'));

    let instance: Value = serde_json::from_str(&first).expect("record is JSON");
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");
    let errors: Vec<_> = validator
        .iter_errors(&instance)
        .map(|error| error.to_string())
        .collect();
    assert!(errors.is_empty(), "schema validation failed: {errors:#?}");
}

#[test]
fn record_round_trips_through_the_proposal_command_transport() {
    let record = record();
    let json = record.to_canonical_json().expect("serializes");
    let payload: Value = serde_json::from_str(&json).expect("record is JSON");
    let command = json!({
        "schema_version": "agentdoc.cloud.proposal_command.v0",
        "payload": payload
    });
    let transported = serde_json::to_vec(&command).expect("command serializes");
    let received: Value = serde_json::from_slice(&transported).expect("command parses");
    let received_bytes = serde_json::to_vec(&received["payload"]).expect("payload serializes");

    let validated = validate_proposal_record(&received_bytes).expect("payload validates");
    assert_eq!(
        validated.proposal_set_digest(),
        record.proposal_set_digest()
    );
    assert_eq!(validated.to_canonical_json().expect("serializes"), json);
}

#[test]
fn unknown_record_version_is_rejected_exactly() {
    let mut value: Value = serde_json::from_str(&record().to_canonical_json().expect("serializes"))
        .expect("record is JSON");
    value["schema_version"] = json!("adoc.proposal.v99");
    let error = validate_proposal_record(&serde_json::to_vec(&value).expect("serializes"))
        .expect_err("unknown version fails closed");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.invalid_document"
    );
}

#[test]
fn record_with_a_missing_binding_is_unconstructible() {
    let mut incomplete = bindings();
    incomplete.semantic_assessment_digest = String::new();
    let error = build_proposal_record(
        incomplete,
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        None,
    )
    .expect_err("missing binding fails");
    assert!(matches!(
        error,
        ProposalRecordError::BindingInvalid { ref field } if field == "semantic_assessment_digest"
    ));
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.binding_invalid"
    );

    let error = build_proposal_record(bindings(), Vec::new(), None).expect_err("no patches");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.patch_invalid"
    );
}

#[test]
fn tampered_digest_fails_validation() {
    let mut value: Value = serde_json::from_str(&record().to_canonical_json().expect("serializes"))
        .expect("record is JSON");
    value["proposal_set_digest"] = json!(A);
    let error = validate_proposal_record(&serde_json::to_vec(&value).expect("serializes"))
        .expect_err("digest mismatch fails");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.invalid_document"
    );
}

// E5.1.T2 — edit invalidation.
#[test]
fn edit_mints_new_proposal_version() {
    let original = record();
    let original_json = original.to_canonical_json().expect("serializes");
    let revised = original
        .revise(vec![
            patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", D, "billing-team"),
            ),
            patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &create_patch("billing.proposed"),
            ),
        ])
        .expect("revision builds");

    assert_ne!(
        revised.proposal_set_digest(),
        original.proposal_set_digest()
    );
    // The invalidation consequence is visible on the new record before it is
    // submitted: it names exactly the digest it replaces.
    assert_eq!(revised.supersedes(), Some(original.proposal_set_digest()));
    // The prior version is untouched.
    assert_eq!(
        original.to_canonical_json().expect("serializes"),
        original_json
    );
    assert_eq!(original.supersedes(), None);

    // A byte-identical revision is not a new version.
    let same = original
        .revise(vec![
            patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &create_patch("billing.proposed"),
            ),
            patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", D, "billing"),
            ),
        ])
        .expect_err("unchanged bytes cannot supersede themselves");
    assert_eq!(
        same.diagnostic_code().as_str(),
        "proposal_record.binding_invalid"
    );
}

// E5.1.T3 — model-originated submissions can only create reviewable knowledge.
#[test]
fn model_path_cannot_touch_active_state() {
    let authority_patches = [
        json!({
            "schema_version": "adoc.patch.v0", "op": "revoke", "target": "billing.credits",
            "base_hash": D, "changes": {}, "reason": "revoke"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "supersede", "target": "billing.credits",
            "base_hash": D, "changes": {"supersedes": ["billing.old"]}, "reason": "supersede"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "create_object", "target": "billing.verified",
            "changes": {"kind": "claim", "status": "verified", "body": "Authority.",
                        "placement": {"page_id": "billing.kb"}},
            "reason": "create verified"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "create_object", "target": "billing.policy",
            "changes": {"kind": "policy", "status": "draft", "body": "Policy.",
                        "placement": {"page_id": "billing.kb"}},
            "reason": "create outside floors"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "update_fields", "target": "billing.credits",
            "base_hash": D, "changes": {"fields": {"approved_by": "model"}}, "reason": "approve"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "update_fields", "target": "billing.credits",
            "base_hash": D, "changes": {"fields": {"status": "verified"}}, "reason": "promote"
        }),
        json!({
            "schema_version": "adoc.patch.v0", "op": "create_object", "target": "billing.reviewed",
            "changes": {"kind": "claim", "status": "draft", "body": "Reviewed.",
                        "fields": {"reviewed_by": "model"},
                        "placement": {"page_id": "billing.kb"}},
            "reason": "create with authority field"
        }),
    ];
    for patch in authority_patches {
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-009",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("authority is never proposable");
        assert!(
            matches!(error, ProposalRecordError::AuthorityRejected { .. }),
            "{patch}: {error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.authority_rejected"
        );
    }
}

#[test]
fn conflicting_content_bindings_for_one_target_are_rejected() {
    let error = build_proposal_record(
        bindings(),
        vec![
            patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", D, "billing"),
            ),
            patch_input(
                "finding-003",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", A, "finance"),
            ),
        ],
        None,
    )
    .expect_err("one target binds one content hash");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.patch_invalid"
    );
}

// E5.1 acceptance — E1.1 hash twins: a position-only source-placement move
// leaves the proposal-set digest unchanged; a content change changes it.
mod support;

#[test]
fn placement_only_move_keeps_the_proposal_set_digest_and_content_change_changes_it() {
    use adoc_core::{BuildEmbeddingMode, BuildInput};
    use support::TestWorkspace;

    let source = |verb: &str, prefix: &str| {
        format!(
            concat!(
                "# Billing @doc(billing.kb)\n",
                "\n",
                "{prefix}",
                "::claim billing.credits\n",
                "status: draft\n",
                "--\n",
                "Credits {verb} after successful payment.\n",
                "::\n",
            ),
            verb = verb,
            prefix = prefix,
        )
    };
    let content_hash = |file: &str, source: &str| {
        let workspace = TestWorkspace::new("proposal-hash-twins");
        let root = workspace.write(file, source);
        let result = adoc_core::build_workspace(BuildInput {
            root,
            embeddings: BuildEmbeddingMode::Skipped,
            prior_search_artifact_path: None,
        });
        assert!(!result.has_errors(), "{:?}", result.diagnostics);
        let graph: Value = serde_json::from_str(&result.artifacts.expect("artifacts").graph_json)
            .expect("graph is JSON");
        graph["nodes"]
            .as_array()
            .expect("nodes")
            .iter()
            .find(|node| node["id"] == "billing.credits")
            .and_then(|node| node["content_hash"].as_str())
            .expect("content_hash")
            .to_string()
    };
    let record_for = |hash: &str| {
        build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &update_patch("billing.credits", hash, "billing"),
            )],
            None,
        )
        .expect("record builds")
    };

    let original = content_hash("billing.adoc", &source("post", ""));
    let moved = content_hash(
        "moved/renamed.adoc",
        &source("post", "Intro prose shifts lines.\n\n"),
    );
    let edited = content_hash("billing.adoc", &source("settle", ""));

    assert_eq!(original, moved);
    assert_ne!(original, edited);
    assert_eq!(
        record_for(&original)
            .to_canonical_json()
            .expect("serializes"),
        record_for(&moved).to_canonical_json().expect("serializes")
    );
    assert_ne!(
        record_for(&original).proposal_set_digest(),
        record_for(&edited).proposal_set_digest()
    );
}

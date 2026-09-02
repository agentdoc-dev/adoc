//! E5.1.T4 — `adoc proposal-record`: identifier-only cross-links and
//! delivery parity between producers.

mod support;

use adoc_core::{
    ExactRevision, ProposalBindings, ProposalChangeRequest, ProposalPatchInput,
    build_proposal_record, validate_proposal_record,
};
use serde_json::{Value, json};
use support::{TestWorkspace, adoc_command, stderr, stdout};

const A: &str = "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const B: &str = "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const C: &str = "sha256:cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const D: &str = "sha256:dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";

fn patch_bytes(patch: &Value) -> Vec<u8> {
    let mut bytes = serde_json::to_vec(patch).expect("patch serializes");
    bytes.push(b'\n');
    bytes
}

fn create_patch() -> Value {
    json!({
        "schema_version": "adoc.patch.v0",
        "op": "create_object",
        "target": "billing.proposed",
        "changes": {"kind": "claim", "status": "draft", "body": "A proposed claim.",
                    "placement": {"page_id": "billing.kb"}},
        "reason": format!("AgentDoc assessment {A} finding finding-001."),
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code@2.1.215/claude-sonnet-5"}
    })
}

fn update_patch() -> Value {
    json!({
        "schema_version": "adoc.patch.v0",
        "op": "update_fields",
        "target": "billing.credits",
        "base_hash": D,
        "changes": {"fields": {"owner": "billing", "status": "draft"}},
        "reason": format!("AgentDoc assessment {A} finding finding-002."),
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code@2.1.215/claude-sonnet-5"}
    })
}

fn bindings_json() -> Value {
    json!({
        "base_revision": {"system": "git", "value": "1111111111111111111111111111111111111111"},
        "head_revision": {"system": "git", "value": "2222222222222222222222222222222222222222"},
        "change_request": {"system": "github_pull_request", "id": "42"},
        "assessment_digest": A,
        "semantic_context_digest": B,
        "semantic_assessment_digest": C
    })
}

fn write_patches(workspace: &TestWorkspace) {
    workspace.write(
        "patches/create.json",
        std::str::from_utf8(&patch_bytes(&create_patch())).expect("utf-8"),
    );
    workspace.write(
        "patches/update.json",
        std::str::from_utf8(&patch_bytes(&update_patch())).expect("utf-8"),
    );
}

fn entries(first_create: bool) -> Value {
    let create = json!({"finding_id": "finding-001", "placement_path": "docs/billing.adoc",
        "page_id": "billing.kb", "patch_path": "patches/create.json"});
    let update = json!({"finding_id": "finding-002", "placement_path": "docs/billing.adoc",
        "page_id": "billing.kb", "patch_path": "patches/update.json"});
    if first_create {
        json!([create, update])
    } else {
        json!([update, create])
    }
}

fn run(workspace: &TestWorkspace, input: &str, out: &str) -> std::process::Output {
    adoc_command()
        .current_dir(&workspace.root)
        .args(["proposal-record", "--input", input, "--out", out])
        .output()
        .expect("proposal-record runs")
}

#[test]
fn git_delivered_and_api_submitted_proposals_are_byte_equivalent() {
    let workspace = TestWorkspace::new("proposal-record-cli");
    write_patches(&workspace);
    // The Action's manifest order and an API client's order differ; the
    // canonical record does not.
    workspace.write(
        "action-input.json",
        &json!({"bindings": bindings_json(), "patches": entries(false)}).to_string(),
    );
    workspace.write(
        "api-input.json",
        &json!({"bindings": bindings_json(), "supersedes": null, "patches": entries(true)})
            .to_string(),
    );

    let action = run(&workspace, "action-input.json", "action-record.json");
    assert_eq!(action.status.code(), Some(0), "{}", stderr(&action));
    let api = run(&workspace, "api-input.json", "api-record.json");
    assert_eq!(api.status.code(), Some(0), "{}", stderr(&api));

    let action_record = std::fs::read(workspace.root.join("action-record.json")).expect("record");
    let api_record = std::fs::read(workspace.root.join("api-record.json")).expect("record");
    assert_eq!(action_record, api_record);
    assert_eq!(stdout(&action).as_bytes(), action_record.as_slice());

    // Library construction is the same bytes as the CLI adapter.
    let library = build_proposal_record(
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
        },
        vec![
            ProposalPatchInput {
                finding_id: "finding-001".to_string(),
                placement_path: "docs/billing.adoc".to_string(),
                page_id: "billing.kb".to_string(),
                patch_bytes: patch_bytes(&create_patch()),
            },
            ProposalPatchInput {
                finding_id: "finding-002".to_string(),
                placement_path: "docs/billing.adoc".to_string(),
                page_id: "billing.kb".to_string(),
                patch_bytes: patch_bytes(&update_patch()),
            },
        ],
        None,
    )
    .expect("library record builds");
    assert_eq!(
        library.to_canonical_json().expect("serializes").as_bytes(),
        action_record.as_slice()
    );
    validate_proposal_record(&action_record).expect("emitted record validates");
}

#[test]
fn cross_links_are_identifiers_and_digests_only() {
    let workspace = TestWorkspace::new("proposal-record-links");
    write_patches(&workspace);
    // Branch names and titles are not inputs at all: a producer that tries to
    // bind them is rejected instead of silently dropped, while an exact
    // revision change — the only link that legitimately moves — changes the
    // record.
    workspace.write(
        "before.json",
        &json!({"bindings": bindings_json(), "patches": entries(true)}).to_string(),
    );
    let mut rebased = bindings_json();
    rebased["head_revision"] =
        json!({"system": "git", "value": "3333333333333333333333333333333333333333"});
    workspace.write(
        "rebased.json",
        &json!({"bindings": rebased, "patches": entries(true)}).to_string(),
    );
    workspace.write(
        "decoy.json",
        &json!({
            "bindings": bindings_json(),
            "delivery": {"branch": "feature/renamed-branch", "title": "Edited PR title"},
            "patches": entries(true)
        })
        .to_string(),
    );

    let before = run(&workspace, "before.json", "before-record.json");
    let rebased = run(&workspace, "rebased.json", "rebased-record.json");
    assert_eq!(before.status.code(), Some(0), "{}", stderr(&before));
    assert_eq!(rebased.status.code(), Some(0), "{}", stderr(&rebased));
    let before_record =
        std::fs::read_to_string(workspace.root.join("before-record.json")).expect("record");
    let rebased_record =
        std::fs::read_to_string(workspace.root.join("rebased-record.json")).expect("record");
    assert_ne!(before_record, rebased_record);
    assert!(rebased_record.contains("3333333333333333333333333333333333333333"));
    assert!(!before_record.contains("feature/"));
    assert!(!before_record.contains("title"));

    let decoy = run(&workspace, "decoy.json", "decoy-record.json");
    assert_eq!(decoy.status.code(), Some(2));
    assert!(stderr(&decoy).contains("unknown field `delivery`"));
    assert!(!workspace.root.join("decoy-record.json").exists());
}

#[test]
fn authority_patches_fail_with_the_registered_code() {
    let workspace = TestWorkspace::new("proposal-record-authority");
    let mut promote = update_patch();
    promote["changes"] = json!({"fields": {"status": "verified"}});
    workspace.write(
        "patches/promote.json",
        std::str::from_utf8(&patch_bytes(&promote)).expect("utf-8"),
    );
    workspace.write(
        "input.json",
        &json!({"bindings": bindings_json(), "patches": [{
            "finding_id": "finding-009", "placement_path": "docs/billing.adoc",
            "page_id": "billing.kb", "patch_path": "patches/promote.json"
        }]})
        .to_string(),
    );
    // A stale record from an earlier run never survives a failed rebuild.
    workspace.write("record.json", "{\"stale\": true}\n");
    let output = run(&workspace, "input.json", "record.json");
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("[proposal_record.authority_rejected]"));
    assert!(!workspace.root.join("record.json").exists());

    // Aliased input/output spellings are refused before any write.
    let aliased = run(&workspace, "input.json", "./input.json");
    assert_eq!(aliased.status.code(), Some(2));
    assert!(stderr(&aliased).contains("must be distinct"));

    // Review round 2: --out naming a patch file is an input alias too, and
    // is refused before the stale-output clear can destroy the patch.
    let clobber = run(&workspace, "input.json", "patches/../patches/promote.json");
    assert_eq!(clobber.status.code(), Some(2));
    assert!(stderr(&clobber).contains("must be distinct"));
    assert!(workspace.root.join("patches/promote.json").exists());
}

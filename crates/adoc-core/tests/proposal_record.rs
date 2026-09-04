//! E5.1 — canonical proposal record (`adoc.proposal.v0`).

use std::fs;
use std::path::PathBuf;

use adoc_core::{
    ExactRevision, PROPOSAL_SCHEMA_VERSION, ProposalBindings, ProposalChangeRequest,
    ProposalDispositionInput, ProposalDispositionKind, ProposalPatchInput, ProposalRecord,
    ProposalRecordError, build_proposal_record, build_proposal_record_with_dispositions,
    is_semantic_context_text, validate_proposal_record,
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
        "changes": {"fields": {"owner": owner, "status": "draft"}},
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

fn disposition(finding_id: &str) -> ProposalDispositionInput {
    ProposalDispositionInput {
        finding_id: finding_id.to_string(),
        disposition: ProposalDispositionKind::NoChangeRequired,
        acceptance_receipt_digest: D.to_string(),
    }
}

#[test]
fn accepted_no_change_disposition_does_not_change_patch_set_identity() {
    let patch = patch_input(
        "finding-001",
        "docs/billing.adoc",
        "billing.kb",
        &create_patch("billing.proposed"),
    );
    let without = build_proposal_record(bindings(), vec![patch.clone()], None).expect("builds");
    let with = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch],
        vec![disposition("finding-002")],
        None,
    )
    .expect("accepted disposition builds");

    assert_eq!(with.proposal_set_digest(), without.proposal_set_digest());
    let value: Value = serde_json::from_str(&with.to_canonical_json().expect("serializes"))
        .expect("record is JSON");
    assert_eq!(value["dispositions"][0]["finding_id"], "finding-002");
    assert_eq!(
        value["dispositions"][0]["disposition"],
        "no_change_required"
    );
}

#[test]
fn published_schema_accepts_a_receipted_no_change_disposition() {
    let record = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        vec![disposition("finding-002")],
        None,
    )
    .expect("accepted disposition builds");
    let instance: Value = serde_json::from_str(&record.to_canonical_json().expect("serializes"))
        .expect("record is JSON");

    assert!(
        jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&instance)
    );

    let mut duplicated = instance;
    duplicated["dispositions"] = json!([disposition("finding-002"), disposition("finding-002")]);
    assert!(
        !jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&duplicated)
    );
}

#[test]
fn proposal_dispositions_are_canonical_and_require_a_receipt_digest() {
    let patch = patch_input(
        "finding-001",
        "docs/billing.adoc",
        "billing.kb",
        &create_patch("billing.proposed"),
    );
    let record = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch.clone()],
        vec![disposition("finding-003"), disposition("finding-002")],
        None,
    )
    .expect("dispositions build");
    let ids: Vec<_> = record
        .dispositions()
        .iter()
        .map(|entry| entry.finding_id.as_str())
        .collect();
    assert_eq!(ids, ["finding-002", "finding-003"]);

    let mut invalid_receipt = disposition("finding-002");
    invalid_receipt.acceptance_receipt_digest = "not-a-digest".to_string();
    assert!(matches!(
        build_proposal_record_with_dispositions(
            bindings(),
            vec![patch.clone()],
            vec![invalid_receipt],
            None,
        ),
        Err(ProposalRecordError::BindingInvalid { ref field })
            if field == "dispositions.acceptance_receipt_digest"
    ));

    assert!(matches!(
        build_proposal_record_with_dispositions(
            bindings(),
            vec![patch],
            vec![disposition("finding-002"), disposition("finding-002")],
            None,
        ),
        Err(ProposalRecordError::BindingInvalid { ref field })
            if field == "dispositions.finding_id"
    ));
}

#[test]
fn one_finding_cannot_have_both_a_patch_and_no_change_disposition() {
    let error = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        vec![disposition("finding-001")],
        None,
    )
    .expect_err("one finding needs exactly one proposal disposition");

    assert!(matches!(
        error,
        ProposalRecordError::BindingInvalid { ref field }
            if field == "dispositions.finding_id"
    ));
}

#[test]
fn patch_revision_preserves_accepted_no_change_dispositions() {
    let original = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        vec![disposition("finding-002"), disposition("finding-003")],
        None,
    )
    .expect("proposal builds");

    let revised = original
        .revise(vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.revised"),
        )])
        .expect("revision builds");

    assert_eq!(revised.dispositions(), original.dispositions());

    let disposition_replaced_by_patch = original
        .revise(vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.contradiction"),
        )])
        .expect("a patch supersedes the retained no-change disposition");

    assert_eq!(
        disposition_replaced_by_patch
            .dispositions()
            .iter()
            .map(|disposition| disposition.finding_id.as_str())
            .collect::<Vec<_>>(),
        ["finding-003"],
        "only the disposition the new patch supersedes is dropped"
    );
}

#[test]
fn wire_validation_rejects_noncanonical_disposition_arrays() {
    let record = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        vec![disposition("finding-002"), disposition("finding-003")],
        None,
    )
    .expect("proposal builds");
    let canonical = record.to_canonical_json().expect("serializes");
    assert!(validate_proposal_record(canonical.as_bytes()).is_ok());

    let mut unsorted: Value = serde_json::from_str(&canonical).expect("proposal is JSON");
    unsorted["dispositions"]
        .as_array_mut()
        .expect("dispositions")
        .swap(0, 1);
    assert!(validate_proposal_record(&serde_json::to_vec(&unsorted).expect("serializes")).is_err());

    let mut overlapping: Value = serde_json::from_str(&canonical).expect("proposal is JSON");
    overlapping["dispositions"][0]["finding_id"] = json!("finding-001");
    assert!(
        validate_proposal_record(&serde_json::to_vec(&overlapping).expect("serializes")).is_err()
    );

    let mut explicitly_empty: Value = serde_json::from_str(&canonical).expect("proposal is JSON");
    explicitly_empty["dispositions"] = json!([]);
    assert!(
        validate_proposal_record(&serde_json::to_vec(&explicitly_empty).expect("serializes"))
            .is_err()
    );
}

#[test]
fn wire_validation_rejects_duplicate_top_level_members_before_normalization() {
    let record = build_proposal_record_with_dispositions(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &create_patch("billing.proposed"),
        )],
        vec![disposition("finding-002")],
        None,
    )
    .expect("proposal builds");
    let canonical = record.to_canonical_json().expect("proposal serializes");

    let duplicated_dispositions = canonical.replacen(
        "\"dispositions\": [",
        "\"dispositions\": [],\n  \"dispositions\": [",
        1,
    );
    let digest_member = format!(
        "\"proposal_set_digest\": \"{}\"",
        record.proposal_set_digest()
    );
    let duplicated_digest = canonical.replacen(
        &digest_member,
        &format!("\"proposal_set_digest\": \"{A}\",\n  {digest_member}"),
        1,
    );

    for (member, document) in [
        ("dispositions", duplicated_dispositions),
        ("proposal_set_digest", duplicated_digest),
    ] {
        validate_proposal_record(document.as_bytes()).expect_err(&format!(
            "duplicate {member} members must not be normalized"
        ));
    }
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
    // Ordering is by patch digest alone (placement-blind): the update of
    // `billing.credits` sorts before the create of `billing.proposed`
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
    assert!(value.get("dispositions").is_none());
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
fn published_schema_accepts_a_valid_create_anchor() {
    let mut patch = create_patch("billing.anchored");
    patch["changes"]["placement"]["after"] = json!("billing.credits");
    let record = build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("existing-object anchor is valid");
    let instance: Value = serde_json::from_str(&record.to_canonical_json().expect("serializes"))
        .expect("record is JSON");
    assert!(
        jsonschema::validator_for(&schema())
            .expect("schema compiles")
            .is_valid(&instance)
    );
}

#[test]
fn published_schema_rejects_text_the_domain_rejects() {
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");
    let canonical: Value = serde_json::from_str(&record().to_canonical_json().expect("serializes"))
        .expect("record is JSON");

    for (pointer, invalid) in [
        ("/bindings/change_request/id", " ticket-1 "),
        ("/bindings/change_request/id", "ticket\0"),
        ("/bindings/change_request/id", "ticket-1\n"),
        ("/patches/0/target", " billing.credits "),
        ("/patches/0/target", "billing"),
        ("/patches/0/target", "billing.credits\n"),
        ("/patches/0/page_id", "billing"),
    ] {
        let mut instance = canonical.clone();
        *instance.pointer_mut(pointer).expect("field exists") = json!(invalid);
        assert!(
            !validator.is_valid(&instance),
            "schema accepted {invalid:?} at {pointer}"
        );
    }

    let mut trailing_line_terminator = canonical.clone();
    trailing_line_terminator["proposal_set_digest"] = json!(format!("{A}\n"));
    assert!(!validator.is_valid(&trailing_line_terminator));

    let mut missing_proposer = canonical.clone();
    missing_proposer["patches"][0]["patch"]
        .as_object_mut()
        .expect("patch is an object")
        .remove("proposer");
    assert!(!validator.is_valid(&missing_proposer));

    for proposer in [
        json!({"type": "human", "id": "maintainer"}),
        json!({"type": "agent", "id": " "}),
    ] {
        let mut instance = canonical.clone();
        instance["patches"][0]["patch"]["proposer"] = proposer;
        assert!(!validator.is_valid(&instance));
    }

    let mut unknown_patch_member = canonical.clone();
    unknown_patch_member["patches"][0]["patch"]["admin"] = json!(true);
    assert!(!validator.is_valid(&unknown_patch_member));

    let mut unknown_change_member = canonical.clone();
    unknown_change_member["patches"][0]["patch"]["changes"]["admin"] = json!(true);
    assert!(!validator.is_valid(&unknown_change_member));

    let mut malformed_anchor = canonical;
    let create = malformed_anchor["patches"]
        .as_array_mut()
        .expect("patches")
        .iter_mut()
        .find(|entry| entry["operation"] == "create_object")
        .expect("create patch");
    create["patch"]["changes"]["placement"]["after"] = json!("Billing");
    assert!(!validator.is_valid(&malformed_anchor));
}

#[test]
fn published_schema_matches_each_patch_operation_shape() {
    let published_schema = schema();
    let validator = jsonschema::validator_for(&published_schema).expect("schema compiles");
    let canonical: Value = serde_json::from_str(&record().to_canonical_json().expect("serializes"))
        .expect("record is JSON");

    for (operation, changes, base_hash, foreign_field) in [
        (
            "replace_body",
            json!({"body": "Updated body."}),
            Some(D),
            ("status", json!("draft")),
        ),
        (
            "update_fields",
            json!({"fields": {"status": "draft"}}),
            Some(D),
            ("body", json!("Foreign body.")),
        ),
        (
            "create_object",
            json!({
                "kind": "claim", "status": "draft", "body": "Claim.",
                "placement": {"page_id": "billing.kb"}
            }),
            None,
            ("supersedes", json!(["billing.old"])),
        ),
    ] {
        let mut instance = canonical.clone();
        {
            let entry = &mut instance["patches"][0];
            entry["operation"] = json!(operation);
            entry["patch"]["op"] = json!(operation);
            entry["patch"]["changes"] = changes;
            match base_hash {
                Some(base_hash) => entry["patch"]["base_hash"] = json!(base_hash),
                None => {
                    entry["patch"]
                        .as_object_mut()
                        .expect("patch is an object")
                        .remove("base_hash");
                }
            }
        }
        assert!(
            validator.is_valid(&instance),
            "schema rejected valid {operation} shape"
        );

        instance["patches"][0]["patch"]["changes"][foreign_field.0] = foreign_field.1;
        assert!(
            !validator.is_valid(&instance),
            "schema accepted foreign {operation} change"
        );
    }

    for missing in ["status", "placement"] {
        let mut instance = canonical.clone();
        let create = instance["patches"]
            .as_array_mut()
            .expect("patches")
            .iter_mut()
            .find(|entry| entry["operation"] == "create_object")
            .expect("create patch");
        create["patch"]["changes"]
            .as_object_mut()
            .expect("changes")
            .remove(missing);
        assert!(
            !validator.is_valid(&instance),
            "schema accepted create without {missing}"
        );
    }

    for (status, valid) in [
        (None, false),
        (Some("draft"), true),
        (Some("proposed"), true),
        (Some("open"), true),
        (Some("verified"), false),
    ] {
        let mut instance = canonical.clone();
        let update = instance["patches"]
            .as_array_mut()
            .expect("patches")
            .iter_mut()
            .find(|entry| entry["operation"] == "update_fields")
            .expect("update patch");
        match status {
            Some(status) => update["patch"]["changes"]["fields"]["status"] = json!(status),
            None => {
                update["patch"]["changes"]["fields"]
                    .as_object_mut()
                    .expect("fields")
                    .remove("status");
            }
        }
        assert_eq!(
            validator.is_valid(&instance),
            valid,
            "schema verdict for update status {status:?}"
        );
    }

    for (kind, floor, wrong) in [
        ("claim", "draft", "proposed"),
        ("decision", "proposed", "draft"),
        ("api", "draft", "open"),
        ("task", "open", "draft"),
    ] {
        for (status, valid) in [(floor, true), (wrong, false), ("verified", false)] {
            let mut instance = canonical.clone();
            let create = instance["patches"]
                .as_array_mut()
                .expect("patches")
                .iter_mut()
                .find(|entry| entry["operation"] == "create_object")
                .expect("create patch");
            create["patch"]["changes"]["kind"] = json!(kind);
            create["patch"]["changes"]["status"] = json!(status);
            assert_eq!(
                validator.is_valid(&instance),
                valid,
                "schema verdict for create floor {kind}/{status}"
            );
        }
    }

    let text_pattern = published_schema["$defs"]["text"]["pattern"]
        .as_str()
        .expect("text pattern");
    assert!(
        !text_pattern.contains("\\s"),
        "ECMA-262 \\s excludes U+FEFF unlike Rust trim; enumerate whitespace instead"
    );
    let text_with_byte_order_mark = "Assessment finding.\u{feff}";
    assert!(is_semantic_context_text(text_with_byte_order_mark));
    let mut byte_order_mark = canonical;
    byte_order_mark["patches"][0]["patch"]["reason"] = json!(text_with_byte_order_mark);
    assert!(validator.is_valid(&byte_order_mark));
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
        "proposal_record.revision_unchanged"
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
    for mut patch in authority_patches {
        patch["proposer"] = json!({"type": "agent", "id": "agentdoc-action/claude-code@2.1.215"});
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
fn governance_operation_precedes_patch_content_validation() {
    let null_patch = json!({
        "schema_version": "adoc.patch.v0", "op": "revoke", "target": "billing.credits",
        "base_hash": D, "changes": {}, "reason": "revoke\n", "proposer": null
    });
    let malformed_target = json!({
        "schema_version": "adoc.patch.v0", "op": "revoke", "target": "credits",
        "base_hash": D, "changes": {}, "reason": "revoke",
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code"}
    });
    let canonical_patch = json!({
        "schema_version": "adoc.patch.v0", "op": "revoke", "target": "billing.credits",
        "base_hash": D, "changes": {}, "reason": "revoke",
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code"}
    });
    let mut noncanonical_bytes = patch_input(
        "finding-009",
        "docs/billing.adoc",
        "billing.kb",
        &canonical_patch,
    );
    noncanonical_bytes.patch_bytes =
        serde_json::to_vec_pretty(&canonical_patch).expect("patch serializes");

    for input in [
        patch_input(
            "finding-009",
            "docs/billing.adoc",
            "billing.kb",
            &null_patch,
        ),
        patch_input(
            "finding-009",
            "docs/billing.adoc",
            "billing.kb",
            &malformed_target,
        ),
        noncanonical_bytes,
    ] {
        let error = build_proposal_record(bindings(), vec![input], None)
            .expect_err("governance operation is never proposable");
        assert!(
            error.to_string().contains("changes governance state"),
            "{error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.authority_rejected"
        );
    }
}

#[test]
fn governance_operation_precedes_other_patches_set_level_validation() {
    let mut create = create_patch("billing.proposed");
    create["changes"]["placement"]["after"] = json!("billing.proposed");
    let mut unattributed_create = create.clone();
    unattributed_create
        .as_object_mut()
        .expect("patch is an object")
        .remove("proposer");
    let revoke = json!({
        "schema_version": "adoc.patch.v0", "op": "revoke", "target": "billing.credits",
        "base_hash": D, "changes": {}, "reason": "revoke",
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code"}
    });

    for create in [create, unattributed_create] {
        let error = build_proposal_record(
            bindings(),
            vec![
                patch_input("finding-001", "docs/billing.adoc", "billing.kb", &create),
                patch_input("finding-002", "docs/billing.adoc", "billing.kb", &revoke),
            ],
            None,
        )
        .expect_err("governance authority is categorical across the proposal set");

        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.authority_rejected"
        );
        assert!(
            error.to_string().contains("changes governance state"),
            "{error}"
        );
    }
}

#[test]
fn proposal_record_rejects_noncanonical_reason_text() {
    let mut patch = update_patch("billing.credits", D, "billing");
    patch["reason"] = json!("assessment finding\n");
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
    .expect_err("canonical proposal reason must be semantic text");
    assert!(error.to_string().contains("reason is missing or invalid"));
}

#[test]
fn proposal_record_requires_an_agent_proposer() {
    for proposer in [
        None,
        Some(json!({"type": "human", "id": "maintainer"})),
        Some(json!({"type": "agent", "id": ""})),
        Some(json!({"type": "agent", "id": "  "})),
    ] {
        let mut patch = update_patch("billing.credits", D, "billing");
        match proposer {
            Some(proposer) => patch["proposer"] = proposer,
            None => {
                patch
                    .as_object_mut()
                    .expect("patch is an object")
                    .remove("proposer");
            }
        }

        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("canonical proposal patches require an agent proposer");

        assert!(
            matches!(error, ProposalRecordError::AuthorityRejected { .. }),
            "{error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.authority_rejected"
        );
        assert!(error.to_string().contains("agent proposer"), "{error}");
    }
}

#[test]
fn proposal_record_requires_an_agent_proposer_on_replace_body() {
    for proposer in [
        None,
        Some(json!({"type": "human", "id": "maintainer"})),
        Some(json!({"type": "agent", "id": ""})),
    ] {
        let mut body_patch = replace_body_patch("billing.credits", B, "Updated body.");
        match proposer {
            Some(proposer) => body_patch["proposer"] = proposer,
            None => {
                body_patch
                    .as_object_mut()
                    .expect("patch is an object")
                    .remove("proposer");
            }
        }

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
                    "finding-002",
                    "docs/billing.adoc",
                    "billing.kb",
                    &body_patch,
                ),
            ],
            None,
        )
        .expect_err("every canonical proposal patch requires an agent proposer");

        assert!(matches!(
            error,
            ProposalRecordError::AuthorityRejected { .. }
        ));
        assert!(error.to_string().contains("agent proposer"), "{error}");
    }
}

// Review round 4 (PR #194): a patch target is an Object ID. An empty or
// malformed target would otherwise reach the record and break the published
// schema's `target` (text, minLength 1) as well as every apply-time check.
#[test]
fn patch_target_must_be_an_object_id() {
    for target in ["", "credits", "Billing.Credits", "billing..credits"] {
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &create_patch(target),
            )],
            None,
        )
        .expect_err("target outside the Object ID grammar");
        assert!(
            matches!(error, ProposalRecordError::PatchInvalid { .. }),
            "{target:?}: {error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.patch_invalid"
        );
    }
}

#[test]
fn patch_page_must_be_an_object_id() {
    for patch in [
        update_patch("billing.credits", D, "billing"),
        replace_body_patch("billing.credits", B, "Updated body."),
    ] {
        let mut inputs = vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing",
            &patch,
        )];
        if patch["op"] == "replace_body" {
            inputs.insert(
                0,
                patch_input(
                    "finding-002",
                    "docs/billing.adoc",
                    "billing.kb",
                    &update_patch("billing.credits", D, "billing"),
                ),
            );
        }
        let error = build_proposal_record(bindings(), inputs, None)
            .expect_err("page outside the Object ID grammar");
        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
        assert!(error.to_string().contains("page_id"), "{error}");
    }
}

#[test]
fn patch_placement_path_must_be_logical() {
    for path in [
        "../outside.adoc",
        "/tmp/page.adoc",
        "docs\\page.adoc",
        "C:/docs/page.adoc",
        "docs//page.adoc",
    ] {
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-002",
                path,
                "billing.kb",
                &update_patch("billing.credits", D, "billing"),
            )],
            None,
        )
        .expect_err("placement path must be project-relative and slash-normalized");
        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
        assert!(error.to_string().contains("placement_path"), "{error}");
    }
}

#[test]
fn proposal_record_rejects_null_patch_members() {
    let mut null_base_hash = create_patch("billing.proposed");
    null_base_hash["base_hash"] = Value::Null;
    let mut null_status = create_patch("billing.proposed");
    null_status["changes"]["status"] = Value::Null;
    let validator = jsonschema::validator_for(&schema()).expect("schema compiles");

    for patch in [null_base_hash, null_status] {
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("null members are digest-visible but semantically absent");
        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
        assert!(error.to_string().contains("null member"), "{error}");

        let mut instance: Value =
            serde_json::from_str(&record().to_canonical_json().expect("serializes"))
                .expect("record is JSON");
        instance["patches"][0]["operation"] = json!("create_object");
        instance["patches"][0]["patch"] = patch;
        assert!(!validator.is_valid(&instance));
    }
}

// Review round 4 (PR #194): ADR-0053 §3 — generated fields never duplicate a
// structural member. `fields.status: verified` beside the floor-checked
// top-level status would otherwise pass, and the apply planner overwrites the
// nested value, so the record would carry content the exact patch discards.
#[test]
fn create_fields_cannot_duplicate_structural_members() {
    for (field, value) in [
        ("status", "verified"),
        ("kind", "policy"),
        ("id", "billing.other"),
        ("body", "Another body."),
        ("placement", "billing.kb"),
    ] {
        let mut patch = create_patch("billing.proposed");
        patch["changes"]["fields"] = json!({field: value});
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("structural members are never generated fields");
        assert!(
            matches!(error, ProposalRecordError::AuthorityRejected { .. }),
            "{field}: {error}"
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

// Review round 1 (PR #194): a malformed base_hash must never reach
// content_bindings; a placement-only move of a multi-patch set keeps its
// identity; an unchanged revision names its own cause.
#[test]
fn malformed_base_hash_is_rejected_before_any_record_exists() {
    let error = build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing.kb",
            &update_patch("billing.credits", "not-a-digest", "billing"),
        )],
        None,
    )
    .expect_err("a non-sha256 base_hash cannot bind content");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.patch_invalid"
    );
}

#[test]
fn multi_patch_identity_is_placement_blind() {
    let build = |first_path: &str| {
        let mut create = create_patch("billing.proposed");
        create["changes"]["placement"]["page_id"] = json!("proposal.kb");
        build_proposal_record(
            bindings(),
            vec![
                patch_input(
                    "finding-002",
                    first_path,
                    "billing.kb",
                    &update_patch("billing.credits", D, "billing"),
                ),
                patch_input("finding-001", "docs/billing.adoc", "proposal.kb", &create),
            ],
            None,
        )
        .expect("record builds")
    };
    // Renaming the first patch's source page would reorder a placement-sorted
    // set; the proposal-set digest must not notice.
    let before = build("docs/a.adoc");
    let after = build("docs/z.adoc");
    assert_eq!(before.proposal_set_digest(), after.proposal_set_digest());
    let digests: Vec<_> = before
        .patches()
        .iter()
        .map(|patch| patch.patch_digest().to_string())
        .collect();
    let mut sorted = digests.clone();
    sorted.sort();
    assert_eq!(digests, sorted, "patches are ordered by patch digest alone");
}

#[test]
fn unchanged_revision_names_its_own_cause() {
    let original = record();
    let error = original
        .revise(vec![
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
        ])
        .expect_err("unchanged bytes are not a version");
    assert!(matches!(
        error,
        ProposalRecordError::RevisionUnchanged { .. }
    ));
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.revision_unchanged"
    );
}

// Review round 2 (PR #194): ADR-0054 §5 — one logical update is
// `update_fields` then `replace_body`, the body patch bound to the hash
// re-derived after the field patch; the record binds the exact-head hash.
fn replace_body_patch(target: &str, base_hash: &str, body: &str) -> Value {
    json!({
        "schema_version": "adoc.patch.v0",
        "op": "replace_body",
        "target": target,
        "base_hash": base_hash,
        "changes": {"body": body},
        "reason": format!("AgentDoc assessment {A} finding finding-002."),
        "proposer": {"type": "agent", "id": "agentdoc-action/claude-code@2.1.215/claude-sonnet-5"}
    })
}

#[test]
fn two_patch_logical_update_binds_the_exact_head_hash() {
    let input =
        |patch: &Value| patch_input("finding-002", "docs/billing.adoc", "billing.kb", patch);
    let record = build_proposal_record(
        bindings(),
        vec![
            input(&replace_body_patch("billing.credits", B, "Re-hashed body.")),
            input(&update_patch("billing.credits", D, "billing")),
        ],
        None,
    )
    .expect("a sequential body hash is not a conflict");
    let bound: Vec<_> = record
        .content_bindings()
        .iter()
        .map(|binding| (binding.object_id(), binding.content_hash()))
        .collect();
    assert_eq!(bound, vec![("billing.credits", D)]);
    validate_proposal_record(record.to_canonical_json().expect("serializes").as_bytes())
        .expect("round-trips");

    // Two patches of one operation for one target are not a sequence.
    let error = build_proposal_record(
        bindings(),
        vec![
            input(&replace_body_patch("billing.credits", B, "One.")),
            input(&replace_body_patch("billing.credits", D, "Two.")),
        ],
        None,
    )
    .expect_err("one target has at most one replace_body");
    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.patch_invalid"
    );

    // Review round 3 (PR #194): a body-only edit of an already-reviewable
    // object carries the mandatory status write as a no-op, so the field
    // patch does not re-hash the object and the body patch legitimately
    // binds the same base_hash. The record cannot see whether the field
    // patch changes anything; whether the second hash is right is checked
    // where the patches are applied (PRD §51.5), not here.
    let mut status_only = update_patch("billing.credits", D, "billing");
    status_only["changes"] = json!({"fields": {"status": "draft"}});
    let record = build_proposal_record(
        bindings(),
        vec![
            input(&status_only),
            input(&replace_body_patch("billing.credits", D, "Body only.")),
        ],
        None,
    )
    .expect("a no-op status write leaves the body patch on the exact-head hash");
    let bound: Vec<_> = record
        .content_bindings()
        .iter()
        .map(|binding| (binding.object_id(), binding.content_hash()))
        .collect();
    assert_eq!(bound, vec![("billing.credits", D)]);
    validate_proposal_record(record.to_canonical_json().expect("serializes").as_bytes())
        .expect("round-trips");
}

#[test]
fn unicode_whitespace_body_is_proposable() {
    let input =
        |patch: &Value| patch_input("finding-002", "docs/billing.adoc", "billing.kb", patch);
    let mut status_only = update_patch("billing.credits", D, "billing");
    status_only["changes"] = json!({"fields": {"status": "draft"}});

    build_proposal_record(
        bindings(),
        vec![
            input(&status_only),
            input(&replace_body_patch("billing.credits", D, "\u{a0}")),
        ],
        None,
    )
    .expect("source Body treats only ASCII edge whitespace as blank");
}

#[test]
fn one_target_edit_uses_one_exact_head_coordinate() {
    for (body_path, body_page) in [
        ("docs/other.adoc", "billing.kb"),
        ("docs/billing.adoc", "billing.other"),
    ] {
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
                    "finding-002",
                    body_path,
                    body_page,
                    &replace_body_patch("billing.credits", B, "Re-hashed body."),
                ),
            ],
            None,
        )
        .expect_err("one exact-head object cannot have conflicting coordinates");

        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.patch_invalid"
        );
        assert!(
            error.to_string().contains("conflicting coordinates"),
            "{error}"
        );
    }
}

// Review round 2 (PR #194): the record cannot see an object's current
// lifecycle, so every existing-object edit must carry its downgrade — an
// `update_fields` setting a reviewable status. Otherwise a verified object
// would keep its authority over model-written content.
#[test]
fn existing_object_update_without_a_reviewable_status_is_rejected() {
    let input =
        |patch: &Value| patch_input("finding-002", "docs/billing.adoc", "billing.kb", patch);
    let mut no_status = update_patch("billing.credits", D, "billing");
    no_status["changes"] = json!({"fields": {"owner": "billing"}});
    for patches in [
        vec![input(&no_status)],
        vec![input(&replace_body_patch(
            "billing.credits",
            D,
            "Body only.",
        ))],
        vec![
            input(&no_status),
            input(&replace_body_patch(
                "billing.credits",
                B,
                "Fields without status.",
            )),
        ],
    ] {
        let error = build_proposal_record(bindings(), patches, None)
            .expect_err("an edit that preserves the current lifecycle is not proposable");
        assert!(
            matches!(error, ProposalRecordError::AuthorityRejected { .. }),
            "{error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.authority_rejected"
        );
    }
}

// Review round 5 (PR #194): a create the standard patch path would refuse in
// draft validation (`adoc patch --check`) never enters a record — a blank
// body, or a task with no owner — so a canonical record cannot carry an
// intrinsically unapplyable create.
#[test]
fn intrinsically_invalid_creates_are_rejected() {
    let mut blank_body = create_patch("billing.proposed");
    blank_body["changes"]["body"] = json!("   ");
    let mut ownerless_task = create_patch("billing.followup");
    ownerless_task["changes"]["kind"] = json!("task");
    ownerless_task["changes"]["status"] = json!("open");
    let mut missing_placement = create_patch("billing.unplaced");
    missing_placement["changes"]
        .as_object_mut()
        .expect("changes is an object")
        .remove("placement");
    let mut invalid_page = create_patch("billing.bad-page");
    invalid_page["changes"]["placement"]["page_id"] = json!("billing");
    let mut invalid_after = create_patch("billing.bad-anchor");
    invalid_after["changes"]["placement"]["after"] = json!("not-an-object-id");
    let mut invalid_evidence_ref = create_patch("billing.bad-evidence");
    invalid_evidence_ref["changes"]["fields"] = json!({"evidence_ref": "not-an-object-id"});
    let mut invalid_impacts = create_patch("billing.bad-impacts");
    invalid_impacts["changes"]["fields"] = json!({"impacts": "../outside"});
    let mut invalid_api_path = create_patch("billing.bad-api-path");
    invalid_api_path["changes"]["kind"] = json!("api");
    invalid_api_path["changes"]["fields"] = json!({"method": "GET", "path": "\u{a0}/foo"});
    let mut unsafe_body = create_patch("billing.unsafe-body");
    unsafe_body["changes"]["body"] = json!("Body.\n::\nInjected.");
    let mut multiline_field = create_patch("billing.multiline-field");
    multiline_field["changes"]["fields"] = json!({"owner": "team\nother"});
    let mut mismatched_page = create_patch("billing.wrong-page");
    mismatched_page["changes"]["placement"]["page_id"] = json!("billing.other-page");
    for patch in [
        blank_body,
        ownerless_task,
        missing_placement,
        invalid_page,
        invalid_after,
        invalid_evidence_ref,
        invalid_impacts,
        invalid_api_path,
        unsafe_body,
        multiline_field,
        mismatched_page,
    ] {
        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("a create the patch path rejects is not proposable");
        assert!(
            matches!(error, ProposalRecordError::PatchInvalid { .. }),
            "{error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.patch_invalid"
        );
    }
}

#[test]
fn proposer_authority_precedes_same_set_create_anchor_validation() {
    let mut patch = create_patch("billing.proposed");
    patch["changes"]["placement"]["after"] = json!("billing.proposed");
    patch
        .as_object_mut()
        .expect("patch is an object")
        .remove("proposer");

    let error = build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect_err("unattributed proposal authority is the actionable defect");

    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.authority_rejected"
    );
}

#[test]
fn proposer_authority_precedes_entry_validation_for_updates() {
    let mut patch = update_patch("billing.credits", D, "billing");
    patch
        .as_object_mut()
        .expect("patch is an object")
        .remove("proposer");

    let error = build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing",
            &patch,
        )],
        None,
    )
    .expect_err("unattributed proposal authority is the actionable defect");

    assert_eq!(
        error.diagnostic_code().as_str(),
        "proposal_record.authority_rejected"
    );
}

#[test]
fn proposal_record_rejects_normalized_create_status_bytes() {
    for (status, expected_code, expected_message) in [
        (
            " draft ",
            "proposal_record.patch_invalid",
            "must not require normalization",
        ),
        (
            " verified ",
            "proposal_record.authority_rejected",
            "outside the create-only floors",
        ),
    ] {
        let mut patch = create_patch("billing.proposed");
        patch["changes"]["status"] = json!(status);

        let error = build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-001",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect_err("record bytes must carry the exact create status floor");

        assert_eq!(error.diagnostic_code().as_str(), expected_code, "{status}");
        assert!(error.to_string().contains(expected_message), "{error}");
    }
}

#[test]
fn bracketed_evidence_refs_are_intrinsically_valid() {
    let mut patch = create_patch("billing.bracketed-evidence");
    patch["changes"]["fields"] = json!({
        "evidence_ref": "[source.one, source.two]"
    });

    build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("source-compatible bracketed evidence_ref is proposable");
}

#[test]
fn unparsed_impacts_metadata_is_proposable() {
    let mut patch = create_patch("billing.follow-up");
    patch["changes"]["kind"] = json!("task");
    patch["changes"]["status"] = json!("open");
    patch["changes"]["fields"] = json!({
        "owner": "support-ops",
        "impacts": "/outside"
    });

    build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("task retains impacts as metadata instead of parsing repository paths");
}

#[test]
fn intrinsically_invalid_edits_are_rejected() {
    let mut blank_reason = update_patch("billing.credits", D, "billing");
    blank_reason["reason"] = json!("  ");
    let mut unicode_blank_reason = update_patch("billing.credits", D, "billing");
    unicode_blank_reason["reason"] = json!("\u{a0}");
    let mut invalid_key = update_patch("billing.credits", D, "billing");
    invalid_key["changes"]["fields"] = json!({"Bad-Key": "value", "status": "draft"});
    let mut unknown_field = update_patch("billing.credits", D, "billing");
    unknown_field["changes"]["fields"] = json!({"status": "draft", "totally_unknown": "x"});
    let mut relation = update_patch("billing.credits", D, "billing");
    relation["changes"]["fields"] = json!({"status": "draft", "depends_on": "billing.other"});
    let mut blank_value = update_patch("billing.credits", D, "billing");
    blank_value["changes"]["fields"] = json!({"owner": " ", "status": "draft"});
    let mut status_only = update_patch("billing.credits", D, "billing");
    status_only["changes"]["fields"] = json!({"status": "draft"});
    let mut structural = update_patch("billing.credits", D, "billing");
    structural["changes"]["fields"] = json!({"body": "Replacement", "status": "draft"});
    let mut invalid_visibility = update_patch("billing.credits", D, "billing");
    invalid_visibility["changes"]["fields"] = json!({"status": "draft", "visibility": "secret"});
    let mut multiline_field = update_patch("billing.credits", D, "billing");
    multiline_field["changes"]["fields"] = json!({"status": "draft", "owner": "team\nother"});

    for patches in [
        vec![blank_reason],
        vec![unicode_blank_reason],
        vec![invalid_key],
        vec![unknown_field],
        vec![relation],
        vec![blank_value],
        vec![structural],
        vec![invalid_visibility],
        vec![multiline_field],
        vec![status_only, replace_body_patch("billing.credits", D, " ")],
        vec![
            update_patch("billing.credits", D, "billing"),
            replace_body_patch("billing.credits", D, "Body.\n::\nInjected."),
        ],
    ] {
        let inputs = patches
            .iter()
            .map(|patch| patch_input("finding-002", "docs/billing.adoc", "billing.kb", patch))
            .collect();
        let error = build_proposal_record(bindings(), inputs, None)
            .expect_err("a patch the standard validator rejects is not proposable");
        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
    }
}

#[test]
fn update_evidence_syntax_waits_for_exact_head_kind() {
    for evidence_ref in ["not-an-object-id", "source.one,,source.two"] {
        let mut patch = update_patch("billing.credits", D, "billing");
        patch["changes"]["fields"] = json!({"status": "draft", "evidence_ref": evidence_ref});

        build_proposal_record(
            bindings(),
            vec![patch_input(
                "finding-002",
                "docs/billing.adoc",
                "billing.kb",
                &patch,
            )],
            None,
        )
        .expect("the graph-independent record cannot know whether the target parses evidence_ref");
    }
}

#[test]
fn update_impacts_syntax_waits_for_exact_head_kind() {
    let mut patch = update_patch("billing.credits", D, "billing");
    patch["changes"]["fields"] = json!({"status": "draft", "impacts": "../outside"});

    build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("the graph-independent record cannot know whether the target parses impacts");
}

#[test]
fn unicode_whitespace_field_value_is_proposable() {
    let mut patch = update_patch("billing.credits", D, "billing");
    patch["changes"]["fields"] = json!({"status": "draft", "owner": "\u{a0}"});

    build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-002",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("update matches the source parser's ASCII-only blank-value rule");
}

// Review round 5 (PR #194): creates carry no base_hash, so two creates of one
// target — or a create beside an edit of the same target — used to bypass the
// per-target sequence; sequential application would refuse the set.
#[test]
fn one_target_is_created_at_most_once_and_never_also_edited() {
    let input = |finding: &str, patch: &Value| {
        patch_input(finding, "docs/billing.adoc", "billing.kb", patch)
    };
    let mut other_create = create_patch("billing.proposed");
    other_create["changes"]["body"] = json!("A different proposed claim.");
    for patches in [
        vec![
            input("finding-001", &create_patch("billing.proposed")),
            input("finding-002", &other_create),
        ],
        vec![
            input("finding-001", &create_patch("billing.proposed")),
            input(
                "finding-002",
                &update_patch("billing.proposed", D, "billing"),
            ),
        ],
        vec![
            input(
                "finding-002",
                &update_patch("billing.proposed", D, "billing"),
            ),
            input("finding-001", &create_patch("billing.proposed")),
        ],
    ] {
        let error = build_proposal_record(bindings(), patches, None)
            .expect_err("one target is created at most once and never also edited");
        assert!(
            matches!(error, ProposalRecordError::PatchInvalid { .. }),
            "{error}"
        );
        assert_eq!(
            error.diagnostic_code().as_str(),
            "proposal_record.patch_invalid"
        );
    }
}

#[test]
fn creates_cannot_use_proposed_objects_as_placement_anchors() {
    let anchored = |target: &str, after: &str| {
        let mut patch = create_patch(target);
        patch["changes"]["placement"]["after"] = json!(after);
        patch
    };
    let input = |finding: &str, patch: &Value| {
        patch_input(finding, "docs/billing.adoc", "billing.kb", patch)
    };

    for patches in [
        vec![input(
            "finding-001",
            &anchored("billing.proposed", "billing.proposed"),
        )],
        vec![
            input("finding-001", &anchored("billing.second", "billing.first")),
            input("finding-002", &create_patch("billing.first")),
        ],
    ] {
        let error = build_proposal_record(bindings(), patches, None)
            .expect_err("a proposed object cannot anchor another create");
        assert!(matches!(error, ProposalRecordError::PatchInvalid { .. }));
    }
}

#[test]
fn page_and_object_id_namespaces_are_independent() {
    let patch = create_patch("billing.kb");

    let record = build_proposal_record(
        bindings(),
        vec![patch_input(
            "finding-001",
            "docs/billing.adoc",
            "billing.kb",
            &patch,
        )],
        None,
    )
    .expect("an existing page may share a new object's identifier");

    assert_eq!(record.patches()[0].target(), "billing.kb");
}

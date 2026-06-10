//! V3.4 proof-obligation trigger rules.
//!
//! Two pure dispatch functions that map V3 review primitives onto the shared
//! [`ProofObligation`] value object promoted in ADR-0020:
//!
//! - [`obligations_for_change`] dispatches on a [`FieldChange`] projection
//!   inside a [`ChangedObject`] entry.
//! - [`obligations_for_impact`] emits one impact-review obligation against
//!   the impacted claim's `source` evidence.
//!
//! See V3-DESIGN.md §V3.4 for the trigger table and the verified-claim gate.
//! Deduplication by `(object_id, reason)` happens at the application layer
//! in [`crate::application::review::proof_obligations`].

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::knowledge_object::claim::{OWNER_FIELD, VERIFIED_AT_FIELD};
use crate::domain::obligation::ProofObligation;

use super::field_change::FieldChange;
use super::impact::ImpactedObject;
use super::object_change::ChangedObject;

const CLAIM_KIND: &str = "claim";
const POLICY_KIND: &str = "policy";
const AGENT_INSTRUCTION_KIND: &str = "agent_instruction";
const CONTRADICTION_KIND: &str = "contradiction";
const VERIFIED_STATUS: &str = "verified";
const ACTIVE_STATUS: &str = "active";
const NEEDS_REVIEW_STATUS: &str = "needs_review";
const DRAFT_STATUS: &str = "draft";
const UNRESOLVED_STATUS: &str = "unresolved";

// Stable reason strings — module-scoped so tests and the application-layer
// dedup compare against constants instead of duplicating literals.
pub(crate) const REASON_REVERIFY_BODY: &str = "re-verify body";
pub(crate) const REASON_STALE_VERIFIED: &str = "stale verified claim";
pub(crate) const REASON_VERIFIED_DEMOTED: &str = "verified claim demoted";
pub(crate) const REASON_REASSIGN_OWNER: &str = "reassign owner";
pub(crate) const REASON_NEW_OWNER_ACK: &str = "new owner must acknowledge";
pub(crate) const REASON_REVERIFY_AT_CLEARED: &str = "re-verify (verified_at cleared)";
pub(crate) const REASON_REVIEW_IMPACT: &str = "review impacted claim";
pub(crate) const REASON_REEVIDENCE_PREFIX: &str = "re-evidence";
pub(crate) const REASON_REAPPROVE_EFFECTIVE_AT: &str = "re-approve (effective_at changed)";
pub(crate) const REASON_REAPPROVE_APPROVER_REMOVED: &str = "re-approve (approver removed)";
pub(crate) const REASON_SECURITY_REVIEW_TRUST_UPGRADE: &str = "security review (trust upgraded)";
pub(crate) const REASON_SECURITY_REVIEW_FORBIDDEN_REMOVED: &str =
    "security review (forbidden action removed)";
pub(crate) const REASON_OWNER_REASSERT: &str = "owner re-assert (unresolved contradiction changed)";

/// Dispatch the V3.4 trigger table against one `Changed` entry.
///
/// V3.4 emits no obligations for `Created` or `Deleted` `ObjectChange`
/// variants, so the aggregator (`application::review::proof_obligations`)
/// calls this directly against `ObjectDiff::changed[..]` without paying the
/// wrapper-clone cost of round-tripping through `ObjectChange::Changed`.
pub(crate) fn obligations_for_change(changed: &ChangedObject) -> Vec<ProofObligation> {
    let mut out = Vec::new();
    for field_change in changed.field_changes() {
        push_for_field_change(&mut out, changed, field_change);
    }
    // V5.6: any field change on an unresolved contradiction requires the
    // owner to re-assert the conflict. "Any field change" is an existence
    // condition, so this fires once per changed object, not once per field.
    // Reading `head` means edits that resolve the contradiction (head status
    // `resolved` or `dismissed`) do not fire.
    if !changed.field_changes().is_empty() && is_unresolved_contradiction(&changed.head) {
        out.push(ProofObligation {
            object_id: changed.id.to_string(),
            reason: REASON_OWNER_REASSERT.to_string(),
            required_evidence: vec![OWNER_FIELD.to_string()],
        });
    }
    out
}

fn push_for_field_change(
    out: &mut Vec<ProofObligation>,
    changed: &ChangedObject,
    field_change: &FieldChange,
) {
    let head = &changed.head;
    let id = changed.id.as_str();
    match field_change {
        FieldChange::Body { .. } if is_verified_claim(head) => {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_REVERIFY_BODY.to_string(),
                required_evidence: present_evidence_fields(head),
            });
        }
        FieldChange::Status { before, after } => {
            if before.as_deref() == Some(VERIFIED_STATUS)
                && after.as_deref() == Some(NEEDS_REVIEW_STATUS)
            {
                out.push(ProofObligation {
                    object_id: id.to_string(),
                    reason: REASON_STALE_VERIFIED.to_string(),
                    required_evidence: Vec::new(),
                });
            } else if before.as_deref() == Some(VERIFIED_STATUS)
                && after.as_deref() == Some(DRAFT_STATUS)
            {
                out.push(ProofObligation {
                    object_id: id.to_string(),
                    reason: REASON_VERIFIED_DEMOTED.to_string(),
                    required_evidence: Vec::new(),
                });
            }
        }
        FieldChange::Owner { before, after } if is_verified_claim(head) => {
            match (before.as_deref(), after.as_deref()) {
                (Some(_), None) => out.push(ProofObligation {
                    object_id: id.to_string(),
                    reason: REASON_REASSIGN_OWNER.to_string(),
                    required_evidence: vec![OWNER_FIELD.to_string()],
                }),
                (Some(a), Some(b)) if a != b => out.push(ProofObligation {
                    object_id: id.to_string(),
                    reason: REASON_NEW_OWNER_ACK.to_string(),
                    required_evidence: vec![OWNER_FIELD.to_string()],
                }),
                _ => {}
            }
        }
        FieldChange::VerifiedAt { before, after }
            if is_verified_claim(head) && before.is_some() && after.is_none() =>
        {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_REVERIFY_AT_CLEARED.to_string(),
                required_evidence: vec![VERIFIED_AT_FIELD.to_string()],
            });
        }
        FieldChange::EvidenceRemoved { field, .. } if is_verified_claim(head) => {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: format!("{REASON_REEVIDENCE_PREFIX}: {field}"),
                required_evidence: vec![field.clone()],
            });
        }
        FieldChange::EffectiveAt { .. } if is_active_policy(head) => {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_REAPPROVE_EFFECTIVE_AT.to_string(),
                required_evidence: vec![
                    crate::domain::knowledge_object::APPROVED_BY_FIELD.to_string(),
                ],
            });
        }
        FieldChange::ApprovedByRemoved { .. } if is_active_policy(head) => {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_REAPPROVE_APPROVER_REMOVED.to_string(),
                required_evidence: vec![
                    crate::domain::knowledge_object::APPROVED_BY_FIELD.to_string(),
                ],
            });
        }
        // V5.5: a trust upgrade on an agent_instruction requires a security review.
        FieldChange::Trust { before, after }
            if is_agent_instruction(head) && trust_is_upgrade(before, after) =>
        {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_SECURITY_REVIEW_TRUST_UPGRADE.to_string(),
                required_evidence: Vec::new(),
            });
        }
        // V5.5: removing a forbidden action from an agent_instruction requires a
        // security review.
        FieldChange::ForbiddenActionsRemoved { .. } if is_agent_instruction(head) => {
            out.push(ProofObligation {
                object_id: id.to_string(),
                reason: REASON_SECURITY_REVIEW_FORBIDDEN_REMOVED.to_string(),
                required_evidence: Vec::new(),
            });
        }
        // EvidenceAdded, ApprovedByAdded, RelationAdded/Removed,
        // ImpactsAdded/Removed, AllowedActionsAdded/Removed,
        // ForbiddenActionsAdded, Trust (downgrade/same), plus future
        // non-exhaustive variants — explicitly emit nothing.
        _ => {}
    }
}

/// Emit the impact-review obligation against an [`ImpactedObject`].
///
/// `compute_impact` already filters for verified subjects, so this trigger
/// is unconditional — one obligation per impact entry, with
/// `required_evidence: ["source_code"]` (the V5.8 EvidenceKind string).
pub(crate) fn obligations_for_impact(impact: &ImpactedObject) -> Vec<ProofObligation> {
    use crate::domain::value_objects::evidence_kind::EvidenceKind;
    vec![ProofObligation {
        object_id: impact.id.clone(),
        reason: REASON_REVIEW_IMPACT.to_string(),
        required_evidence: vec![EvidenceKind::SourceCode.as_str().to_string()],
    }]
}

fn is_verified_claim(node: &GraphKnowledgeObjectNode) -> bool {
    node.kind == CLAIM_KIND && node.status.as_deref() == Some(VERIFIED_STATUS)
}

fn is_active_policy(node: &GraphKnowledgeObjectNode) -> bool {
    node.kind == POLICY_KIND && node.status.as_deref() == Some(ACTIVE_STATUS)
}

fn is_agent_instruction(node: &GraphKnowledgeObjectNode) -> bool {
    node.kind == AGENT_INSTRUCTION_KIND
}

/// A contradiction's lifecycle `ContradictionStatus` is the metadata
/// discriminant landing in the node's `status` slot, so the comparison
/// against `"unresolved"` is well-defined for `contradiction` nodes.
fn is_unresolved_contradiction(node: &GraphKnowledgeObjectNode) -> bool {
    node.kind == CONTRADICTION_KIND && node.status.as_deref() == Some(UNRESOLVED_STATUS)
}

/// Return `true` when a trust change is an upgrade (after > before).
///
/// Both sides are optional strings taken from the graph node's `status` slot,
/// where trust is stored for `agent_instruction` nodes. Returns `false` if
/// either side is absent or cannot be parsed as a valid `Trust`.
fn trust_is_upgrade(before: &Option<String>, after: &Option<String>) -> bool {
    use crate::domain::value_objects::trust::Trust;
    let (Some(b), Some(a)) = (before, after) else {
        return false;
    };
    match (Trust::try_new(b), Trust::try_new(a)) {
        (Ok(before_trust), Ok(after_trust)) => after_trust > before_trust,
        _ => false,
    }
}

fn present_evidence_fields(node: &GraphKnowledgeObjectNode) -> Vec<String> {
    // V5.8: evidence is in node.evidence, keyed by EvidenceKind string.
    node.evidence
        .iter()
        .filter(|entry| entry.value.is_some())
        .map(|entry| entry.kind.clone())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::GraphEvidence;
    use crate::domain::review::field_change::RelationKind;
    use crate::domain::review::object_diff::test_support::test_node;
    use crate::domain::value_objects::evidence_kind::EvidenceKind;

    fn verified_claim(id: &str) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.status = Some(VERIFIED_STATUS.to_string());
        node
    }

    fn verified_claim_with_evidence(id: &str) -> GraphKnowledgeObjectNode {
        let mut node = verified_claim(id);
        node.evidence.push(GraphEvidence::inline(
            EvidenceKind::SourceCode.as_str(),
            "ledger",
        ));
        node.evidence.push(GraphEvidence::inline(
            EvidenceKind::Test.as_str(),
            "integration",
        ));
        node.evidence.push(GraphEvidence::inline(
            EvidenceKind::HumanReview.as_str(),
            "team-billing",
        ));
        node
    }

    fn changed_with(
        id: &str,
        base: GraphKnowledgeObjectNode,
        head: GraphKnowledgeObjectNode,
        field_changes: Vec<FieldChange>,
    ) -> ChangedObject {
        let mut c = ChangedObject::new(id.to_string(), base, head);
        c.field_changes = field_changes;
        c
    }

    // -- Acceptance: V3-DESIGN.md §V3.4 --

    #[test]
    fn body_change_on_verified_claim_with_three_evidence_fields_emits_one_obligation() {
        let base = verified_claim_with_evidence("billing.credits");
        let head = verified_claim_with_evidence("billing.credits");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Body {
                before: "old".to_string(),
                after: "new".to_string(),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].object_id, "billing.credits");
        assert_eq!(obligations[0].reason, REASON_REVERIFY_BODY);
        // V5.8: required_evidence now uses EvidenceKind strings.
        assert_eq!(
            obligations[0].required_evidence,
            vec!["source_code", "test", "human_review"]
        );
    }

    #[test]
    fn draft_claim_change_produces_zero_obligations() {
        let mut base = test_node("billing.draft", "sha256:a");
        base.status = Some(DRAFT_STATUS.to_string());
        let mut head = test_node("billing.draft", "sha256:b");
        head.status = Some(DRAFT_STATUS.to_string());
        let change = changed_with(
            "billing.draft",
            base,
            head,
            vec![FieldChange::Body {
                before: "x".to_string(),
                after: "y".to_string(),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn impacted_verified_claim_emits_source_evidence_obligation() {
        let impact = ImpactedObject {
            id: "billing.refunds".to_string(),
            paths: vec!["crates/billing/src/refund.rs".to_string()],
        };

        let obligations = obligations_for_impact(&impact);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].object_id, "billing.refunds");
        assert_eq!(obligations[0].reason, REASON_REVIEW_IMPACT);
        // V5.8: source evidence is now "source_code".
        assert_eq!(obligations[0].required_evidence, vec!["source_code"]);
    }

    // -- Per-trigger dispatch table coverage --

    #[test]
    fn body_change_with_only_two_evidence_fields_present_emits_two_in_required_evidence() {
        let mut head = verified_claim("billing.credits");
        head.evidence.push(GraphEvidence::inline(
            EvidenceKind::SourceCode.as_str(),
            "ledger",
        ));
        head.evidence.push(GraphEvidence::inline(
            EvidenceKind::HumanReview.as_str(),
            "team-billing",
        ));
        let change = changed_with(
            "billing.credits",
            verified_claim("billing.credits"),
            head,
            vec![FieldChange::Body {
                before: "old".to_string(),
                after: "new".to_string(),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        // V5.8: EvidenceKind strings.
        assert_eq!(
            obligations[0].required_evidence,
            vec!["source_code", "human_review"]
        );
    }

    #[test]
    fn verified_to_needs_review_emits_stale_claim_notice() {
        let mut base = test_node("billing.credits", "sha256:a");
        base.status = Some(VERIFIED_STATUS.to_string());
        let mut head = test_node("billing.credits", "sha256:b");
        head.status = Some(NEEDS_REVIEW_STATUS.to_string());
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Status {
                before: Some(VERIFIED_STATUS.to_string()),
                after: Some(NEEDS_REVIEW_STATUS.to_string()),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_STALE_VERIFIED);
        assert!(obligations[0].required_evidence.is_empty());
    }

    #[test]
    fn verified_to_draft_emits_demotion_review() {
        let mut head = test_node("billing.credits", "sha256:b");
        head.status = Some(DRAFT_STATUS.to_string());
        let change = changed_with(
            "billing.credits",
            verified_claim("billing.credits"),
            head,
            vec![FieldChange::Status {
                before: Some(VERIFIED_STATUS.to_string()),
                after: Some(DRAFT_STATUS.to_string()),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_VERIFIED_DEMOTED);
    }

    #[test]
    fn draft_to_verified_emits_no_obligation() {
        let mut base = test_node("billing.credits", "sha256:a");
        base.status = Some(DRAFT_STATUS.to_string());
        let head = verified_claim("billing.credits");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Status {
                before: Some(DRAFT_STATUS.to_string()),
                after: Some(VERIFIED_STATUS.to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn owner_removal_on_verified_claim_emits_reassign_obligation() {
        let mut base = verified_claim("billing.credits");
        base.fields
            .insert(OWNER_FIELD.to_string(), "team-billing".to_string());
        let head = verified_claim("billing.credits");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Owner {
                before: Some("team-billing".to_string()),
                after: None,
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_REASSIGN_OWNER);
        assert_eq!(obligations[0].required_evidence, vec!["owner"]);
    }

    #[test]
    fn owner_change_on_verified_claim_emits_new_owner_acknowledge_obligation() {
        let mut base = verified_claim("billing.credits");
        base.fields
            .insert(OWNER_FIELD.to_string(), "team-old".to_string());
        let mut head = verified_claim("billing.credits");
        head.fields
            .insert(OWNER_FIELD.to_string(), "team-new".to_string());
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Owner {
                before: Some("team-old".to_string()),
                after: Some("team-new".to_string()),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_NEW_OWNER_ACK);
        assert_eq!(obligations[0].required_evidence, vec!["owner"]);
    }

    #[test]
    fn initial_owner_assignment_emits_no_obligation() {
        let base = verified_claim("billing.credits");
        let mut head = verified_claim("billing.credits");
        head.fields
            .insert(OWNER_FIELD.to_string(), "team-billing".to_string());
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Owner {
                before: None,
                after: Some("team-billing".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn owner_removal_on_draft_claim_emits_no_obligation() {
        let mut base = test_node("billing.draft", "sha256:a");
        base.status = Some(DRAFT_STATUS.to_string());
        base.fields
            .insert(OWNER_FIELD.to_string(), "team-x".to_string());
        let mut head = test_node("billing.draft", "sha256:b");
        head.status = Some(DRAFT_STATUS.to_string());
        let change = changed_with(
            "billing.draft",
            base,
            head,
            vec![FieldChange::Owner {
                before: Some("team-x".to_string()),
                after: None,
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn verified_at_removal_on_verified_claim_emits_re_verify_obligation() {
        let mut base = verified_claim("billing.credits");
        base.fields
            .insert(VERIFIED_AT_FIELD.to_string(), "2026-05-05".to_string());
        let head = verified_claim("billing.credits");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::VerifiedAt {
                before: Some("2026-05-05".to_string()),
                after: None,
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_REVERIFY_AT_CLEARED);
        assert_eq!(obligations[0].required_evidence, vec!["verified_at"]);
    }

    #[test]
    fn evidence_removed_on_verified_claim_emits_re_evidence_against_field() {
        let base = verified_claim("billing.credits");
        let head = verified_claim("billing.credits");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::EvidenceRemoved {
                field: EvidenceKind::Test.as_str().to_string(),
                value: "integration".to_string(),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, "re-evidence: test");
        assert_eq!(obligations[0].required_evidence, vec!["test"]);
    }

    #[test]
    fn evidence_added_emits_no_obligation() {
        let base = verified_claim("billing.credits");
        let mut head = verified_claim("billing.credits");
        head.evidence.push(GraphEvidence::inline(
            EvidenceKind::SourceCode.as_str(),
            "ledger",
        ));
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::EvidenceAdded {
                field: EvidenceKind::SourceCode.as_str().to_string(),
                value: "ledger".to_string(),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn relation_changes_emit_no_obligation() {
        let base = verified_claim("billing.credits");
        let mut head = verified_claim("billing.credits");
        head.relations.depends_on = vec!["billing.ledger".to_string()];
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![
                FieldChange::RelationAdded {
                    kind: RelationKind::DependsOn,
                    target: "billing.ledger".to_string(),
                },
                FieldChange::RelationRemoved {
                    kind: RelationKind::Supersedes,
                    target: "billing.old".to_string(),
                },
            ],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    fn active_policy(id: &str) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.kind = POLICY_KIND.to_string();
        node.status = Some(ACTIVE_STATUS.to_string());
        node
    }

    fn proposed_policy(id: &str) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.kind = POLICY_KIND.to_string();
        node.status = Some("proposed".to_string());
        node
    }

    #[test]
    fn effective_at_change_on_active_policy_emits_reapprove_obligation() {
        let base = active_policy("security.data-retention");
        let head = active_policy("security.data-retention");
        let change = changed_with(
            "security.data-retention",
            base,
            head,
            vec![FieldChange::EffectiveAt {
                before: Some("2026-01-01".to_string()),
                after: Some("2026-06-01".to_string()),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].object_id, "security.data-retention");
        assert_eq!(obligations[0].reason, REASON_REAPPROVE_EFFECTIVE_AT);
        assert_eq!(obligations[0].required_evidence, vec!["approved_by"]);
    }

    #[test]
    fn approver_removed_from_active_policy_emits_reapprove_obligation() {
        let base = active_policy("security.data-retention");
        let head = active_policy("security.data-retention");
        let change = changed_with(
            "security.data-retention",
            base,
            head,
            vec![FieldChange::ApprovedByRemoved {
                value: "security-lead".to_string(),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].reason, REASON_REAPPROVE_APPROVER_REMOVED);
        assert_eq!(obligations[0].required_evidence, vec!["approved_by"]);
    }

    #[test]
    fn approver_added_to_active_policy_emits_no_obligation() {
        let base = active_policy("security.data-retention");
        let head = active_policy("security.data-retention");
        let change = changed_with(
            "security.data-retention",
            base,
            head,
            vec![FieldChange::ApprovedByAdded {
                value: "new-approver".to_string(),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn effective_at_change_on_non_active_policy_emits_no_obligation() {
        let base = proposed_policy("security.data-retention");
        let head = proposed_policy("security.data-retention");
        let change = changed_with(
            "security.data-retention",
            base,
            head,
            vec![FieldChange::EffectiveAt {
                before: Some("2026-01-01".to_string()),
                after: Some("2026-06-01".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn approver_removed_from_non_active_policy_emits_no_obligation() {
        let base = proposed_policy("security.data-retention");
        let head = proposed_policy("security.data-retention");
        let change = changed_with(
            "security.data-retention",
            base,
            head,
            vec![FieldChange::ApprovedByRemoved {
                value: "security-lead".to_string(),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn impacts_changes_emit_no_obligation() {
        let base = verified_claim("billing.credits");
        let mut head = verified_claim("billing.credits");
        head.impacts = vec!["crates/billing/src/refund.rs".to_string()];
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![
                FieldChange::ImpactsAdded {
                    path: "crates/billing/src/refund.rs".to_string(),
                },
                FieldChange::ImpactsRemoved {
                    path: "src/old.rs".to_string(),
                },
            ],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn multiple_triggers_in_one_changed_object_emit_one_obligation_each() {
        let mut base = verified_claim_with_evidence("billing.credits");
        base.fields
            .insert(OWNER_FIELD.to_string(), "team-old".to_string());
        let mut head = verified_claim_with_evidence("billing.credits");
        head.fields
            .insert(OWNER_FIELD.to_string(), "team-new".to_string());
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![
                FieldChange::Body {
                    before: "old".to_string(),
                    after: "new".to_string(),
                },
                FieldChange::Owner {
                    before: Some("team-old".to_string()),
                    after: Some("team-new".to_string()),
                },
            ],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 2);
        let reasons: Vec<&str> = obligations.iter().map(|o| o.reason.as_str()).collect();
        assert!(reasons.contains(&REASON_REVERIFY_BODY));
        assert!(reasons.contains(&REASON_NEW_OWNER_ACK));
    }

    fn agent_instruction(id: &str, trust: &str) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.kind = AGENT_INSTRUCTION_KIND.to_string();
        node.status = Some(trust.to_string());
        node
    }

    #[test]
    fn trust_upgrade_on_agent_instruction_emits_security_review_obligation() {
        let base = agent_instruction("auth.docs-answering-policy", "team");
        let head = agent_instruction("auth.docs-answering-policy", "authoritative");
        let change = changed_with(
            "auth.docs-answering-policy",
            base,
            head,
            vec![FieldChange::Trust {
                before: Some("team".to_string()),
                after: Some("authoritative".to_string()),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].object_id, "auth.docs-answering-policy");
        assert_eq!(obligations[0].reason, REASON_SECURITY_REVIEW_TRUST_UPGRADE);
        assert!(obligations[0].required_evidence.is_empty());
    }

    #[test]
    fn trust_downgrade_on_agent_instruction_emits_no_obligation() {
        let base = agent_instruction("auth.docs-answering-policy", "system");
        let head = agent_instruction("auth.docs-answering-policy", "team");
        let change = changed_with(
            "auth.docs-answering-policy",
            base,
            head,
            vec![FieldChange::Trust {
                before: Some("system".to_string()),
                after: Some("team".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn trust_same_level_on_agent_instruction_emits_no_obligation() {
        let base = agent_instruction("auth.docs-answering-policy", "team");
        let head = agent_instruction("auth.docs-answering-policy", "team");
        let change = changed_with(
            "auth.docs-answering-policy",
            base,
            head,
            vec![FieldChange::Trust {
                before: Some("team".to_string()),
                after: Some("team".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn forbidden_action_removed_from_agent_instruction_emits_security_review_obligation() {
        let base = agent_instruction("auth.docs-answering-policy", "team");
        let head = agent_instruction("auth.docs-answering-policy", "team");
        let change = changed_with(
            "auth.docs-answering-policy",
            base,
            head,
            vec![FieldChange::ForbiddenActionsRemoved {
                value: "execute_shell".to_string(),
            }],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(
            obligations[0].reason,
            REASON_SECURITY_REVIEW_FORBIDDEN_REMOVED
        );
        assert!(obligations[0].required_evidence.is_empty());
    }

    #[test]
    fn forbidden_action_added_to_agent_instruction_emits_no_obligation() {
        let base = agent_instruction("auth.docs-answering-policy", "team");
        let head = agent_instruction("auth.docs-answering-policy", "team");
        let change = changed_with(
            "auth.docs-answering-policy",
            base,
            head,
            vec![FieldChange::ForbiddenActionsAdded {
                value: "new_forbidden".to_string(),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    fn contradiction(id: &str, status: &str) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.kind = CONTRADICTION_KIND.to_string();
        node.status = Some(status.to_string());
        node
    }

    fn body_change() -> FieldChange {
        FieldChange::Body {
            before: "old".to_string(),
            after: "new".to_string(),
        }
    }

    #[test]
    fn field_change_on_unresolved_contradiction_emits_one_owner_reassert_obligation() {
        let base = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let head = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let change = changed_with("auth.session.conflict", base, head, vec![body_change()]);

        let obligations = obligations_for_change(&change);

        assert_eq!(obligations.len(), 1);
        assert_eq!(obligations[0].object_id, "auth.session.conflict");
        assert_eq!(obligations[0].reason, REASON_OWNER_REASSERT);
        assert_eq!(obligations[0].required_evidence, vec!["owner"]);
    }

    #[test]
    fn multiple_field_changes_on_unresolved_contradiction_emit_one_obligation() {
        let base = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let head = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let change = changed_with(
            "auth.session.conflict",
            base,
            head,
            vec![
                body_change(),
                FieldChange::ContradictionClaimsAdded {
                    value: "auth.new-claim".to_string(),
                },
            ],
        );

        let obligations = obligations_for_change(&change);

        assert_eq!(
            obligations.len(),
            1,
            "owner re-assert fires once per changed object, not per field"
        );
        assert_eq!(obligations[0].reason, REASON_OWNER_REASSERT);
    }

    #[test]
    fn field_change_on_resolved_contradiction_emits_no_obligation() {
        let base = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let head = contradiction("auth.session.conflict", "resolved");
        let change = changed_with(
            "auth.session.conflict",
            base,
            head,
            vec![FieldChange::Status {
                before: Some(UNRESOLVED_STATUS.to_string()),
                after: Some("resolved".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn field_change_on_dismissed_contradiction_emits_no_obligation() {
        let base = contradiction("auth.session.conflict", "dismissed");
        let head = contradiction("auth.session.conflict", "dismissed");
        let change = changed_with("auth.session.conflict", base, head, vec![body_change()]);

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn unresolved_contradiction_with_no_field_changes_emits_no_obligation() {
        let base = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let head = contradiction("auth.session.conflict", UNRESOLVED_STATUS);
        let change = changed_with("auth.session.conflict", base, head, Vec::new());

        assert!(obligations_for_change(&change).is_empty());
    }

    #[test]
    fn trust_upgrade_on_non_agent_instruction_emits_no_obligation() {
        // A Trust variant on a claim node should not trigger the obligation.
        let base = test_node("billing.credits", "sha256:a");
        let head = test_node("billing.credits", "sha256:b");
        let change = changed_with(
            "billing.credits",
            base,
            head,
            vec![FieldChange::Trust {
                before: Some("team".to_string()),
                after: Some("system".to_string()),
            }],
        );

        assert!(obligations_for_change(&change).is_empty());
    }
}

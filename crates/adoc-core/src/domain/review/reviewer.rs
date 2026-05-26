//! V3.3 required reviewer projection.
//!
//! Aggregates the `owner` field of every changed verified Knowledge Object and
//! every impacted Knowledge Object, grouping changed-object IDs and impacted
//! Object IDs under the responsible owner.
//!
//! Pure domain projection — no I/O. See V3-DESIGN.md §V3.3.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::knowledge_object::metadata::KnowledgeObjectMetadata;

use super::impact::ImpactedObject;
use super::object_diff::ObjectDiff;

const CLAIM_KIND: &str = "claim";
const DECISION_KIND: &str = "decision";
const VERIFIED_STATUS: &str = "verified";
const ACCEPTED_STATUS: &str = "accepted";

/// One reviewer entry. Lists every Knowledge Object ID for which this owner is
/// the required reviewer — both changed verified objects and impacted objects.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequiredReviewer {
    pub owner: String,
    pub object_ids: Vec<String>,
}

/// Compute the required-reviewer list from a [`ObjectDiff`] and the V3.3
/// [`ImpactedObject`] list.
///
/// Sources of reviewer obligation in V3.3:
/// - Each `changed[]` entry whose **head** record is a verified claim
///   (`kind=claim`, `status=verified`) or accepted decision (`kind=decision`,
///   `status=accepted`) and carries an `owner` field.
/// - Each `created[]` entry that is a verified claim / accepted decision with
///   an `owner` field.
/// - Each [`ImpactedObject`] — the corresponding head record's `owner` field.
///
/// Owners missing from the head record are silently skipped — V3.4
/// reassignment obligations cover the "no owner" case.
///
/// Output is sorted by `owner`; within each entry, `object_ids` is sorted
/// ascending and deduplicated.
pub fn required_reviewers(diff: &ObjectDiff, impact: &[ImpactedObject]) -> Vec<RequiredReviewer> {
    let mut by_owner: BTreeMap<String, std::collections::BTreeSet<String>> = BTreeMap::new();

    let head_subjects = diff
        .created
        .iter()
        .chain(diff.changed.iter().map(|c| &c.head))
        .filter(|node| is_verified_subject(node));

    for node in head_subjects {
        if let Some(owner) = owner_of(node) {
            by_owner
                .entry(owner.to_string())
                .or_default()
                .insert(node.id.clone());
        }
    }

    let head_by_id: BTreeMap<&str, &GraphKnowledgeObjectNode> = diff
        .created
        .iter()
        .chain(diff.changed.iter().map(|c| &c.head))
        .map(|node| (node.id.as_str(), node))
        .collect();

    for entry in impact {
        if let Some(node) = head_by_id.get(entry.id.as_str())
            && let Some(owner) = owner_of(node)
        {
            by_owner
                .entry(owner.to_string())
                .or_default()
                .insert(entry.id.clone());
        }
    }

    by_owner
        .into_iter()
        .map(|(owner, ids)| RequiredReviewer {
            owner,
            object_ids: ids.into_iter().collect(),
        })
        .collect()
}

fn owner_of(node: &GraphKnowledgeObjectNode) -> Option<&str> {
    KnowledgeObjectMetadata::from_node(node).owner
}

fn is_verified_subject(node: &GraphKnowledgeObjectNode) -> bool {
    let Some(status) = node.status.as_deref() else {
        return false;
    };
    match node.kind.as_str() {
        CLAIM_KIND => status == VERIFIED_STATUS,
        DECISION_KIND => status == ACCEPTED_STATUS,
        _ => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::graph::GraphKnowledgeObjectNode;
    use crate::domain::knowledge_object::claim::OWNER_FIELD;
    use crate::domain::review::object_diff::test_support::test_node;

    fn verified_claim(id: &str, owner: Option<&str>) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.status = Some(VERIFIED_STATUS.to_string());
        if let Some(owner) = owner {
            node.fields
                .insert(OWNER_FIELD.to_string(), owner.to_string());
        }
        node
    }

    fn impacted(id: &str) -> ImpactedObject {
        ImpactedObject {
            id: id.to_string(),
            paths: vec!["a.rs".to_string()],
        }
    }

    #[test]
    fn required_reviewers_groups_changed_objects_by_owner() {
        let diff = ObjectDiff::compute(
            &[],
            &[
                verified_claim("billing.refunds", Some("team-billing")),
                verified_claim("billing.credits", Some("team-billing")),
                verified_claim("auth.session", Some("team-auth")),
            ],
        );

        let reviewers = required_reviewers(&diff, &[]);

        assert_eq!(reviewers.len(), 2);
        assert_eq!(reviewers[0].owner, "team-auth");
        assert_eq!(reviewers[0].object_ids, vec!["auth.session"]);
        assert_eq!(reviewers[1].owner, "team-billing");
        assert_eq!(
            reviewers[1].object_ids,
            vec!["billing.credits", "billing.refunds"]
        );
    }

    #[test]
    fn required_reviewers_includes_impacted_objects_owners() {
        let diff = ObjectDiff::compute(
            &[],
            &[verified_claim("billing.refunds", Some("team-billing"))],
        );

        let reviewers = required_reviewers(&diff, &[impacted("billing.refunds")]);

        assert_eq!(reviewers.len(), 1);
        assert_eq!(reviewers[0].owner, "team-billing");
        assert_eq!(reviewers[0].object_ids, vec!["billing.refunds"]);
    }

    #[test]
    fn required_reviewers_dedupes_when_changed_and_impacted_overlap() {
        let diff = ObjectDiff::compute(
            &[],
            &[verified_claim("billing.refunds", Some("team-billing"))],
        );

        let reviewers = required_reviewers(&diff, &[impacted("billing.refunds")]);

        assert_eq!(reviewers.len(), 1);
        assert_eq!(reviewers[0].object_ids.len(), 1);
        assert_eq!(reviewers[0].object_ids[0], "billing.refunds");
    }

    #[test]
    fn required_reviewers_skips_objects_without_owner_field() {
        let diff = ObjectDiff::compute(&[], &[verified_claim("billing.refunds", None)]);

        assert!(required_reviewers(&diff, &[]).is_empty());
    }

    #[test]
    fn required_reviewers_ignores_unverified_changed_objects() {
        let mut node = test_node("billing.draft", "sha256:n");
        node.status = Some("draft".to_string());
        node.fields
            .insert(OWNER_FIELD.to_string(), "team-x".to_string());

        let diff = ObjectDiff::compute(&[], &[node]);

        assert!(required_reviewers(&diff, &[]).is_empty());
    }
}

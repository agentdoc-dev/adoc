//! V3.3 source-path impact analysis.
//!
//! Given a mechanical [`ObjectDiff`] and the set of changed files (from
//! `ChangedFilesProvider`), [`compute_impact`] flags every verified Knowledge
//! Object whose declared `impacts:` list intersects the changed-file set.
//!
//! Pure domain projection — no I/O. Knowledge Object scope only. See
//! V3-DESIGN.md §V3.3 and ADR-0019.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::value_objects::rel_path::RelPath;

use super::object_diff::ObjectDiff;

const CLAIM_KIND: &str = "claim";
const DECISION_KIND: &str = "decision";
const VERIFIED_STATUS: &str = "verified";
const ACCEPTED_STATUS: &str = "accepted";

/// One entry of the V3.3 impact list. Carries the Knowledge Object's id
/// plus the subset of its `impacts:` paths that are in the changed-file set.
///
/// Sorted ascending by `id` in the containing list; `paths` is itself
/// sorted ascending.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImpactedObject {
    pub id: String,
    pub paths: Vec<String>,
}

/// Flag verified claims / accepted decisions whose `impacts` list intersects
/// `changed_files`.
///
/// Considers the **head** side of [`ObjectDiff::changed`] entries and the
/// records in [`ObjectDiff::created`]. Deleted entries never appear in the
/// impact list — proof obligations for deletions are V3.4 territory.
///
/// The returned vector is sorted by Object ID, with each entry's `paths`
/// sorted ascending. `compute_impact(g, [])` is empty.
pub fn compute_impact(diff: &ObjectDiff, changed_files: &[RelPath]) -> Vec<ImpactedObject> {
    if changed_files.is_empty() {
        return Vec::new();
    }
    let changed: BTreeSet<&str> = changed_files.iter().map(RelPath::as_str).collect();

    let mut impacted: Vec<ImpactedObject> = Vec::new();
    for node in diff
        .created
        .iter()
        .chain(diff.changed.iter().map(|c| &c.head))
    {
        if let Some(entry) = impact_entry_for(node, &changed) {
            impacted.push(entry);
        }
    }
    impacted.sort_by(|a, b| a.id.cmp(&b.id));
    impacted
}

fn impact_entry_for(
    node: &GraphKnowledgeObjectNode,
    changed: &BTreeSet<&str>,
) -> Option<ImpactedObject> {
    if !is_verified_subject(node) || node.impacts.is_empty() {
        return None;
    }
    let mut hits: BTreeSet<&str> = BTreeSet::new();
    for declared in &node.impacts {
        if changed.contains(declared.as_str()) {
            hits.insert(declared.as_str());
        }
    }
    if hits.is_empty() {
        return None;
    }
    Some(ImpactedObject {
        id: node.id.clone(),
        paths: hits.into_iter().map(str::to_string).collect(),
    })
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
    use crate::domain::review::object_change::ChangedObject;
    use crate::domain::review::object_diff::test_support::test_node;

    fn rel(s: &str) -> RelPath {
        RelPath::try_new(s).expect("valid test path")
    }

    fn verified_claim_with_impacts(id: &str, paths: &[&str]) -> GraphKnowledgeObjectNode {
        let mut node = test_node(id, "sha256:dummy");
        node.status = Some(VERIFIED_STATUS.to_string());
        node.impacts = paths.iter().map(|p| (*p).to_string()).collect();
        node
    }

    fn diff_with_created(nodes: Vec<GraphKnowledgeObjectNode>) -> ObjectDiff {
        ObjectDiff::compute(&[], &nodes)
    }

    fn diff_with_changed(
        base_head_pairs: Vec<(GraphKnowledgeObjectNode, GraphKnowledgeObjectNode)>,
    ) -> ObjectDiff {
        // Hand-build a diff via Changed entries — distinct content_hash forces them
        // into `changed[]` when fed to `compute`.
        let base: Vec<GraphKnowledgeObjectNode> =
            base_head_pairs.iter().map(|(b, _)| b.clone()).collect();
        let head: Vec<GraphKnowledgeObjectNode> =
            base_head_pairs.iter().map(|(_, h)| h.clone()).collect();
        ObjectDiff::compute(&base, &head)
    }

    // The "first failing test" listed in V3-DESIGN.md §Test Pyramid for V3.3.
    #[test]
    fn compute_impact_flags_verified_claim_when_impacts_path_in_changed_set() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["crates/billing/src/refund.rs"],
        )]);

        let impacted = compute_impact(&diff, &[rel("crates/billing/src/refund.rs")]);

        assert_eq!(
            impacted,
            vec![ImpactedObject {
                id: "billing.refunds".to_string(),
                paths: vec!["crates/billing/src/refund.rs".to_string()],
            }]
        );
    }

    #[test]
    fn compute_impact_returns_empty_when_no_files_changed() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["a.rs"],
        )]);

        assert!(compute_impact(&diff, &[]).is_empty());
    }

    #[test]
    fn compute_impact_returns_empty_when_no_overlap() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["a.rs"],
        )]);

        assert!(compute_impact(&diff, &[rel("b.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_sorts_overlapping_paths_ascending() {
        let diff = diff_with_created(vec![verified_claim_with_impacts(
            "billing.refunds",
            &["z.rs", "a.rs", "m.rs"],
        )]);

        let impacted = compute_impact(
            &diff,
            &[rel("m.rs"), rel("z.rs"), rel("a.rs"), rel("other.rs")],
        );

        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].paths, vec!["a.rs", "m.rs", "z.rs"]);
    }

    #[test]
    fn compute_impact_sorts_results_by_object_id() {
        let diff = diff_with_created(vec![
            verified_claim_with_impacts("zz.late", &["a.rs"]),
            verified_claim_with_impacts("aa.early", &["a.rs"]),
        ]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);

        let ids: Vec<&str> = impacted.iter().map(|i| i.id.as_str()).collect();
        assert_eq!(ids, vec!["aa.early", "zz.late"]);
    }

    #[test]
    fn compute_impact_ignores_non_verified_claim_even_when_impacts_match() {
        let mut node = test_node("billing.refunds", "sha256:n");
        node.status = Some("draft".to_string()); // not verified
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_includes_accepted_decision_intersection() {
        let mut node = test_node("billing.policy", "sha256:n");
        node.kind = "decision".to_string();
        node.status = Some(ACCEPTED_STATUS.to_string());
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "billing.policy");
    }

    #[test]
    fn compute_impact_ignores_proposed_decision() {
        let mut node = test_node("billing.policy", "sha256:n");
        node.kind = "decision".to_string();
        node.status = Some("proposed".to_string());
        node.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_created(vec![node]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    #[test]
    fn compute_impact_uses_head_side_of_changed_entries() {
        // base side has no impacts; head side declares impacts.
        let mut base = test_node("billing.refunds", "sha256:base");
        base.status = Some(VERIFIED_STATUS.to_string());
        let mut head = test_node("billing.refunds", "sha256:head");
        head.status = Some(VERIFIED_STATUS.to_string());
        head.impacts = vec!["a.rs".to_string()];

        let diff = diff_with_changed(vec![(base, head)]);

        let impacted = compute_impact(&diff, &[rel("a.rs")]);
        assert_eq!(impacted.len(), 1);
        assert_eq!(impacted[0].id, "billing.refunds");
    }

    #[test]
    fn compute_impact_ignores_deleted_objects() {
        // Deleted-only side: base has the verified claim, head omits it.
        let mut base = test_node("billing.refunds", "sha256:base");
        base.status = Some(VERIFIED_STATUS.to_string());
        base.impacts = vec!["a.rs".to_string()];

        let diff = ObjectDiff::compute(&[base], &[]);

        assert!(compute_impact(&diff, &[rel("a.rs")]).is_empty());
    }

    #[test]
    fn changed_object_referenced_so_dead_code_lint_is_satisfied() {
        // Suppress unused import warning for ChangedObject in this test module
        // (it is read transitively through ObjectDiff::changed but the lint
        // sometimes flags the direct `use` line).
        let _ = std::mem::size_of::<ChangedObject>();
    }
}

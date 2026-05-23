//! The Object Diff aggregate.
//!
//! `ObjectDiff` is the value object produced by comparing two Knowledge
//! Object slices. Its sole constructor is [`ObjectDiff::compute`], so the
//! invariants — pure-mechanical Knowledge-Object scope, deterministic
//! Object-ID ordering, no duplicate entries — hold by construction rather
//! than by external validation.
//!
//! See V3-DESIGN.md §V3.1 and ADR-0018 for the wire-contract rationale.

use std::collections::BTreeSet;

use serde::Serialize;

use crate::domain::graph::GraphKnowledgeObjectNode;

use super::object_change::ChangedObject;

/// The Object Diff aggregate. Knowledge Object scope only; sorted by Object
/// ID within each array; deterministic across runs.
///
/// Serialized as the `created` / `deleted` / `changed` sub-trees of an
/// `adoc.diff.v0` envelope.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ObjectDiff {
    pub(crate) created: Vec<GraphKnowledgeObjectNode>,
    pub(crate) deleted: Vec<GraphKnowledgeObjectNode>,
    pub(crate) changed: Vec<ChangedObject>,
}

impl ObjectDiff {
    /// Compute the diff between two Knowledge Object slices.
    ///
    /// Identity invariant: `compute(g, g)` is empty for any `g` whose nodes
    /// have stable `content_hash` values (the graph builder guarantees this —
    /// see `infrastructure/artifact/graph_json.rs::graph_knowledge_object_content_hash`).
    pub(crate) fn compute(
        base: &[GraphKnowledgeObjectNode],
        head: &[GraphKnowledgeObjectNode],
    ) -> Self {
        let base_by_id = index_by_id(base);
        let head_by_id = index_by_id(head);

        let all_ids: BTreeSet<&str> = base_by_id
            .keys()
            .chain(head_by_id.keys())
            .copied()
            .collect();

        let mut created = Vec::new();
        let mut deleted = Vec::new();
        let mut changed = Vec::new();

        for id in all_ids {
            match (base_by_id.get(id), head_by_id.get(id)) {
                (None, Some(head_node)) => created.push((*head_node).clone()),
                (Some(base_node), None) => deleted.push((*base_node).clone()),
                (Some(base_node), Some(head_node)) => {
                    if base_node.content_hash != head_node.content_hash {
                        changed.push(ChangedObject::new(
                            id.to_string(),
                            (*base_node).clone(),
                            (*head_node).clone(),
                        ));
                    }
                }
                (None, None) => {
                    // Unreachable: every id in `all_ids` came from at least
                    // one of the two indexes.
                }
            }
        }

        Self {
            created,
            deleted,
            changed,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.created.is_empty() && self.deleted.is_empty() && self.changed.is_empty()
    }

    // Accessor methods are used by inline domain and application unit tests.
    // Production callers serialize the diff via serde and never touch its
    // typed structure directly (V3-DESIGN.md §"Public Core API Additions").
    // Promoting these to `pub(crate)` outside `#[cfg(test)]` would trip the
    // dead-code lint enforced by the prek hook.
    #[cfg(test)]
    pub(crate) fn created(&self) -> &[GraphKnowledgeObjectNode] {
        &self.created
    }

    #[cfg(test)]
    pub(crate) fn deleted(&self) -> &[GraphKnowledgeObjectNode] {
        &self.deleted
    }

    #[cfg(test)]
    pub(crate) fn changed(&self) -> &[ChangedObject] {
        &self.changed
    }
}

fn index_by_id(
    nodes: &[GraphKnowledgeObjectNode],
) -> std::collections::BTreeMap<&str, &GraphKnowledgeObjectNode> {
    let mut index = std::collections::BTreeMap::new();
    for node in nodes {
        index.insert(node.id.as_str(), node);
    }
    index
}

#[cfg(test)]
pub(crate) mod test_support {
    use std::collections::BTreeMap;

    use crate::domain::graph::{GraphKnowledgeObjectNode, GraphRelations, GraphSourceSpan};

    /// Build a minimal `GraphKnowledgeObjectNode` for use in unit tests.
    ///
    /// Lives alongside `ObjectDiff` so domain-level tests in this module and
    /// its siblings (e.g. `object_change.rs`) share one constructor.
    pub(crate) fn test_node(id: &str, content_hash: &str) -> GraphKnowledgeObjectNode {
        GraphKnowledgeObjectNode {
            id: id.to_string(),
            kind: "claim".to_string(),
            content_hash: content_hash.to_string(),
            status: Some("draft".to_string()),
            body: format!("{id} body"),
            page_id: "team.billing".to_string(),
            source_span: GraphSourceSpan {
                path: "docs/billing.adoc".to_string(),
                line: 1,
                column: 1,
            },
            fields: BTreeMap::new(),
            relations: GraphRelations::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::test_support::test_node;
    use super::*;

    #[test]
    fn empty_inputs_produce_empty_diff() {
        let diff = ObjectDiff::compute(&[], &[]);

        assert!(diff.is_empty());
        assert!(diff.created().is_empty());
        assert!(diff.deleted().is_empty());
        assert!(diff.changed().is_empty());
    }

    #[test]
    fn identical_inputs_produce_empty_diff() {
        let nodes = vec![
            test_node("billing.credits", "sha256:credits"),
            test_node("billing.refunds", "sha256:refunds"),
        ];

        let diff = ObjectDiff::compute(&nodes, &nodes);

        assert!(diff.is_empty(), "compute(g, g) must be empty");
    }

    #[test]
    fn created_objects_appear_only_in_head() {
        let base = vec![test_node("billing.credits", "sha256:credits")];
        let head = vec![
            test_node("billing.credits", "sha256:credits"),
            test_node("billing.holds", "sha256:holds"),
        ];

        let diff = ObjectDiff::compute(&base, &head);

        assert_eq!(diff.created().len(), 1);
        assert_eq!(diff.created()[0].id, "billing.holds");
        assert!(diff.deleted().is_empty());
        assert!(diff.changed().is_empty());
    }

    #[test]
    fn deleted_objects_appear_only_in_base() {
        let base = vec![
            test_node("billing.credits", "sha256:credits"),
            test_node("billing.legacy-credits", "sha256:legacy"),
        ];
        let head = vec![test_node("billing.credits", "sha256:credits")];

        let diff = ObjectDiff::compute(&base, &head);

        assert_eq!(diff.deleted().len(), 1);
        assert_eq!(diff.deleted()[0].id, "billing.legacy-credits");
        assert!(diff.created().is_empty());
        assert!(diff.changed().is_empty());
    }

    #[test]
    fn changed_objects_have_different_content_hash_on_each_side() {
        let base = vec![test_node("billing.credits", "sha256:base-credits")];
        let head = vec![test_node("billing.credits", "sha256:head-credits")];

        let diff = ObjectDiff::compute(&base, &head);

        assert_eq!(diff.changed().len(), 1);
        let entry = &diff.changed()[0];
        assert_eq!(entry.id, "billing.credits");
        assert_eq!(entry.base.content_hash, "sha256:base-credits");
        assert_eq!(entry.head.content_hash, "sha256:head-credits");
    }

    #[test]
    fn full_diff_aggregates_created_deleted_and_changed_in_sorted_order() {
        let base = vec![
            test_node("billing.credits", "sha256:base-credits"),
            test_node("billing.refunds", "sha256:refunds"),
            test_node("billing.legacy-credits", "sha256:legacy"),
        ];
        let head = vec![
            test_node("billing.credits", "sha256:head-credits"),
            test_node("billing.refunds", "sha256:refunds"),
            test_node("billing.holds", "sha256:holds"),
        ];

        let diff = ObjectDiff::compute(&base, &head);

        assert_eq!(diff.created().len(), 1);
        assert_eq!(diff.created()[0].id, "billing.holds");

        assert_eq!(diff.deleted().len(), 1);
        assert_eq!(diff.deleted()[0].id, "billing.legacy-credits");

        assert_eq!(diff.changed().len(), 1);
        assert_eq!(diff.changed()[0].id, "billing.credits");
        assert_ne!(
            diff.changed()[0].base.content_hash,
            diff.changed()[0].head.content_hash
        );
    }

    #[test]
    fn diff_arrays_are_sorted_by_object_id_regardless_of_input_order() {
        let base = vec![
            test_node("z.b", "sha256:zb"),
            test_node("a.x", "sha256:ax"),
            test_node("m.y", "sha256:my"),
        ];

        let diff = ObjectDiff::compute(&base, &[]);

        let ids: Vec<&str> = diff.deleted().iter().map(|node| node.id.as_str()).collect();
        assert_eq!(ids, vec!["a.x", "m.y", "z.b"]);
    }
}

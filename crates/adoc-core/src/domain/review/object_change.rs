//! Sealed enum representing one entry of an [`ObjectDiff`].
//!
//! Only constructible via the sibling [`super::object_diff::ObjectDiff::compute`]
//! function — the variants encode the three exhaustive outcomes of a
//! Knowledge-Object-scoped set diff: a record appears only on the head side
//! (`Created`), only on the base side (`Deleted`), or on both sides with a
//! different `content_hash` (`Changed`).
//!
//! See V3-DESIGN.md §V3.1 and ADR-0018 for the wire-contract rationale.

use serde::Serialize;

use crate::domain::graph::GraphKnowledgeObjectNode;
use crate::domain::review::field_change::FieldChange;

/// One entry of an [`ObjectDiff`]. Closed enum; new variants would require a
/// `v1` envelope bump and are out of scope for V3.
// V3.4 will iterate `ObjectChange` for obligation dispatch; for V3.1 it is
// only exercised by the inline tests below. `#[allow(dead_code)]` documents
// the deferred consumer rather than silencing a real warning.
#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub(crate) enum ObjectChange {
    Created { record: GraphKnowledgeObjectNode },
    Deleted { record: GraphKnowledgeObjectNode },
    Changed(Box<ChangedObject>),
}

/// The before/after view emitted when an Object ID exists on both sides of the
/// diff with a different `content_hash`.
///
/// Fields are `pub` so external consumers can read the projection;
/// instances are only constructed by
/// [`super::object_diff::ObjectDiff::compute`] via the `pub(super)` factory.
///
/// `field_changes` is the V3.2 typed projection over the base/head pair; it is
/// populated by [`crate::application::review::diff_objects`] after the
/// mechanical diff is computed. The field is `#[serde(skip_serializing_if =
/// "Vec::is_empty")]` so V3.1 envelopes (and any code path that builds a
/// `ChangedObject` without running the projection) stay byte-identical.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ChangedObject {
    pub id: String,
    pub(crate) base: GraphKnowledgeObjectNode,
    pub(crate) head: GraphKnowledgeObjectNode,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub(crate) field_changes: Vec<FieldChange>,
}

impl ChangedObject {
    /// Factory used by [`super::object_diff::ObjectDiff::compute`] (the sole
    /// production constructor) and by application-layer unit tests that need
    /// to drive [`crate::application::review::project_changed`] against
    /// hand-built records.
    pub(crate) fn new(
        id: String,
        base: GraphKnowledgeObjectNode,
        head: GraphKnowledgeObjectNode,
    ) -> Self {
        Self {
            id,
            base,
            head,
            field_changes: Vec::new(),
        }
    }

    /// Read-only access to the typed projection. Empty until
    /// [`crate::application::review::diff_objects`] decorates the diff.
    pub fn field_changes(&self) -> &[FieldChange] {
        &self.field_changes
    }

    /// The `status:` field on the head-side record, if present. Consumed by
    /// the V3.5 Markdown presenter for the per-object status icon; the wire
    /// envelope already exposes head/base records, so this accessor adds no
    /// new contract — it only widens the in-crate visibility.
    pub fn head_status(&self) -> Option<&str> {
        self.head.status.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::review::object_diff::test_support::test_node;
    use serde_json::json;

    #[test]
    fn created_variant_serializes_with_type_tag_and_record() {
        let change = ObjectChange::Created {
            record: test_node("billing.holds", "sha256:head-holds"),
        };

        let value = serde_json::to_value(&change).expect("ObjectChange serializes");

        assert_eq!(value["type"], "created");
        assert_eq!(value["record"]["id"], "billing.holds");
        assert_eq!(value["record"]["content_hash"], "sha256:head-holds");
    }

    #[test]
    fn deleted_variant_serializes_with_type_tag_and_record() {
        let change = ObjectChange::Deleted {
            record: test_node("billing.legacy-credits", "sha256:base-legacy"),
        };

        let value = serde_json::to_value(&change).expect("ObjectChange serializes");

        assert_eq!(value["type"], "deleted");
        assert_eq!(value["record"]["id"], "billing.legacy-credits");
    }

    #[test]
    fn changed_variant_flattens_id_base_head_alongside_type_tag() {
        let change = ObjectChange::Changed(Box::new(ChangedObject::new(
            "billing.credits".to_string(),
            test_node("billing.credits", "sha256:base-credits"),
            test_node("billing.credits", "sha256:head-credits"),
        )));

        let value = serde_json::to_value(&change).expect("ObjectChange serializes");

        assert_eq!(value["type"], "changed");
        assert_eq!(value["id"], "billing.credits");
        assert_eq!(value["base"]["content_hash"], "sha256:base-credits");
        assert_eq!(value["head"]["content_hash"], "sha256:head-credits");
    }

    #[test]
    fn changed_object_serializes_without_type_tag() {
        // The envelope's `changed[]` array contains `ChangedObject` directly,
        // not the wrapping `ObjectChange::Changed` — so its JSON has no
        // `"type":"changed"` field. This is the wire contract V3.1 publishes.
        let entry = ChangedObject::new(
            "billing.credits".to_string(),
            test_node("billing.credits", "sha256:base-credits"),
            test_node("billing.credits", "sha256:head-credits"),
        );

        let value = serde_json::to_value(&entry).expect("ChangedObject serializes");

        assert_eq!(
            value,
            json!({
                "id": "billing.credits",
                "base": test_node_json("billing.credits", "sha256:base-credits"),
                "head": test_node_json("billing.credits", "sha256:head-credits"),
            })
        );
    }

    fn test_node_json(id: &str, content_hash: &str) -> serde_json::Value {
        serde_json::to_value(test_node(id, content_hash)).expect("node serializes")
    }
}

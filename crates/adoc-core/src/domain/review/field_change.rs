//! Typed projection over a [`super::object_change::ChangedObject`].
//!
//! `FieldChange` is the V3.2 vocabulary that explains *what* differs inside a
//! Knowledge Object whose `content_hash` flipped between two snapshots. It is
//! pure data — the projection logic lives in
//! `application::review::project_changed` and is decorated onto each
//! `Changed` entry by `application::review::diff_objects`.
//!
//! Sealed `#[non_exhaustive]` enum: V3.3 will add `ImpactsAdded` /
//! `ImpactsRemoved` and tolerant readers must already ignore unknown variants.
//!
//! See V3-DESIGN.md §V3.2 and ADR-0018.

use serde::Serialize;

/// One typed difference between the base and head sides of a `Changed`
/// Object Change. Variants are scoped to the V3.2 vocabulary; the enum is
/// `#[non_exhaustive]` so future slices (V3.3 `Impacts*`) can extend without
/// bumping the wire schema.
#[non_exhaustive]
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum FieldChange {
    Body {
        before: String,
        after: String,
    },
    Status {
        before: Option<String>,
        after: Option<String>,
    },
    Owner {
        before: Option<String>,
        after: Option<String>,
    },
    VerifiedAt {
        before: Option<String>,
        after: Option<String>,
    },
    EvidenceAdded {
        field: String,
        value: String,
    },
    EvidenceRemoved {
        field: String,
        value: String,
    },
    RelationAdded {
        kind: RelationKind,
        target: String,
    },
    RelationRemoved {
        kind: RelationKind,
        target: String,
    },
    /// V3.3: a new path appeared in the `impacts:` list on the head side.
    ImpactsAdded {
        path: String,
    },
    /// V3.3: a path that was present on the base side disappeared on head.
    ImpactsRemoved {
        path: String,
    },
}

/// Discriminator for the three V0 relation slots on a Knowledge Object.
/// Mirrors the field names on [`crate::domain::graph::GraphRelations`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    DependsOn,
    Supersedes,
    RelatedTo,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn body_variant_serializes_with_snake_case_tag_and_before_after() {
        let change = FieldChange::Body {
            before: "old".to_string(),
            after: "new".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "body", "before": "old", "after": "new" })
        );
    }

    #[test]
    fn status_variant_carries_optional_before_after() {
        let change = FieldChange::Status {
            before: Some("draft".to_string()),
            after: Some("verified".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "status");
        assert_eq!(value["before"], "draft");
        assert_eq!(value["after"], "verified");
    }

    #[test]
    fn owner_variant_serializes_with_owner_tag_and_optional_sides() {
        let change = FieldChange::Owner {
            before: None,
            after: Some("team-billing".to_string()),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "owner");
        assert!(value["before"].is_null());
        assert_eq!(value["after"], "team-billing");
    }

    #[test]
    fn verified_at_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::VerifiedAt {
            before: Some("2026-05-05".to_string()),
            after: None,
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "verified_at");
        assert_eq!(value["before"], "2026-05-05");
        assert!(value["after"].is_null());
    }

    #[test]
    fn evidence_added_variant_carries_field_and_value() {
        let change = FieldChange::EvidenceAdded {
            field: "source".to_string(),
            value: "ledger".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "evidence_added", "field": "source", "value": "ledger" })
        );
    }

    #[test]
    fn evidence_removed_variant_serializes_with_evidence_removed_tag() {
        let change = FieldChange::EvidenceRemoved {
            field: "test".to_string(),
            value: "integration".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "evidence_removed");
        assert_eq!(value["field"], "test");
        assert_eq!(value["value"], "integration");
    }

    #[test]
    fn relation_added_variant_serializes_kind_in_snake_case() {
        let change = FieldChange::RelationAdded {
            kind: RelationKind::DependsOn,
            target: "billing.payments".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({
                "type": "relation_added",
                "kind": "depends_on",
                "target": "billing.payments",
            })
        );
    }

    #[test]
    fn relation_removed_variant_carries_kind_and_target() {
        let change = FieldChange::RelationRemoved {
            kind: RelationKind::Supersedes,
            target: "billing.legacy-credits".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(value["type"], "relation_removed");
        assert_eq!(value["kind"], "supersedes");
        assert_eq!(value["target"], "billing.legacy-credits");
    }

    #[test]
    fn relation_kind_related_to_serializes_in_snake_case() {
        let value = serde_json::to_value(RelationKind::RelatedTo).expect("RelationKind serializes");

        assert_eq!(value, json!("related_to"));
    }

    #[test]
    fn impacts_added_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ImpactsAdded {
            path: "crates/billing/src/refund.rs".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "impacts_added", "path": "crates/billing/src/refund.rs" })
        );
    }

    #[test]
    fn impacts_removed_variant_serializes_with_snake_case_tag() {
        let change = FieldChange::ImpactsRemoved {
            path: "src/old.rs".to_string(),
        };

        let value = serde_json::to_value(&change).expect("FieldChange serializes");

        assert_eq!(
            value,
            json!({ "type": "impacts_removed", "path": "src/old.rs" })
        );
    }

    #[test]
    fn cloned_field_change_equals_original() {
        let original = FieldChange::Body {
            before: "a".to_string(),
            after: "b".to_string(),
        };
        let cloned = original.clone();

        assert_eq!(original, cloned);
    }
}
